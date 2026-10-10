//! POKéMON Simulator V4.5 — Rust/egui front-end.
//!
//! Usage: `pokegame [GAME_DIR] [SAVE.pgs]`
//! GAME_DIR must contain PokeGame.exe and POKéGAME.dat from your own copy of PokeGame32.exe
//! (extract the installer with e.g. `7z x PokeGame32.exe`). Defaults to $POKEGAME_DIR, then the
//! current directory, then asks with a folder picker.

use pokegame::assets::Assets;
use pokegame::debug::Harness;
use pokegame::game::world::World;
use pokegame::ui::GameUi;
use pokegame::vcl::runtime::{Bridge, Dialog, DialogAnswer, Input, Snapshot};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};

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

struct App {
    assets: Arc<Assets>,
    snapshot: Arc<Mutex<Snapshot>>,
    tx: Sender<Input>,
    ui: GameUi,
    answered: HashSet<u64>,
    screen: (i32, i32),
    harness: Harness,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let (snap, fx) = {
            let mut s = self.snapshot.lock().unwrap();
            let fx = std::mem::take(&mut s.fx);
            (s.clone(), fx)
        };
        if snap.quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        let size = ctx.screen_rect().size();
        let screen = (size.x as i32, size.y as i32);
        if screen != self.screen {
            self.screen = screen;
            let _ = self.tx.send(Input::ScreenSize { width: screen.0, height: screen.1 });
        }
        // Closing the desktop window = closing the main game window (saves, like the original).
        if ctx.input(|i| i.viewport().close_requested()) {
            let main_visible = snap.forms.iter().any(|f| f.var == "Form4" && f.visible);
            if main_visible && snap.dialog.is_none() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                let _ = self.tx.send(Input::CloseForm { form: "Form4".into() });
            } else {
                let _ = self.tx.send(Input::Quit);
            }
        }
        if self.harness.tick(ctx, &snap, &self.tx) {
            let _ = self.tx.send(Input::Quit);
        }
        // Native file dialogs for TOpenDialog / TSaveDialog.Execute.
        if let Some((id, Dialog::File { save, filter, default_ext, initial, .. })) = &snap.dialog {
            if let Some(path) = self.harness.pending_file.take() {
                self.answered.insert(*id);
                let _ = self.tx.send(Input::Answer { id: *id, answer: DialogAnswer::File(Some(path)) });
            } else if self.answered.insert(*id) {
                let mut d = rfd::FileDialog::new();
                let mut parts = filter.split('|');
                while let (Some(name), Some(pat)) = (parts.next(), parts.next()) {
                    let exts: Vec<String> = pat
                        .split(';')
                        .filter_map(|p| p.trim().strip_prefix("*."))
                        .filter(|e| *e != "*")
                        .map(str::to_string)
                        .collect();
                    if !exts.is_empty() {
                        d = d.add_filter(name, &exts);
                    }
                }
                if !initial.is_empty() {
                    let p = Path::new(initial);
                    if let Some(dir) = p.parent() {
                        d = d.set_directory(dir);
                    }
                    if let Some(n) = p.file_name() {
                        d = d.set_file_name(n.to_string_lossy());
                    }
                }
                let picked = if *save { d.save_file() } else { d.pick_file() };
                let path = picked.map(|mut p| {
                    // TSaveDialog.DefaultExt: appended when the user typed no extension.
                    let ext = default_ext.trim_start_matches('*').trim_start_matches('.');
                    if *save && p.extension().is_none() && !ext.is_empty() {
                        p.set_extension(ext);
                    }
                    p.to_string_lossy().into_owned()
                });
                let _ = self.tx.send(Input::Answer { id: *id, answer: DialogAnswer::File(path) });
            }
        }
        for c in std::mem::take(&mut self.harness.ui_commands) {
            self.ui.debug_command(&c);
        }
        self.ui.show(ctx, &snap, fx, &self.assets, &self.tx);
    }
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
    let assets = match Assets::from_bytes(&exe, &dat) {
        Ok(a) => Arc::new(a),
        Err(e) => fail(e),
    };
    let data = pokegame::assets::data_section(&exe);
    drop((exe, dat));
    let snapshot = Arc::new(Mutex::new(Snapshot::default()));
    let (tx, rx) = channel();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("POKéMON Simulator")
            .with_inner_size(window_size())
            .with_min_inner_size([360.0, 480.0]),
        ..Default::default()
    };
    let snap2 = snapshot.clone();
    let assets2 = assets.clone();
    let result = eframe::run_native(
        "POKéMON Simulator",
        options,
        Box::new(move |cc| {
            let ui = GameUi::new(&cc.egui_ctx);
            let ctx = cc.egui_ctx.clone();
            let bridge = Bridge::new(snap2.clone(), rx, Box::new(move || ctx.request_repaint()));
            let assets_t = assets2.clone();
            std::thread::Builder::new()
                .name("delphi-main".into())
                .spawn(move || {
                    let mut w = World::new(assets_t, &data, Some(bridge));
                    w.params = params;
                    w.autosave = true;
                    w.run();
                })
                .expect("spawn logic thread");
            Ok(Box::new(App {
                assets: assets2,
                snapshot: snap2,
                tx,
                ui,
                answered: HashSet::new(),
                screen: (0, 0),
                harness: Harness::from_env(),
            }))
        }),
    );
    if let Err(e) = result {
        eprintln!("{e}");
    }
}
