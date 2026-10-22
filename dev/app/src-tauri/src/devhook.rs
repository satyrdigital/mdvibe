//! Debug builds only: a file-based test hook for driving and inspecting the
//! app without UI automation. Set `MDVIBE_DEVHOOK=<dir>`; the app watches that
//! folder and handles these request files (each is deleted, then `<name>.done`
//! is written):
//!   shot      every window, title bar included → `<label>.png` (needs an
//!             unlocked screen: occluded windows are not drawn)
//!   snap      the page of every window as WebKit renders it → `<label>-web.png`
//!             (works while the screen is locked)
//!   info      windows, focus, documents, window-control frames → `info.json`
//!   eval      JavaScript run in the focused window (or `eval.<label>`)
//!   menu      a menu bar command id, as if chosen from the menu bar
//!   open      a path, as if handed over by Finder
//!   pdf       `<label>|<path>`: Save as PDF of that window, A4, no dialog
//!   awake     pages keep rendering while their window is occluded (locked
//!             screen): turns off WebKit's occlusion detection (private API)
//! Not compiled into release builds.

use std::path::{Path, PathBuf};
use std::time::Duration;

use objc2::runtime::NSObjectProtocol;
use objc2::AnyThread;
use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSWindow, NSWindowButton};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
#[allow(deprecated)]
use objc2_core_graphics::{CGWindowImageOption, CGWindowListCreateImage, CGWindowListOption};
use objc2_foundation::{MainThreadMarker, NSDictionary};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager, Runtime};

use crate::AppState;

pub fn start<R: Runtime>(app: &AppHandle<R>) {
    let Some(dir) = std::env::var_os("MDVIBE_DEVHOOK").map(PathBuf::from) else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    log::warn!("dev hook active: {}", dir.display());
    let app = app.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(250));
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if [".done", ".png", ".json", ".tmp"]
                .iter()
                .any(|x| name.ends_with(x))
            {
                continue;
            }
            let body = std::fs::read_to_string(e.path()).unwrap_or_default();
            let _ = std::fs::remove_file(e.path());
            handle(&app, &dir, &name, body.trim());
        }
    });
}

fn done(dir: &Path, name: &str, msg: &str) {
    let _ = std::fs::write(dir.join(format!("{name}.done")), msg);
}

fn handle<R: Runtime>(app: &AppHandle<R>, dir: &Path, name: &str, body: &str) {
    let dir = dir.to_path_buf();
    let name = name.to_owned();
    let body = body.to_owned();
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let app = &handle;
        let msg = match name.as_str() {
            "shot" => shot(app, &dir),
            "info" => info(app, &dir),
            "snap" => snap(app, &dir),
            "awake" => awake(app),
            "pdf" => pdf(app, &dir, &body),
            "menu" => {
                crate::mac::on_menu_event(app, &body);
                "ok".into()
            }
            "open" => {
                crate::open_from_system(app, PathBuf::from(&body));
                "ok".into()
            }
            n if n == "eval" || n.starts_with("eval.") => {
                let w = match n.strip_prefix("eval.") {
                    Some(label) => app.get_webview_window(label),
                    None => crate::focused_window(app)
                        .or_else(|| app.webview_windows().into_values().next()),
                };
                match w {
                    Some(w) => w
                        .eval(&body)
                        .map(|_| "ok".into())
                        .unwrap_or_else(|e| e.to_string()),
                    None => "no window".into(),
                }
            }
            other => format!("unknown request {other}"),
        };
        done(&dir, &name, &msg);
    });
}

fn ns_window<R: Runtime>(w: &tauri::WebviewWindow<R>) -> Option<&'static NSWindow> {
    let ptr = w.ns_window().ok()?;
    (!ptr.is_null()).then(|| unsafe { &*ptr.cast::<NSWindow>() })
}

fn shot<R: Runtime>(app: &AppHandle<R>, dir: &Path) -> String {
    let Some(_mtm) = MainThreadMarker::new() else {
        return "not on main thread".into();
    };
    let mut out = Vec::new();
    for (label, w) in app.webview_windows() {
        let Some(nw) = ns_window(&w) else { continue };
        let id = nw.windowNumber() as u32;
        #[allow(deprecated)]
        let image = CGWindowListCreateImage(
            // CGRectNull: just the bounds of the window.
            CGRect::new(
                CGPoint::new(f64::INFINITY, f64::INFINITY),
                CGSize::new(0.0, 0.0),
            ),
            CGWindowListOption::OptionIncludingWindow,
            id,
            CGWindowImageOption::BoundsIgnoreFraming | CGWindowImageOption::BestResolution,
        );
        let Some(image) = image else {
            out.push(format!("{label}: capture failed"));
            continue;
        };
        let rep = NSBitmapImageRep::initWithCGImage(NSBitmapImageRep::alloc(), &image);
        let data = unsafe {
            rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
        };
        match data {
            Some(d) => {
                let path = dir.join(format!("{label}.png"));
                let _ = std::fs::write(&path, d.to_vec());
                out.push(format!("{label}: {}", path.display()));
            }
            None => out.push(format!("{label}: encode failed")),
        }
    }
    out.join("\n")
}

fn pdf<R: Runtime>(app: &AppHandle<R>, dir: &Path, body: &str) -> String {
    let Some((label, path)) = body.split_once('|') else {
        return "expected <label>|<path>".into();
    };
    let Some(w) = app.get_webview_window(label) else {
        return format!("no window {label}");
    };
    let opt = crate::pdf::PdfOptions {
        page_width_mm: 210.0,
        page_height_mm: 297.0,
        margin_mm: 18.0,
        landscape: false,
        backgrounds: false,
        header_footer: false,
        title: "devhook".into(),
    };
    let (dir, path) = (dir.to_path_buf(), PathBuf::from(path));
    tauri::async_runtime::spawn(async move {
        let r = crate::mac::print(&w, opt, Some(path)).await;
        done(&dir, "pdf-result", &format!("{r:?}"));
    });
    "started".into()
}

fn awake<R: Runtime>(app: &AppHandle<R>) -> String {
    let mut n = 0;
    for w in app.webview_windows().into_values() {
        let ok = w.with_webview(|wv| {
            let webview: &objc2_web_kit::WKWebView = unsafe { &*wv.inner().cast() };
            let sel = objc2::sel!(_setWindowOcclusionDetectionEnabled:);
            if webview.respondsToSelector(sel) {
                let _: () = unsafe {
                    objc2::msg_send![webview, _setWindowOcclusionDetectionEnabled: false]
                };
            }
        });
        n += ok.is_ok() as usize;
    }
    format!("{n} window(s)")
}

fn snap<R: Runtime>(app: &AppHandle<R>, dir: &Path) -> String {
    let mut n = 0;
    for (label, w) in app.webview_windows() {
        let path = dir.join(format!("{label}-web.png"));
        let ok = w.with_webview(move |wv| {
            let webview: &objc2_web_kit::WKWebView = unsafe { &*wv.inner().cast() };
            let path = path.clone();
            let block = block2::RcBlock::new(
                move |img: *mut objc2_app_kit::NSImage, _err: *mut objc2_foundation::NSError| {
                    let Some(img) = (unsafe { img.as_ref() }) else {
                        return;
                    };
                    let png = img
                        .TIFFRepresentation()
                        .and_then(|t| NSBitmapImageRep::imageRepWithData(&t))
                        .and_then(|rep| unsafe {
                            rep.representationUsingType_properties(
                                NSBitmapImageFileType::PNG,
                                &NSDictionary::new(),
                            )
                        });
                    if let Some(d) = png {
                        let _ = std::fs::write(&path, d.to_vec());
                    }
                },
            );
            unsafe { webview.takeSnapshotWithConfiguration_completionHandler(None, &block) };
        });
        if ok.is_ok() {
            n += 1;
        }
    }
    format!("{n} snapshot(s) requested")
}

fn info<R: Runtime>(app: &AppHandle<R>, dir: &Path) -> String {
    let state = app.try_state::<AppState>();
    let mut windows = Vec::new();
    for (label, w) in app.webview_windows() {
        let mut buttons = Vec::new();
        if let Some(nw) = ns_window(&w) {
            for (n, b) in [
                ("close", NSWindowButton::CloseButton),
                ("min", NSWindowButton::MiniaturizeButton),
                ("zoom", NSWindowButton::ZoomButton),
            ] {
                if let Some(btn) = nw.standardWindowButton(b) {
                    // Position from the top-left of the window content, in points.
                    let r = btn.convertRect_toView(btn.bounds(), None);
                    let top = nw.frame().size.height - (r.origin.y + r.size.height);
                    buttons.push(json!({ "name": n, "x": r.origin.x, "top": top, "w": r.size.width, "h": r.size.height }));
                }
            }
        }
        windows.push(json!({
            "label": label,
            "title": w.title().unwrap_or_default(),
            "focused": w.is_focused().unwrap_or(false),
            "visible": w.is_visible().unwrap_or(false),
            "fullscreen": w.is_fullscreen().unwrap_or(false),
            "position": w.outer_position().ok().map(|p| [p.x, p.y]),
            "size": w.inner_size().ok().map(|s| [s.width, s.height]),
            "scale": w.scale_factor().unwrap_or(1.0),
            "doc": state.as_ref().and_then(|s| s.doc_path(&label)).map(|p| p.display().to_string()),
            "buttons": buttons,
        }));
    }
    let monitors: Vec<Value> = app
        .available_monitors()
        .unwrap_or_default()
        .iter()
        .map(|m| {
            let wa = m.work_area();
            json!({ "name": m.name(), "scale": m.scale_factor(), "work_area": [wa.position.x, wa.position.y, wa.size.width, wa.size.height] })
        })
        .collect();
    let v: Value = json!({ "pid": std::process::id(), "monitors": monitors, "windows": windows });
    let _ = std::fs::write(
        dir.join("info.json"),
        serde_json::to_string_pretty(&v).unwrap_or_default(),
    );
    "ok".into()
}
