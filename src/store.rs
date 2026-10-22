//! File access for the game and the front-end (saves, settings, auto-save, HTML export).
//! Native builds use the file system. The web build keeps files in memory, mirrored to the
//! browser's localStorage by `flush` (called by the UI thread every frame); files written under
//! `/downloads/` are handed to the browser as downloads instead of being kept.

use std::io;
use std::path::Path;
pub use web_time::SystemTime;

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use super::*;

    pub fn read(p: &Path) -> io::Result<Vec<u8>> {
        std::fs::read(p)
    }
    pub fn write(p: &Path, bytes: &[u8]) -> io::Result<()> {
        std::fs::write(p, bytes)
    }
    pub fn rename(from: &Path, to: &Path) -> io::Result<()> {
        std::fs::rename(from, to)
    }
    pub fn remove(p: &Path) -> io::Result<()> {
        std::fs::remove_file(p)
    }
    pub fn exists(p: &Path) -> bool {
        p.exists()
    }
    pub fn modified(p: &Path) -> Option<SystemTime> {
        std::fs::metadata(p).and_then(|m| m.modified()).ok()
    }
    pub fn create_dir_all(p: &Path) -> io::Result<()> {
        std::fs::create_dir_all(p)
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::{Mutex, MutexGuard, TryLockError};

    /// Files by path, with their modification time.
    static FILES: Mutex<BTreeMap<String, (Vec<u8>, SystemTime)>> = Mutex::new(BTreeMap::new());
    /// Paths changed since the last `flush`.
    static DIRTY: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());
    const KEY: &str = "pokegame:";
    pub const DOWNLOADS: &str = "/downloads/";

    /// The browser's main thread may not block on a lock, so every thread spins instead (the
    /// critical sections are tiny).
    fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
        loop {
            match m.try_lock() {
                Ok(g) => return g,
                Err(TryLockError::Poisoned(p)) => return p.into_inner(),
                Err(TryLockError::WouldBlock) => std::hint::spin_loop(),
            }
        }
    }

    fn key(p: &Path) -> String {
        p.to_string_lossy().replace('\\', "/")
    }

    fn not_found(p: &Path) -> io::Error {
        io::Error::new(io::ErrorKind::NotFound, format!("{}: file not found", p.display()))
    }

    pub fn read(p: &Path) -> io::Result<Vec<u8>> {
        lock(&FILES).get(&key(p)).map(|(b, _)| b.clone()).ok_or_else(|| not_found(p))
    }
    pub fn write(p: &Path, bytes: &[u8]) -> io::Result<()> {
        let k = key(p);
        lock(&FILES).insert(k.clone(), (bytes.to_vec(), SystemTime::now()));
        lock(&DIRTY).insert(k);
        Ok(())
    }
    pub fn rename(from: &Path, to: &Path) -> io::Result<()> {
        let (fk, tk) = (key(from), key(to));
        let mut files = lock(&FILES);
        let entry = files.remove(&fk).ok_or_else(|| not_found(from))?;
        files.insert(tk.clone(), entry);
        drop(files);
        let mut dirty = lock(&DIRTY);
        dirty.insert(fk);
        dirty.insert(tk);
        Ok(())
    }
    pub fn remove(p: &Path) -> io::Result<()> {
        let k = key(p);
        lock(&FILES).remove(&k).ok_or_else(|| not_found(p))?;
        lock(&DIRTY).insert(k);
        Ok(())
    }
    pub fn exists(p: &Path) -> bool {
        lock(&FILES).contains_key(&key(p))
    }
    pub fn modified(p: &Path) -> Option<SystemTime> {
        lock(&FILES).get(&key(p)).map(|(_, t)| *t)
    }
    pub fn create_dir_all(_: &Path) -> io::Result<()> {
        Ok(())
    }

    fn storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok().flatten()
    }

    /// Bytes as a string of U+0000..U+00FF (localStorage only holds strings).
    fn to_latin1(b: &[u8]) -> String {
        b.iter().map(|&c| c as char).collect()
    }

    /// Load the files kept in localStorage (UI thread, before the game starts).
    pub fn load() {
        let Some(s) = storage() else { return };
        let mut files = lock(&FILES);
        for i in 0..s.length().unwrap_or(0) {
            let Some(k) = s.key(i).ok().flatten() else { continue };
            let Some(path) = k.strip_prefix(KEY) else { continue };
            let Some(v) = s.get_item(&k).ok().flatten() else { continue };
            // "<ms since epoch>\n<latin-1 bytes>"
            let Some((ms, data)) = v.split_once('\n') else { continue };
            let t = SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(ms.parse().unwrap_or(0));
            files.insert(path.to_string(), (data.chars().map(|c| c as u32 as u8).collect(), t));
        }
    }

    /// Write changed files to localStorage and hand finished downloads to the browser
    /// (UI thread).
    pub fn flush() {
        let changed: Vec<String> = std::mem::take(&mut *lock(&DIRTY)).into_iter().collect();
        if changed.is_empty() {
            return;
        }
        let Some(s) = storage() else { return };
        for k in changed {
            let entry = lock(&FILES).get(&k).cloned();
            if let Some(name) = k.strip_prefix(DOWNLOADS) {
                if let Some((bytes, _)) = entry {
                    crate::web::download(name, &bytes);
                    lock(&FILES).remove(&k);
                }
                continue;
            }
            match entry {
                Some((bytes, t)) => {
                    let ms = t.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
                    if s.set_item(&format!("{KEY}{k}"), &format!("{ms}\n{}", to_latin1(&bytes))).is_err() {
                        web_sys::console::error_1(&format!("pokegame: could not store {k} (browser storage full?)").into());
                    }
                }
                None => {
                    let _ = s.remove_item(&format!("{KEY}{k}"));
                }
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use imp::{flush, load, DOWNLOADS};

pub fn read(p: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    imp::read(p.as_ref())
}
pub fn read_to_string(p: impl AsRef<Path>) -> io::Result<String> {
    String::from_utf8(read(p)?).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}
pub fn write(p: impl AsRef<Path>, bytes: impl AsRef<[u8]>) -> io::Result<()> {
    imp::write(p.as_ref(), bytes.as_ref())
}
pub fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
    imp::rename(from.as_ref(), to.as_ref())
}
pub fn copy(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
    write(to, read(from)?)
}
pub fn remove(p: impl AsRef<Path>) -> io::Result<()> {
    imp::remove(p.as_ref())
}
pub fn exists(p: impl AsRef<Path>) -> bool {
    imp::exists(p.as_ref())
}
pub fn modified(p: impl AsRef<Path>) -> Option<SystemTime> {
    imp::modified(p.as_ref())
}
pub fn create_dir_all(p: impl AsRef<Path>) -> io::Result<()> {
    imp::create_dir_all(p.as_ref())
}
