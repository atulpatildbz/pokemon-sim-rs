//! The hub: Form4 between battles. A trainer card (from the Statistics window's labels), the
//! first POKéMON in the PC, and big tiles for the Actions menu. Every tile fires the original
//! menu item; groups (trainers, tournaments, wild) open a sheet of opponent cards.

use super::theme as t;
use crate::assets::Assets;
use crate::vcl::model::{Form, MenuItem};
use crate::vcl::render::{strip_amp, Renderer};
use crate::vcl::runtime::{Input, Snapshot};
use egui::{Align2, Color32, CornerRadius, Pos2, Rect, Sense, Shape, Stroke, Vec2};
use std::f32::consts::PI;
use std::sync::mpsc::Sender;

pub enum Request {
    OpenSheet(String),
    /// Open the opponent-card sheet for this menu path (item names from the top level).
    OpenCards(Vec<String>),
}

/// Form4 shows nothing of its own (no battle, starter list, wild search or evolution choice).
pub fn active(snap: &Snapshot) -> bool {
    let Some(f4) = snap.forms.iter().find(|f| f.var == "Form4" && f.visible) else { return false };
    let shown = |n: &str| f4.has(n) && f4.showing(f4.idx(n));
    !["GroupBox1", "ListView1", "ComboBox1", "EvolveChoice"].iter().any(|n| shown(n))
}

#[derive(Clone, Copy)]
enum Glyph {
    Trainer,
    Wild,
    Trophy,
    Mart,
    Dex,
    Evolve,
    Release,
    Types,
    Coin,
    Pc,
}

struct Tile {
    label: &'static str,
    /// Menu path from Form4's top level.
    path: &'static [&'static str],
    glyph: Glyph,
    color: Color32,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

const TILES: [Tile; 9] = [
    Tile { label: "TRAINERS", path: &["Action1", "FightTrainer1"], glyph: Glyph::Trainer, color: rgb(0xE0, 0x50, 0x48) },
    Tile { label: "WILD", path: &["Action1", "FightWild1"], glyph: Glyph::Wild, color: rgb(0x48, 0xA8, 0x58) },
    Tile { label: "TOURNAMENTS", path: &["Action1", "Tournaments1"], glyph: Glyph::Trophy, color: rgb(0xE0, 0xA0, 0x20) },
    Tile { label: "MART", path: &["Action1", "Buy1"], glyph: Glyph::Mart, color: rgb(0x38, 0x80, 0xD8) },
    Tile { label: "POKéDEX", path: &["Action1", "ViewDEX1"], glyph: Glyph::Dex, color: rgb(0xD0, 0x38, 0x38) },
    Tile { label: "EVOLVE", path: &["Action1", "Evolve1"], glyph: Glyph::Evolve, color: rgb(0x90, 0x58, 0xD0) },
    Tile { label: "RELEASE", path: &["Action1", "ReleasePKMN1"], glyph: Glyph::Release, color: rgb(0x60, 0x90, 0xA8) },
    Tile { label: "TYPE CHART", path: &["Action1", "TypeComparisons1"], glyph: Glyph::Types, color: rgb(0x58, 0x70, 0xB0) },
    Tile { label: "GAME CORNER", path: &["Action1", "GameCorner1"], glyph: Glyph::Coin, color: rgb(0xC8, 0x80, 0x30) },
];

/// The item at `path` and whether it and all its ancestors are enabled and visible.
pub fn find_item<'a>(form: &'a Form, path: &[impl AsRef<str>]) -> Option<(&'a MenuItem, bool)> {
    let mut items: &[MenuItem] = &form.menu;
    let mut ok = true;
    let mut found = None;
    for name in path {
        let m = items.iter().find(|m| m.name == name.as_ref())?;
        ok &= m.enabled && m.visible;
        items = &m.children;
        found = Some(m);
    }
    found.map(|m| (m, ok))
}

fn label_of(f: Option<&Form>, name: &str) -> String {
    f.filter(|f| f.has(name)).map(|f| strip_amp(&f.c(name).caption).trim().to_string()).unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
pub fn show(ui: &mut egui::Ui, area: Rect, snap: &Snapshot, renderer: &mut Renderer, assets: &Assets, tx: &Sender<Input>, now: f64, animate: bool) -> Option<Request> {
    let f4 = snap.forms.iter().find(|f| f.var == "Form4")?;
    let f5 = snap.forms.iter().find(|f| f.var == "Form5");
    let free = snap.dialog.is_none();
    let mut req = None;
    let area = Rect::from_center_size(area.center(), Vec2::new(area.width().min(860.0), area.height())).shrink(10.0);
    let wide = area.width() > 620.0;

    // ---------------- trainer card
    let card = Rect::from_min_size(area.min, Vec2::new(area.width(), 118.0));
    let p = &ui.painter().clone();
    p.rect_filled(card.translate(Vec2::new(3.0, 5.0)), 14.0, Color32::from_black_alpha(45));
    p.rect_filled(card, 14.0, t::FRAME);
    let inner = card.shrink(t::PX * 1.5);
    p.rect_filled(inner, 12.0, rgb(0xF8, 0xE8, 0xB0));
    // Diagonal stripes for a card-stock feel.
    let stripes = p.with_clip_rect(inner);
    let mut x = inner.left() - inner.height();
    while x < inner.right() {
        stripes.line_segment([Pos2::new(x, inner.bottom()), Pos2::new(x + inner.height(), inner.top())], Stroke::new(10.0, Color32::from_white_alpha(40)));
        x += 26.0;
    }
    let name = label_of(f5, "PlayerNameLbl");
    t::text(p, inner.left_top() + Vec2::new(16.0, 12.0), Align2::LEFT_TOP, "TRAINER CARD", t::title(11.0), rgb(0xA0, 0x70, 0x20));
    t::text(p, inner.left_top() + Vec2::new(16.0, 32.0), Align2::LEFT_TOP, if name.is_empty() { "-" } else { &name }, t::bold(30.0), t::INK);
    let stats = [
        ("MONEY", format!("¥{}", label_of(f5, "MoneyLbl"))),
        ("WINS", format!("{}/{}", label_of(f5, "WinsLbl"), label_of(f5, "FightsLbl"))),
        ("CAUGHT", label_of(f5, "CapturesLbl")),
        ("DEX", format!("{}/386", label_of(f5, "OwnedPKMNLbl"))),
    ];
    let col_w = (inner.width() - 32.0) / 4.0;
    for (k, (k_label, v)) in stats.iter().enumerate() {
        let pos = inner.left_bottom() + Vec2::new(16.0 + k as f32 * col_w, -38.0);
        t::text(p, pos, Align2::LEFT_TOP, k_label, t::bold(13.0), rgb(0xA0, 0x70, 0x20));
        t::text(p, pos + Vec2::new(0.0, 14.0), Align2::LEFT_TOP, v, t::bold(19.0), t::INK);
    }
    // PC button on the card.
    let pc = Rect::from_min_size(inner.right_top() + Vec2::new(-112.0, 12.0), Vec2::new(100.0, 42.0));
    if t::button(ui, pc, "  PC", t::ButtonKind::Primary, f5.is_some_and(|f| f.visible)).clicked() {
        req = Some(Request::OpenSheet("Form5".into()));
    }
    glyph(ui.painter(), pc.left_center() + Vec2::new(22.0, -2.0), 9.0, Glyph::Pc, Color32::WHITE);

    // ---------------- party strip: the first POKéMON in the PC
    let strip = Rect::from_min_size(Pos2::new(area.left(), card.bottom() + 10.0), Vec2::new(area.width(), 64.0));
    if let Some(lv) = f5.filter(|f| f.has("ListView1")).map(|f| f.c("ListView1")) {
        let key = lv.small_images.clone().or(lv.large_images.clone()).unwrap_or_else(|| "Form5.SmallPKMNImg".into());
        let n = ((strip.width() + 6.0) / 70.0).floor() as usize;
        for (k, it) in lv.list_items.iter().take(n).enumerate() {
            let slot = Rect::from_min_size(strip.min + Vec2::new(k as f32 * 70.0, 0.0), Vec2::new(64.0, 64.0));
            p.rect_filled(slot, 10.0, Color32::from_white_alpha(150));
            p.rect_stroke(slot, 10.0, Stroke::new(2.0, Color32::from_white_alpha(220)), egui::StrokeKind::Inside);
            if let Some(tex) = renderer.texture(ui.ctx(), assets, &key, it.image_index) {
                let hop = if animate { (((now * 3.0 + k as f64 * 0.7).sin()).max(0.0) * 3.0) as f32 } else { 0.0 };
                let sz = Vec2::splat(if tex.size()[0] <= 32 { 64.0 } else { 56.0 });
                p.image(tex.id(), Rect::from_center_size(slot.center() - Vec2::new(0.0, hop), sz), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
            }
        }
        if lv.list_items.len() > n {
            t::text(p, strip.right_center(), Align2::RIGHT_CENTER, &format!("+{}", lv.list_items.len() - n), t::bold(18.0), Color32::WHITE);
        }
    }

    // ---------------- mode banner (Evolve / Release are toggles in the original menu)
    let mut top = strip.bottom() + 12.0;
    for (item, text, color) in [("Evolve1", "EVOLUTION MODE — double-tap a POKéMON in the PC", rgb(0x90, 0x58, 0xD0)), ("ReleasePKMN1", "RELEASE MODE — double-tap a POKéMON in the PC", rgb(0x60, 0x90, 0xA8))] {
        if find_item(f4, &["Action1", item]).is_some_and(|(m, _)| m.checked) {
            let r = Rect::from_min_size(Pos2::new(area.left(), top), Vec2::new(area.width(), 52.0));
            p.rect_filled(r, 10.0, color);
            t::text(p, r.left_center() + Vec2::new(14.0, 0.0), Align2::LEFT_CENTER, text, t::bold(16.0), t::PAPER);
            let b = Rect::from_min_size(r.right_top() + Vec2::new(-200.0, 6.0), Vec2::new(90.0, 40.0));
            if t::button(ui, b, "OPEN PC", t::ButtonKind::Plain, free).clicked() {
                req = Some(Request::OpenSheet("Form5".into()));
            }
            let c = Rect::from_min_size(r.right_top() + Vec2::new(-104.0, 6.0), Vec2::new(96.0, 40.0));
            if t::button(ui, c, "CANCEL", t::ButtonKind::Danger, free).clicked() {
                let _ = tx.send(Input::Menu { form: "Form4".into(), item: item.into() });
            }
            top = r.bottom() + 10.0;
        }
    }

    // ---------------- tiles
    let cols = if wide { 3 } else { 2 };
    let gap = 10.0;
    let tw = (area.width() - gap * (cols as f32 - 1.0)) / cols as f32;
    let rows = TILES.len().div_ceil(cols);
    let th = ((area.bottom() - top - gap * (rows as f32 - 1.0)) / rows as f32).clamp(56.0, 92.0);
    for (k, tile) in TILES.iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(area.left() + (k % cols) as f32 * (tw + gap), top + (k / cols) as f32 * (th + gap)), Vec2::new(tw, th));
        let Some((item, enabled)) = find_item(f4, tile.path) else { continue };
        let enabled = enabled && free;
        let resp = t::button(ui, r, "", t::ButtonKind::Plain, enabled);
        let p = ui.painter();
        let press = if resp.is_pointer_button_down_on() { t::PX * 2.0 } else { 0.0 };
        let face = Rect::from_min_max(r.min + Vec2::new(t::PX, t::PX + press), Pos2::new(r.max.x - t::PX, r.max.y - t::PX * 3.0 + press));
        let badge = Rect::from_center_size(face.left_center() + Vec2::new(face.height() * 0.5 + 2.0, 0.0), Vec2::splat(face.height() * 0.72));
        p.rect_filled(badge, CornerRadius::same(10), if enabled { tile.color } else { t::GREY });
        glyph(p, badge.center(), badge.width() * 0.3, tile.glyph, Color32::WHITE);
        let ink = if enabled { t::INK } else { t::GREY };
        let size = if tw < 190.0 { 15.0 } else { 19.0 };
        t::text(p, Pos2::new(badge.right() + 10.0, face.center().y), Align2::LEFT_CENTER, tile.label, t::bold(size), ink);
        if item.checked {
            t::icon(p, face.right_top() + Vec2::new(-14.0, 14.0), 14.0, t::Icon::Check, tile.color);
        } else if !item.enabled {
            t::icon(p, face.right_top() + Vec2::new(-14.0, 14.0), 12.0, t::Icon::Lock, t::GREY);
        }
        if resp.clicked() {
            if item.children.is_empty() {
                let _ = tx.send(Input::Menu { form: "Form4".into(), item: item.name.clone() });
                if matches!(tile.glyph, Glyph::Evolve | Glyph::Release) && !item.checked {
                    req = Some(Request::OpenSheet("Form5".into()));
                }
            } else {
                req = Some(Request::OpenCards(tile.path.iter().map(|s| s.to_string()).collect()));
            }
        }
    }
    req
}

/// Opponent cards for a menu group: nested groups become headed sections, leaves become cards.
pub fn cards(ui: &mut egui::Ui, form: &Form, path: &[String], tx: &Sender<Input>, free: bool) -> bool {
    let Some((root, root_ok)) = find_item(form, path) else { return false };
    let width = ui.available_width();
    let mut fired = false;
    fn walk<'a>(m: &'a MenuItem, trail: Vec<String>, ok: bool, out: &mut Vec<(Vec<String>, Vec<(&'a MenuItem, bool)>)>) {
        let leaves: Vec<(&MenuItem, bool)> = m.children.iter().filter(|c| c.visible && c.children.is_empty() && strip_amp(&c.caption) != "-").map(|c| (c, ok && c.enabled)).collect();
        if !leaves.is_empty() {
            out.push((trail.clone(), leaves));
        }
        for c in m.children.iter().filter(|c| c.visible && !c.children.is_empty()) {
            let mut t2 = trail.clone();
            t2.push(strip_amp(&c.caption));
            walk(c, t2, ok && c.enabled, out);
        }
    }
    let mut sections = Vec::new();
    walk(root, Vec::new(), root_ok, &mut sections);
    // The group's own leaves (Elite Four, Ash, Me…) come after the tiered groups.
    if sections.first().is_some_and(|(trail, _)| trail.is_empty()) && sections.len() > 1 {
        let own = sections.remove(0);
        sections.push((vec!["Special".to_string()], own.1));
    }
    let cols = if width > 520.0 { 3 } else { 2 };
    let gap = 8.0;
    let cw = (width - gap * (cols as f32 - 1.0)) / cols as f32;
    egui::ScrollArea::vertical().id_salt(("cards", path.join("/"))).show(ui, |ui| {
        for (trail, leaves) in &sections {
            let title = if trail.is_empty() { strip_amp(&root.caption) } else { trail.join(" · ") };
            let (r, _) = ui.allocate_exact_size(Vec2::new(width, 30.0), Sense::hover());
            t::text(ui.painter(), r.left_bottom() + Vec2::new(2.0, -6.0), Align2::LEFT_BOTTOM, &title, t::bold(18.0), t::INK);
            for row in leaves.chunks(cols) {
                let (r, _) = ui.allocate_exact_size(Vec2::new(width, 62.0), Sense::hover());
                for (k, (m, ok)) in row.iter().enumerate() {
                    let br = Rect::from_min_size(r.min + Vec2::new(k as f32 * (cw + gap), 0.0), Vec2::new(cw, 56.0));
                    let enabled = *ok && free;
                    let resp = t::button(ui, br, "", t::ButtonKind::Plain, enabled);
                    let p = ui.painter();
                    let name = strip_amp(&m.caption);
                    let face_c = br.left_center() + Vec2::new(26.0, -2.0);
                    let hue = name.bytes().fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32));
                    let col = if enabled { Color32::from_rgb(120 + (hue % 100) as u8, 90 + ((hue / 100) % 100) as u8, 110 + ((hue / 10000) % 110) as u8) } else { t::GREY };
                    p.circle_filled(face_c, 15.0, t::FRAME);
                    p.circle_filled(face_c, 13.0, col);
                    let initial: String = name.chars().filter(|c| c.is_alphanumeric()).take(1).collect();
                    p.text(face_c, Align2::CENTER_CENTER, initial, t::bold(16.0), Color32::WHITE);
                    let ink = if enabled { t::INK } else { t::GREY };
                    let size = if name.chars().count() > 16 { 14.0 } else { 17.0 };
                    t::text(p, Pos2::new(face_c.x + 24.0, face_c.y), Align2::LEFT_CENTER, &name, t::bold(size), ink);
                    if !m.enabled {
                        t::icon(p, br.right_top() + Vec2::new(-14.0, 14.0), 12.0, t::Icon::Lock, t::GREY);
                    }
                    if resp.clicked() && m.on_click.is_some() {
                        let _ = tx.send(Input::Menu { form: form.var.clone(), item: m.name.clone() });
                        fired = true;
                    }
                }
            }
        }
    });
    fired
}

fn glyph(p: &egui::Painter, c: Pos2, s: f32, g: Glyph, col: Color32) {
    let st = Stroke::new((s * 0.22).max(2.0), col);
    match g {
        Glyph::Trainer => {
            p.circle_filled(c - Vec2::new(0.0, s * 0.55), s * 0.45, col);
            p.add(Shape::convex_polygon(vec![c + Vec2::new(-s * 0.85, s), c + Vec2::new(-s * 0.55, 0.0), c + Vec2::new(s * 0.55, 0.0), c + Vec2::new(s * 0.85, s)], col, Stroke::NONE));
        }
        Glyph::Wild => {
            for k in -1..=1 {
                let x = k as f32 * s * 0.55;
                p.line_segment([c + Vec2::new(x, s), c + Vec2::new(x - s * 0.25, -s * 0.6)], st);
                p.line_segment([c + Vec2::new(x, s), c + Vec2::new(x + s * 0.35, -s * 0.2)], st);
            }
        }
        Glyph::Trophy => {
            p.add(Shape::convex_polygon(vec![c + Vec2::new(-s * 0.7, -s), c + Vec2::new(s * 0.7, -s), c + Vec2::new(s * 0.4, s * 0.1), c + Vec2::new(-s * 0.4, s * 0.1)], col, Stroke::NONE));
            p.rect_filled(Rect::from_center_size(c + Vec2::new(0.0, s * 0.45), Vec2::new(s * 0.25, s * 0.6)), 0.0, col);
            p.rect_filled(Rect::from_center_size(c + Vec2::new(0.0, s * 0.9), Vec2::new(s * 1.0, s * 0.25)), 1.0, col);
        }
        Glyph::Mart => {
            // Shop front: striped awning over a counter with a door.
            let body = Rect::from_center_size(c + Vec2::new(0.0, s * 0.45), Vec2::new(s * 1.6, s * 1.0));
            p.rect_stroke(body, 1.0, st, egui::StrokeKind::Middle);
            p.rect_filled(Rect::from_center_size(body.center_bottom() - Vec2::new(0.0, s * 0.3), Vec2::new(s * 0.45, s * 0.6)), 0.0, col);
            for k in 0..4 {
                let x0 = c.x - s * 0.95 + k as f32 * s * 0.475;
                let awn = Rect::from_min_size(Pos2::new(x0, c.y - s * 0.95), Vec2::new(s * 0.475, s * 0.75));
                if k % 2 == 0 {
                    p.rect_filled(awn, 0.0, col);
                } else {
                    p.rect_stroke(awn, 0.0, Stroke::new(1.5, col), egui::StrokeKind::Inside);
                }
            }
        }
        Glyph::Dex => {
            let r = Rect::from_center_size(c, Vec2::new(s * 1.3, s * 1.8));
            p.rect_stroke(r, 3.0, st, egui::StrokeKind::Middle);
            p.circle_filled(r.center_top() + Vec2::new(0.0, s * 0.5), s * 0.28, col);
            p.line_segment([r.left_center() + Vec2::new(s * 0.2, s * 0.25), r.right_center() + Vec2::new(-s * 0.2, s * 0.25)], st);
        }
        Glyph::Evolve => {
            p.add(Shape::convex_polygon(vec![c + Vec2::new(0.0, -s), c + Vec2::new(s * 0.8, 0.0), c + Vec2::new(-s * 0.8, 0.0)], col, Stroke::NONE));
            p.rect_filled(Rect::from_center_size(c + Vec2::new(0.0, s * 0.5), Vec2::new(s * 0.6, s * 1.0)), 0.0, col);
        }
        Glyph::Release => {
            p.circle_stroke(c, s * 0.8, st);
            p.line_segment([c + Vec2::new(-s * 0.2, s * 0.2), c + Vec2::new(s * 1.0, -s * 1.0)], st);
            p.line_segment([c + Vec2::new(s * 0.3, -s * 1.0), c + Vec2::new(s * 1.0, -s * 1.0)], st);
            p.line_segment([c + Vec2::new(s * 1.0, -s * 0.3), c + Vec2::new(s * 1.0, -s * 1.0)], st);
        }
        Glyph::Types => {
            for k in 0..3 {
                let a = -PI / 2.0 + k as f32 * 2.0 * PI / 3.0;
                p.circle_filled(c + Vec2::new(a.cos(), a.sin()) * s * 0.6, s * 0.38, col);
            }
        }
        Glyph::Coin => {
            p.circle_stroke(c, s * 0.85, st);
            p.text(c, Align2::CENTER_CENTER, "¥", t::bold(s * 1.3), col);
        }
        Glyph::Pc => {
            let r = Rect::from_center_size(c - Vec2::new(0.0, s * 0.2), Vec2::new(s * 1.8, s * 1.2));
            p.rect_stroke(r, 2.0, st, egui::StrokeKind::Middle);
            p.line_segment([r.center_bottom(), r.center_bottom() + Vec2::new(0.0, s * 0.45)], st);
            p.line_segment([r.center_bottom() + Vec2::new(-s * 0.5, s * 0.5), r.center_bottom() + Vec2::new(s * 0.5, s * 0.5)], st);
        }
    }
}
