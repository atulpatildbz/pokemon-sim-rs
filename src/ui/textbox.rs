//! MessageDlg as a Pokémon text box: typewriter text, ▼ prompt, and a YES/NO choice box.
//! Errors keep a centred card so they never read as story text.

use super::settings::Settings;
use super::theme as t;
use crate::vcl::runtime::{DialogAnswer, Input, MsgDlgType, MB_CANCEL, MB_NO, MB_OK, MB_YES};
use egui::text::{LayoutJob, TextFormat};
use egui::{Align2, Color32, Id, Order, Pos2, Rect, Sense, Vec2};
use std::sync::mpsc::Sender;

#[derive(Default)]
pub struct TextBox {
    id: u64,
    started: f64,
    /// Choice-box cursor (keyboard).
    cursor: usize,
}

impl TextBox {
    /// Draw dialog `id` inside `area` (the scene's rect). Answers go straight to the logic thread.
    #[allow(clippy::too_many_arguments)]
    pub fn show(&mut self, ctx: &egui::Context, area: Rect, id: u64, text: &str, kind: MsgDlgType, buttons: u16, settings: &Settings, tx: &Sender<Input>) {
        let now = ctx.input(|i| i.time);
        if self.id != id {
            *self = TextBox { id, started: now, cursor: 0 };
        }
        let choices: Vec<(&str, i32)> = [(MB_YES, "YES", 6), (MB_NO, "NO", 7), (MB_OK, "OK", 1), (MB_CANCEL, "CANCEL", 2)]
            .into_iter()
            .filter(|(bit, _, _)| buttons & bit != 0)
            .map(|(_, l, r)| (l, r))
            .collect();
        let only_ok = choices.len() <= 1;
        let answer = |r: i32| {
            let _ = tx.send(Input::Answer { id, answer: DialogAnswer::Button(r) });
        };
        let total = text.chars().count();
        let shown = match settings.text_speed.chars_per_sec() {
            Some(cps) if settings.animations => (((now - self.started) as f32 * cps) as usize).min(total),
            _ => total,
        };
        let done = shown >= total;
        if !done {
            ctx.request_repaint();
        }

        // Full-screen catcher: dims nothing, but a tap anywhere advances the text.
        let catch_layer = egui::LayerId::new(Order::Foreground, Id::new(("tb_catch", id)));
        ctx.move_to_top(catch_layer);
        ctx.move_to_top(egui::LayerId::new(Order::Foreground, Id::new(("tb", id))));
        let tapped = egui::Area::new(Id::new(("tb_catch", id)))
            .order(Order::Foreground)
            .fixed_pos(Pos2::ZERO)
            .show(ctx, |ui| ui.allocate_rect(ctx.screen_rect(), Sense::click()).clicked())
            .inner;
        let confirm_key = ctx.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Space) || i.key_pressed(egui::Key::Z));
        if !done && (tapped || confirm_key) {
            self.started = -1e9;
        } else if done && only_ok && (tapped || confirm_key) {
            answer(choices.first().map_or(1, |c| c.1));
        }

        let is_error = kind == MsgDlgType::Error || kind == MsgDlgType::Warning && text.starts_with("Access violation");
        let width = (area.width() - 24.0).min(760.0);
        let font = t::body(20.0);
        let mut job = LayoutJob::default();
        let split = text.char_indices().nth(shown).map_or(text.len(), |(i, _)| i);
        job.append(&text[..split], 0.0, TextFormat::simple(font.clone(), t::INK));
        job.append(&text[split..], 0.0, TextFormat::simple(font.clone(), Color32::TRANSPARENT));
        job.wrap.max_width = width - 48.0;
        let galley = ctx.fonts(|f| f.layout_job(job.clone()));
        let mut shadow_job = job;
        for s in &mut shadow_job.sections {
            if s.format.color != Color32::TRANSPARENT {
                s.format.color = t::INK_SHADOW;
            }
        }
        let shadow = ctx.fonts(|f| f.layout_job(shadow_job));
        let header = if is_error { 30.0 } else { 0.0 };
        let height = (galley.size().y + 40.0 + header).max(96.0);
        let rect = if is_error {
            Rect::from_center_size(area.center(), Vec2::new(width.min(520.0), height + 56.0))
        } else {
            Rect::from_min_size(Pos2::new(area.center().x - width / 2.0, area.bottom() - height - 12.0), Vec2::new(width, height))
        };

        egui::Area::new(Id::new(("tb", id))).order(Order::Foreground).fixed_pos(Pos2::ZERO).show(ctx, |ui| {
            let p = ui.painter().clone();
            // Slide up a little as it appears.
            let appear = ctx.animate_bool_with_time(Id::new(("tb_in", id)), true, if settings.animations { 0.12 } else { 0.0 });
            let rect = rect.translate(Vec2::new(0.0, (1.0 - appear) * 24.0));
            if is_error {
                p.rect_filled(ctx.screen_rect(), 0.0, Color32::from_black_alpha(90));
            }
            p.rect_filled(rect.translate(Vec2::splat(4.0)), 10.0, Color32::from_black_alpha(40));
            t::frame(&p, rect);
            let mut text_pos = rect.min + Vec2::new(24.0, 20.0);
            if is_error {
                t::text(&p, text_pos, Align2::LEFT_TOP, kind.title(), t::bold(20.0), t::RED);
                text_pos.y += header;
            }
            p.galley(text_pos + Vec2::splat(1.0), shadow, t::INK_SHADOW);
            p.galley(text_pos, galley, t::INK);
            if done && only_ok && !is_error {
                // Bobbing ▼ prompt.
                let bob = ((now * 4.0).sin() * 2.0) as f32;
                t::icon(&p, rect.right_bottom() + Vec2::new(-26.0, -20.0 + bob), 14.0, t::Icon::Down, t::RED);
            }
            if is_error && done {
                let b = Rect::from_center_size(Pos2::new(rect.center().x, rect.bottom() - 34.0), Vec2::new(140.0, 44.0));
                if t::button(ui, b, "OK", t::ButtonKind::Primary, true).clicked() {
                    answer(1);
                }
            } else if done && !only_ok {
                // Choice box above the text box's right edge.
                let row = 40.0;
                let cw = 170.0;
                let ch = row * choices.len() as f32 + 24.0;
                let cb = Rect::from_min_size(Pos2::new(rect.right() - cw, rect.top() - ch - 6.0), Vec2::new(cw, ch));
                t::frame(&p, cb);
                let (up, down) = ctx.input(|i| (i.key_pressed(egui::Key::ArrowUp), i.key_pressed(egui::Key::ArrowDown)));
                if up {
                    self.cursor = (self.cursor + choices.len() - 1) % choices.len();
                }
                if down {
                    self.cursor = (self.cursor + 1) % choices.len();
                }
                for (k, (label, result)) in choices.iter().enumerate() {
                    let r = Rect::from_min_size(cb.min + Vec2::new(8.0, 12.0 + k as f32 * row), Vec2::new(cw - 16.0, row));
                    let resp = ui.interact(r, Id::new(("tb_choice", id, k)), Sense::click());
                    if resp.hovered() {
                        self.cursor = k;
                    }
                    if self.cursor == k {
                        t::icon(&p, r.left_center() + Vec2::new(14.0, 0.0), 14.0, t::Icon::Right, t::INK);
                    }
                    t::text(&p, r.left_center() + Vec2::new(30.0, 0.0), Align2::LEFT_CENTER, label, t::bold(20.0), t::INK);
                    if resp.clicked() || (confirm_key && self.cursor == k) {
                        answer(*result);
                    }
                }
            }
        });
    }
}
