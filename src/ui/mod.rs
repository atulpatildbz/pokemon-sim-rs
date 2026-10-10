//! The game front-end: scenes, slide-up sheets, the Pokémon text box and the Options screen,
//! layered over the faithful VCL model. Nothing here changes game state except by sending the
//! same [`Input`]s the original windows would have produced.
//!
//! Forms without a bespoke screen yet are hosted generically: drawn by the VCL renderer in the
//! game skin, scaled to fit their scene or sheet.

pub mod battle;
pub mod hub;
pub mod naming;
pub mod screens;
pub mod settings;
pub mod slots;
pub mod starter;
pub mod textbox;
pub mod theme;
pub mod title;

use crate::assets::Assets;
use crate::vcl::model::{Form, MenuItem};
use crate::vcl::render::{strip_amp, Renderer, Skin, GAME_FACE, MENU_HEIGHT};
use crate::vcl::runtime::{Dialog, Fx, Input, Snapshot};
use egui::emath::TSTransform;
use egui::{Align2, Color32, Id, LayerId, Order, Pos2, Rect, Sense, Vec2};
use settings::{Settings, TextSpeed};
use std::sync::mpsc::Sender;
use theme as t;

/// Forms that fill the screen; every other form is a sheet.
const SCENES: [&str; 3] = ["Form3", "Form1", "Form4"];
const DOCK_H: f32 = 64.0;

/// Sheets that are not forms.
pub const OPTIONS: &str = "#options";
pub const SLOTS_NEW: &str = "#slots_new";
pub const SLOTS_LOAD: &str = "#slots_load";
const MENU: &str = "#menu";
pub const BATTLE_INFO: &str = "#battleinfo";
/// Opponent cards for a menu group (path in `cards_path`).
const CARDS: &str = "#cards";

pub struct GameUi {
    pub settings: Settings,
    renderer: Renderer,
    textbox: textbox::TextBox,
    raise_seq: u64,
    /// The sheet the player is looking at (a form var, or OPTIONS / MENU).
    sheet: Option<String>,
    /// Last sheet shown, kept while it animates out.
    sheet_closing: Option<String>,
    /// Drill-down path in the menu sheet (item names).
    menu_path: Vec<String>,
    scene: String,
    scene_changed_at: f64,
    battle: battle::BattleView,
    /// A form the program raised while a battle was still replaying; opened afterwards.
    pending_raise: Option<String>,
    own_was_out: bool,
    cards_path: Vec<String>,
    autosaves: u64,
    saved_at: f64,
    title: title::Title,
    naming: naming::Naming,
    starter: starter::Starter,
    /// Path to give the program's next Save/Open dialog (a save slot).
    pending_file: Option<String>,
    dex: screens::Dex,
    types: screens::TypeChart,
    corner: screens::GameCorner,
    /// Evolution being shown: (from, to, start time).
    evolving: Option<(i32, i32, f64)>,
}

impl GameUi {
    pub fn new(ctx: &egui::Context) -> GameUi {
        let mut fonts = egui::FontDefinitions::default();
        crate::vcl::render::add_system_fonts(&mut fonts);
        t::install_fonts(&mut fonts);
        ctx.set_fonts(fonts);
        let ui = GameUi {
            settings: Settings::load(),
            renderer: Renderer::default(),
            textbox: textbox::TextBox::default(),
            raise_seq: 0,
            sheet: None,
            sheet_closing: None,
            menu_path: Vec::new(),
            scene: String::new(),
            scene_changed_at: 0.0,
            battle: battle::BattleView::default(),
            pending_raise: None,
            own_was_out: false,
            cards_path: Vec::new(),
            autosaves: 0,
            saved_at: f64::NEG_INFINITY,
            title: title::Title::default(),
            naming: naming::Naming::default(),
            starter: starter::Starter::default(),
            pending_file: None,
            dex: screens::Dex::default(),
            types: screens::TypeChart::default(),
            corner: screens::GameCorner::default(),
            evolving: None,
        };
        ui.apply_visuals(ctx);
        ui
    }

    /// Answer the program's Save/Open dialog with the slot chosen in the game UI. Returns true
    /// if it was answered (the native dialog must not open).
    pub fn answer_file_dialog(&mut self, snap: &Snapshot, tx: &Sender<Input>) -> bool {
        if self.settings.classic {
            return false;
        }
        match (&snap.dialog, self.pending_file.take()) {
            (Some((id, Dialog::File { .. })), Some(path)) => {
                let _ = tx.send(Input::Answer { id: *id, answer: crate::vcl::runtime::DialogAnswer::File(Some(path)) });
                self.title.refresh();
                true
            }
            (_, p) => {
                self.pending_file = p;
                false
            }
        }
    }

    /// Scripted front-end actions for the screenshot harness (`ui sheet Form5`, `ui cards Action1/FightTrainer1`, `ui close`).
    pub fn debug_command(&mut self, cmd: &str) {
        let mut it = cmd.split_whitespace();
        match (it.next(), it.next()) {
            (Some("sheet"), Some(name)) => self.sheet = Some(name.to_string()),
            (Some("cards"), Some(path)) => {
                self.cards_path = path.split('/').map(str::to_string).collect();
                self.sheet = Some(CARDS.into());
            }
            (Some("close"), _) => self.sheet = None,
            (Some("start"), _) => self.title.start(),
            (Some("continue"), _) => self.title.debug_continue = true,
            _ => eprintln!("ui: unknown command {cmd}"),
        }
    }

    fn apply_visuals(&self, ctx: &egui::Context) {
        if self.settings.classic {
            crate::vcl::render::classic_visuals(ctx);
        } else {
            ctx.set_visuals(t::game_visuals());
        }
    }

    fn set_classic(&mut self, ctx: &egui::Context, on: bool) {
        self.settings.classic = on;
        self.settings.save();
        self.apply_visuals(ctx);
    }

    pub fn show(&mut self, ctx: &egui::Context, snap: &Snapshot, fx: Vec<Fx>, assets: &Assets, tx: &Sender<Input>) {
        if ctx.input(|i| i.key_pressed(egui::Key::F2)) {
            let on = !self.settings.classic;
            self.set_classic(ctx, on);
        }
        if self.settings.classic {
            self.renderer.skin = Skin::Classic;
            self.renderer.show(ctx, snap, assets, tx);
            self.classic_overlay(ctx, snap, tx);
            return;
        }
        self.renderer.skin = Skin::Game;
        let (evolve, fx): (Vec<Fx>, Vec<Fx>) = fx.into_iter().partition(|f| matches!(f, Fx::Evolve { .. }));
        if let Some(Fx::Evolve { from, to }) = evolve.last() {
            self.evolving = Some((*from, *to, ctx.input(|i| i.time)));
        }
        if battle::active(snap) {
            self.battle.feed(fx);
        } else {
            self.battle = battle::BattleView::default();
        }
        self.game(ctx, snap, assets, tx);
    }

    /// Classic mode: a small gear in the corner opens Options as a window.
    fn classic_overlay(&mut self, ctx: &egui::Context, snap: &Snapshot, tx: &Sender<Input>) {
        let screen = ctx.screen_rect();
        egui::Area::new(Id::new("classic_gear")).order(Order::Foreground).fixed_pos(screen.right_bottom() - Vec2::new(36.0, 36.0)).show(ctx, |ui| {
            if ui.add(egui::Button::new("⚙").min_size(Vec2::splat(28.0))).on_hover_text("Options (F2: game mode)").clicked() {
                self.sheet = if self.sheet.as_deref() == Some(OPTIONS) { None } else { Some(OPTIONS.into()) };
            }
        });
        if self.sheet.as_deref() == Some(OPTIONS) {
            let mut open = true;
            egui::Window::new("Options").open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
                ui.set_width(320.0);
                self.options_body(ui, ctx, snap, tx);
            });
            if !open {
                self.sheet = None;
            }
        }
    }

    // ------------------------------------------------------------------ game mode

    fn game(&mut self, ctx: &egui::Context, snap: &Snapshot, assets: &Assets, tx: &Sender<Input>) {
        let now = ctx.input(|i| i.time);
        let screen = ctx.screen_rect();
        let wide = screen.width() >= 820.0 && screen.width() > screen.height() * 1.15;
        let anim = |secs: f32| if self.settings.animations { secs } else { 0.0 };

        egui::CentralPanel::default().frame(egui::Frame::new()).show(ctx, |ui| {
            t::backdrop(ui.painter(), screen, if self.settings.animations { now } else { 0.0 });
        });
        if self.settings.animations {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }

        let modal_form = match &snap.dialog {
            Some((_, Dialog::ModalForm { var })) => Some(var.clone()),
            _ => None,
        };
        let any_dialog = snap.dialog.is_some();
        let visible: Vec<&Form> = snap.forms.iter().filter(|f| f.visible).collect();
        let by_z = |a: &&Form| snap.z_order.iter().position(|v| *v == a.var).unwrap_or(0);

        // Scene = the top-most visible full-screen form.
        let scene = visible.iter().filter(|f| SCENES.contains(&f.var.as_str())).max_by_key(|f| by_z(f)).map(|f| f.var.clone()).unwrap_or_default();
        if scene != self.scene {
            self.scene = scene.clone();
            self.scene_changed_at = now;
        }
        let sheets: Vec<&Form> = {
            // The name dialog is drawn as a full screen of its own.
            let mut v: Vec<&Form> = visible.iter().copied().filter(|f| !SCENES.contains(&f.var.as_str()) && f.var != "OKRightDlg").collect();
            v.sort_by_key(by_z);
            v
        };

        // A form the program just brought to the front opens as the sheet (after any battle replay).
        if snap.raise_seq != self.raise_seq {
            self.raise_seq = snap.raise_seq;
            if let Some(top) = snap.z_order.last() {
                // The battle log comes forward on every battle; it stays a chip instead of covering the fight.
                if top != "Form7" && sheets.iter().any(|f| f.var == *top) {
                    self.pending_raise = Some(top.clone());
                }
            }
        }
        if !self.battle.busy() {
            if let Some(top) = self.pending_raise.take() {
                if sheets.iter().any(|f| f.var == top) {
                    self.sheet = Some(top);
                }
            }
        }
        if let Some(m) = &modal_form {
            if sheets.iter().any(|f| f.var == *m) {
                self.sheet = Some(m.clone());
            }
        }
        // A form sheet whose form was hidden closes.
        if let Some(s) = self.sheet.clone() {
            let gone = !s.starts_with('#') && !sheets.iter().any(|f| f.var == s);
            let no_menu = s == MENU && !visible.iter().any(|f| f.var == self.scene && !f.menu.is_empty());
            if gone || no_menu {
                self.sheet = None;
            }
        }

        // Layout.
        let open = ctx.animate_bool_with_time(Id::new("sheet_open"), self.sheet.is_some(), anim(0.22));
        let open = ease_out(open);
        if self.sheet.is_some() {
            self.sheet_closing = self.sheet.clone();
        } else if open == 0.0 {
            self.sheet_closing = None;
        }
        let panel_w = (screen.width() * 0.42).clamp(380.0, 620.0);
        let naming = modal_form.as_deref() == Some("OKRightDlg");
        let show_dock = self.scene == "Form4" && !naming;
        let dock_h = if show_dock { DOCK_H } else { 0.0 };
        let scene_rect = if wide {
            Rect::from_min_max(screen.min, Pos2::new(screen.right() - panel_w * open, screen.bottom() - dock_h))
        } else {
            Rect::from_min_max(screen.min, Pos2::new(screen.right(), screen.bottom() - dock_h))
        };

        // Scene.
        let scene_fade = ctx.animate_bool_with_time(Id::new(("scene_in", self.scene.as_str())), true, anim(0.35));
        let in_battle = self.scene == "Form4" && battle::active(snap);
        if !starter::active(snap) {
            self.starter.reset();
        }
        if naming {
            let id = snap.dialog.as_ref().map_or(0, |(id, _)| *id);
            egui::Area::new(Id::new("naming")).order(Order::Middle).fixed_pos(Pos2::ZERO).show(ctx, |ui| {
                ui.set_clip_rect(scene_rect);
                self.naming.show(ui, scene_rect, snap, id, tx, now);
            });
        } else if self.scene == "Form1" {
            let mut req = None;
            let animate = self.settings.animations;
            egui::Area::new(Id::new("title")).order(Order::Middle).fixed_pos(Pos2::ZERO).show(ctx, |ui| {
                ui.set_clip_rect(scene_rect);
                ui.multiply_opacity(scene_fade);
                ui.add_enabled_ui(self.sheet.is_none() || wide, |ui| {
                    req = self.title.show(ui, scene_rect, snap, &mut self.renderer, assets, tx, now, animate);
                });
            });
            match req {
                Some(title::Request::OpenSheet(s)) => self.sheet = Some(s),
                Some(title::Request::Prepare(p)) => self.pending_file = Some(p),
                None => {}
            }
        } else if self.scene == "Form4" && screens::evolve_choice_active(snap) {
            egui::Area::new(Id::new("evolve_choice")).order(Order::Middle).fixed_pos(Pos2::ZERO).show(ctx, |ui| {
                ui.set_clip_rect(scene_rect);
                ui.add_enabled_ui(self.sheet.is_none() || wide, |ui| {
                    screens::evolve_choice(ui, scene_rect, snap, &mut self.renderer, assets, tx);
                });
            });
        } else if self.scene == "Form4" && starter::active(snap) {
            let animate = self.settings.animations;
            egui::Area::new(Id::new("starter")).order(Order::Middle).fixed_pos(Pos2::ZERO).show(ctx, |ui| {
                ui.set_clip_rect(scene_rect);
                ui.multiply_opacity(scene_fade);
                self.starter.show(ui, scene_rect, snap, &mut self.renderer, assets, tx, animate, now);
            });
        } else if in_battle {
            let mut req = None;
            egui::Area::new(Id::new("battle")).order(Order::Middle).fixed_pos(Pos2::ZERO).show(ctx, |ui| {
                ui.set_clip_rect(scene_rect);
                ui.add_enabled_ui(self.sheet.is_none() || wide, |ui| {
                    req = self.battle.show(ui, scene_rect, snap, &mut self.renderer, assets, &self.settings, tx);
                });
            });
            if let Some(battle::Request::OpenSheet(s)) = req {
                self.sheet = Some(s);
            }
            // Picking a POKéMON from Statistics sends it out: back to the fight.
            let out = self.battle.own_out();
            if out && !self.own_was_out && self.sheet.as_deref() == Some("Form5") {
                self.sheet = None;
            }
            self.own_was_out = out;
        } else if self.scene == "Form4" && hub::active(snap) {
            let mut req = None;
            let animate = self.settings.animations;
            egui::Area::new(Id::new("hub")).order(Order::Middle).fixed_pos(Pos2::ZERO).show(ctx, |ui| {
                ui.set_clip_rect(scene_rect);
                ui.multiply_opacity(scene_fade);
                ui.add_enabled_ui(self.sheet.is_none() || wide, |ui| {
                    req = hub::show(ui, scene_rect, snap, &mut self.renderer, assets, tx, now, animate);
                });
            });
            match req {
                Some(hub::Request::OpenSheet(s)) => self.sheet = Some(s),
                Some(hub::Request::OpenCards(path)) => {
                    self.cards_path = path;
                    self.sheet = Some(CARDS.into());
                }
                None => {}
            }
        } else if let Some(form) = visible.iter().find(|f| f.var == self.scene) {
            let interactive = !any_dialog || modal_form.as_deref() == Some(form.var.as_str());
            self.host_form(ctx, form, scene_rect.shrink(8.0), LayerId::new(Order::Middle, Id::new(("scene", &form.var))), interactive && (self.sheet.is_none() || wide), assets, tx, scene_fade, true);
        }
        if scene_fade < 1.0 {
            // Fade in from white on scene changes.
            let a = ((1.0 - scene_fade) * 255.0) as u8;
            ctx.layer_painter(LayerId::new(Order::Middle, Id::new("scene_fade"))).rect_filled(screen, 0.0, Color32::from_white_alpha(a));
        }

        if show_dock {
            self.dock(ctx, screen, scene_rect, &sheets, snap, wide);
        }

        // Sheet.
        if let Some(name) = self.sheet_closing.clone() {
            let sheet_rect = if wide {
                Rect::from_min_size(Pos2::new(screen.right() - panel_w * open, screen.top()), Vec2::new(panel_w, screen.height()))
            } else {
                let h = (screen.height() * 0.86).min(screen.height() - 40.0);
                Rect::from_min_size(Pos2::new(screen.left(), screen.bottom() - h * open), Vec2::new(screen.width(), h))
            };
            if !wide {
                // Dim the scene; tapping it closes the sheet (unless a modal form needs an answer).
                let dim = egui::Area::new(Id::new("sheet_dim")).order(Order::Foreground).fixed_pos(Pos2::ZERO).show(ctx, |ui| {
                    ui.painter().rect_filled(screen, 0.0, Color32::from_black_alpha((110.0 * open) as u8));
                    ui.allocate_rect(screen, Sense::click()).clicked()
                });
                if dim.inner && modal_form.is_none() && !any_dialog {
                    self.sheet = None;
                }
            }
            let layer = LayerId::new(Order::Foreground, Id::new("sheet"));
            egui::Area::new(layer.id).order(Order::Foreground).fixed_pos(sheet_rect.min).constrain(false).show(ctx, |ui| {
                let p = ui.painter();
                let r = Rect::from_min_size(sheet_rect.min, sheet_rect.size() + Vec2::new(0.0, 20.0));
                p.rect_filled(r.translate(Vec2::new(0.0, -4.0)), 18.0, Color32::from_black_alpha(40));
                p.rect_filled(r, 18.0, t::FRAME);
                p.rect_filled(r.shrink(t::PX * 1.5), 16.0, GAME_FACE);
                // Header: grab handle, title, close.
                let title = match name.as_str() {
                    OPTIONS => "Options".to_string(),
                    MENU => "Menu".to_string(),
                    BATTLE_INFO => "Battle Info".to_string(),
                    SLOTS_NEW => "New Game".to_string(),
                    SLOTS_LOAD => "Load Game".to_string(),
                    CARDS => visible
                        .iter()
                        .find(|f| f.var == "Form4")
                        .and_then(|f| hub::find_item(f, &self.cards_path))
                        .map(|(m, _)| strip_amp(&m.caption))
                        .unwrap_or_default(),
                    v => sheets.iter().find(|f| f.var == v).map(|f| sheet_title(&f.caption)).unwrap_or_default(),
                };
                if !wide {
                    p.rect_filled(Rect::from_center_size(Pos2::new(r.center().x, r.top() + 10.0), Vec2::new(48.0, 5.0)), 3.0, t::GREY);
                }
                t::text(p, r.left_top() + Vec2::new(20.0, 34.0), Align2::LEFT_CENTER, &title, t::bold(22.0), t::INK);
                let can_close = modal_form.as_deref() != Some(name.as_str());
                if can_close {
                    let b = Rect::from_center_size(r.right_top() + Vec2::new(-34.0, 34.0), Vec2::splat(40.0));
                    if t::button(ui, b, "", t::ButtonKind::Plain, true).clicked() {
                        self.sheet = None;
                    }
                    t::icon(ui.painter(), b.center() - Vec2::new(0.0, 2.0), 18.0, t::Icon::Close, t::INK);
                }
                let body = Rect::from_min_max(sheet_rect.min + Vec2::new(12.0, 62.0), sheet_rect.max - Vec2::new(12.0, 12.0));
                ui.allocate_rect(sheet_rect, Sense::hover());
                match name.as_str() {
                    OPTIONS => {
                        ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| self.options_body(ui, ctx, snap, tx));
                    }
                    MENU => {
                        if let Some(form) = visible.iter().find(|f| f.var == self.scene) {
                            ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| self.menu_body(ui, form, tx, !any_dialog));
                        }
                    }
                    CARDS => {
                        if let Some(f4) = visible.iter().find(|f| f.var == "Form4") {
                            let path = self.cards_path.clone();
                            let fired = ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| hub::cards(ui, f4, &path, tx, !any_dialog)).inner;
                            if fired {
                                self.sheet = None;
                            }
                        }
                    }
                    SLOTS_NEW | SLOTS_LOAD => {
                        let new_game = name == SLOTS_NEW;
                        let req = ui
                            .scope_builder(egui::UiBuilder::new().max_rect(body), |ui| {
                                egui::ScrollArea::vertical().id_salt("slots").show(ui, |ui| self.title.slots_sheet(ui, new_game, snap, &mut self.renderer, assets, tx, now)).inner
                            })
                            .inner;
                        if let Some(title::Request::Prepare(p)) = req {
                            self.pending_file = Some(p);
                            self.sheet = None;
                        }
                    }
                    BATTLE_INFO => {
                        ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| battle_info_body(ui, snap));
                    }
                    _ => {}
                }
            });
            ctx.move_to_top(layer);
            let bespoke = matches!(name.as_str(), "Form5" | "Form8" | "Form9" | "FormA" | "FormB");
            let animate = self.settings.animations;
            if bespoke {
                let body = Rect::from_min_max(sheet_rect.min + Vec2::new(14.0, 62.0), sheet_rect.max - Vec2::new(14.0, 12.0));
                let interactive = !any_dialog && self.sheet.is_some();
                let blayer = LayerId::new(Order::Foreground, Id::new(("sheetbody", name.as_str())));
                egui::Area::new(blayer.id).order(Order::Foreground).fixed_pos(body.min).constrain(false).show(ctx, |ui| {
                    ui.set_clip_rect(body);
                    ui.set_width(body.width());
                    ui.set_max_height(body.height());
                    ui.add_enabled_ui(interactive, |ui| match name.as_str() {
                        "Form5" => screens::pc(ui, snap, &mut self.renderer, assets, tx),
                        "Form8" => self.dex.show(ui, snap, &mut self.renderer, assets, tx),
                        "FormA" => self.types.show(ui, snap, tx),
                        "FormB" => egui::ScrollArea::vertical().id_salt("corner").show(ui, |ui| self.corner.show(ui, snap, tx, now, animate)).inner,
                        _ => egui::ScrollArea::vertical().id_salt("mart").show(ui, |ui| screens::mart(ui, snap, tx)).inner,
                    });
                });
                ctx.move_to_top(blayer);
            } else if let Some(form) = sheets.iter().find(|f| f.var == name) {
                let body = Rect::from_min_max(sheet_rect.min + Vec2::new(12.0, 62.0), sheet_rect.max - Vec2::new(12.0, 12.0));
                let interactive = !any_dialog || modal_form.as_deref() == Some(form.var.as_str());
                let flayer = LayerId::new(Order::Foreground, Id::new(("sheetform", &form.var)));
                self.host_form(ctx, form, body, flayer, interactive && self.sheet.is_some(), assets, tx, 1.0, false);
                ctx.move_to_top(flayer);
            }
        }

        // "Saved" hint after each auto-save.
        if snap.autosaves != self.autosaves {
            if self.autosaves != 0 || snap.autosaves > 1 {
                self.saved_at = now;
            }
            self.autosaves = snap.autosaves;
        }
        let age = (now - self.saved_at) as f32;
        if age < 1.8 {
            let a = (1.0 - ((age - 1.2) / 0.6).max(0.0)).clamp(0.0, 1.0);
            let r = Rect::from_min_size(Pos2::new(scene_rect.right() - 118.0, scene_rect.top() + 10.0), Vec2::new(106.0, 34.0));
            let p = ctx.layer_painter(LayerId::new(Order::Tooltip, Id::new("saved")));
            p.rect_filled(r, 17.0, t::FRAME.gamma_multiply(0.85 * a));
            t::icon(&p, r.left_center() + Vec2::new(18.0, 0.0), 14.0, t::Icon::Check, Color32::from_rgb(0x88, 0xE0, 0x98).gamma_multiply(a));
            p.text(r.left_center() + Vec2::new(32.0, 0.0), Align2::LEFT_CENTER, "Saved", t::bold(18.0), t::PAPER.gamma_multiply(a));
            ctx.request_repaint();
        }

        // Evolution overlay; its "X evolved into Y!" message waits for the animation.
        let evolving_busy = self.evolution(ctx, screen, snap, assets, now);

        // Message boxes wait until the battle has replayed what led to them.
        if let Some((id, Dialog::Message { text, kind, buttons })) = &snap.dialog {
            let corner_busy = self.settings.animations && self.corner.busy(now);
            if !(in_battle && self.battle.busy()) && !evolving_busy && !corner_busy {
                self.textbox.show(ctx, scene_rect, *id, text, *kind, *buttons, &self.settings, tx);
            }
        }
    }

    /// The evolution sequence: the old and new sprite alternate faster and faster in white,
    /// then the new one bursts out. Returns true while the animation is still running.
    fn evolution(&mut self, ctx: &egui::Context, screen: Rect, snap: &Snapshot, assets: &Assets, now: f64) -> bool {
        let Some((from, to, at)) = self.evolving else { return false };
        let dur: f32 = if self.settings.animations { 3.2 } else { 0.0 };
        let k = ((now - at) as f32 / dur.max(0.001)).min(1.0);
        let done = dur == 0.0 || k >= 1.0;
        // Stays up behind its message; leaves once the message is answered.
        if done && snap.dialog.is_none() && now - at > dur as f64 + 0.3 {
            self.evolving = None;
            return false;
        }
        let layer = LayerId::new(Order::Foreground, Id::new("evolution"));
        ctx.move_to_top(layer);
        let p = ctx.layer_painter(layer);
        p.rect_filled(screen, 0.0, Color32::from_rgb(0x18, 0x20, 0x38));
        let c = screen.center() - Vec2::new(0.0, screen.height() * 0.12);
        // Rays.
        for i in 0..16 {
            let a = i as f32 * std::f32::consts::TAU / 16.0 + now as f32 * 0.4;
            let len = screen.width().max(screen.height());
            p.add(egui::Shape::convex_polygon(vec![c, c + Vec2::angled(a) * len, c + Vec2::angled(a + 0.12) * len], Color32::from_white_alpha(5 + (10.0 * k) as u8), egui::Stroke::NONE));
        }
        // Alternate between the two forms, speeding up; white silhouette until the end.
        let phase = (k * k * 30.0) as i32;
        let show_new = done || phase % 2 == 1;
        let species = if show_new { to } else { from };
        let px = ((screen.width().min(screen.height()) * 0.5) / 64.0).floor().clamp(2.0, 5.0) * 64.0;
        if let Some(tex) = self.renderer.texture(ctx, assets, "Form4.PKMNImages", species - 1) {
            let r = Rect::from_center_size(c, Vec2::splat(px));
            let tint = if done { Color32::WHITE } else { Color32::from_rgb(0xFF, 0xFF, 0xFF) };
            p.image(tex.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), tint);
            if !done {
                // Glow over the sprite (a white silhouette look).
                p.circle_filled(c, px * 0.42, Color32::from_white_alpha((90.0 + 120.0 * k) as u8));
            }
        }
        if done {
            let burst = ((now - at) as f32 - dur).clamp(0.0, 1.0);
            if burst < 1.0 {
                p.rect_filled(screen, 0.0, Color32::from_white_alpha((255.0 * (1.0 - burst)) as u8));
            }
        } else {
            let name = |s: i32| assets.data.pkmn_name.get(s as usize).cloned().unwrap_or_default();
            t::text(&p, Pos2::new(screen.center().x, screen.bottom() - screen.height() * 0.22), Align2::CENTER_CENTER, &format!("What? {} is evolving!", name(from)), t::bold(24.0), t::PAPER);
        }
        // A tap skips to the end.
        egui::Area::new(Id::new("evolution_tap")).order(Order::Foreground).fixed_pos(Pos2::ZERO).show(ctx, |ui| {
            if ui.allocate_rect(screen, Sense::click()).clicked() && !done {
                self.evolving = Some((from, to, now - dur as f64));
            }
        });
        ctx.request_repaint();
        !done
    }

    /// Draw `form` scaled to fit `area` on its own transformed layer.
    #[allow(clippy::too_many_arguments)]
    /// `card`: draw a framed card behind it (scenes); sheets are their own card and top-align.
    fn host_form(&mut self, ctx: &egui::Context, form: &Form, area: Rect, layer: LayerId, interactive: bool, assets: &Assets, tx: &Sender<Input>, alpha: f32, card: bool) {
        let size = Vec2::new(form.client_width as f32, form.client_height as f32);
        let scale = (area.width() / size.x).min(area.height() / size.y).clamp(0.4, 1.8);
        let shown = size * scale;
        let top_gap = if card { ((area.height() - shown.y) / 2.0).max(0.0) * 0.4 } else { 0.0 };
        let origin = Pos2::new(area.center().x - shown.x / 2.0, area.top() + top_gap);
        ctx.set_transform_layer(layer, TSTransform::from_translation(origin.to_vec2()) * TSTransform::from_scaling(scale));
        egui::Area::new(layer.id).order(layer.order).fixed_pos(Pos2::ZERO).constrain(false).show(ctx, |ui| {
            let local = Rect::from_min_size(Pos2::ZERO, size);
            ui.set_clip_rect(local.expand(400.0));
            ui.set_min_size(size);
            ui.set_max_size(size);
            ui.multiply_opacity(alpha);
            if card {
                // Card behind the form, in the form's own (scaled) coordinates.
                let r = local.expand(10.0 / scale);
                let p = ui.painter();
                p.rect_filled(r.translate(Vec2::new(0.0, 5.0 / scale)), 14.0 / scale, Color32::from_black_alpha(50));
                p.rect_filled(r, 14.0 / scale, t::FRAME);
                p.rect_filled(r.shrink(3.0 / scale), 12.0 / scale, GAME_FACE);
            }
            self.renderer.form_body(ui, form, assets, tx, interactive, false);
        });
        ctx.move_to_top(layer);
    }

    /// Bottom dock: Menu (when the scene has one), a chip per open window, Options.
    fn dock(&mut self, ctx: &egui::Context, screen: Rect, scene_rect: Rect, sheets: &[&Form], snap: &Snapshot, wide: bool) {
        let dock = Rect::from_min_max(Pos2::new(scene_rect.left(), screen.bottom() - DOCK_H), Pos2::new(scene_rect.right(), screen.bottom()));
        let has_menu = snap.forms.iter().any(|f| f.var == self.scene && f.visible && !f.menu.is_empty());
        egui::Area::new(Id::new("dock")).order(Order::Middle).fixed_pos(Pos2::ZERO).show(ctx, |ui| {
            let p = ui.painter();
            p.rect_filled(dock, 0.0, Color32::from_black_alpha(60));
            let mut x = dock.left() + 10.0;
            let y = dock.center().y;
            if has_menu {
                if t::button(ui, Rect::from_center_size(Pos2::new(x + 46.0, y), Vec2::new(92.0, 46.0)), "MENU", t::ButtonKind::Danger, true).clicked() {
                    self.menu_path.clear();
                    self.sheet = if self.sheet.as_deref() == Some(MENU) { None } else { Some(MENU.into()) };
                }
                x += 100.0;
            }
            // Window chips share the space up to the Options button; scroll sideways if crowded.
            let room = Rect::from_min_max(Pos2::new(x, dock.top()), Pos2::new(dock.right() - 66.0, dock.bottom()));
            let labels: Vec<(String, String)> = sheets.iter().map(|f| (short_caption(&f.caption), f.var.clone())).collect();
            let natural: Vec<f32> = labels.iter().map(|(l, _)| (l.chars().count() as f32 * 11.0 + 34.0).clamp(72.0, 190.0)).collect();
            let total: f32 = natural.iter().sum::<f32>() + 8.0 * labels.len() as f32;
            let squeeze = if total > room.width() { (room.width() / total).max(0.62) } else { 1.0 };
            ui.scope_builder(egui::UiBuilder::new().max_rect(room), |ui| {
                egui::ScrollArea::horizontal().id_salt("dock_chips").scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
                    let w_all: f32 = natural.iter().map(|w| w * squeeze + 8.0).sum();
                    let (strip, _) = ui.allocate_exact_size(Vec2::new(w_all, room.height()), Sense::hover());
                    let mut cx = strip.left();
                    for ((label, var), w) in labels.iter().zip(&natural) {
                        let w = w * squeeze;
                        let r = Rect::from_center_size(Pos2::new(cx + w / 2.0, y), Vec2::new(w, 46.0));
                        cx += w + 8.0;
                        let active = self.sheet.as_deref() == Some(var.as_str());
                        if t::button(ui, r, label, if active { t::ButtonKind::Primary } else { t::ButtonKind::Plain }, true).clicked() {
                            self.sheet = if active { None } else { Some(var.clone()) };
                        }
                    }
                });
            });
            let r = Rect::from_center_size(Pos2::new(dock.right() - 34.0, y), Vec2::splat(46.0));
            let active = self.sheet.as_deref() == Some(OPTIONS);
            if t::button(ui, r, "⚙", if active { t::ButtonKind::Primary } else { t::ButtonKind::Plain }, true).clicked() {
                self.sheet = if active { None } else { Some(OPTIONS.into()) };
            }
            let _ = wide;
        });
    }

    /// The scene form's main menu as big touch buttons with drill-down.
    fn menu_body(&mut self, ui: &mut egui::Ui, form: &Form, tx: &Sender<Input>, interactive: bool) {
        let mut items: &[MenuItem] = &form.menu;
        let mut crumbs = Vec::new();
        for name in &self.menu_path {
            match items.iter().find(|m| m.name == *name) {
                Some(m) => {
                    crumbs.push(strip_amp(&m.caption));
                    items = &m.children;
                }
                None => break,
            }
        }
        let width = ui.available_width();
        if !self.menu_path.is_empty() {
            let r = ui.allocate_exact_size(Vec2::new(width, 44.0), Sense::hover()).0;
            let back = Rect::from_min_size(r.min, Vec2::new(96.0, 40.0));
            let clicked = t::button(ui, back, "  Back", t::ButtonKind::Plain, true).clicked();
            t::icon(ui.painter(), back.left_center() + Vec2::new(18.0, -2.0), 12.0, t::Icon::Left, t::INK);
            if clicked {
                self.menu_path.pop();
            }
            t::text(ui.painter(), r.left_center() + Vec2::new(108.0, -2.0), Align2::LEFT_CENTER, &crumbs.join(" / "), t::bold(18.0), t::INK);
            ui.add_space(6.0);
        }
        let cols = if width > 520.0 { 3 } else { 2 };
        let gap = 8.0;
        let bw = (width - gap * (cols as f32 - 1.0)) / cols as f32;
        let items: Vec<&MenuItem> = items.iter().filter(|m| m.visible && strip_amp(&m.caption) != "-").collect();
        egui::ScrollArea::vertical().id_salt(("menu", self.menu_path.join("/"))).show(ui, |ui| {
            for row in items.chunks(cols) {
                let (r, _) = ui.allocate_exact_size(Vec2::new(width, 56.0), Sense::hover());
                for (k, m) in row.iter().enumerate() {
                    let br = Rect::from_min_size(r.min + Vec2::new(k as f32 * (bw + gap), 0.0), Vec2::new(bw, 50.0));
                    let label = strip_amp(&m.caption);
                    let enabled = interactive && m.enabled;
                    let resp = t::button(ui, br, &label, t::ButtonKind::Plain, enabled);
                    let p = ui.painter();
                    if !m.children.is_empty() {
                        t::icon(p, br.right_center() + Vec2::new(-14.0, -2.0), 10.0, t::Icon::Right, if enabled { t::INK } else { t::GREY });
                    }
                    if m.checked {
                        t::icon(p, br.left_center() + Vec2::new(16.0, -2.0), 14.0, t::Icon::Check, t::BLUE);
                    }
                    if !m.enabled {
                        t::icon(p, br.right_top() + Vec2::new(-14.0, 14.0), 12.0, t::Icon::Lock, t::GREY);
                    }
                    if resp.clicked() {
                        if !m.children.is_empty() {
                            self.menu_path.push(m.name.clone());
                        } else if m.on_click.is_some() {
                            let _ = tx.send(Input::Menu { form: form.var.clone(), item: m.name.clone() });
                            self.sheet = None;
                            self.menu_path.clear();
                        }
                    }
                }
            }
        });
    }

    fn options_body(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, snap: &Snapshot, tx: &Sender<Input>) {
        let width = ui.available_width();
        let row = |ui: &mut egui::Ui, label: &str| {
            ui.add_space(8.0);
            let (r, _) = ui.allocate_exact_size(Vec2::new(width, 26.0), Sense::hover());
            t::text(ui.painter(), r.left_center(), Align2::LEFT_CENTER, label, t::bold(20.0), t::INK);
        };
        row(ui, "Text speed");
        let (r, _) = ui.allocate_exact_size(Vec2::new(width, 50.0), Sense::hover());
        let w = (width - 3.0 * 6.0) / 4.0;
        for (k, s) in TextSpeed::ALL.into_iter().enumerate() {
            let b = Rect::from_min_size(r.min + Vec2::new(k as f32 * (w + 6.0), 0.0), Vec2::new(w, 46.0));
            let kind = if self.settings.text_speed == s { t::ButtonKind::Primary } else { t::ButtonKind::Plain };
            if t::button(ui, b, s.label(), kind, true).clicked() {
                self.settings.text_speed = s;
                self.settings.save();
            }
        }
        row(ui, "Display");
        let (r, _) = ui.allocate_exact_size(Vec2::new(width, 50.0), Sense::hover());
        let half = (width - 6.0) / 2.0;
        let anim_label = if self.settings.animations { "Animations: ON" } else { "Animations: OFF" };
        if t::button(ui, Rect::from_min_size(r.min, Vec2::new(half, 46.0)), anim_label, t::ButtonKind::Plain, true).clicked() {
            self.settings.animations = !self.settings.animations;
            self.settings.save();
        }
        let classic_label = if self.settings.classic { "Classic: ON" } else { "Classic look" };
        if t::button(ui, Rect::from_min_size(r.min + Vec2::new(half + 6.0, 0.0), Vec2::new(half, 46.0)), classic_label, t::ButtonKind::Plain, true).clicked() {
            let on = !self.settings.classic;
            self.set_classic(ctx, on);
        }
        // Music: the original Form4 menu items, so their checked state stays in the program.
        if let Some(f4) = snap.forms.iter().find(|f| f.var == "Form4" && f.visible) {
            row(ui, "Music");
            let (r, _) = ui.allocate_exact_size(Vec2::new(width, 50.0), Sense::hover());
            let free = snap.dialog.is_none();
            let item = |name: &str| f4.menu.iter().find_map(|m| m.find(name));
            if let Some(stop) = item("StopMusic1") {
                if t::button(ui, Rect::from_min_size(r.min, Vec2::new(half, 46.0)), "Stop music", t::ButtonKind::Plain, free && stop.enabled).clicked() {
                    let _ = tx.send(Input::Menu { form: "Form4".into(), item: stop.name.clone() });
                }
            }
            if let Some(rep) = item("RepeatMusic1") {
                let label = if rep.checked { "Repeat: ON" } else { "Repeat: OFF" };
                if t::button(ui, Rect::from_min_size(r.min + Vec2::new(half + 6.0, 0.0), Vec2::new(half, 46.0)), label, t::ButtonKind::Plain, free && rep.enabled).clicked() {
                    let _ = tx.send(Input::Menu { form: "Form4".into(), item: rep.name.clone() });
                }
            }
        }
        ui.add_space(14.0);
        let (r, _) = ui.allocate_exact_size(Vec2::new(width, 22.0), Sense::hover());
        t::text(ui.painter(), r.left_center(), Align2::LEFT_CENTER, "F2 switches between game and classic look.", t::body(16.0), t::GREY);
        let _ = MENU_HEIGHT;
    }
}

/// Battle Info sheet: the enemy's moves, then the whole battle log.
fn battle_info_body(ui: &mut egui::Ui, snap: &Snapshot) {
    let width = ui.available_width();
    let f4 = snap.forms.iter().find(|f| f.var == "Form4");
    if let Some(f4) = f4.filter(|f| f.c("GroupBox1").visible) {
        let (r, _) = ui.allocate_exact_size(Vec2::new(width, 28.0), Sense::hover());
        t::text(ui.painter(), r.left_center(), Align2::LEFT_CENTER, &strip_amp(&f4.c("GroupBox2").caption), t::bold(19.0), t::INK);
        let half = (width - 8.0) / 2.0;
        let (r, _) = ui.allocate_exact_size(Vec2::new(width, 2.0 * 30.0 + 8.0), Sense::hover());
        for (k, l) in ["Label4", "Label5", "Label6", "Label7"].iter().enumerate() {
            let cell = Rect::from_min_size(r.min + Vec2::new((k % 2) as f32 * (half + 8.0), (k / 2) as f32 * 34.0), Vec2::new(half, 30.0));
            ui.painter().rect_filled(cell, 6.0, Color32::WHITE);
            ui.painter().rect_stroke(cell, 6.0, egui::Stroke::new(1.5, t::FRAME_INNER), egui::StrokeKind::Inside);
            t::text(ui.painter(), cell.left_center() + Vec2::new(10.0, 0.0), Align2::LEFT_CENTER, &strip_amp(&f4.c(l).caption), t::body(18.0), t::INK);
        }
        ui.add_space(10.0);
    }
    let (r, _) = ui.allocate_exact_size(Vec2::new(width, 28.0), Sense::hover());
    t::text(ui.painter(), r.left_center(), Align2::LEFT_CENTER, "Battle log", t::bold(19.0), t::INK);
    let lines: Vec<String> = snap.forms.iter().find(|f| f.var == "Form7").map(|f| f.c("Memo1").items.clone()).unwrap_or_default();
    egui::Frame::new().fill(Color32::WHITE).corner_radius(8.0).stroke(egui::Stroke::new(1.5, t::FRAME_INNER)).inner_margin(10.0).show(ui, |ui| {
        ui.set_width(width - 20.0);
        egui::ScrollArea::vertical().stick_to_bottom(true).id_salt("battle_log").show(ui, |ui| {
            for l in &lines {
                ui.label(egui::RichText::new(l).font(t::body(18.0)).color(t::INK));
            }
        });
    });
}

fn ease_out(x: f32) -> f32 {
    1.0 - (1.0 - x).powi(3)
}

/// Sheet header: a long caption ("Welcome to POKéMON Mart! Click an item…") keeps its first sentence.
fn sheet_title(c: &str) -> String {
    let c = strip_amp(c);
    let first = match c.find(['!', '.']) {
        Some(i) if c.chars().count() > 24 => c[..=i].to_string(),
        _ => c.clone(),
    };
    if first.chars().count() > 26 {
        first.chars().take(25).collect::<String>() + "…"
    } else {
        first
    }
}

/// Window captions are long ("POKéMON Simulator V4.5 - Statistics"); keep the meaningful part.
fn short_caption(c: &str) -> String {
    let c = strip_amp(c);
    let c = c.rsplit(" - ").next().unwrap_or(&c).trim().to_string();
    if c.chars().count() > 16 {
        c.chars().take(15).collect::<String>() + "…"
    } else {
        c
    }
}
