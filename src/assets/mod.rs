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
    /// Directory of PokeGame.exe; Delphi code builds paths from ExtractFilePath(ExeName).
    pub game_dir: PathBuf,
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
        let exe_path = find_file(game_dir, "PokeGame.exe")?;
        let dat_path = find_file(game_dir, "POKéGAME.dat")?;
        let exe = std::fs::read(&exe_path).map_err(|e| format!("{}: {e}", exe_path.display()))?;
        let pe = pe::Pe::parse(exe)?;
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
        let raw = std::fs::read(&dat_path).map_err(|e| format!("{}: {e}", dat_path.display()))?;
        let data = GameData::parse(&raw)?;
        Ok(Assets { forms, image_lists, pictures, data, game_dir: game_dir.to_path_buf() })
    }

    pub fn form(&self, res_name: &str) -> &Component {
        &self.forms[res_name]
    }
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
