//! POKéMON Simulator V4.5 — Rust/egui front-end.
//!
//! Usage: `pokegame [GAME_DIR] [SAVE.pgs]`
//! GAME_DIR must contain PokeGame.exe and POKéGAME.dat from your own copy of PokeGame32.exe
//! (extract the installer with e.g. `7z x PokeGame32.exe`). Defaults to $POKEGAME_DIR, then the
//! current directory, then asks with a folder picker.

// No console window behind the game on Windows.
#![cfg_attr(windows, windows_subsystem = "windows")]

use std::path::{Path, PathBuf};

fn has_game(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|it| it.flatten().any(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case("PokeGame.exe")))
        .unwrap_or(false)
}

/// Where the folder picked last time is remembered.
fn remembered_dir_file() -> Option<PathBuf> {
    pokegame::ui::settings::app_dir().map(|d| d.join("game_dir.txt"))
}

fn find_game_dir(arg: Option<String>) -> Option<PathBuf> {
    let remembered = remembered_dir_file().and_then(|f| std::fs::read_to_string(f).ok()).map(|s| PathBuf::from(s.trim()));
    let candidates = [
        arg.map(PathBuf::from),
        std::env::var_os("POKEGAME_DIR").map(PathBuf::from),
        remembered,
        std::env::current_dir().ok(),
        std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)),
        // Development checkout: the extracted original next to this crate.
        Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("../downloads/pokegame/extracted")),
    ];
    if let Some(d) = candidates.into_iter().flatten().find(|d| has_game(d)) {
        return Some(d);
    }
    let picked = rfd::FileDialog::new().set_title("Select the folder containing PokeGame.exe and POKéGAME.dat").pick_folder()?;
    if has_game(&picked) {
        if let Some(f) = remembered_dir_file() {
            if let Some(dir) = f.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(f, picked.to_string_lossy().as_bytes());
        }
    }
    Some(picked)
}

/// Initial window size; `POKEGAME_WINDOW=WxH` overrides it (e.g. 390x844 to try a phone layout).
fn window_size() -> [f32; 2] {
    std::env::var("POKEGAME_WINDOW")
        .ok()
        .and_then(|v| {
            let (w, h) = v.split_once('x')?;
            Some([w.parse().ok()?, h.parse().ok()?])
        })
        .unwrap_or([1024.0, 740.0])
}

fn main() {
    let mut args = std::env::args().skip(1);
    let first = args.next();
    // A .pgs argument is a save file to load (TForm3.FormHide: ParamStr), not the game folder.
    let (dir_arg, mut params): (Option<String>, Vec<String>) = match first {
        Some(a) if Path::new(&a).is_dir() => (Some(a), Vec::new()),
        Some(a) => (None, vec![a]),
        None => (None, Vec::new()),
    };
    params.extend(args);
    let fail = |e: String| -> ! {
        rfd::MessageDialog::new().set_title("POKéMON Simulator").set_description(&e).show();
        std::process::exit(1);
    };
    // A bundled build carries the game files; a folder argument still overrides them.
    let (exe, dat) = match (pokegame::assets::bundled(), &dir_arg) {
        (Some((exe, dat)), None) => (exe.to_vec(), dat.to_vec()),
        _ => {
            let Some(game_dir) = find_game_dir(dir_arg) else {
                eprintln!("PokeGame.exe / POKéGAME.dat not found.");
                std::process::exit(1);
            };
            pokegame::assets::read_game_files(&game_dir).unwrap_or_else(|e| fail(e))
        }
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("POKéMON Simulator")
            .with_inner_size(window_size())
            .with_min_inner_size([360.0, 480.0])
            .with_icon(pokegame::ui::icon::window_icon()),
        ..Default::default()
    };
    if let Err(e) = pokegame::app::run(options, &exe, &dat, params) {
        fail(e);
    }
}
