//! macOS integration: window style, the native menu bar, system appearance,
//! printing and PDF export through WKWebView, and the system's recent documents.
//!
//! The menu is built here in English and relabelled by the renderer
//! (`menu_sync`), so its strings stay in the UI catalogs (`src/i18n/*.json`).
//! Menu commands go to the focused window as a `menu` event and run the same
//! code as the in-window menu and shortcuts.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol};
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadOnly};
use objc2_app_kit::{
    NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSApplication, NSDocumentController,
    NSPaperOrientation, NSPrintHeaderAndFooter, NSPrintInfo, NSPrintJobSavingURL, NSPrintOperation,
    NSPrintSaveJob, NSPrintingPaginationMode, NSTitlebarSeparatorStyle, NSToolbar, NSWindow,
    NSWindowStyleMask, NSWindowToolbarStyle,
};
use objc2_foundation::{ns_string, MainThreadMarker, NSArray, NSNumber, NSSize, NSString, NSURL};
use objc2_web_kit::WKWebView;
use serde_json::Value;
use tauri::menu::{
    CheckMenuItem, IsMenuItem, Menu, MenuItem, MenuItemKind, PredefinedMenuItem, Submenu,
};
use tauri::{AppHandle, Emitter, Manager, Runtime, WebviewWindow, WebviewWindowBuilder};

use crate::pdf::{PdfOptions, MM_PER_INCH};
use crate::{commands, instance, window_state, AppState};

// ───────────────────────────── window ─────────────────────────────

/// The page runs under a transparent title bar; the renderer draws the toolbar.
pub fn style_window<'a, R: Runtime, M: Manager<R>>(
    builder: WebviewWindowBuilder<'a, R, M>,
) -> WebviewWindowBuilder<'a, R, M> {
    builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true)
        // Force Touch previews would load linked pages outside the app.
        .allow_link_preview(false)
}

/// An empty unified toolbar makes AppKit size the title bar like a toolbar
/// window (Notes, Preview): the window controls sit vertically centred in the
/// renderer's toolbar, and stay there through resizing and full screen.
pub fn attach_toolbar<R: Runtime>(window: &WebviewWindow<R>) {
    let _ = window.with_webview(|wv| {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let ns_window: &NSWindow = unsafe { &*wv.ns_window().cast() };
        let toolbar = NSToolbar::new(mtm);
        ns_window.setToolbar(Some(&toolbar));
        ns_window.setToolbarStyle(NSWindowToolbarStyle::Unified);
        ns_window.setTitlebarSeparatorStyle(NSTitlebarSeparatorStyle::None);
    });
}

/// Keeps a window in step with full screen, which AppKit enters and leaves with
/// an animation (the green button, ⌃⌘F, the renderer's own toggle):
/// * in full screen AppKit shows a window's toolbar as an opaque band above the
///   content, covering the renderer's toolbar — the empty toolbar is hidden
///   there and shown again outside;
/// * the renderer is told (`fullscreen` event), so it drops the room it keeps
///   for the window controls only while they are really gone.
///
/// Called on every resize (from the event loop, so the work happens on a
/// helper thread); checks again once the animation has surely ended.
pub fn sync_fullscreen<R: Runtime>(window: &WebviewWindow<R>) {
    static PENDING: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let label = window.label().to_owned();
    {
        let Ok(mut pending) = PENDING.lock() else {
            return;
        };
        if pending.contains(&label) {
            return;
        }
        pending.push(label.clone());
    }
    let w = window.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(150));
        apply_fullscreen(&w);
        std::thread::sleep(std::time::Duration::from_millis(700));
        if let Ok(mut pending) = PENDING.lock() {
            pending.retain(|l| *l != label);
        }
        apply_fullscreen(&w);
    });
}

/// Runs on a helper thread: AppKit work on the main thread, then the event.
fn apply_fullscreen<R: Runtime>(window: &WebviewWindow<R>) {
    let (tx, rx) = std::sync::mpsc::channel();
    let queued = window.with_webview(move |wv| {
        let ns_window: &NSWindow = unsafe { &*wv.ns_window().cast() };
        let full = ns_window
            .styleMask()
            .contains(NSWindowStyleMask::FullScreen);
        if let Some(toolbar) = ns_window.toolbar() {
            if toolbar.isVisible() == full {
                toolbar.setVisible(!full);
            }
        }
        let _ = tx.send(full);
    });
    if queued.is_err() {
        return;
    }
    if let Ok(full) = rx.recv_timeout(std::time::Duration::from_secs(2)) {
        let _ = window.emit_to(window.label(), "fullscreen", full);
    }
}

/// The application runs in the App Store sandbox (not the DMG build).
pub fn sandboxed() -> bool {
    std::env::var_os("APP_SANDBOX_CONTAINER_ID").is_some()
}

/// Dark Mode of the system (or of the app, if it overrides the appearance).
pub fn prefers_dark() -> bool {
    if let Some(mtm) = MainThreadMarker::new() {
        let app = NSApplication::sharedApplication(mtm);
        let names =
            unsafe { NSArray::from_slice(&[NSAppearanceNameAqua, NSAppearanceNameDarkAqua]) };
        if let Some(best) = app
            .effectiveAppearance()
            .bestMatchFromAppearancesWithNames(&names)
        {
            return best.isEqualToString(unsafe { NSAppearanceNameDarkAqua });
        }
    }
    // Off the main thread: the user default behind Dark Mode.
    objc2_foundation::NSUserDefaults::standardUserDefaults()
        .stringForKey(ns_string!("AppleInterfaceStyle"))
        .map(|s| s.to_string().eq_ignore_ascii_case("dark"))
        .unwrap_or(false)
}

/// The user chose an accent color in System Settings (not "Multicolor"):
/// controls then follow it instead of the brand blue.
pub fn uses_system_accent() -> bool {
    objc2_foundation::NSUserDefaults::standardUserDefaults()
        .objectForKey(ns_string!("AppleAccentColor"))
        .is_some()
}

/// A window now shows `path`: title it by the document (not the app name)
/// and list it in the system's recent documents (Dock, Apple menu).
pub fn show_document<R: Runtime>(window: &WebviewWindow<R>, path: &Path, name: &str) {
    let _ = window.set_title(name);
    let remember = window
        .try_state::<AppState>()
        .map(|s| s.settings.remember_recent())
        .unwrap_or(false);
    let path = path.to_path_buf();
    let _ = window.with_webview(move |wv| {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let Some(url) = NSURL::from_file_path(&path) else {
            return;
        };
        let ns_window: &NSWindow = unsafe { &*wv.ns_window().cast() };
        ns_window.setRepresentedURL(Some(&url));
        if remember {
            NSDocumentController::sharedDocumentController(mtm).noteNewRecentDocumentURL(&url);
        }
    });
}

/// "Clear recent": the system list goes too.
pub fn clear_system_recent<R: Runtime>(app: &AppHandle<R>) {
    let _ = app.run_on_main_thread(|| {
        if let Some(mtm) = MainThreadMarker::new() {
            unsafe {
                NSDocumentController::sharedDocumentController(mtm).clearRecentDocuments(None)
            };
        }
    });
}

/// Keeps the geometry of the window the user looked at last (⌘Q and the
/// Dock's Quit close windows without `CloseRequested`).
pub fn save_geometry<R: Runtime>(app: &AppHandle<R>) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let window = crate::focused_window(app).or_else(|| app.webview_windows().into_values().next());
    if let Some(w) = window {
        window_state::save(&w, &state.window_store);
    }
}

// ───────────────────────────── printing ─────────────────────────────

struct PrintDone {
    tx: Cell<Option<tauri::async_runtime::Sender<Result<(), String>>>>,
    to_file: bool,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = PrintDone]
    struct PrintDelegate;

    unsafe impl NSObjectProtocol for PrintDelegate {}

    impl PrintDelegate {
        #[unsafe(method(printOperationDidRun:success:contextInfo:))]
        fn did_run(&self, _op: &NSPrintOperation, success: bool, _ctx: *mut std::ffi::c_void) {
            if let Some(tx) = self.ivars().tx.take() {
                let result = if success || !self.ivars().to_file {
                    Ok(())
                } else {
                    Err("The PDF could not be written.".to_string())
                };
                let _ = tx.try_send(result);
            }
            PENDING.with(|p| p.borrow_mut().retain(|d| !std::ptr::eq(&**d, self)));
        }
    }
);

thread_local! {
    /// Delegates of running print operations (AppKit does not retain them).
    static PENDING: RefCell<Vec<Retained<PrintDelegate>>> = const { RefCell::new(Vec::new()) };
}

impl PrintDelegate {
    fn new(
        mtm: MainThreadMarker,
        tx: tauri::async_runtime::Sender<Result<(), String>>,
        to_file: bool,
    ) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(PrintDone {
            tx: Cell::new(Some(tx)),
            to_file,
        });
        unsafe { msg_send![super(this), init] }
    }
}

fn points(mm: f64) -> f64 {
    mm / MM_PER_INCH * 72.0
}

/// Page setup from the reading/page settings; `dest` saves to a PDF file.
fn print_info(opt: &PdfOptions, dest: Option<&Path>) -> Retained<NSPrintInfo> {
    let info = NSPrintInfo::new();
    let (w, h) = if opt.landscape {
        (opt.page_height_mm, opt.page_width_mm)
    } else {
        (opt.page_width_mm, opt.page_height_mm)
    };
    info.setOrientation(if opt.landscape {
        NSPaperOrientation::Landscape
    } else {
        NSPaperOrientation::Portrait
    });
    info.setPaperSize(NSSize::new(points(w), points(h)));
    let m = points(opt.margin_mm);
    info.setTopMargin(m);
    info.setBottomMargin(m);
    info.setLeftMargin(m);
    info.setRightMargin(m);
    info.setHorizontalPagination(NSPrintingPaginationMode::Fit);
    info.setVerticalPagination(NSPrintingPaginationMode::Automatic);
    info.setHorizontallyCentered(false);
    info.setVerticallyCentered(false);
    unsafe {
        info.dictionary().insert(
            NSPrintHeaderAndFooter,
            &*Retained::into_super(Retained::into_super(NSNumber::new_bool(opt.header_footer))),
        );
    }
    if let Some(url) = dest.and_then(NSURL::from_file_path) {
        unsafe {
            info.setJobDisposition(NSPrintSaveJob);
            info.dictionary()
                .insert(NSPrintJobSavingURL, &*Retained::into_super(url));
        }
    }
    info
}

/// Prints the document with the print stylesheet. With `dest` the result is
/// written to that PDF without any panel; without it the Print dialog opens.
/// Resolves when the operation has finished (or the dialog was closed).
pub async fn print<R: Runtime>(
    window: &WebviewWindow<R>,
    opt: PdfOptions,
    dest: Option<PathBuf>,
) -> Result<(), String> {
    let (tx, mut rx) = tauri::async_runtime::channel::<Result<(), String>>(1);
    let to_file = dest.is_some();
    let fail = tx.clone();
    window
        .with_webview(move |wv| {
            let Some(mtm) = MainThreadMarker::new() else {
                let _ = fail.try_send(Err("Printing must start on the main thread.".into()));
                return;
            };
            let webview: &WKWebView = unsafe { &*wv.inner().cast() };
            let ns_window: &NSWindow = unsafe { &*wv.ns_window().cast() };
            unsafe {
                webview
                    .configuration()
                    .preferences()
                    .setShouldPrintBackgrounds(opt.backgrounds);
            }
            let info = print_info(&opt, dest.as_deref());
            let op = unsafe { webview.printOperationWithPrintInfo(&info) };
            op.setShowsPrintPanel(!to_file);
            op.setShowsProgressPanel(!to_file);
            op.setJobTitle(Some(&NSString::from_str(&opt.title)));
            // The print view needs a real frame, otherwise pages come out blank.
            if let Some(view) = op.view() {
                view.setFrame(webview.frame());
            }
            let delegate = PrintDelegate::new(mtm, tx, to_file);
            PENDING.with(|p| p.borrow_mut().push(delegate.clone()));
            unsafe {
                op.runOperationModalForWindow_delegate_didRunSelector_contextInfo(
                    ns_window,
                    Some(&delegate),
                    Some(sel!(printOperationDidRun:success:contextInfo:)),
                    std::ptr::null_mut(),
                );
            }
        })
        .map_err(|e| e.to_string())?;
    // A PDF is written in seconds; the Print dialog may stay open a while.
    let limit = std::time::Duration::from_secs(if to_file { 120 } else { 3600 });
    match tokio::time::timeout(limit, rx.recv()).await {
        Ok(Some(r)) => r,
        Ok(None) => Err("Printing was interrupted.".into()),
        Err(_) => Err("Printing timed out.".into()),
    }
}

// ───────────────────────────── menu ─────────────────────────────

/// Menu items the renderer may relabel, check or disable, by id.
struct MenuItems<R: Runtime> {
    items: HashMap<String, MenuItemKind<R>>,
    recent: Submenu<R>,
    recent_paths: Vec<PathBuf>,
    labels: HashMap<String, String>,
}

struct MenuState<R: Runtime>(Mutex<MenuItems<R>>);

fn with_menu<R: Runtime, T>(
    app: &AppHandle<R>,
    f: impl FnOnce(&mut MenuItems<R>) -> T,
) -> Option<T> {
    let state = app.try_state::<MenuState<R>>()?;
    let mut g = state.0.lock().ok()?;
    Some(f(&mut g))
}

struct Builder<'a, R: Runtime> {
    app: &'a AppHandle<R>,
    items: HashMap<String, MenuItemKind<R>>,
}

impl<R: Runtime> Builder<'_, R> {
    fn item(&mut self, id: &str, text: &str, accel: Option<&str>) -> tauri::Result<MenuItem<R>> {
        let item = MenuItem::with_id(self.app, id, text, true, accel)?;
        self.items
            .insert(id.into(), MenuItemKind::MenuItem(item.clone()));
        Ok(item)
    }

    fn check(
        &mut self,
        id: &str,
        text: &str,
        accel: Option<&str>,
    ) -> tauri::Result<CheckMenuItem<R>> {
        let item = CheckMenuItem::with_id(self.app, id, text, true, false, accel)?;
        self.items
            .insert(id.into(), MenuItemKind::Check(item.clone()));
        Ok(item)
    }

    fn predefined(&mut self, key: &str, item: PredefinedMenuItem<R>) -> PredefinedMenuItem<R> {
        self.items
            .insert(key.into(), MenuItemKind::Predefined(item.clone()));
        item
    }

    fn submenu(
        &mut self,
        key: &str,
        text: &str,
        items: &[&dyn IsMenuItem<R>],
    ) -> tauri::Result<Submenu<R>> {
        let sub = Submenu::with_items(self.app, text, true, items)?;
        self.items
            .insert(key.into(), MenuItemKind::Submenu(sub.clone()));
        Ok(sub)
    }
}

fn project_url(key: &str) -> Option<String> {
    let meta: Value = serde_json::from_str(include_str!("../../../project.json")).ok()?;
    meta["urls"][key]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

/// The macOS menu bar (English; the renderer relabels it).
pub fn build_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let mut b = Builder {
        app,
        items: HashMap::new(),
    };
    let sep = || PredefinedMenuItem::separator(app);
    let info = app.package_info();
    let store = sandboxed();

    let about = b.item("about", "About MDVibe", None)?;
    let settings = b.item("settings", "Settings…", Some("CmdOrCtrl+,"))?;
    let services = b.predefined(
        "services",
        PredefinedMenuItem::services(app, Some("Services"))?,
    );
    let hide = b.predefined("hide", PredefinedMenuItem::hide(app, Some("Hide MDVibe"))?);
    let hide_others = b.predefined(
        "hide-others",
        PredefinedMenuItem::hide_others(app, Some("Hide Others"))?,
    );
    let show_all = b.predefined(
        "show-all",
        PredefinedMenuItem::show_all(app, Some("Show All"))?,
    );
    let quit = b.item("quit", "Quit MDVibe", Some("CmdOrCtrl+Q"))?;
    let app_menu = Submenu::with_items(
        app,
        &info.name,
        true,
        &[
            &about,
            &sep()?,
            &settings,
            &sep()?,
            &services,
            &sep()?,
            &hide,
            &hide_others,
            &show_all,
            &sep()?,
            &quit,
        ],
    )?;

    let new_window = b.item("new-window", "New Window", Some("CmdOrCtrl+N"))?;
    let open = b.item("open", "Open…", Some("CmdOrCtrl+O"))?;
    let recent = b.submenu("open-recent", "Open Recent", &[])?;
    let close = b.predefined(
        "close",
        PredefinedMenuItem::close_window(app, Some("Close Window"))?,
    );
    let reload = b.item("reload", "Reload", Some("CmdOrCtrl+R"))?;
    let reveal = b.item("reveal", "Show in Finder", Some("CmdOrCtrl+Shift+R"))?;
    let export = b.item("export-pdf", "Export as PDF…", Some("CmdOrCtrl+Shift+S"))?;
    let print = b.item("print", "Print…", Some("CmdOrCtrl+P"))?;
    let file = b.submenu(
        "file",
        "File",
        &[
            &new_window,
            &open,
            &recent,
            &sep()?,
            &close,
            &sep()?,
            &reload,
            &reveal,
            &sep()?,
            &export,
            &print,
        ],
    )?;

    let copy = b.predefined("copy", PredefinedMenuItem::copy(app, Some("Copy"))?);
    let copy_path = b.item("copy-path", "Copy File Path", Some("CmdOrCtrl+Alt+C"))?;
    let copy_source = b.item("copy-source", "Copy Markdown Source", None)?;
    let select_all = b.predefined(
        "select-all",
        PredefinedMenuItem::select_all(app, Some("Select All"))?,
    );
    let find = b.item("find", "Find…", Some("CmdOrCtrl+F"))?;
    let find_next = b.item("find-next", "Find Next", Some("CmdOrCtrl+G"))?;
    let find_prev = b.item("find-prev", "Find Previous", Some("CmdOrCtrl+Shift+G"))?;
    let find_menu = b.submenu("find-menu", "Find", &[&find, &find_next, &find_prev])?;
    let edit = b.submenu(
        "edit",
        "Edit",
        &[
            &copy,
            &copy_path,
            &copy_source,
            &select_all,
            &sep()?,
            &find_menu,
        ],
    )?;

    let sidebar = b.check("sidebar", "Show Sidebar", Some("CmdOrCtrl+Ctrl+S"))?;
    let outline = b.item("outline", "Outline", None)?;
    let recent_tab = b.item("recent-tab", "Recent Documents", None)?;
    let source = b.check("source", "Markdown Source", Some("CmdOrCtrl+U"))?;
    let page_view = b.check("page-view", "Page View", None)?;
    let layout = b.item("layout", "Reading Layout…", Some("CmdOrCtrl+Shift+L"))?;
    let theme_system = b.check("theme-system", "System", None)?;
    let theme_light = b.check("theme-light", "Light", None)?;
    let theme_dark = b.check("theme-dark", "Dark", None)?;
    let theme_next = b.item("theme-next", "Next Theme", Some("CmdOrCtrl+Shift+T"))?;
    let theme = b.submenu(
        "theme",
        "Theme",
        &[
            &theme_system,
            &theme_light,
            &theme_dark,
            &sep()?,
            &theme_next,
        ],
    )?;
    let zoom_reset = b.item("zoom-reset", "Actual Size", Some("CmdOrCtrl+0"))?;
    let zoom_in = b.item("zoom-in", "Zoom In", Some("CmdOrCtrl+="))?;
    let zoom_out = b.item("zoom-out", "Zoom Out", Some("CmdOrCtrl+-"))?;
    let on_top = b.check("on-top", "Keep on Top", None)?;
    let fullscreen = b.predefined(
        "fullscreen",
        PredefinedMenuItem::fullscreen(app, Some("Enter Full Screen"))?,
    );
    let view = b.submenu(
        "view",
        "View",
        &[
            &sidebar,
            &outline,
            &recent_tab,
            &sep()?,
            &source,
            &page_view,
            &layout,
            &sep()?,
            &theme,
            &sep()?,
            &zoom_reset,
            &zoom_in,
            &zoom_out,
            &sep()?,
            &on_top,
            &fullscreen,
        ],
    )?;

    let minimize = b.predefined(
        "minimize",
        PredefinedMenuItem::minimize(app, Some("Minimize"))?,
    );
    let zoom = b.predefined("zoom", PredefinedMenuItem::maximize(app, Some("Zoom"))?);
    let front = b.predefined(
        "bring-all-to-front",
        PredefinedMenuItem::bring_all_to_front(app, Some("Bring All to Front"))?,
    );
    let window = b.submenu("window", "Window", &[&minimize, &zoom, &sep()?, &front])?;

    let shortcuts = b.item("shortcuts", "Keyboard Shortcuts", None)?;
    let mut help_items: Vec<MenuItem<R>> = vec![shortcuts];
    help_items.push(b.item("repository", "MDVibe on GitHub", None)?);
    help_items.push(b.item("issues", "Report an Issue…", None)?);
    // App Store rules: no donation link and no own update check in a store build.
    if !store && project_url("support").is_some() {
        help_items.push(b.item("support", "Support the Developer…", None)?);
    }
    if !store && project_url("releases").is_some() {
        help_items.push(b.item("updates", "Check for Updates…", None)?);
    }
    help_items.push(b.item("log-folder", "Open Log Folder", None)?);
    let help_refs: Vec<&dyn IsMenuItem<R>> =
        help_items.iter().map(|i| i as &dyn IsMenuItem<R>).collect();
    let help = b.submenu("help", "Help", &help_refs)?;

    let menu = Menu::with_items(app, &[&app_menu, &file, &edit, &view, &window, &help])?;
    window.set_as_windows_menu_for_nsapp()?;
    help.set_as_help_menu_for_nsapp()?;
    app.manage(MenuState(Mutex::new(MenuItems {
        items: b.items,
        recent,
        recent_paths: Vec::new(),
        labels: HashMap::new(),
    })));
    Ok(menu)
}

/// Rebuilds File ▸ Open Recent from the recent list.
pub fn refresh_recent_menu<R: Runtime>(app: &AppHandle<R>) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let list = if state.settings.remember_recent() {
        state.recent.list()
    } else {
        Vec::new()
    };
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        with_menu(&handle, |m| {
            let label = |id: &str, en: &str| m.labels.get(id).cloned().unwrap_or_else(|| en.into());
            let none = label("recent-none", "No Recent Documents");
            let clear = label("recent-clear", "Clear Menu");
            let sub = &m.recent;
            if let Ok(items) = sub.items() {
                for it in items {
                    let _ = sub.remove(&it);
                }
            }
            m.recent_paths.clear();
            for (i, it) in list.iter().take(15).enumerate() {
                let id = format!("recent:{i}");
                if let Ok(item) = MenuItem::with_id(&handle, id, &it.name, it.exists, None::<&str>)
                {
                    let _ = sub.append(&item);
                }
                m.recent_paths.push(PathBuf::from(&it.path));
            }
            if list.is_empty() {
                if let Ok(item) =
                    MenuItem::with_id(&handle, "recent-none", none, false, None::<&str>)
                {
                    let _ = sub.append(&item);
                }
            } else {
                if let Ok(sep) = PredefinedMenuItem::separator(&handle) {
                    let _ = sub.append(&sep);
                }
                if let Ok(item) =
                    MenuItem::with_id(&handle, "recent-clear", clear, true, None::<&str>)
                {
                    let _ = sub.append(&item);
                }
            }
        });
    });
}

fn set_label<R: Runtime>(item: &MenuItemKind<R>, text: &str) {
    let _ = match item {
        MenuItemKind::MenuItem(i) => i.set_text(text),
        MenuItemKind::Check(i) => i.set_text(text),
        MenuItemKind::Predefined(i) => i.set_text(text),
        MenuItemKind::Submenu(i) => i.set_text(text),
        MenuItemKind::Icon(i) => i.set_text(text),
    };
}

/// Applies what the renderer reports: `{ labels: {id: text}, checks: {id: bool},
/// enabled: {id: bool} }`. Checks and enabled states only come from the
/// focused window.
pub fn sync_menu<R: Runtime>(window: &WebviewWindow<R>, payload: &Value) {
    let focused =
        crate::focused_window(window.app_handle()).is_some_and(|w| w.label() == window.label());
    let payload = payload.clone();
    let handle = window.app_handle().clone();
    let _ = window.run_on_main_thread(move || {
        let mut relabel_recent = false;
        with_menu(&handle, |m| {
            if let Some(labels) = payload["labels"].as_object() {
                for (id, text) in labels {
                    let Some(text) = text.as_str() else { continue };
                    if let Some(item) = m.items.get(id) {
                        set_label(item, text);
                    }
                    relabel_recent |= id.starts_with("recent-")
                        && m.labels.get(id).map(String::as_str) != Some(text);
                    m.labels.insert(id.clone(), text.to_owned());
                }
            }
            if !focused {
                return;
            }
            if let Some(checks) = payload["checks"].as_object() {
                for (id, on) in checks {
                    if let (Some(MenuItemKind::Check(item)), Some(on)) =
                        (m.items.get(id), on.as_bool())
                    {
                        let _ = item.set_checked(on);
                    }
                }
            }
            if let Some(enabled) = payload["enabled"].as_object() {
                for (id, on) in enabled {
                    let Some(on) = on.as_bool() else { continue };
                    let _ = match m.items.get(id) {
                        Some(MenuItemKind::MenuItem(i)) => i.set_enabled(on),
                        Some(MenuItemKind::Check(i)) => i.set_enabled(on),
                        _ => Ok(()),
                    };
                }
            }
        });
        if relabel_recent {
            refresh_recent_menu(&handle);
        }
    });
}

/// Routes a menu command: app-level commands run here, everything else goes
/// to the focused window (or to a new window when none is open).
pub fn on_menu_event<R: Runtime>(app: &AppHandle<R>, id: &str) {
    let key = crate::focused_window(app);
    match id {
        "quit" => {
            save_geometry(app);
            app.exit(0);
        }
        "new-window" => {
            let origin = key.as_ref().and_then(commands::origin_of);
            let _ = instance::open(app, None, origin);
        }
        _ if id.starts_with("recent:") => {
            let path = id["recent:".len()..]
                .parse::<usize>()
                .ok()
                .and_then(|i| with_menu(app, |m| m.recent_paths.get(i).cloned()).flatten());
            if let Some(p) = path {
                crate::open_from_system(app, p);
            }
        }
        _ => match key {
            Some(w) => {
                let _ = w.emit_to(w.label(), "menu", id);
            }
            // No window: open one; it runs the command once it is ready.
            None => {
                if let Ok(w) = crate::open_window(app, None, None) {
                    if let Some(state) = app.try_state::<AppState>() {
                        state.set_pending_menu(w.label(), id);
                    }
                }
            }
        },
    }
}
