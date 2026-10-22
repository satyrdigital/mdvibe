//! Window model.
//!
//! Windows: every document gets its own MDVibe process and window. There is
//! deliberately no single-instance lock or IPC between instances.
//! macOS: one process; every document gets its own window in it.

use std::path::Path;
use std::process::{Command, Stdio};
use tauri::{AppHandle, Runtime};

/// Opens `path` (or an empty window) next to the window at `origin`.
pub fn open<R: Runtime>(
    app: &AppHandle<R>,
    path: Option<&Path>,
    origin: Option<(i32, i32)>,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        if let Some(p) = path {
            let p = crate::document::canonical(p).unwrap_or_else(|| p.to_path_buf());
            if focus_existing(app, &p) {
                return Ok(());
            }
        }
        crate::open_window(app, path.map(Path::to_path_buf), origin)
            .map(|_| ())
            .map_err(|e| {
                log::error!("cannot open a window: {e}");
                e.to_string()
            })
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        spawn(path, origin)
    }
}

/// Brings the window that already shows `path` to the front.
#[cfg(target_os = "macos")]
pub fn focus_existing<R: Runtime>(app: &AppHandle<R>, path: &Path) -> bool {
    use tauri::Manager;
    let Some(state) = app.try_state::<crate::AppState>() else {
        return false;
    };
    let found = app
        .webview_windows()
        .into_values()
        .find(|w| state.doc_path(w.label()).as_deref() == Some(path));
    match found {
        Some(w) => {
            let _ = w.unminimize();
            let _ = w.set_focus();
            true
        }
        None => false,
    }
}

/// Environment variable through which a window tells the instance it starts
/// where it is ("x,y" in physical pixels), so the new window opens cascaded
/// instead of exactly on top of it.
pub const CASCADE_ENV: &str = "MDVIBE_CASCADE_FROM";

/// Starts a new, fully independent MDVibe process, optionally with a document.
#[cfg_attr(target_os = "macos", allow(dead_code))]
pub fn spawn(path: Option<&Path>, origin: Option<(i32, i32)>) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut cmd = Command::new(exe);
    if let Some(path) = path {
        // `--` guarantees the path is never interpreted as a flag.
        cmd.arg("--").arg(path);
        if let Some(dir) = path.parent().filter(|d| d.is_dir()) {
            cmd.current_dir(dir);
        }
    }
    match origin {
        Some((x, y)) => cmd.env(CASCADE_ENV, format!("{x},{y}")),
        None => cmd.env_remove(CASCADE_ENV),
    };
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    cmd.spawn().map(|_| ()).map_err(|e| {
        log::error!("cannot start a new instance: {e}");
        e.to_string()
    })
}

/// Parses the cascade origin set by the parent window.
pub fn cascade_origin(value: Option<String>) -> Option<(i32, i32)> {
    let v = value?;
    let (x, y) = v.split_once(',')?;
    Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cascade_origin() {
        assert_eq!(cascade_origin(Some("100,-20".into())), Some((100, -20)));
        assert_eq!(cascade_origin(Some("x,1".into())), None);
        assert_eq!(cascade_origin(None), None);
    }
}
