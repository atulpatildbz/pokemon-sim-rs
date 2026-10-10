//! Starter choice (Form4.ListView1 + Label1) as a swipeable card carousel. Choosing sends what a
//! click on the original list item would: select it, then ListView1's OnClick.

use super::battle::type_color;
use super::theme as t;
use crate::assets::Assets;
use crate::vcl::render::Renderer;
use crate::vcl::runtime::{Event, Input, Snapshot};
use egui::{Align2, Color32, Pos2, Rect, Sense, Shape, Vec2};
use std::sync::mpsc::Sender;

#[derive(Default)]
pub struct Starter {
    index: usize,
    /// Smoothed carousel position (in cards).
    pos: f32,
    drag: f32,
    sent: bool,
}

pub fn active(snap: &Snapshot) -> bool {
    snap.forms.iter().find(|f| f.var == "Form4" && f.visible).is_some_and(|f| f.has("ListView1") && f.showing(f.idx("ListView1")))
}

impl Starter {
    #[allow(clippy::too_many_arguments)]
    pub fn show(&mut self, ui: &mut egui::Ui, area: Rect, snap: &Snapshot, renderer: &mut Renderer, assets: &Assets, tx: &Sender<Input>, animate: bool, now: f64) {
        let Some(f4) = snap.forms.iter().find(|f| f.var == "Form4") else { return };
        let lv = f4.c("ListView1");
        let n = lv.list_items.len();
        if n == 0 {
            return;
        }
        self.index = self.index.min(n - 1);
        let free = snap.dialog.is_none();
        let p = ui.painter().clone();
        let col = Rect::from_center_size(area.center(), Vec2::new(area.width().min(620.0), area.height())).shrink(10.0);
        t::text(&p, col.center_top() + Vec2::new(0.0, 34.0), Align2::CENTER_CENTER, "CHOOSE YOUR", t::title(14.0), t::PAPER);
        t::text(&p, col.center_top() + Vec2::new(0.0, 62.0), Align2::CENTER_CENTER, "FIRST POKéMON!", t::title(18.0), Color32::from_rgb(0xF8, 0xD0, 0x30));

        // Carousel with swipe.
        let card_w = (col.width() * 0.72).min(360.0);
        let card_h = (col.height() - 230.0).clamp(260.0, 420.0);
        let strip = Rect::from_center_size(Pos2::new(col.center().x, col.top() + 100.0 + card_h / 2.0), Vec2::new(col.width(), card_h));
        let resp = ui.interact(strip, ui.id().with("starter_swipe"), Sense::click_and_drag());
        if resp.dragged() {
            self.drag += resp.drag_delta().x;
        }
        if resp.drag_stopped() {
            if self.drag < -card_w * 0.2 && self.index + 1 < n {
                self.index += 1;
            } else if self.drag > card_w * 0.2 && self.index > 0 {
                self.index -= 1;
            }
            self.drag = 0.0;
        }
        let (left, right) = ui.input(|i| (i.key_pressed(egui::Key::ArrowLeft), i.key_pressed(egui::Key::ArrowRight)));
        if left && self.index > 0 {
            self.index -= 1;
        }
        if right && self.index + 1 < n {
            self.index += 1;
        }
        let target = self.index as f32 - self.drag / card_w;
        self.pos = if animate { self.pos + (target - self.pos) * 0.25 } else { target };
        if (self.pos - target).abs() > 0.001 {
            ui.ctx().request_repaint();
        }
        let key = lv.small_images.clone().unwrap_or_else(|| "Form4.PKMNImages".into());
        for (k, it) in lv.list_items.iter().enumerate() {
            let off = k as f32 - self.pos;
            if off.abs() > 1.6 {
                continue;
            }
            let scale = 1.0 - off.abs().min(1.0) * 0.18;
            let c = Pos2::new(strip.center().x + off * card_w * 0.92, strip.center().y);
            let r = Rect::from_center_size(c, Vec2::new(card_w, card_h) * scale);
            let species = it.image_index + 1;
            let types: Vec<String> = [assets.data.pkmn_type1.get(species as usize), assets.data.pkmn_type2.get(species as usize)]
                .into_iter()
                .flatten()
                .filter(|&&ty| ty > 0)
                .map(|&ty| assets.data.type_name.get(ty as usize).cloned().unwrap_or_default())
                .fold(Vec::<String>::new(), |mut v, ty| {
                    if !v.contains(&ty) {
                        v.push(ty);
                    }
                    v
                });
            let main = types.first().map_or(t::BLUE, |ty| type_color(ty));
            p.rect_filled(r.translate(Vec2::new(4.0, 6.0)), 18.0, Color32::from_black_alpha(50));
            p.rect_filled(r, 18.0, t::FRAME);
            p.rect_filled(r.shrink(3.0), 16.0, t::lerp_color(main, Color32::WHITE, 0.55));
            // Sun-burst behind the sprite.
            let sc = Pos2::new(r.center().x, r.top() + r.height() * 0.42);
            for i in 0..12 {
                let a = i as f32 * std::f32::consts::PI / 6.0 + if animate { now as f32 * 0.3 } else { 0.0 };
                let a2 = a + 0.22;
                let len = r.width() * 0.55;
                p.add(Shape::convex_polygon(vec![sc, sc + Vec2::angled(a) * len, sc + Vec2::angled(a2) * len], Color32::from_white_alpha(40), egui::Stroke::NONE));
            }
            p.add(Shape::ellipse_filled(sc + Vec2::new(0.0, r.height() * 0.2), Vec2::new(r.width() * 0.32, r.width() * 0.07), Color32::from_black_alpha(35)));
            if let Some(tex) = renderer.texture(ui.ctx(), assets, &key, it.image_index) {
                let px = ((r.width() * 0.55) / 64.0).floor().max(1.0) * 64.0;
                let hop = if animate && k == self.index { (((now * 3.0).sin()).max(0.0) * 6.0) as f32 } else { 0.0 };
                let sr = Rect::from_center_size(sc - Vec2::new(0.0, hop), Vec2::splat(px));
                p.image(tex.id(), sr, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
            }
            t::text(&p, Pos2::new(r.center().x, r.bottom() - r.height() * 0.24), Align2::CENTER_CENTER, &it.caption, t::bold(28.0 * scale), t::INK);
            // Type pills.
            let pills: Vec<_> = types.iter().map(|ty| p.layout_no_wrap(ty.to_uppercase(), t::bold(14.0), Color32::WHITE)).collect();
            let total: f32 = pills.iter().map(|g| g.size().x + 18.0).sum::<f32>() - 8.0;
            let mut x = r.center().x - total / 2.0;
            for (g, ty) in pills.into_iter().zip(&types) {
                let pr = Rect::from_min_size(Pos2::new(x, r.bottom() - r.height() * 0.14), g.size() + Vec2::new(10.0, 4.0));
                p.rect_filled(pr, 5.0, type_color(ty));
                x += pr.width() + 8.0;
                p.galley(pr.min + Vec2::new(5.0, 2.0), g, Color32::WHITE);
            }
        }
        // Arrows and dots.
        let arrow = |ui: &mut egui::Ui, dx: f32, icon: t::Icon, enabled: bool| {
            let r = Rect::from_center_size(Pos2::new(strip.center().x + dx, strip.center().y), Vec2::splat(48.0));
            let resp = t::button(ui, r, "", t::ButtonKind::Plain, enabled);
            t::icon(ui.painter(), r.center() - Vec2::new(0.0, 2.0), 16.0, icon, if enabled { t::INK } else { t::GREY });
            resp.clicked()
        };
        let edge = (col.width() / 2.0 - 26.0).min(card_w / 2.0 + 40.0);
        if arrow(ui, -edge, t::Icon::Left, self.index > 0) {
            self.index -= 1;
        }
        if arrow(ui, edge, t::Icon::Right, self.index + 1 < n) {
            self.index += 1;
        }
        for k in 0..n {
            let c = Pos2::new(strip.center().x + (k as f32 - (n as f32 - 1.0) / 2.0) * 18.0, strip.bottom() + 18.0);
            p.circle_filled(c, if k == self.index { 6.0 } else { 4.0 }, if k == self.index { t::PAPER } else { Color32::from_white_alpha(120) });
        }
        let b = Rect::from_center_size(Pos2::new(col.center().x, strip.bottom() + 70.0), Vec2::new(card_w.min(320.0), 60.0));
        let name = &lv.list_items[self.index].caption;
        if t::button(ui, b, &format!("I CHOOSE {}!", name.to_uppercase()), t::ButtonKind::Primary, free && !self.sent).clicked() {
            self.sent = true;
            let _ = tx.send(Input::SetItemIndex { form: "Form4".into(), control: "ListView1".into(), index: self.index as i32 });
            let _ = tx.send(Input::Event { form: "Form4".into(), control: "ListView1".into(), event: Event::Click });
        }
    }

    /// Reset when the list goes away (a new game shows it again).
    pub fn reset(&mut self) {
        self.sent = false;
    }
}
