//! GBA-era look: palette, embedded pixel fonts, and pixel-art frames/buttons drawn in code.

use egui::{Align2, Color32, CornerRadius, FontFamily, FontId, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui, Vec2};

pub const INK: Color32 = Color32::from_rgb(0x40, 0x40, 0x48);
pub const INK_SHADOW: Color32 = Color32::from_rgb(0xD0, 0xD0, 0xC8);
pub const PAPER: Color32 = Color32::from_rgb(0xF8, 0xF8, 0xF8);
pub const FRAME: Color32 = Color32::from_rgb(0x38, 0x40, 0x50);
pub const FRAME_INNER: Color32 = Color32::from_rgb(0x70, 0xA8, 0xE0);
pub const SKY_TOP: Color32 = Color32::from_rgb(0x58, 0x98, 0xE8);
pub const SKY_BOTTOM: Color32 = Color32::from_rgb(0xB8, 0xE8, 0xF8);
pub const RED: Color32 = Color32::from_rgb(0xE0, 0x40, 0x38);
pub const BLUE: Color32 = Color32::from_rgb(0x38, 0x70, 0xD8);
pub const GREY: Color32 = Color32::from_rgb(0xA8, 0xA8, 0xB0);
pub const HP_GREEN: Color32 = Color32::from_rgb(0x58, 0xD0, 0x80);
pub const HP_YELLOW: Color32 = Color32::from_rgb(0xF8, 0xC0, 0x18);
pub const HP_RED: Color32 = Color32::from_rgb(0xF0, 0x58, 0x38);
pub const TRACK: Color32 = Color32::from_rgb(0x48, 0x50, 0x58);

/// One art pixel in points.
pub const PX: f32 = 2.0;

pub fn body(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("pixel".into()))
}
pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("pixel_bold".into()))
}
pub fn title(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("title".into()))
}

/// Register the embedded fonts next to whatever families are already installed.
pub fn install_fonts(fonts: &mut egui::FontDefinitions) {
    for (name, bytes) in [
        ("pixel", &include_bytes!("../../fonts/PixelOperator.ttf")[..]),
        ("pixel_bold", &include_bytes!("../../fonts/PixelOperator-Bold.ttf")[..]),
        ("title", &include_bytes!("../../fonts/PressStart2P-Regular.ttf")[..]),
    ] {
        fonts.font_data.insert(name.into(), egui::FontData::from_static(bytes).into());
        let mut list = vec![name.to_string()];
        list.extend(fonts.families[&FontFamily::Proportional].iter().filter(|f| *f != name).cloned());
        fonts.families.insert(FontFamily::Name(name.into()), list);
    }
}

/// Text with the GBA one-pixel drop shadow.
pub fn text(p: &egui::Painter, pos: Pos2, anchor: Align2, s: &str, font: FontId, color: Color32) -> Rect {
    let shadow = if color == PAPER { FRAME } else { INK_SHADOW };
    p.text(pos + Vec2::splat(PX * 0.5), anchor, s, font.clone(), shadow);
    p.text(pos, anchor, s, font, color)
}

/// The classic white text-box frame: dark outline, light-blue inner rule, white paper.
pub fn frame(p: &egui::Painter, r: Rect) {
    p.rect_filled(r, CornerRadius::same((PX * 3.0) as u8), FRAME);
    let inner = r.shrink(PX);
    p.rect_filled(inner, CornerRadius::same((PX * 2.0) as u8), FRAME_INNER);
    p.rect_filled(inner.shrink(PX * 1.5), CornerRadius::same((PX * 1.5) as u8), PAPER);
}

/// A soft sky gradient with a faint dither so the backdrop never looks flat.
pub fn backdrop(p: &egui::Painter, r: Rect, t: f64) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(r.left_top(), SKY_TOP);
    mesh.colored_vertex(r.right_top(), SKY_TOP);
    mesh.colored_vertex(r.left_bottom(), SKY_BOTTOM);
    mesh.colored_vertex(r.right_bottom(), SKY_BOTTOM);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 2, 3);
    p.add(Shape::mesh(mesh));
    // Drifting diagonal bands.
    let step = 48.0;
    let off = ((t * 12.0) % step as f64) as f32;
    let col = Color32::from_white_alpha(14);
    let mut x = r.left() - r.height() - step + off;
    while x < r.right() {
        let a = Pos2::new(x, r.bottom());
        let b = Pos2::new(x + r.height(), r.top());
        p.line_segment([a, b], Stroke::new(step * 0.35, col));
        x += step;
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    Plain,
    Primary,
    Danger,
}

/// A chunky pixel button with a press-down animation. Returns the response (clicked on release).
pub fn button(ui: &mut Ui, rect: Rect, label: &str, kind: ButtonKind, enabled: bool) -> Response {
    let resp = ui.interact(rect, ui.id().with(("pxbtn", rect.min.x as i32, rect.min.y as i32, label)), if enabled { Sense::click() } else { Sense::hover() });
    let pressed = enabled && resp.is_pointer_button_down_on();
    let depth = ui.ctx().animate_bool_with_time(resp.id.with("press"), pressed, 0.06);
    let hover = ui.ctx().animate_bool_with_time(resp.id.with("hover"), enabled && resp.hovered(), 0.1);
    let p = ui.painter();
    let lift = PX * 2.0;
    let face_rect = rect.translate(Vec2::new(0.0, lift * depth)).with_max_y(rect.max.y - lift + lift * depth);
    let (face, ink) = match (kind, enabled) {
        (_, false) => (Color32::from_rgb(0xD8, 0xD8, 0xD8), GREY),
        (ButtonKind::Plain, _) => (PAPER, INK),
        (ButtonKind::Primary, _) => (BLUE, PAPER),
        (ButtonKind::Danger, _) => (RED, PAPER),
    };
    let face = lerp_color(face, Color32::WHITE, hover * 0.18);
    let cr = CornerRadius::same((PX * 2.5) as u8);
    // Shadow/base, outline, face, top highlight.
    p.rect_filled(rect.with_min_y(rect.min.y + lift), cr, FRAME);
    p.rect_filled(face_rect, cr, FRAME);
    let inner = face_rect.shrink(PX);
    p.rect_filled(inner, CornerRadius::same((PX * 1.5) as u8), face);
    p.rect_filled(Rect::from_min_size(inner.min + Vec2::new(PX * 2.0, PX), Vec2::new(inner.width() - PX * 4.0, PX)), 0.0, Color32::from_white_alpha(90));
    // Fit the label: by height, then shrink for narrow buttons.
    let chars = label.chars().count().max(1) as f32;
    let size = (rect.height() * 0.42).clamp(9.0, 26.0).min((inner.width() - PX * 2.0) / (chars * 0.62)).max(7.0);
    text(p, inner.center(), Align2::CENTER_CENTER, label, bold(size), ink);
    resp
}

/// HP-style bar. `frac` in 0..=1; colour follows the GBA thresholds (½ and ⅕).
pub fn hp_bar(p: &egui::Painter, r: Rect, frac: f32) {
    let frac = frac.clamp(0.0, 1.0);
    p.rect_filled(r, CornerRadius::same(PX as u8 * 2), FRAME);
    let track = r.shrink(PX);
    p.rect_filled(track, CornerRadius::same(PX as u8), TRACK);
    let col = if frac > 0.5 { HP_GREEN } else if frac > 0.2 { HP_YELLOW } else { HP_RED };
    if frac > 0.0 {
        let fill = Rect::from_min_size(track.min, Vec2::new(track.width() * frac, track.height()));
        p.rect_filled(fill, CornerRadius::same(PX as u8), col);
        p.rect_filled(Rect::from_min_size(fill.min, Vec2::new(fill.width(), PX)), 0.0, Color32::from_white_alpha(70));
    }
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}

pub fn outline(p: &egui::Painter, r: Rect, w: f32, c: Color32) {
    p.rect_stroke(r, CornerRadius::same((PX * 2.0) as u8), Stroke::new(w, c), StrokeKind::Inside);
}

/// Visuals for egui's own widgets (combo boxes, scroll bars, text edits) in game mode.
pub fn game_visuals() -> egui::Visuals {
    let mut v = egui::Visuals::light();
    v.panel_fill = PAPER;
    v.window_fill = PAPER;
    v.extreme_bg_color = Color32::WHITE;
    v.window_corner_radius = CornerRadius::same(6);
    v.menu_corner_radius = CornerRadius::same(6);
    v.window_stroke = Stroke::new(PX, FRAME);
    v.selection.bg_fill = FRAME_INNER;
    v.selection.stroke = Stroke::new(1.0, FRAME);
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open, &mut v.widgets.noninteractive] {
        w.corner_radius = CornerRadius::same(4);
        w.fg_stroke.color = INK;
    }
    v.widgets.inactive.bg_fill = PAPER;
    v.widgets.inactive.weak_bg_fill = PAPER;
    v.widgets.inactive.bg_stroke = Stroke::new(PX * 0.75, FRAME);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0xE8, 0xF0, 0xF8);
    v.widgets.hovered.bg_stroke = Stroke::new(PX * 0.75, FRAME);
    v.widgets.active.weak_bg_fill = Color32::from_rgb(0xD0, 0xE0, 0xF0);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, FRAME_INNER);
    v
}

#[derive(Clone, Copy)]
pub enum Icon {
    Down,
    Right,
    Left,
    Close,
    Lock,
    Check,
}

/// Small vector icons (the pixel font has no symbol glyphs).
pub fn icon(p: &egui::Painter, c: Pos2, size: f32, which: Icon, color: Color32) {
    let h = size / 2.0;
    let stroke = Stroke::new((size / 6.0).max(1.5), color);
    match which {
        Icon::Down => {
            p.add(Shape::convex_polygon(vec![c + Vec2::new(-h, -h * 0.6), c + Vec2::new(h, -h * 0.6), c + Vec2::new(0.0, h * 0.7)], color, Stroke::NONE));
        }
        Icon::Right => {
            p.add(Shape::convex_polygon(vec![c + Vec2::new(-h * 0.6, -h), c + Vec2::new(h * 0.7, 0.0), c + Vec2::new(-h * 0.6, h)], color, Stroke::NONE));
        }
        Icon::Left => {
            p.add(Shape::convex_polygon(vec![c + Vec2::new(h * 0.6, -h), c + Vec2::new(h * 0.6, h), c + Vec2::new(-h * 0.7, 0.0)], color, Stroke::NONE));
        }
        Icon::Close => {
            p.line_segment([c + Vec2::new(-h, -h) * 0.7, c + Vec2::new(h, h) * 0.7], stroke);
            p.line_segment([c + Vec2::new(-h, h) * 0.7, c + Vec2::new(h, -h) * 0.7], stroke);
        }
        Icon::Lock => {
            let body = Rect::from_center_size(c + Vec2::new(0.0, h * 0.3), Vec2::new(size * 0.8, size * 0.6));
            p.rect_filled(body, 2.0, color);
            p.circle_stroke(c + Vec2::new(0.0, -h * 0.1), h * 0.45, stroke);
        }
        Icon::Check => {
            p.line_segment([c + Vec2::new(-h * 0.8, 0.0), c + Vec2::new(-h * 0.2, h * 0.6)], stroke);
            p.line_segment([c + Vec2::new(-h * 0.2, h * 0.6), c + Vec2::new(h * 0.8, -h * 0.6)], stroke);
        }
    }
}
