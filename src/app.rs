//! The application: game logic thread + egui front-end, shared by the desktop binary and the
//! Android library.

use crate::assets::Assets;
use crate::debug::Harness;
use crate::game::world::World;
use crate::ui::GameUi;
use crate::vcl::runtime::{Bridge, Dialog, DialogAnswer, Input, Snapshot};
use std::collections::HashSet;
#[cfg(not(target_os = "android"))]
use std::path::Path;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};

pub struct App {
    assets: Arc<Assets>,
    snapshot: Arc<Mutex<Snapshot>>,
    tx: Sender<Input>,
    ui: GameUi,
    audio: Option<crate::ui::audio::Audio>,
    answered: HashSet<u64>,
    screen: (i32, i32),
    harness: Harness,
}

impl eframe::App for App {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        self.harness.inject(raw);
    }

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
        // A save slot chosen in the game UI answers the program's Save/Open dialog.
        if let Some((id, Dialog::File { .. })) = &snap.dialog {
            if !self.answered.contains(id) && self.ui.answer_file_dialog(&snap, &self.tx) {
                self.answered.insert(*id);
            }
        }
        // Native file dialogs for TOpenDialog / TSaveDialog.Execute.
        if let Some((id, Dialog::File { save, filter, default_ext, initial, .. })) = &snap.dialog {
            if let Some(path) = self.harness.pending_file.take() {
                self.answered.insert(*id);
                let _ = self.tx.send(Input::Answer { id: *id, answer: DialogAnswer::File(Some(path)) });
            } else if self.answered.insert(*id) {
                #[cfg(target_os = "android")]
                {
                    // No file picker on Android (Music > Open file, HTML export): cancel.
                    let _ = (save, filter, default_ext, initial);
                    let _ = self.tx.send(Input::Answer { id: *id, answer: DialogAnswer::File(None) });
                }
                #[cfg(not(target_os = "android"))]
                {
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
        }
        for c in std::mem::take(&mut self.harness.ui_commands) {
            self.ui.debug_command(&c);
        }
        self.ui.show(ctx, &snap, fx, &self.assets, &self.tx);
        if let Some(a) = &mut self.audio {
            a.sync_media(&snap.media, &self.tx);
            let cue = if a.media_playing() { None } else { self.ui.music_cue(&snap) };
            a.set_cue(cue);
            if !snap.media.file_name.is_empty() {
                ctx.request_repaint_after(std::time::Duration::from_millis(200));
            }
        }
    }
}

/// Start the program with the two original files' contents and ParamStr(1..).
pub fn run(options: eframe::NativeOptions, exe: &[u8], dat: &[u8], params: Vec<String>) -> Result<(), String> {
    let assets = Arc::new(Assets::from_bytes(exe, dat)?);
    let data = crate::assets::data_section(exe);
    let snapshot = Arc::new(Mutex::new(Snapshot::default()));
    let (tx, rx) = channel();
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
                audio: crate::ui::audio::Audio::new(),
                answered: HashSet::new(),
                screen: (0, 0),
                harness: Harness::from_env(),
            }))
        }),
    );
    result.map_err(|e| e.to_string())
}
