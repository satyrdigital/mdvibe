//! MDVibe application shell.
//!
//! One window = one document. On Windows every window is its own process
//! (see instance.rs); on macOS all windows live in one process, as Mac apps do.

mod cli;
mod commands;
#[cfg(all(debug_assertions, target_os = "macos"))]
mod devhook;
mod document;
mod imgproto;
mod instance;
mod links;
mod logging;
#[cfg(target_os = "macos")]
mod mac;
mod pdf;
mod recent;
mod settings;
mod storage;
mod watcher;
#[cfg(windows)]
mod win;
mod window_state;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;
use tauri::webview::NewWindowResponse;
use tauri::window::Color;
use tauri::{
    AppHandle, Emitter, Manager, Runtime, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    WindowEvent,
};

use recent::Recent;
use settings::Settings;
use storage::JsonStore;
use watcher::DirWatcher;

/// What one window shows: its document, the watcher on it and, until the
/// renderer asks for it, the file the window was opened for.
#[derive(Default)]
struct WindowDoc {
    launch_file: Option<PathBuf>,
    /// The renderer has asked for its startup info (the launch file is taken).
    started: bool,
    doc: Option<PathBuf>,
    watcher: Option<DirWatcher>,
    /// macOS: a menu command chosen while no window was open, run once ready.
    pending_menu: Option<String>,
}

pub struct AppState {
    pub started: Instant,
    windows: Mutex<HashMap<String, WindowDoc>>,
    /// The window that had keyboard focus last (macOS: menu commands go there).
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    last_focused: Mutex<Option<String>>,
    _config_watcher: Mutex<Option<DirWatcher>>,
    pub settings: Settings,
    pub recent: Recent,
    pub window_store: JsonStore,
    pub log_dir: PathBuf,
}

/// Where a document from the system (Finder, Dock, `open`) should go.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
enum Target {
    /// A window that has not started yet: it opens the file as its launch file.
    Fresh,
    /// A started window without a document: the renderer is asked to open it.
    Empty(String),
    /// Every window is busy: a new one is needed.
    New,
}

impl AppState {
    fn with<T>(&self, label: &str, f: impl FnOnce(&mut WindowDoc) -> T) -> Option<T> {
        let mut g = self.windows.lock().ok()?;
        Some(f(g.entry(label.to_owned()).or_default()))
    }

    pub fn doc_path(&self, label: &str) -> Option<PathBuf> {
        self.with(label, |w| w.doc.clone()).flatten()
    }

    pub fn doc_dir(&self, label: &str) -> Option<PathBuf> {
        self.doc_path(label)
            .and_then(|p| p.parent().map(Path::to_path_buf))
    }

    pub fn set_doc(&self, label: &str, p: Option<PathBuf>) {
        self.with(label, |w| w.doc = p);
    }

    pub fn set_doc_watcher(&self, label: &str, watcher: Option<DirWatcher>) {
        self.with(label, |w| w.watcher = watcher);
    }

    pub fn take_launch_file(&self, label: &str) -> Option<PathBuf> {
        self.with(label, |w| {
            w.started = true;
            w.launch_file.take()
        })
        .flatten()
    }

    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    fn set_pending_menu(&self, label: &str, id: &str) {
        self.with(label, |w| w.pending_menu = Some(id.to_owned()));
    }

    pub fn take_pending_menu(&self, label: &str) -> Option<String> {
        self.with(label, |w| w.pending_menu.take()).flatten()
    }

    fn register(&self, label: &str, launch_file: Option<PathBuf>) {
        self.with(label, |w| {
            *w = WindowDoc {
                launch_file,
                ..WindowDoc::default()
            }
        });
    }

    /// Drops the window's document and stops watching it.
    fn forget(&self, label: &str) {
        if let Ok(mut g) = self.windows.lock() {
            g.remove(label);
        }
    }

    /// Finds the window a system "open" request should use, preferring `key`
    /// (the focused window). A window that has not started yet takes `path`
    /// as its launch file right away.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    fn target_for(&self, path: &Path, key: Option<&str>) -> Target {
        let Ok(mut g) = self.windows.lock() else {
            return Target::New;
        };
        let mut labels: Vec<&String> = g.keys().collect();
        labels.sort_by_key(|l| (Some(l.as_str()) != key, (*l).clone()));
        let free = labels.into_iter().find(|l| {
            let w = &g[*l];
            w.doc.is_none() && w.launch_file.is_none()
        });
        let Some(label) = free.cloned() else {
            return Target::New;
        };
        let w = g.get_mut(&label).expect("label from the map");
        if w.started {
            Target::Empty(label)
        } else {
            w.launch_file = Some(path.to_path_buf());
            Target::Fresh
        }
    }
}

/// Only the bundled UI may be displayed; any other navigation is cancelled.
fn is_app_url(url: &tauri::Url) -> bool {
    match url.scheme() {
        "tauri" => true,
        "http" | "https" => {
            let host = url.host_str().unwrap_or("");
            host == "tauri.localhost" || (cfg!(debug_assertions) && host == "localhost")
        }
        _ => false,
    }
}

fn background_for(theme: &str) -> Color {
    let dark = match theme {
        "dark" => true,
        "light" => false,
        _ => system_prefers_dark(),
    };
    if dark {
        Color(0x16, 0x18, 0x1d, 0xff)
    } else {
        Color(0xff, 0xff, 0xff, 0xff)
    }
}

#[cfg(windows)]
fn system_prefers_dark() -> bool {
    // HKCU\...\Themes\Personalize\AppsUseLightTheme = 0 → dark.
    let key = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
    read_hkcu_dword(key, "AppsUseLightTheme")
        .map(|v| v == 0)
        .unwrap_or(false)
}

#[cfg(windows)]
fn read_hkcu_dword(key: &str, value: &str) -> Option<u32> {
    use windows::core::HSTRING;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
    let mut data: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    let r = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            &HSTRING::from(key),
            &HSTRING::from(value),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut data as *mut u32 as *mut _),
            Some(&mut size),
        )
    };
    r.is_ok().then_some(data)
}

#[cfg(target_os = "macos")]
fn system_prefers_dark() -> bool {
    mac::prefers_dark()
}

#[cfg(not(any(windows, target_os = "macos")))]
fn system_prefers_dark() -> bool {
    false
}

/// Label of the first window. Further windows (macOS only) are `doc-<n>`.
const MAIN_LABEL: &str = "main";
static WINDOW_SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Creates a document window, hidden until the renderer reports that it is
/// ready. `file` is opened as the window's launch file; `cascade_from` is the
/// top-left corner of the window it was opened from.
pub(crate) fn open_window<R: Runtime>(
    app: &AppHandle<R>,
    file: Option<PathBuf>,
    cascade_from: Option<(i32, i32)>,
) -> tauri::Result<WebviewWindow<R>> {
    let label = match WINDOW_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed) {
        0 => MAIN_LABEL.to_owned(),
        n => format!("doc-{n}"),
    };
    let state = app.state::<AppState>();
    state.register(&label, file);
    let theme = state.settings.theme();
    let builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("index.html".into()))
        .title("MDVibe")
        .inner_size(1100.0, 800.0)
        .min_inner_size(480.0, 360.0)
        .visible(false)
        .background_color(background_for(&theme))
        .theme(match theme.as_str() {
            "dark" => Some(tauri::Theme::Dark),
            "light" => Some(tauri::Theme::Light),
            _ => None,
        })
        .zoom_hotkeys_enabled(false)
        .on_navigation(is_app_url)
        .on_new_window(|_, _| NewWindowResponse::Deny)
        .on_download(|_, _| false);
    #[cfg(target_os = "macos")]
    let builder = mac::style_window(builder);
    let window = match builder.build() {
        Ok(w) => w,
        Err(e) => {
            state.forget(&label);
            return Err(e);
        }
    };

    #[cfg(windows)]
    win::harden(&window);
    #[cfg(target_os = "macos")]
    mac::attach_toolbar(&window);

    window_state::apply(&window, &state.window_store, cascade_from);

    // Safety net: show the window even if the renderer never reports ready.
    let w = window.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(4));
        if !w.is_visible().unwrap_or(true) {
            log::warn!("renderer did not report ready; showing window");
            let _ = w.show();
        }
    });
    Ok(window)
}

/// The window that has keyboard focus or, when none has (the app is in the
/// background), the one that had it last.
#[cfg(target_os = "macos")]
pub(crate) fn focused_window<R: Runtime>(app: &AppHandle<R>) -> Option<WebviewWindow<R>> {
    let windows = app.webview_windows();
    if let Some(w) = windows.values().find(|w| w.is_focused().unwrap_or(false)) {
        return Some(w.clone());
    }
    let last = app
        .try_state::<AppState>()
        .and_then(|s| s.last_focused.lock().ok().and_then(|g| g.clone()));
    last.and_then(|l| windows.get(&l).cloned())
        .or_else(|| windows.into_values().next())
}

/// A document handed over by the system (Finder, Dock, `open`): an already
/// open document comes to the front, otherwise an empty window takes it, or
/// a new window opens cascaded from the focused one.
#[cfg(target_os = "macos")]
pub(crate) fn open_from_system<R: Runtime>(app: &AppHandle<R>, path: PathBuf) {
    let path = document::canonical(&path).unwrap_or_else(|| cli::normalize(&path));
    if instance::focus_existing(app, &path) {
        return;
    }
    let key = focused_window(app);
    let state = app.state::<AppState>();
    match state.target_for(&path, key.as_ref().map(|w| w.label())) {
        Target::Fresh => {}
        Target::Empty(label) => {
            let _ = app.emit_to(label.as_str(), "open-file", document::display_path(&path));
        }
        Target::New => {
            let origin = key.as_ref().and_then(commands::origin_of);
            if let Err(e) = open_window(app, Some(path), origin) {
                log::error!("cannot open a window: {e}");
            }
        }
    }
}

pub fn run() {
    let started = Instant::now();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let launch = cli::parse(std::env::args_os().skip(1), &cwd);

    // Extra documents passed on the command line each get their own instance
    // (on macOS: their own window, opened in setup).
    #[cfg(not(target_os = "macos"))]
    for extra in &launch.extra {
        let _ = instance::spawn(Some(extra), None);
    }

    #[cfg(windows)]
    if win::webview2_version().is_none() {
        win::error_box(
            "MDVibe",
            "MDVibe needs the Microsoft Edge WebView2 Runtime, which is not installed on this PC.\n\n\
             Install it from https://go.microsoft.com/fwlink/p/?LinkId=2124703 (or use the MDVibe installer, \
             which installs it automatically), then start MDVibe again.",
        );
        return;
    }

    let launch_file = launch.file.clone();
    #[cfg(target_os = "macos")]
    let extra_files = launch.extra.clone();
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .register_asynchronous_uri_scheme_protocol("mdimg", imgproto::handle);
    #[cfg(target_os = "macos")]
    let builder = builder
        .menu(mac::build_menu)
        .on_menu_event(|app, event| mac::on_menu_event(app, event.id().as_ref()));
    let built = builder
        .setup(move |app| {
            let config_dir = app.path().app_config_dir()?;
            let local_dir = app.path().app_local_data_dir()?;
            let log_dir = app
                .path()
                .app_log_dir()
                .unwrap_or_else(|_| local_dir.join("logs"));
            logging::init(&log_dir);
            logging::install_panic_hook();
            log::info!(
                "MDVibe {} starting ({} {})",
                app.package_info().version,
                std::env::consts::OS,
                std::env::consts::ARCH
            );

            let state = AppState {
                started,
                windows: Mutex::new(HashMap::new()),
                last_focused: Mutex::new(None),
                _config_watcher: Mutex::new(None),
                settings: Settings::new(JsonStore::new(config_dir.join("settings.json"))),
                recent: Recent::new(JsonStore::new(config_dir.join("recent.json"))),
                window_store: JsonStore::new(local_dir.join("window-state.json")),
                log_dir,
            };
            app.manage(state);

            let cascade = instance::cascade_origin(std::env::var(instance::CASCADE_ENV).ok());
            let window = open_window(app.handle(), launch_file, cascade)?;
            #[cfg(target_os = "macos")]
            {
                let mut origin = commands::origin_of(&window);
                for extra in extra_files {
                    let w = open_window(app.handle(), Some(extra), origin)?;
                    origin = commands::origin_of(&w);
                }
                mac::refresh_recent_menu(app.handle());
                #[cfg(debug_assertions)]
                devhook::start(app.handle());
            }
            #[cfg(not(target_os = "macos"))]
            let _ = window;

            // Keep settings and the recent list in sync across windows and instances.
            let names = vec![
                config_dir.join("settings.json"),
                config_dir.join("recent.json"),
            ];
            let _ = std::fs::create_dir_all(&config_dir);
            let handle = app.handle().clone();
            match watcher::watch_files(&config_dir, names, move |p| {
                let event = if p.file_name().map(|n| n == "settings.json").unwrap_or(false) {
                    "settings-changed"
                } else {
                    #[cfg(target_os = "macos")]
                    mac::refresh_recent_menu(&handle);
                    "recent-changed"
                };
                let _ = handle.emit(event, ());
            }) {
                Ok(cw) => {
                    if let Ok(mut g) = app.state::<AppState>()._config_watcher.lock() {
                        *g = Some(cw);
                    }
                }
                Err(e) => log::warn!("config watching unavailable: {e}"),
            }
            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { .. } => {
                if let (Some(ww), Some(state)) = (
                    window.app_handle().get_webview_window(window.label()),
                    window.app_handle().try_state::<AppState>(),
                ) {
                    window_state::save(&ww, &state.window_store);
                }
            }
            WindowEvent::Focused(true) => {
                if let Some(state) = window.app_handle().try_state::<AppState>() {
                    if let Ok(mut g) = state.last_focused.lock() {
                        *g = Some(window.label().to_owned());
                    }
                }
            }
            WindowEvent::Destroyed => {
                if let Some(state) = window.app_handle().try_state::<AppState>() {
                    state.forget(window.label());
                }
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            commands::startup_info,
            commands::open_document,
            commands::reload_document,
            commands::pick_markdown_file,
            commands::open_in_new_window,
            commands::activate_link,
            commands::open_external,
            commands::reveal_in_folder,
            commands::settings_save,
            commands::recent_list,
            commands::recent_remove,
            commands::recent_set_pinned,
            commands::recent_clear,
            commands::export_pdf,
            commands::print_document,
            commands::menu_sync,
            commands::log_message,
            commands::open_log_folder,
            commands::app_ready,
            commands::settings_load,
            commands::open_notices,
        ])
        .build(tauri::generate_context!());

    let app = match built {
        Ok(app) => app,
        Err(e) => {
            log::error!("fatal: {e}");
            #[cfg(windows)]
            win::error_box("MDVibe", &format!("MDVibe could not start.\n\n{e}"));
            return;
        }
    };

    // Documents the system hands over before setup has run wait here.
    #[cfg(target_os = "macos")]
    let mut early: Vec<PathBuf> = Vec::new();
    app.run(move |app, event| {
        #[cfg(not(target_os = "macos"))]
        let _ = (app, event);
        #[cfg(target_os = "macos")]
        match event {
            tauri::RunEvent::Opened { urls } => {
                let paths = urls.iter().filter_map(|u| u.to_file_path().ok());
                if app.try_state::<AppState>().is_none() {
                    early.extend(paths);
                } else {
                    for p in paths {
                        open_from_system(app, p);
                    }
                }
            }
            tauri::RunEvent::Ready => {
                for p in early.drain(..) {
                    open_from_system(app, p);
                }
            }
            // Dock icon clicked: bring a window back, or open an empty one.
            tauri::RunEvent::Reopen {
                has_visible_windows,
                ..
            } => {
                if has_visible_windows {
                    return;
                }
                match app.webview_windows().into_values().next() {
                    Some(w) => {
                        let _ = w.unminimize();
                        let _ = w.show();
                        let _ = w.set_focus();
                    }
                    None => {
                        if let Err(e) = open_window(app, None, None) {
                            log::error!("cannot open a window: {e}");
                        }
                    }
                }
            }
            // A Mac app keeps running when its last window closes.
            tauri::RunEvent::ExitRequested {
                code: None, api, ..
            } => api.prevent_exit(),
            // Quit (⌘Q, Dock): windows get no CloseRequested, keep the geometry here.
            tauri::RunEvent::Exit => mac::save_geometry(app),
            _ => {}
        }
    });
}
