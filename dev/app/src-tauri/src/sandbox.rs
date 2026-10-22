//! Mac App Store sandbox: remembered access to documents and folders.
//!
//! Inside the sandbox the app may read a document the user opened (Finder,
//! Open panel, drag and drop) but nothing next to it — no images — and, after
//! a restart, not even the documents in Recent. Access the user granted is
//! kept as security-scoped bookmarks in `bookmarks.json`:
//!   * every opened document (so Recent keeps working),
//!   * folders the user allowed from the "show images" bar.
//!
//! Everything here is inert outside the sandbox (the DMG build), where the
//! app reads files directly.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use objc2::rc::Retained;
use objc2_foundation::{
    NSData, NSURLBookmarkCreationOptions, NSURLBookmarkResolutionOptions, NSURL,
};
use serde::{Deserialize, Serialize};

use crate::storage::JsonStore;

/// Most recent documents whose access is kept (Recent shows at most 20).
const MAX_FILES: usize = 40;

#[derive(Default, Serialize, Deserialize)]
struct Bookmarks {
    /// Folder path → bookmark (hex).
    #[serde(default)]
    folders: BTreeMap<String, String>,
    /// Document path → bookmark (hex), oldest first.
    #[serde(default)]
    files: Vec<(String, String)>,
}

pub struct Sandbox {
    store: JsonStore,
    /// Security scopes in use, by path (kept until the app quits).
    active: Mutex<BTreeMap<PathBuf, Retained<NSURL>>>,
}

pub fn active() -> bool {
    crate::mac::sandboxed()
}

fn to_hex(data: &NSData) -> String {
    data.to_vec().iter().map(|b| format!("{b:02x}")).collect()
}

fn from_hex(s: &str) -> Option<Retained<NSData>> {
    let bytes: Option<Vec<u8>> = (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect();
    Some(NSData::with_bytes(&bytes?))
}

fn bookmark(path: &Path) -> Option<String> {
    let url = NSURL::from_file_path(path)?;
    let data = url
        .bookmarkDataWithOptions_includingResourceValuesForKeys_relativeToURL_error(
            NSURLBookmarkCreationOptions::WithSecurityScope
                | NSURLBookmarkCreationOptions::SecurityScopeAllowOnlyReadAccess,
            None,
            None,
        )
        .map_err(|e| log::warn!("bookmark not created: {}", e.localizedDescription()))
        .ok()?;
    Some(to_hex(&data))
}

fn resolve(hex: &str) -> Option<(PathBuf, Retained<NSURL>)> {
    let data = from_hex(hex)?;
    let mut stale = objc2::runtime::Bool::NO;
    let url = unsafe {
        NSURL::URLByResolvingBookmarkData_options_relativeToURL_bookmarkDataIsStale_error(
            &data,
            NSURLBookmarkResolutionOptions::WithSecurityScope
                | NSURLBookmarkResolutionOptions::WithoutUI,
            None,
            &mut stale,
        )
    }
    .ok()?;
    let path = url.to_file_path()?;
    Some((path, url))
}

impl Sandbox {
    pub fn new(store: JsonStore) -> Self {
        Self {
            store,
            active: Mutex::new(BTreeMap::new()),
        }
    }

    fn start(&self, path: PathBuf, url: Retained<NSURL>) {
        let Ok(mut active) = self.active.lock() else {
            return;
        };
        if active.contains_key(&path) {
            return;
        }
        if unsafe { url.startAccessingSecurityScopedResource() } {
            active.insert(path, url);
        }
    }

    /// At launch: regain access to everything granted before.
    pub fn restore(&self) {
        let Some(b) = self.store.read::<Bookmarks>() else {
            return;
        };
        let hexes = b.folders.values().chain(b.files.iter().map(|(_, h)| h));
        for (path, url) in hexes.filter_map(|h| resolve(h)) {
            self.start(path, url);
        }
        let n = self.active.lock().map(|a| a.len()).unwrap_or(0);
        log::info!("sandbox: {n} remembered location(s) restored");
    }

    /// A document was opened (access granted by the system right now):
    /// remember it for Recent.
    pub fn remember_file(&self, path: &Path) {
        let Some(hex) = bookmark(path) else { return };
        let key = path.to_string_lossy().into_owned();
        self.store.locked(|| {
            let mut b = self.store.read::<Bookmarks>().unwrap_or_default();
            b.files.retain(|(p, _)| *p != key);
            b.files.push((key, hex));
            let excess = b.files.len().saturating_sub(MAX_FILES);
            b.files.drain(..excess);
            let _ = self.store.write(&b);
        });
    }

    /// The user allowed `folder` in the Open panel: keep and use that access.
    pub fn remember_folder(&self, folder: &Path) -> bool {
        let Some(hex) = bookmark(folder) else {
            return false;
        };
        if let Some((path, url)) = resolve(&hex) {
            self.start(path, url);
        }
        let key = folder.to_string_lossy().into_owned();
        self.store.locked(|| {
            let mut b = self.store.read::<Bookmarks>().unwrap_or_default();
            b.folders.insert(key, hex);
            let _ = self.store.write(&b);
        });
        true
    }

    /// Whether the folder of `doc` (and so the images next to it) is readable.
    pub fn covers_folder_of(&self, doc: &Path) -> bool {
        let Some(dir) = doc.parent() else {
            return false;
        };
        self.active
            .lock()
            .map(|a| a.keys().any(|p| dir.starts_with(p) && p.is_dir()))
            .unwrap_or(false)
    }

    /// Forgets every remembered document (Clear Recent).
    pub fn forget_files(&self) {
        self.store.locked(|| {
            let mut b = self.store.read::<Bookmarks>().unwrap_or_default();
            b.files.clear();
            let _ = self.store.write(&b);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip() {
        let data = NSData::with_bytes(&[0, 1, 0xab, 0xff]);
        let hex = to_hex(&data);
        assert_eq!(hex, "0001abff");
        assert_eq!(from_hex(&hex).unwrap().to_vec(), vec![0, 1, 0xab, 0xff]);
        assert!(from_hex("zz").is_none());
    }
}
