//! Everything copyrighted (sprites, form layouts, game data) is read at runtime from the
//! user's own copy of POKéMON Simulator V4.5 — nothing is embedded in this program.

pub mod bitmap;
pub mod datfile;
pub mod dfm;
pub mod pe;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub use bitmap::{ImageList, Rgba};
pub use datfile::GameData;
pub use dfm::Component;

pub struct Assets {
    /// Form name (e.g. "TFORM4") -> component tree.
    pub forms: HashMap<String, Component>,
    /// "Form.Component" -> decoded ImageList (e.g. "Form4.PKMNImages").
    pub image_lists: HashMap<String, ImageList>,
    /// "Form.Component" -> decoded TImage picture.
    pub pictures: HashMap<String, Rgba>,
    pub data: GameData,
}

/// The Delphi form resource names in this program, with the global variable each is assigned to.
pub const FORMS: &[(&str, &str)] = &[
    ("TFORM1", "Form1"),
    ("TFORM3", "Form3"),
    ("TFORM4", "Form4"),
    ("TFORM5", "Form5"),
    ("TFORM7", "Form7"),
    ("TFORM8", "Form8"),
    ("TFORM9", "Form9"),
    ("TFORMA", "FormA"),
    ("TFORMB", "FormB"),
    ("TABOUTBOX", "AboutBox"),
    ("TOKRIGHTDLG", "OKRightDlg"),
];

impl Assets {
    /// `game_dir` must contain PokeGame.exe and POKéGAME.dat (as extracted from PokeGame32.exe).
    pub fn load(game_dir: &Path) -> Result<Assets, String> {
        let (exe, dat) = read_game_files(game_dir)?;
        Assets::from_bytes(&exe, &dat)
    }

    /// The two original files' contents (from a folder, or compiled in with `--features bundled`).
    pub fn from_bytes(exe: &[u8], dat: &[u8]) -> Result<Assets, String> {
        let pe = pe::Pe::parse(exe.to_vec())?;
        let res = pe.resources()?;
        let mut forms = HashMap::new();
        for (res_name, _) in FORMS {
            let key = (pe::ResId::Id(pe::RT_RCDATA), pe::ResId::Name(res_name.to_string()));
            let data = res.get(&key).ok_or_else(|| format!("form resource {res_name} missing from PokeGame.exe"))?;
            forms.insert(res_name.to_string(), dfm::parse(data)?);
        }
        let mut image_lists = HashMap::new();
        let mut pictures = HashMap::new();
        for (res_name, var) in FORMS {
            let mut all = Vec::new();
            forms[*res_name].walk(&mut all);
            for c in all {
                if let Some(dfm::Value::Binary(b)) = c.prop("Bitmap") {
                    if c.class == "TImageList" {
                        image_lists.insert(format!("{var}.{}", c.name), bitmap::decode_imagelist(b)?);
                    }
                }
                if let Some(dfm::Value::Binary(b)) = c.prop("Picture.Data") {
                    pictures.insert(format!("{var}.{}", c.name), bitmap::decode_picture(b)?);
                }
            }
        }
        let data = GameData::parse(dat)?;
        Ok(Assets { forms, image_lists, pictures, data })
    }

    pub fn form(&self, res_name: &str) -> &Component {
        &self.forms[res_name]
    }
}

/// Read PokeGame.exe and POKéGAME.dat from `dir`.
pub fn read_game_files(dir: &Path) -> Result<(Vec<u8>, Vec<u8>), String> {
    let read = |name: &str| {
        let p = find_file(dir, name)?;
        std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))
    };
    Ok((read("PokeGame.exe")?, read("POKéGAME.dat")?))
}

/// The original DATA section bytes (VA 0x48e000), for the emulated data segment.
pub fn data_section(exe: &[u8]) -> Vec<u8> {
    let Ok(p) = pe::Pe::parse(exe.to_vec()) else { return Vec::new() };
    p.sections
        .iter()
        .find(|s| s.name == "DATA")
        .map(|s| p.data[s.raw_ptr as usize..(s.raw_ptr + s.raw_size) as usize].to_vec())
        .unwrap_or_default()
}

/// The game files compiled into this binary (feature `bundled`), if any.
pub fn bundled() -> Option<(&'static [u8], &'static [u8])> {
    #[cfg(has_bundle)]
    return Some((include_bytes!(concat!(env!("OUT_DIR"), "/PokeGame.exe")), include_bytes!(concat!(env!("OUT_DIR"), "/POKEGAME.dat"))));
    #[cfg(not(has_bundle))]
    None
}

/// Case-insensitive lookup that also tolerates the 'é' being stored as Latin-1 or decomposed.
fn find_file(dir: &Path, name: &str) -> Result<PathBuf, String> {
    let norm = |s: &str| s.to_lowercase().replace("e\u{301}", "é");
    let want = norm(name);
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for e in entries.flatten() {
        let fname = e.file_name().to_string_lossy().into_owned();
        if norm(&fname) == want || norm(&fname).replace('\u{fffd}', "é") == want {
            return Ok(e.path());
        }
    }
    Err(format!("{name} not found in {} — extract it from PokeGame32.exe (e.g. `7z x PokeGame32.exe`)", dir.display()))
}
