//! "Enter your name:" (OKRightDlg) as a GBA-style letter grid. Physical keyboards work too.
//! OK sends exactly what the original dialog would: Edit1's text, then OKBtn's click.

use super::theme as t;
use crate::vcl::runtime::{Event, Input, Snapshot};
use egui::{Align2, Color32, Pos2, Rect, Vec2};
use std::sync::mpsc::Sender;

const COLS: usize = 10;
const UPPER: [&str; 4] = ["ABCDEFGHIJ", "KLMNOPQRST", "UVWXYZ .-'", "0123456789"];
const LOWER: [&str; 4] = ["abcdefghij", "klmnopqrst", "uvwxyz .-'", "!?&é,0123"];

#[derive(Default)]
pub struct Naming {
    name: String,
    lower: bool,
    /// Dialog id this state belongs to.
    for_dialog: u64,
    /// OK already sent for this dialog (never click twice).
    sent: bool,
}

impl Naming {
    pub fn show(&mut self, ui: &mut egui::Ui, area: Rect, snap: &Snapshot, dialog_id: u64, tx: &Sender<Input>, now: f64) {
        let Some(dlg) = snap.forms.iter().find(|f| f.var == "OKRightDlg") else { return };
        let max = dlg.c("Edit1").max_length.max(1) as usize;
        if self.for_dialog != dialog_id {
            *self = Naming { name: dlg.c("Edit1").text.clone(), lower: false, for_dialog: dialog_id, sent: false };
        }
        let col = Rect::from_center_size(area.center(), Vec2::new(area.width().min(560.0), area.height())).shrink(10.0);
        let p = ui.painter().clone();

        // Header and the name with blinking cursor.
        let head = Rect::from_min_size(col.min + Vec2::new(0.0, 10.0), Vec2::new(col.width(), 150.0));
        t::frame(&p, head);
        t::text(&p, head.center_top() + Vec2::new(0.0, 26.0), Align2::CENTER_CENTER, "YOUR NAME?", t::title(15.0), t::pal().ink);
        let slots_w = (col.width() - 60.0).min(15.0 * 26.0);
        let cell = slots_w / max as f32;
        let x0 = head.center().x - slots_w / 2.0;
        let y = head.top() + 92.0;
        let chars: Vec<char> = self.name.chars().collect();
        for k in 0..max {
            let cx = x0 + (k as f32 + 0.5) * cell;
            p.line_segment([Pos2::new(cx - cell * 0.38, y + 16.0), Pos2::new(cx + cell * 0.38, y + 16.0)], egui::Stroke::new(2.0, if k == chars.len() { t::pal().danger } else { t::pal().grey }));
            if let Some(c) = chars.get(k) {
                t::text(&p, Pos2::new(cx, y), Align2::CENTER_CENTER, &c.to_string(), t::bold((cell * 1.1).min(28.0)), t::pal().ink);
            }
        }
        if chars.is_empty() {
            t::text(&p, Pos2::new(head.center().x, y - 36.0), Align2::CENTER_CENTER, "(empty = Ash)", t::body(15.0), t::pal().grey);
        }
        if (now * 2.0).fract() < 0.5 && chars.len() < max {
            let cx = x0 + (chars.len() as f32 + 0.5) * cell;
            t::icon(&p, Pos2::new(cx, y - 24.0), 10.0, t::Icon::Down, t::pal().danger);
        }

        // Letter grid.
        let rows = if self.lower { LOWER } else { UPPER };
        let grid_top = head.bottom() + 14.0;
        let gw = col.width();
        let kw = gw / COLS as f32;
        let kh = (kw * 1.15).clamp(44.0, 56.0);
        let mut typed: Option<char> = None;
        for (ri, row) in rows.iter().enumerate() {
            for (ci, ch) in row.chars().enumerate() {
                let r = Rect::from_min_size(Pos2::new(col.left() + ci as f32 * kw, grid_top + ri as f32 * (kh + 6.0)), Vec2::new(kw - 6.0, kh));
                let label = if ch == ' ' { "␣".to_string() } else { ch.to_string() };
                let resp = t::button(ui, r, if ch == ' ' { "" } else { &label }, t::ButtonKind::Plain, true);
                if ch == ' ' {
                    p.line_segment([r.left_bottom() + Vec2::new(kw * 0.25, -16.0), r.right_bottom() + Vec2::new(-kw * 0.25, -16.0)], egui::Stroke::new(2.0, t::pal().ink));
                }
                if resp.clicked() {
                    typed = Some(ch);
                }
            }
        }
        // Bottom row: case, delete, OK.
        let by = grid_top + 4.0 * (kh + 6.0) + 6.0;
        let third = (gw - 12.0) / 3.0;
        let b = |k: usize| Rect::from_min_size(Pos2::new(col.left() + k as f32 * (third + 6.0), by), Vec2::new(third, 54.0));
        if t::button(ui, b(0), if self.lower { "ABC" } else { "abc" }, t::ButtonKind::Plain, true).clicked() {
            self.lower = !self.lower;
        }
        let mut delete = t::button(ui, b(1), "DEL", t::ButtonKind::Danger, !self.name.is_empty()).clicked();
        let mut ok = t::button(ui, b(2), "OK", t::ButtonKind::Primary, true).clicked();

        // Physical keyboard.
        ui.input(|i| {
            for e in &i.events {
                match e {
                    egui::Event::Text(s) => {
                        for c in s.chars().filter(|c| (*c as u32) < 256 && !c.is_control()) {
                            typed = Some(c);
                        }
                    }
                    egui::Event::Key { key: egui::Key::Backspace, pressed: true, .. } => delete = true,
                    egui::Event::Key { key: egui::Key::Enter, pressed: true, .. } => ok = true,
                    _ => {}
                }
            }
        });
        if let Some(c) = typed {
            if self.name.chars().count() < max {
                self.name.push(c);
            }
        }
        if delete {
            self.name.pop();
        }
        if ok && !self.sent && snap.dialog.as_ref().is_some_and(|(id, _)| *id == dialog_id) {
            self.sent = true;
            let _ = tx.send(Input::SetText { form: "OKRightDlg".into(), control: "Edit1".into(), text: self.name.clone() });
            let _ = tx.send(Input::Event { form: "OKRightDlg".into(), control: "OKBtn".into(), event: Event::Click });
        }
        let _ = Color32::WHITE;
    }
}
