//! The battle screen: a GBA-style view over Form4's battle controls.
//!
//! The program resolves a whole turn instantly; the [`Fx`] events it emitted on the way are
//! replayed here one by one (text, lunges, HP drains, faints, ball throws), and the screen only
//! takes input again once playback has caught up with the program's state. Every action sends
//! exactly the inputs the original controls would (e.g. a move = select it in RadioGroup1, then
//! click Button1).

use super::settings::Settings;
use super::theme as t;
use crate::assets::Assets;
use crate::vcl::model::Form;
use crate::vcl::render::{strip_amp, Renderer};
use crate::vcl::runtime::{Event, Fx, Input, Side, Snapshot};
use egui::{Align2, Color32, CornerRadius, Id, Pos2, Rect, Sense, Shape, Stroke, Vec2};
use std::collections::VecDeque;
use std::f32::consts::PI;
use std::sync::mpsc::Sender;

/// What the battle screen asks the shell to do.
pub enum Request {
    OpenSheet(String),
}

#[derive(Clone, Copy)]
struct HpAnim {
    from: f32,
    to: f32,
    max: i32,
    at: f64,
}

impl HpAnim {
    fn value(&self, now: f64, dur: f32) -> f32 {
        let k = if dur <= 0.0 { 1.0 } else { (((now - self.at) as f32) / dur).clamp(0.0, 1.0) };
        self.from + (self.to - self.from) * ease_in_out(k)
    }
}

const NEVER: f64 = -1e9;

pub struct BattleView {
    queue: VecDeque<Fx>,
    /// The step being played and when it started.
    step: Option<(Fx, f64)>,
    enemy_species: i32,
    own_species: i32,
    hp: [HpAnim; 2],
    text: String,
    text_at: f64,
    // Animation clocks (start times).
    intro_at: f64,
    sendout_at: f64,
    lunge_at: [f64; 2],
    hit_at: [f64; 2],
    heal_at: f64,
    faint_at: [f64; 2],
    ball_at: f64,
    ball_result: Option<(bool, f64)>,
    levelup_at: f64,
    fainted: [bool; 2],
    caught: bool,
    bag_open: bool,
    was_active: bool,
    /// Where the battle text box was drawn last frame (game messages go there too).
    pub text_rect: Option<Rect>,
}

fn si(s: Side) -> usize {
    match s {
        Side::Own => 0,
        Side::Enemy => 1,
    }
}

impl Default for BattleView {
    fn default() -> Self {
        let hp = HpAnim { from: 0.0, to: 0.0, max: 1, at: NEVER };
        BattleView {
            queue: VecDeque::new(),
            step: None,
            enemy_species: 0,
            own_species: 0,
            hp: [hp, hp],
            text: String::new(),
            text_at: NEVER,
            intro_at: NEVER,
            sendout_at: NEVER,
            lunge_at: [NEVER; 2],
            hit_at: [NEVER; 2],
            heal_at: NEVER,
            faint_at: [NEVER; 2],
            ball_at: NEVER,
            ball_result: None,
            levelup_at: NEVER,
            fainted: [false; 2],
            caught: false,
            bag_open: false,
            was_active: false,
            text_rect: None,
        }
    }
}

/// The player's POKéMON is out: its name label is up and filled, and it has a sprite. (After
/// loading a save the program leaves an empty Label2 visible.)
fn own_chosen(f4: &Form) -> bool {
    let l = f4.c("Label2");
    l.visible && !l.caption.trim().is_empty() && f4.c("ListView2").list_items.first().is_some_and(|it| it.image_index >= 0)
}

fn form<'a>(snap: &'a Snapshot, var: &str) -> Option<&'a Form> {
    snap.forms.iter().find(|f| f.var == var)
}

/// True while Form4 shows its battle arena.
pub fn active(snap: &Snapshot) -> bool {
    form(snap, "Form4").is_some_and(|f| f.visible && f.has("GroupBox1") && f.c("GroupBox1").visible)
}

impl BattleView {
    /// How the fight has gone so far, as shown on screen: Some(true) = enemy fainted or caught,
    /// Some(false) = own POKéMON fainted.
    pub fn outcome(&self) -> Option<bool> {
        if self.fainted[1] || self.caught {
            Some(true)
        } else if self.fainted[0] {
            Some(false)
        } else {
            None
        }
    }

    /// The player's POKéMON is on the field.
    pub fn own_out(&self) -> bool {
        self.own_species > 0
    }

    /// Take the newly published effects.
    pub fn feed(&mut self, fx: Vec<Fx>) {
        self.queue.extend(fx);
    }

    /// Still replaying what the program already did.
    pub fn busy(&self) -> bool {
        self.step.is_some() || !self.queue.is_empty()
    }

    fn dur(&self, settings: &Settings, secs: f32) -> f32 {
        if settings.animations { secs } else { 0.0 }
    }

    fn text_time(&self, settings: &Settings, s: &str) -> f32 {
        let typing = match settings.text_speed.chars_per_sec() {
            Some(cps) => s.chars().count() as f32 / cps,
            None => 0.0,
        };
        let hold = match settings.text_speed {
            super::settings::TextSpeed::Slow => 1.6,
            super::settings::TextSpeed::Normal => 1.0,
            super::settings::TextSpeed::Fast => 0.6,
            super::settings::TextSpeed::Instant => 0.7,
        };
        typing + hold
    }

    fn step_len(&self, fx: &Fx, settings: &Settings) -> f32 {
        match fx {
            Fx::Log(s) => self.text_time(settings, s),
            Fx::BattleStart { .. } => self.dur(settings, 1.0),
            Fx::SendOut { .. } => self.dur(settings, 0.6),
            Fx::Attack(_) => self.dur(settings, 0.35),
            Fx::Hp { .. } => self.dur(settings, 0.8),
            Fx::Faint(_) => self.dur(settings, 0.7),
            Fx::BallThrow => self.dur(settings, 0.9),
            Fx::Caught(_) => self.dur(settings, 1.0),
            Fx::LevelUp => self.dur(settings, 0.8),
            Fx::Evolve { .. } => 0.0, // shown by the shell's evolution overlay
        }
    }

    fn start(&mut self, fx: &Fx, now: f64, settings: &Settings) {
        match fx {
            Fx::Log(s) => {
                self.text = s.clone();
                self.text_at = now;
            }
            Fx::BattleStart { species, hp } => {
                *self = BattleView { queue: std::mem::take(&mut self.queue), was_active: true, ..BattleView::default() };
                self.enemy_species = *species;
                self.own_species = 0;
                self.hp[1] = HpAnim { from: *hp as f32, to: *hp as f32, max: *hp, at: NEVER };
                self.intro_at = now;
            }
            Fx::SendOut { species, hp } => {
                self.own_species = *species;
                self.fainted[0] = false;
                self.hp[0] = HpAnim { from: *hp as f32, to: *hp as f32, max: *hp, at: NEVER };
                self.sendout_at = now;
            }
            Fx::Attack(side) => {
                self.lunge_at[si(*side)] = now;
                // Show the hit right after "X used Y!", before damage/effectiveness lines.
                let next_attack = self.queue.iter().position(|f| matches!(f, Fx::Attack(_))).unwrap_or(self.queue.len());
                let hp = self.queue.iter().take(next_attack).position(|f| matches!(f, Fx::Hp { .. }));
                let first_log = self.queue.iter().position(|f| matches!(f, Fx::Log(_)));
                if let (Some(h), Some(l)) = (hp, first_log) {
                    if h > l + 1 {
                        let e = self.queue.remove(h).unwrap();
                        self.queue.insert(l + 1, e);
                    }
                }
            }
            Fx::Hp { side, hp, max } => {
                let i = si(*side);
                let from = self.hp[i].value(now, self.dur(settings, 0.8));
                let to = (*hp).clamp(0, *max) as f32;
                self.hp[i] = HpAnim { from, to, max: *max, at: now };
                if to < from {
                    self.hit_at[i] = now;
                } else if i == 0 {
                    self.heal_at = now;
                }
            }
            Fx::Faint(side) => {
                self.faint_at[si(*side)] = now;
                self.fainted[si(*side)] = true;
            }
            Fx::BallThrow => {
                self.ball_at = now;
                self.ball_result = None;
            }
            Fx::Caught(ok) => {
                self.ball_result = Some((*ok, now));
                self.caught = *ok;
            }
            Fx::LevelUp => self.levelup_at = now,
            Fx::Evolve { .. } => {}
        }
    }

    /// Advance playback to `now`.
    fn advance(&mut self, now: f64, settings: &Settings) {
        loop {
            if let Some((fx, at)) = &self.step {
                if ((now - at) as f32) < self.step_len(fx, settings) {
                    return;
                }
                self.step = None;
            }
            let Some(fx) = self.queue.pop_front() else { return };
            self.start(&fx, now, settings);
            self.step = Some((fx, now));
        }
    }

    /// Tap: finish typing, or end the current step.
    fn skip(&mut self, now: f64, settings: &Settings) {
        if let Some((Fx::Log(s), at)) = &self.step {
            let typed = match settings.text_speed.chars_per_sec() {
                Some(cps) => s.chars().count() as f32 / cps,
                None => 0.0,
            };
            if ((now - at) as f32) < typed {
                self.text_at = NEVER;
                self.step = Some((Fx::Log(s.clone()), now - typed as f64));
                return;
            }
        }
        if self.step.is_some() {
            self.step = None;
            // Jump every running animation to its end.
            for t in [&mut self.intro_at, &mut self.sendout_at, &mut self.heal_at, &mut self.levelup_at, &mut self.ball_at] {
                if *t > NEVER {
                    *t -= 100.0;
                }
            }
            for i in 0..2 {
                for t in [&mut self.lunge_at[i], &mut self.hit_at[i], &mut self.faint_at[i]] {
                    if *t > NEVER {
                        *t -= 100.0;
                    }
                }
                self.hp[i].at -= 100.0;
            }
            if let Some((_, at)) = &mut self.ball_result {
                *at -= 100.0;
            }
        }
    }

    /// When nothing is left to replay, take the program's own values (they are the truth).
    fn sync(&mut self, f4: &Form, now: f64) {
        let lv = |name: &str| f4.has(name).then(|| f4.c(name)).and_then(|c| c.list_items.first()).map(|it| it.image_index + 1).unwrap_or(0);
        let g = |name: &str| {
            let c = f4.c(name);
            (c.progress.clamp(0, c.max_value.max(0)), c.max_value.max(1))
        };
        self.enemy_species = lv("ListView3");
        let own = lv("ListView2");
        let own_out = own_chosen(f4) && own > 0;
        self.own_species = if own_out { own } else { 0 };
        for (i, name) in [(0, "Gauge1"), (1, "Gauge2")] {
            let (v, max) = g(name);
            if self.hp[i].to != v as f32 || self.hp[i].max != max {
                self.hp[i] = HpAnim { from: v as f32, to: v as f32, max, at: now - 100.0 };
            }
        }
    }

    /// Draw the battle into `area`. Returns requests for the shell.
    #[allow(clippy::too_many_arguments)]
    pub fn show(&mut self, ui: &mut egui::Ui, area: Rect, snap: &Snapshot, renderer: &mut Renderer, assets: &Assets, settings: &Settings, tx: &Sender<Input>) -> Option<Request> {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        let f4 = form(snap, "Form4")?;
        if !self.was_active {
            // Entered mid-battle without a BattleStart (e.g. switching from classic mode).
            self.was_active = true;
            self.intro_at = NEVER;
        }
        self.advance(now, settings);
        if !self.busy() {
            self.sync(f4, now);
            self.text_at = NEVER;
        }
        if self.busy() || settings.animations {
            ctx.request_repaint();
        }
        let mut request = None;

        // ---------------- layout
        let content_w = area.width().min(860.0);
        let area = Rect::from_center_size(area.center(), Vec2::new(content_w, area.height())).shrink(10.0);
        let text_h = 92.0;
        let controls_h = 2.0 * 62.0 + 58.0;
        let field_h = (area.height() - text_h - controls_h - 16.0).clamp(160.0, area.width() * 1.3);
        // Centre the whole column when there is room to spare.
        let spare = (area.height() - field_h - text_h - controls_h - 16.0).max(0.0);
        let area = area.translate(Vec2::new(0.0, spare * 0.5));
        let field = Rect::from_min_size(area.min, Vec2::new(area.width(), field_h));
        let text_rect = Rect::from_min_size(Pos2::new(area.left(), field.bottom() + 8.0), Vec2::new(area.width(), text_h));
        self.text_rect = Some(text_rect);
        let controls = Rect::from_min_max(Pos2::new(area.left(), text_rect.bottom() + 8.0), Pos2::new(area.right(), (text_rect.bottom() + 8.0 + controls_h).min(area.bottom())));

        // ---------------- field
        let p = ui.painter_at(field.expand(4.0));
        self.draw_field(&p, field, f4, renderer, &ctx, assets, settings, now);
        let tap = ui.interact(field.union(text_rect), Id::new("battle_tap"), Sense::click());
        if tap.clicked() && self.busy() {
            self.skip(now, settings);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Space) || i.key_pressed(egui::Key::Enter)) && self.busy() && snap.dialog.is_none() {
            self.skip(now, settings);
        }

        // ---------------- text box
        let p = ui.painter();
        t::frame(p, text_rect);
        let idle_text = self.idle_prompt(f4, snap);
        let (text, shown) = if self.busy() && matches!(self.step, Some((Fx::Log(_), _))) {
            let total = self.text.chars().count();
            let shown = match settings.text_speed.chars_per_sec() {
                Some(cps) if settings.animations && self.text_at > NEVER => (((now - self.text_at) as f32 * cps) as usize).min(total),
                _ => total,
            };
            (self.text.clone(), shown)
        } else if self.busy() {
            (self.text.clone(), usize::MAX)
        } else {
            (idle_text, usize::MAX)
        };
        if snap.dialog.is_none() || self.busy() {
            draw_typed(p, text_rect.shrink2(Vec2::new(22.0, 18.0)), &text, shown, &ctx);
        }

        // ---------------- controls
        let can_act = !self.busy() && snap.dialog.is_none();
        let gap = 8.0;
        let row_h = 56.0;
        let f5 = form(snap, "Form5");
        let end_visible = f4.c("EndButton").visible;
        let moves_visible = f4.c("RadioGroup1").visible && f4.c("Button1").visible;
        let own_chosen = own_chosen(f4);
        let bag_items: Vec<(String, String, String, bool)> = f5
            .map(|f| {
                [("Button1", "POKé BALL", "PokeBallsLbl"), ("Button2", "GREAT BALL", "GreatBallsLbl"), ("Button3", "ULTRA BALL", "UltraBallsLbl"), ("Button4", "MASTER BALL", "MasterBallsLbl"), ("Button5", "POTION", "PotionsLbl"), ("Button6", "SUPER POTION", "SuperPotionsLbl")]
                    .iter()
                    .filter(|(b, _, _)| f.c(b).visible)
                    .map(|(b, name, lbl)| (b.to_string(), name.to_string(), f.c(lbl).caption.clone(), f.c(b).enabled))
                    .collect()
            })
            .unwrap_or_default();
        if bag_items.is_empty() {
            self.bag_open = false;
        }
        let half = (controls.width() - gap) / 2.0;
        let cell = |col: usize, row: usize| Rect::from_min_size(controls.min + Vec2::new(col as f32 * (half + gap), row as f32 * (row_h + gap)), Vec2::new(half, row_h));
        let bottom = Rect::from_min_size(controls.min + Vec2::new(0.0, 2.0 * (row_h + gap)), Vec2::new(controls.width(), row_h - 4.0));
        let send = |form: &str, control: &str, ev: Event| {
            let _ = tx.send(Input::Event { form: form.into(), control: control.into(), event: ev });
        };

        if end_visible {
            let r = Rect::from_min_max(controls.min, Pos2::new(controls.right(), controls.min.y + 2.0 * row_h + gap));
            if t::button(ui, r.shrink2(Vec2::new(controls.width() * 0.18, row_h * 0.35)), "CONTINUE", t::ButtonKind::Primary, can_act).clicked() {
                send("Form4", "EndButton", Event::Click);
            }
        } else if self.bag_open {
            let third = (controls.width() - 2.0 * gap) / 3.0;
            for (k, (btn, name, count, enabled)) in bag_items.iter().enumerate() {
                let (col, row) = (k % 3, k / 3);
                let r = Rect::from_min_size(controls.min + Vec2::new(col as f32 * (third + gap), row as f32 * (row_h + gap)), Vec2::new(third, row_h));
                let resp = t::button(ui, r, "", t::ButtonKind::Plain, can_act && *enabled);
                let ink = if *enabled { t::INK } else { t::GREY };
                let p = ui.painter();
                item_icon(p, r.left_center() + Vec2::new(18.0, -2.0), name);
                t::text(p, r.left_center() + Vec2::new(34.0, -10.0), Align2::LEFT_CENTER, name, t::bold(15.0), ink);
                t::text(p, r.left_center() + Vec2::new(34.0, 8.0), Align2::LEFT_CENTER, &format!("×{}", count.trim()), t::body(15.0), ink);
                if resp.clicked() {
                    self.bag_open = false;
                    send("Form5", btn, Event::Click);
                }
            }
        } else if moves_visible {
            let rg = f4.c("RadioGroup1");
            for k in 0..4usize {
                let Some(item) = rg.items.get(k) else { continue };
                let (name, ty) = split_move(item);
                let r = cell(k % 2, k / 2);
                if move_button(ui, r, &name, &ty, can_act).clicked() {
                    let _ = tx.send(Input::SetItemIndex { form: "Form4".into(), control: "RadioGroup1".into(), index: k as i32 });
                    send("Form4", "Button1", Event::Click);
                }
            }
        } else if !own_chosen {
            let r = Rect::from_min_max(controls.min, Pos2::new(controls.right(), controls.min.y + 2.0 * row_h + gap));
            let lv_ok = f5.is_some_and(|f| f.visible && f.c("ListView1").enabled);
            if t::button(ui, r.shrink2(Vec2::new(controls.width() * 0.12, row_h * 0.35)), "CHOOSE POKéMON", t::ButtonKind::Primary, can_act && lv_ok).clicked() {
                request = Some(Request::OpenSheet("Form5".into()));
            }
        }

        // Bottom row: BAG / RUN / INFO.
        let third = (bottom.width() - 2.0 * gap) / 3.0;
        let b = |k: usize| Rect::from_min_size(bottom.min + Vec2::new(k as f32 * (third + gap), 0.0), Vec2::new(third, bottom.height()));
        let bag_label = if self.bag_open { "BACK" } else { "BAG" };
        if t::button(ui, b(0), bag_label, t::ButtonKind::Plain, can_act && !bag_items.is_empty() && !end_visible).clicked() {
            self.bag_open = !self.bag_open;
        }
        let can_run = moves_visible && f4.c("RadioGroup1").items.len() > 4;
        if t::button(ui, b(1), "RUN", t::ButtonKind::Danger, can_act && can_run && !end_visible).clicked() {
            let _ = tx.send(Input::SetItemIndex { form: "Form4".into(), control: "RadioGroup1".into(), index: 4 });
            send("Form4", "Button1", Event::Click);
        }
        if t::button(ui, b(2), "INFO", t::ButtonKind::Plain, true).clicked() {
            request = Some(Request::OpenSheet(super::BATTLE_INFO.into()));
        }
        request
    }

    fn idle_prompt(&self, f4: &Form, snap: &Snapshot) -> String {
        let own_name = name_of(&f4.c("Label2").caption);
        if f4.c("EndButton").visible {
            // The battle is over: keep its last line on screen.
            return form(snap, "Form7").and_then(|f| f.c("Memo1").items.last().cloned()).unwrap_or_default();
        }
        if !own_chosen(f4) {
            return "Choose a POKéMON to send out!".into();
        }
        format!("What will {own_name} do?")
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_field(&mut self, p: &egui::Painter, field: Rect, f4: &Form, renderer: &mut Renderer, ctx: &egui::Context, assets: &Assets, settings: &Settings, now: f64) {
        let prog = |at: f64, dur: f32| -> f32 {
            if at <= NEVER + 1.0 {
                return 1.0;
            }
            let d = if settings.animations { dur } else { 0.0 };
            if d <= 0.0 {
                1.0
            } else {
                (((now - at) as f32) / d).clamp(0.0, 1.0)
            }
        };
        let (w, h) = (field.width(), field.height());
        // Backdrop: pale sky to meadow.
        let mut mesh = egui::Mesh::default();
        let top = Color32::from_rgb(0xF8, 0xF4, 0xE0);
        let mid = Color32::from_rgb(0xE0, 0xF0, 0xC8);
        let low = Color32::from_rgb(0xB8, 0xE0, 0x98);
        let y1 = field.top() + h * 0.5;
        for (y, c) in [(field.top(), top), (y1, mid), (field.bottom(), low)] {
            mesh.colored_vertex(Pos2::new(field.left(), y), c);
            mesh.colored_vertex(Pos2::new(field.right(), y), c);
        }
        for k in [0u32, 2] {
            mesh.add_triangle(k, k + 1, k + 2);
            mesh.add_triangle(k + 1, k + 2, k + 3);
        }
        p.rect_filled(field.expand(3.0), CornerRadius::same(14), t::FRAME);
        let clip = p.with_clip_rect(field);
        clip.add(Shape::mesh(mesh));
        // Grass tufts.
        for k in 0..14 {
            let x = field.left() + (k as f32 * 0.137 % 1.0) * w;
            let y = field.top() + h * (0.55 + (k as f32 * 0.311 % 1.0) * 0.4);
            clip.line_segment([Pos2::new(x, y), Pos2::new(x - 3.0, y - 7.0)], Stroke::new(2.0, Color32::from_rgb(0x88, 0xC0, 0x70)));
            clip.line_segment([Pos2::new(x + 4.0, y), Pos2::new(x + 6.0, y - 8.0)], Stroke::new(2.0, Color32::from_rgb(0x88, 0xC0, 0x70)));
        }
        // Whole-number sprite scales (nearest-neighbour stays crisp).
        let scale = ((w * 0.4).min(h * 0.36) / 64.0).floor().clamp(2.0, 5.0);
        let own_scale = (scale + 1.0).min((h * 0.5 / 64.0).floor()).max(scale);
        let enemy_base = Pos2::new(field.left() + w * 0.72, field.top() + h * 0.46);
        let own_base = Pos2::new(field.left() + w * 0.27, field.bottom() - h * 0.06);
        let platform = |c: Pos2, rx: f32| {
            clip.add(Shape::ellipse_filled(c, Vec2::new(rx, rx * 0.22), Color32::from_rgb(0xA8, 0xC8, 0x80)));
            clip.add(Shape::ellipse_filled(c - Vec2::new(0.0, 2.0), Vec2::new(rx * 0.92, rx * 0.18), Color32::from_rgb(0xC8, 0xE0, 0xA0)));
        };
        platform(enemy_base, 64.0 * scale * 0.75);
        platform(own_base, 64.0 * own_scale * 0.7);

        let bob = |phase: f64| if settings.animations { ((now * 2.4 + phase).sin() * 1.5) as f32 } else { 0.0 };
        let list = f4.c("ListView3").small_images.clone().unwrap_or_else(|| "Form4.PKMNImages".into());

        // Enemy.
        let intro = ease_out(prog(self.intro_at, 0.7));
        let mut enemy_off = Vec2::new((1.0 - intro) * w * 0.7, bob(0.0));
        let lunge_e = prog(self.lunge_at[1], 0.35);
        if lunge_e < 1.0 {
            enemy_off += Vec2::new(-1.0, 0.6) * (lunge_e * PI).sin() * 22.0;
        }
        let hit_e = prog(self.hit_at[1], 0.5);
        if hit_e < 1.0 {
            enemy_off.x += ((hit_e * 40.0).sin()) * 6.0 * (1.0 - hit_e);
        }
        let faint_e = if self.fainted[1] { prog(self.faint_at[1], 0.6) } else { 0.0 };
        let mut enemy_alpha = if hit_e < 1.0 && ((hit_e * 10.0) as i32) % 2 == 1 { 0.25 } else { 1.0 };
        enemy_alpha *= 1.0 - faint_e;
        // Being pulled into a ball.
        let ball_t = prog(self.ball_at, 0.9);
        let mut enemy_shrink = 1.0;
        if self.ball_at > NEVER + 1.0 {
            if ball_t > 0.55 {
                enemy_shrink = 1.0 - ((ball_t - 0.55) / 0.2).clamp(0.0, 1.0);
            }
            if let Some((ok, at)) = self.ball_result {
                let r = prog(at, 1.0);
                enemy_shrink = if ok { 0.0 } else { ((r - 0.5) / 0.3).clamp(0.0, 1.0) };
            }
        }
        if self.caught && !self.busy() {
            enemy_shrink = 0.0;
        }
        if self.enemy_species > 0 && enemy_alpha > 0.0 && enemy_shrink > 0.0 {
            if let Some(tex) = renderer.texture(ctx, assets, &list, self.enemy_species - 1) {
                let size = Vec2::splat(64.0 * scale) * enemy_shrink;
                let sink = faint_e * size.y;
                // Sprites have empty rows at the bottom: sink them a little onto the platform.
                let r = Rect::from_min_size(Pos2::new(enemy_base.x - size.x / 2.0, enemy_base.y - size.y * 0.9 + sink) + enemy_off, size);
                let tint = if enemy_shrink < 1.0 { Color32::from_rgb(0xFF, 0x90, 0x90) } else { Color32::WHITE };
                clip.with_clip_rect(Rect::from_min_max(field.min, Pos2::new(field.right(), enemy_base.y + 64.0 * scale * 0.1 + 2.0))).image(
                    tex.id(),
                    r,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    tint.gamma_multiply(enemy_alpha),
                );
            }
        }
        // Thrown ball.
        if self.ball_at > NEVER + 1.0 && (self.busy() || self.caught) {
            let target = enemy_base - Vec2::new(0.0, 14.0);
            let start = own_base - Vec2::new(-20.0, 64.0 * own_scale * 0.6);
            let k = (ball_t / 0.55).clamp(0.0, 1.0);
            let mut pos = start.lerp(target, k) - Vec2::new(0.0, (k * PI).sin() * h * 0.35);
            if let Some((ok, at)) = self.ball_result {
                let r = prog(at, 1.0);
                if !ok && r > 0.5 {
                    pos = target; // bursts open
                }
                let wobble = if r < 0.6 { ((r * 18.0).sin()) * 0.35 } else { 0.0 };
                if !(!ok && r > 0.5) {
                    poke_ball(&clip, pos, 9.0, wobble);
                }
                if ok && r > 0.6 {
                    sparkle(&clip, pos, (r - 0.6) / 0.4, Color32::from_rgb(0xF8, 0xD0, 0x30));
                }
            } else if k < 1.0 || ball_t < 1.0 {
                poke_ball(&clip, pos, 9.0, if k >= 1.0 { ((ball_t * 30.0).sin()) * 0.3 } else { k * 6.0 });
            }
        }

        // Own POKéMON (front sprite mirrored: there are no back sprites).
        let send = ease_out(prog(self.sendout_at, 0.5));
        let mut own_off = Vec2::new(0.0, bob(1.7));
        let lunge_o = prog(self.lunge_at[0], 0.35);
        if lunge_o < 1.0 {
            own_off += Vec2::new(1.0, -0.6) * (lunge_o * PI).sin() * 26.0;
        }
        let hit_o = prog(self.hit_at[0], 0.5);
        if hit_o < 1.0 {
            own_off.x += ((hit_o * 40.0).sin()) * 6.0 * (1.0 - hit_o);
        }
        let faint_o = if self.fainted[0] { prog(self.faint_at[0], 0.6) } else { 0.0 };
        let own_alpha = (if hit_o < 1.0 && ((hit_o * 10.0) as i32) % 2 == 1 { 0.25 } else { 1.0 }) * (1.0 - faint_o);
        if self.own_species > 0 && own_alpha > 0.0 {
            if let Some(tex) = renderer.texture(ctx, assets, &list, self.own_species - 1) {
                let size = Vec2::splat(64.0 * own_scale) * (0.3 + 0.7 * send);
                let sink = faint_o * size.y;
                let r = Rect::from_min_size(Pos2::new(own_base.x - size.x / 2.0, own_base.y - size.y * 0.9 + sink) + own_off, size);
                let tint = t::lerp_color(Color32::WHITE, Color32::from_rgb(0xFF, 0xFF, 0xFF), 1.0);
                let white = 1.0 - send;
                clip.with_clip_rect(Rect::from_min_max(field.min, Pos2::new(field.right(), own_base.y + 64.0 * own_scale * 0.1 + 2.0))).image(
                    tex.id(),
                    r,
                    Rect::from_min_max(Pos2::new(1.0, 0.0), Pos2::new(0.0, 1.0)),
                    tint.gamma_multiply(own_alpha),
                );
                if white > 0.01 {
                    clip.circle_filled(r.center(), size.x * 0.45 * white, Color32::from_white_alpha((200.0 * white) as u8));
                }
                let lu = prog(self.levelup_at, 0.8);
                if lu < 1.0 {
                    sparkle(&clip, r.center(), lu, Color32::from_rgb(0x58, 0xC8, 0xF8));
                }
                let heal = prog(self.heal_at, 0.8);
                if heal < 1.0 {
                    sparkle(&clip, r.center(), heal, Color32::from_rgb(0x58, 0xD0, 0x80));
                }
            }
        }

        // HP boxes.
        let box_w = (w * 0.5).clamp(170.0, 300.0);
        let enemy_box = Rect::from_min_size(field.min + Vec2::new(10.0, 10.0) - Vec2::new((1.0 - intro) * w * 0.6, 0.0), Vec2::new(box_w, 58.0));
        let own_box = Rect::from_min_size(Pos2::new(field.right() - box_w - 10.0, field.top() + h * 0.56), Vec2::new(box_w, 74.0));
        let ehp = self.hp[1].value(now, if settings.animations { 0.8 } else { 0.0 });
        let (ename, elv) = split_name_level(&f4.c("Label3").caption);
        if self.enemy_species > 0 {
            hp_box(p, enemy_box, &ename, &elv, ehp, self.hp[1].max, false, f4.c("Image1").visible);
        }
        if self.own_species > 0 && own_chosen(f4) {
            let ohp = self.hp[0].value(now, if settings.animations { 0.8 } else { 0.0 });
            let (oname, olv) = split_name_level(&f4.c("Label2").caption);
            let slide = (1.0 - send) * box_w;
            hp_box(p, own_box.translate(Vec2::new(slide, 0.0)), &oname, &olv, ohp, self.hp[0].max, true, false);
        }

        // Battle-start flash.
        let flash = prog(self.intro_at, 0.5);
        if flash < 1.0 {
            let a = if ((flash * 6.0) as i32) % 2 == 0 { 1.0 - flash } else { 0.3 * (1.0 - flash) };
            p.rect_filled(field, 0.0, Color32::from_white_alpha((255.0 * a) as u8));
        }
    }
}

/// "Charmander L.5" / "Enemy Lotad L.7" -> ("Charmander", "5").
fn split_name_level(caption: &str) -> (String, String) {
    let c = strip_amp(caption);
    let c = c.strip_prefix("Enemy ").unwrap_or(&c).to_string();
    match c.rsplit_once(" L.") {
        Some((n, l)) => (n.to_string(), l.to_string()),
        None => (c, String::new()),
    }
}

fn name_of(caption: &str) -> String {
    split_name_level(caption).0
}

/// "Scratch (Normal)" -> ("Scratch", "Normal").
fn split_move(item: &str) -> (String, String) {
    let s = strip_amp(item);
    match s.rsplit_once(" (") {
        Some((n, ty)) => (n.to_string(), ty.trim_end_matches(')').to_string()),
        None => (s, String::new()),
    }
}

pub fn type_color(ty: &str) -> Color32 {
    let rgb = match ty.to_ascii_lowercase().as_str() {
        "normal" => 0xA8A878,
        "fire" => 0xF08030,
        "water" => 0x6890F0,
        "grass" => 0x78C850,
        "electric" => 0xF8D030,
        "ice" => 0x98D8D8,
        "fighting" => 0xC03028,
        "poison" => 0xA040A0,
        "ground" => 0xE0C068,
        "flying" => 0xA890F0,
        "psychic" => 0xF85888,
        "bug" => 0xA8B820,
        "rock" => 0xB8A038,
        "ghost" => 0x705898,
        "dragon" => 0x7038F8,
        "dark" => 0x705848,
        "steel" => 0xB8B8D0,
        _ => 0x9098A0,
    };
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

fn move_button(ui: &mut egui::Ui, r: Rect, name: &str, ty: &str, enabled: bool) -> egui::Response {
    let resp = t::button(ui, r, "", t::ButtonKind::Plain, enabled);
    let p = ui.painter();
    let col = if enabled { type_color(ty) } else { t::GREY };
    let press = if resp.is_pointer_button_down_on() { t::PX * 2.0 } else { 0.0 };
    let face = Rect::from_min_max(r.min + Vec2::new(t::PX, t::PX + press), Pos2::new(r.max.x - t::PX, r.max.y - t::PX * 3.0 + press));
    p.rect_filled(Rect::from_min_size(face.min, Vec2::new(8.0, face.height())), CornerRadius { nw: 4, sw: 4, ne: 0, se: 0 }, col);
    let ink = if enabled { t::INK } else { t::GREY };
    t::text(p, face.left_center() + Vec2::new(18.0, -8.0), Align2::LEFT_CENTER, name, t::bold(19.0), ink);
    if !ty.is_empty() {
        let galley = p.layout_no_wrap(ty.to_uppercase(), t::bold(12.0), Color32::WHITE);
        let pill = Rect::from_min_size(face.left_center() + Vec2::new(18.0, 5.0), galley.size() + Vec2::new(10.0, 2.0));
        p.rect_filled(pill, 4.0, col);
        p.galley(pill.min + Vec2::new(5.0, 1.0), galley, Color32::WHITE);
    }
    resp
}

#[allow(clippy::too_many_arguments)]
fn hp_box(p: &egui::Painter, r: Rect, name: &str, lv: &str, hp: f32, max: i32, own: bool, owned: bool) {
    p.rect_filled(r.translate(Vec2::new(3.0, 4.0)), 10.0, Color32::from_black_alpha(40));
    t::frame(p, r);
    let inner = r.shrink2(Vec2::new(14.0, 8.0));
    t::text(p, inner.left_top() + Vec2::new(0.0, 1.0), Align2::LEFT_TOP, name, t::bold(19.0), t::INK);
    t::text(p, inner.right_top() + Vec2::new(0.0, 1.0), Align2::RIGHT_TOP, &format!("Lv{lv}"), t::bold(17.0), t::INK);
    if owned {
        poke_ball(p, inner.left_top() + Vec2::new(p.layout_no_wrap(name.to_string(), t::bold(19.0), t::INK).size().x + 12.0, 10.0), 6.0, 0.0);
    }
    let bar_y = inner.top() + 26.0;
    let label = Rect::from_min_size(Pos2::new(inner.left(), bar_y), Vec2::new(30.0, 12.0));
    p.rect_filled(label, 3.0, Color32::from_rgb(0xF8, 0xA0, 0x38));
    p.text(label.center(), Align2::CENTER_CENTER, "HP", t::bold(11.0), Color32::WHITE);
    let bar = Rect::from_min_max(Pos2::new(label.right() + 4.0, bar_y), Pos2::new(inner.right(), bar_y + 12.0));
    t::hp_bar(p, bar, hp / max.max(1) as f32);
    if own {
        t::text(p, Pos2::new(inner.right(), bar_y + 16.0), Align2::RIGHT_TOP, &format!("{}/{}", hp.round() as i32, max), t::bold(17.0), t::INK);
    }
}

fn poke_ball(p: &egui::Painter, c: Pos2, r: f32, tilt: f32) {
    let rot = egui::emath::Rot2::from_angle(tilt);
    p.circle_filled(c, r + 1.5, t::FRAME);
    // Top half red, bottom white.
    let n = 16;
    let half = |from: f32, col: Color32| {
        let mut pts = Vec::new();
        for k in 0..=n {
            let a = from + PI * k as f32 / n as f32;
            pts.push(c + rot * Vec2::new(a.cos(), a.sin()) * r);
        }
        p.add(Shape::convex_polygon(pts, col, Stroke::NONE));
    };
    half(PI, t::RED);
    half(0.0, Color32::WHITE);
    p.line_segment([c + rot * Vec2::new(-r, 0.0), c + rot * Vec2::new(r, 0.0)], Stroke::new(1.5, t::FRAME));
    p.circle_filled(c, r * 0.32, t::FRAME);
    p.circle_filled(c, r * 0.2, Color32::WHITE);
}

fn sparkle(p: &egui::Painter, c: Pos2, k: f32, col: Color32) {
    for i in 0..8 {
        let a = i as f32 * PI / 4.0 + k * 1.5;
        let d = 10.0 + k * 40.0;
        let pos = c + Vec2::new(a.cos(), a.sin()) * d - Vec2::new(0.0, k * 20.0);
        let s = 5.0 * (1.0 - k);
        let alpha = ((1.0 - k) * 255.0) as u8;
        let cc = Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), alpha);
        p.line_segment([pos - Vec2::new(s, 0.0), pos + Vec2::new(s, 0.0)], Stroke::new(2.0, cc));
        p.line_segment([pos - Vec2::new(0.0, s), pos + Vec2::new(0.0, s)], Stroke::new(2.0, cc));
    }
}

pub fn item_icon(p: &egui::Painter, c: Pos2, name: &str) {
    if name.contains("BALL") {
        poke_ball(p, c, 9.0, 0.0);
        let band = match name {
            n if n.starts_with("GREAT") => Some(t::BLUE),
            n if n.starts_with("ULTRA") => Some(Color32::from_rgb(0xF8, 0xD0, 0x30)),
            n if n.starts_with("MASTER") => Some(Color32::from_rgb(0xA0, 0x48, 0xC8)),
            _ => None,
        };
        if let Some(b) = band {
            p.circle_filled(c - Vec2::new(0.0, 4.0), 4.0, b);
        }
    } else {
        // Spray bottle.
        let body = Rect::from_center_size(c + Vec2::new(0.0, 2.0), Vec2::new(12.0, 14.0));
        p.rect_filled(body.expand(1.5), 3.0, t::FRAME);
        p.rect_filled(body, 2.0, if name.starts_with("SUPER") { Color32::from_rgb(0xF0, 0x70, 0x40) } else { Color32::from_rgb(0xA0, 0x60, 0xD0) });
        p.rect_filled(Rect::from_center_size(c - Vec2::new(0.0, 8.0), Vec2::new(6.0, 5.0)), 1.0, t::FRAME);
    }
}

/// Typewriter text: the full layout is computed once so words never jump between lines.
fn draw_typed(p: &egui::Painter, r: Rect, text: &str, shown: usize, ctx: &egui::Context) {
    use egui::text::{LayoutJob, TextFormat};
    let font = t::body(21.0);
    let split = text.char_indices().nth(shown).map_or(text.len(), |(i, _)| i);
    let job = |ink: Color32| {
        let mut j = LayoutJob::default();
        j.append(&text[..split], 0.0, TextFormat::simple(font.clone(), ink));
        j.append(&text[split..], 0.0, TextFormat::simple(font.clone(), Color32::TRANSPARENT));
        j.wrap.max_width = r.width();
        ctx.fonts(|f| f.layout_job(j))
    };
    p.galley(r.min + Vec2::splat(1.0), job(t::INK_SHADOW), t::INK_SHADOW);
    p.galley(r.min, job(t::INK), t::INK);
}

fn ease_out(x: f32) -> f32 {
    1.0 - (1.0 - x).powi(3)
}

fn ease_in_out(x: f32) -> f32 {
    if x < 0.5 { 4.0 * x * x * x } else { 1.0 - (-2.0 * x + 2.0).powi(3) / 2.0 }
}
