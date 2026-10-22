//! Three save slots in the app data folder. The game itself still reads and writes ordinary
//! `.pgs` files: the front-end just answers its Save/Open dialogs with a slot's path.

use std::path::PathBuf;
use crate::store::{self, SystemTime};

pub const COUNT: usize = 3;

pub fn dir() -> Option<PathBuf> {
    super::settings::app_dir().map(|d| d.join("saves"))
}

pub fn path(slot: usize) -> Option<PathBuf> {
    dir().map(|d| d.join(format!("slot{}.pgs", slot + 1)))
}

/// What a slot's card shows, read from the save file (format: TForm4.Save).
#[derive(Clone, Debug)]
pub struct Summary {
    pub name: String,
    pub badges: i32,
    pub money: String,
    pub dex: usize,
    pub pc: usize,
    /// Species of the first POKéMON in the PC (1-based), for the card's sprite.
    pub lead: i32,
    pub modified: SystemTime,
}

pub fn summary(slot: usize) -> Option<Summary> {
    let p = path(slot)?;
    let modified = store::modified(&p)?;
    let bytes = store::read(&p).ok()?;
    // Latin-1 text with CRLF lines.
    let text: String = bytes.iter().map(|&b| b as char).collect();
    let lines: Vec<&str> = text.lines().collect();
    if lines.first() != Some(&"VERSION2SAVEFILEIDENTIFIER") || lines.len() < 400 {
        return None;
    }
    let owned = &lines[19..(19 + 386).min(lines.len())];
    let pc = &lines[(19 + 386).min(lines.len())..];
    let species: Vec<i32> = pc.chunks(3).map(|c| c[0].trim().parse().unwrap_or(0)).take_while(|&s| s > 0).collect();
    Some(Summary {
        name: lines[1].to_string(),
        badges: lines[7].trim().parse().unwrap_or(0),
        money: lines[8].trim().to_string(),
        dex: owned.iter().filter(|l| l.trim() == "TRUE").count(),
        pc: species.len(),
        lead: species.first().copied().unwrap_or(0),
        modified,
    })
}

/// The slot saved most recently (for Continue).
pub fn latest() -> Option<usize> {
    (0..COUNT).filter_map(|s| summary(s).map(|m| (s, m.modified))).max_by_key(|(_, t)| *t).map(|(s, _)| s)
}

pub fn ensure_dir() {
    if let Some(d) = dir() {
        let _ = store::create_dir_all(d);
    }
}

/// Copy a `.pgs` file into a slot (Import). Refuses files that are not saves.
pub fn import(slot: usize, from: &std::path::Path) -> Result<(), String> {
    let bytes = store::read(from).map_err(|e| e.to_string())?;
    import_bytes(slot, &bytes)
}

/// Put a save's contents into a slot. Refuses files that are not saves.
pub fn import_bytes(slot: usize, bytes: &[u8]) -> Result<(), String> {
    if !bytes.starts_with(b"VERSION2SAVEFILEIDENTIFIER") {
        return Err("That file is not a POKéMON Simulator save (.pgs).".into());
    }
    ensure_dir();
    let to = path(slot).ok_or("No app data folder")?;
    store::write(to, bytes).map_err(|e| e.to_string())
}

/// Copy a slot out to a `.pgs` file (Export).
pub fn export(slot: usize, to: &std::path::Path) -> Result<(), String> {
    let from = path(slot).ok_or("No app data folder")?;
    store::copy(from, to).map(|_| ()).map_err(|e| e.to_string())
}
