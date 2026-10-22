//! Title screen (Form1) and the save-slot sheets.
//!
//! New / Load still go through the original handlers (Form1.NewButton / LoadButton); the slot
//! the player picked is handed to the program as the answer to its Save/Open dialog.

use super::slots::{self, Summary};
use super::theme as t;
use crate::assets::Assets;
use crate::vcl::render::Renderer;
use crate::vcl::runtime::{Event, Input, Snapshot};
use egui::{Align2, Color32, Pos2, Rect, Sense, Vec2};
use std::sync::mpsc::Sender;

pub enum Request {
    OpenSheet(String),
    /// Answer the program's next Save/Open dialog with this path.
    Prepare(String),
}

#[derive(Default)]
pub struct Title {
    started: bool,
    /// Cached slot summaries and when they were read.
    cache: Option<(f64, [Option<Summary>; slots::COUNT])>,
    /// Slot waiting for a second tap to confirm overwriting it.
    confirm: Option<(usize, f64)>,
    pub message: Option<(String, f64)>,
    /// Harness: press Continue on the next frame.
    pub debug_continue: bool,
}

fn click(tx: &Sender<Input>, control: &str) {
    let _ = tx.send(Input::Event { form: "Form1".into(), control: control.into(), event: Event::Click });
}

impl Title {
    pub fn summaries(&mut self, now: f64) -> [Option<Summary>; slots::COUNT] {
        if self.cache.as_ref().is_none_or(|(at, _)| now - at > 1.0) {
            self.cache = Some((now, std::array::from_fn(slots::summary)));
        }
        self.cache.clone().unwrap().1
    }

    /// Skip "TAP TO START".
    pub fn start(&mut self) {
        self.started = true;
    }

    pub fn refresh(&mut self) {
        self.cache = None;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn show(&mut self, ui: &mut egui::Ui, area: Rect, snap: &Snapshot, renderer: &mut Renderer, assets: &Assets, tx: &Sender<Input>, now: f64, animate: bool) -> Option<Request> {
        let free = snap.dialog.is_none();
        let p = ui.painter().clone();
        let mut req = None;
        let col_w = area.width().min(520.0);
        let cx = area.center().x;

        // Sprite parade along the bottom.
        let parade_y = area.bottom() - 70.0;
        let n = 9;
        for k in 0..n {
            let speed = 40.0;
            let span = area.width() + 140.0;
            let x = if animate { area.left() - 70.0 + ((now as f32 * speed + k as f32 * span / n as f32) % span) } else { area.left() + (k as f32 + 0.5) * area.width() / n as f32 };
            let species = [25, 1, 4, 7, 133, 39, 52, 54, 143][k];
            if let Some(tex) = renderer.texture(ui.ctx(), assets, "Form4.PKMNImages", species - 1) {
                let hop = if animate { ((now * 6.0 + k as f64).sin().abs() * 8.0) as f32 } else { 0.0 };
                let r = Rect::from_center_size(Pos2::new(x, parade_y - hop), Vec2::splat(64.0));
                // Walking right: flip so they face the way they move.
                p.image(tex.id(), r, Rect::from_min_max(Pos2::new(1.0, 0.0), Pos2::new(0.0, 1.0)), Color32::WHITE);
            }
        }

        // Logo.
        let logo_y = area.top() + area.height() * 0.16;
        let bob = if animate { ((now * 1.5).sin() * 4.0) as f32 } else { 0.0 };
        let big = (col_w / 9.0).clamp(26.0, 52.0);
        for (dx, dy, c) in [(4.0, 4.0, t::pal().frame), (0.0, 0.0, Color32::from_rgb(0xF8, 0xD0, 0x30))] {
            p.text(Pos2::new(cx + dx, logo_y + bob + dy), Align2::CENTER_CENTER, "POKéMON", t::title(big), c);
        }
        // Outline effect for the yellow word.
        p.text(Pos2::new(cx, logo_y + bob + big * 0.95), Align2::CENTER_CENTER, "SIMULATOR", t::title(big * 0.42), t::ON_ACCENT);
        p.text(Pos2::new(cx, logo_y + bob + big * 1.55), Align2::CENTER_CENTER, "V4.5", t::title(big * 0.3), t::pal().frame);

        if !self.started {
            let blink = !animate || (now * 1.6).fract() < 0.65;
            if blink {
                t::text(&p, Pos2::new(cx, area.center().y + 40.0), Align2::CENTER_CENTER, "TAP TO START", t::title(16.0), t::ON_ACCENT);
            }
            let tapped = ui.interact(area, ui.id().with("title_tap"), Sense::click()).clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Space));
            if tapped && free {
                self.started = true;
            }
            return None;
        }

        // Menu.
        let latest = slots::latest();
        let sums = self.summaries(now);
        let bw = (col_w - 32.0).min(360.0);
        let mut y = logo_y + big * 2.3;
        let row = 58.0;
        let mut button = |ui: &mut egui::Ui, label: &str, kind: t::ButtonKind, enabled: bool| {
            let r = Rect::from_center_size(Pos2::new(cx, y + row / 2.0), Vec2::new(bw, row - 10.0));
            y += row;
            t::button(ui, r, label, kind, enabled && free).clicked()
        };
        if let Some(s) = latest {
            let label = sums[s].as_ref().map_or("CONTINUE".to_string(), |m| format!("CONTINUE · {}", m.name));
            let pressed = button(ui, &label, t::ButtonKind::Primary, true) || std::mem::take(&mut self.debug_continue);
            if pressed {
                if let Some(p) = slots::path(s) {
                    req = Some(Request::Prepare(p.to_string_lossy().into_owned()));
                    click(tx, "LoadButton");
                }
            }
        }
        if button(ui, "NEW GAME", if latest.is_some() { t::ButtonKind::Plain } else { t::ButtonKind::Primary }, true) {
            req = Some(Request::OpenSheet(super::SLOTS_NEW.into()));
        }
        if button(ui, "LOAD", t::ButtonKind::Plain, true) {
            req = Some(Request::OpenSheet(super::SLOTS_LOAD.into()));
        }
        if button(ui, "OPTIONS", t::ButtonKind::Plain, true) {
            req = Some(Request::OpenSheet(super::OPTIONS.into()));
        }
        if button(ui, "ABOUT", t::ButtonKind::Plain, true) {
            click(tx, "AboutButton");
        }
        if !cfg!(target_os = "android") && button(ui, "EXIT", t::ButtonKind::Danger, true) {
            click(tx, "ExitButton");
        }
        req
    }

    /// Slot list for New Game (`new_game`) or Load.
    #[allow(clippy::too_many_arguments)]
    pub fn slots_sheet(&mut self, ui: &mut egui::Ui, new_game: bool, snap: &Snapshot, renderer: &mut Renderer, assets: &Assets, tx: &Sender<Input>, now: f64) -> Option<Request> {
        let free = snap.dialog.is_none();
        let width = ui.available_width();
        let sums = self.summaries(now);
        let mut req = None;
        if let Some((msg, at)) = &self.message {
            if now - at < 4.0 {
                let (r, _) = ui.allocate_exact_size(Vec2::new(width, 26.0), Sense::hover());
                t::text(ui.painter(), r.left_center(), Align2::LEFT_CENTER, msg, t::bold(16.0), t::pal().danger);
            }
        }
        for slot in 0..slots::COUNT {
            let (r, _) = ui.allocate_exact_size(Vec2::new(width, 150.0), Sense::hover());
            let card = r.shrink2(Vec2::new(0.0, 5.0));
            let p = ui.painter().clone();
            t::frame(&p, card);
            let inner = card.shrink2(Vec2::new(18.0, 12.0));
            t::text(&p, inner.left_top(), Align2::LEFT_TOP, &format!("SLOT {}", slot + 1), t::title(11.0), t::pal().grey);
            let sum = &sums[slot];
            match sum {
                Some(m) => {
                    if m.lead > 0 {
                        if let Some(tex) = renderer.texture(ui.ctx(), assets, "Form4.PKMNImages", m.lead - 1) {
                            p.image(tex.id(), Rect::from_min_size(inner.left_top() + Vec2::new(-6.0, 14.0), Vec2::splat(64.0)), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                        }
                    }
                    t::text(&p, inner.left_top() + Vec2::new(66.0, 18.0), Align2::LEFT_TOP, &m.name, t::bold(24.0), t::pal().ink);
                    t::text(&p, inner.left_top() + Vec2::new(66.0, 48.0), Align2::LEFT_TOP, &format!("Badges {}  ·  ¥{}  ·  Dex {}  ·  PC {}", m.badges, m.money, m.dex, m.pc), t::body(16.0), t::pal().ink);
                }
                None => {
                    t::text(&p, inner.left_top() + Vec2::new(0.0, 34.0), Align2::LEFT_CENTER, "— empty —", t::bold(20.0), t::pal().grey);
                }
            }
            // Actions on the right.
            let bh = 40.0;
            let bw = 104.0;
            let right = inner.right();
            let mut bx = right - bw;
            let mut action = |ui: &mut egui::Ui, label: &str, kind: t::ButtonKind, enabled: bool| {
                let b = Rect::from_min_size(Pos2::new(bx, inner.bottom() - bh), Vec2::new(bw, bh));
                bx -= bw + 8.0;
                t::button(ui, b, label, kind, enabled && free).clicked()
            };
            let path = slots::path(slot).map(|p| p.to_string_lossy().into_owned());
            if new_game {
                let confirming = self.confirm.is_some_and(|(s, at)| s == slot && now - at < 3.0);
                let label = if sum.is_none() { "START" } else if confirming { "SURE?" } else { "OVERWRITE" };
                if action(ui, label, if confirming { t::ButtonKind::Danger } else { t::ButtonKind::Primary }, path.is_some()) {
                    if sum.is_some() && !confirming {
                        self.confirm = Some((slot, now));
                    } else if let Some(p) = path.clone() {
                        slots::ensure_dir();
                        self.confirm = None;
                        req = Some(Request::Prepare(p));
                        click(tx, "NewButton");
                    }
                }
            } else {
                if action(ui, "LOAD", t::ButtonKind::Primary, sum.is_some()) {
                    if let Some(p) = path.clone() {
                        req = Some(Request::Prepare(p));
                        click(tx, "LoadButton");
                    }
                }
                // Web: the browser's download / file picker instead of native dialogs.
                #[cfg(target_arch = "wasm32")]
                {
                    if action(ui, "EXPORT", t::ButtonKind::Plain, sum.is_some()) {
                        if let (Some(p), Some(m)) = (slots::path(slot), sum) {
                            if let Ok(b) = crate::store::read(p) {
                                crate::web::download(&format!("{}.pgs", m.name), &b);
                            }
                        }
                    }
                    let tag = format!("slot{slot}");
                    if let Some((_, bytes)) = crate::web::take_upload(&tag) {
                        match slots::import_bytes(slot, &bytes) {
                            Ok(()) => self.refresh(),
                            Err(e) => self.message = Some((e, now)),
                        }
                    }
                    let confirming = self.confirm.is_some_and(|(s, at)| s == slot && now - at < 3.0);
                    let label = if confirming { "REPLACE?" } else { "IMPORT" };
                    if action(ui, label, if confirming { t::ButtonKind::Danger } else { t::ButtonKind::Plain }, true) {
                        if sum.is_some() && !confirming {
                            self.confirm = Some((slot, now));
                        } else {
                            self.confirm = None;
                            crate::web::pick_file(&tag, ".pgs");
                        }
                    }
                }
                #[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
                {
                    if action(ui, "EXPORT", t::ButtonKind::Plain, sum.is_some()) {
                        let name = sum.as_ref().map_or("save".into(), |m| m.name.clone());
                        if let Some(to) = rfd::FileDialog::new().add_filter("POKéMON Simulator save", &["pgs"]).set_file_name(format!("{name}.pgs")).save_file() {
                            if let Err(e) = slots::export(slot, &to) {
                                self.message = Some((e, now));
                            }
                        }
                    }
                    let confirming = self.confirm.is_some_and(|(s, at)| s == slot && now - at < 3.0);
                    let label = if confirming { "REPLACE?" } else { "IMPORT" };
                    if action(ui, label, if confirming { t::ButtonKind::Danger } else { t::ButtonKind::Plain }, true) {
                        if sum.is_some() && !confirming {
                            self.confirm = Some((slot, now));
                        } else if let Some(from) = rfd::FileDialog::new().add_filter("POKéMON Simulator save", &["pgs"]).pick_file() {
                            match slots::import(slot, &from) {
                                Ok(()) => self.refresh(),
                                Err(e) => self.message = Some((e, now)),
                            }
                        }
                    }
                }
            }
        }
        req
    }
}
