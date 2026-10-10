//! GBA-era look: palette, embedded pixel fonts, and pixel-art frames/buttons drawn in code.

use egui::{Align2, Color32, CornerRadius, FontFamily, FontId, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui, Vec2};

/// Text on coloured buttons, pills and banners (light in every scheme).
pub const ON_ACCENT: Color32 = Color32::from_rgb(0xF8, 0xF8, 0xF8);

/// A colour scheme. Everything the game UI draws takes its colours from the active one.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub key: &'static str,
    pub name: &'static str,
    /// Text and its one-pixel drop shadow.
    pub ink: Color32,
    pub ink_shadow: Color32,
    /// Text-box paper, sheet face, list tiles, selection and hover.
    pub paper: Color32,
    pub face: Color32,
    pub tile: Color32,
    pub highlight: Color32,
    pub hover: Color32,
    pub disabled: Color32,
    /// Outlines and the inner rule of text boxes.
    pub frame: Color32,
    pub frame_inner: Color32,
    /// Backdrop gradient and the strength of its drifting stripes.
    pub sky_top: Color32,
    pub sky_bottom: Color32,
    pub stripe: u8,
    /// Primary (blue) and danger (red) buttons; muted/disabled text; empty HP track.
    pub primary: Color32,
    pub danger: Color32,
    pub grey: Color32,
    pub track: Color32,
    /// Trainer card / money banners.
    pub card: Color32,
    pub card_ink: Color32,
    /// Battle field.
    pub field_top: Color32,
    pub field_mid: Color32,
    pub field_low: Color32,
    pub platform: Color32,
    pub platform_light: Color32,
    pub grass: Color32,
    /// Four-shade look: type, tile and HP colours use the scheme's own shades too.
    pub mono: bool,
}

const fn c(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

pub const PALETTES: [Palette; 7] = [
    Palette {
        key: "sky", name: "Sky",
        ink: c(0x404048), ink_shadow: c(0xD0D0C8),
        paper: c(0xF8F8F8), face: c(0xF2F2EA), tile: c(0xFFFFFF), highlight: c(0xD8E8F8), hover: c(0xEEF4FA), disabled: c(0xD8D8D8),
        frame: c(0x384050), frame_inner: c(0x70A8E0),
        sky_top: c(0x5898E8), sky_bottom: c(0xB8E8F8), stripe: 14,
        primary: c(0x3870D8), danger: c(0xE04038), grey: c(0xA8A8B0), track: c(0x485058),
        card: c(0xF8E8B0), card_ink: c(0xA07020),
        field_top: c(0xF8F4E0), field_mid: c(0xE0F0C8), field_low: c(0xB8E098), platform: c(0xA8C880), platform_light: c(0xC8E0A0), grass: c(0x88C070),
        mono: false,
    },
    Palette {
        key: "twilight", name: "Twilight",
        ink: c(0xEAE8F6), ink_shadow: c(0x14162A),
        paper: c(0x2B2E4C), face: c(0x22243C), tile: c(0x33375A), highlight: c(0x4A4290), hover: c(0x3A3E64), disabled: c(0x3A3C52),
        frame: c(0x0C0D1A), frame_inner: c(0x8C7CF8),
        sky_top: c(0x141633), sky_bottom: c(0x4A2E6E), stripe: 10,
        primary: c(0x6A5CE8), danger: c(0xE0506A), grey: c(0x8A8CA8), track: c(0x0E1020),
        card: c(0x3E3468), card_ink: c(0xF0C460),
        field_top: c(0x2A2C58), field_mid: c(0x3A4A72), field_low: c(0x2C5A54), platform: c(0x3C6A5E), platform_light: c(0x4E8070), grass: c(0x5E9A80),
        mono: false,
    },
    Palette {
        key: "pocket", name: "Pocket",
        ink: c(0x0F380F), ink_shadow: c(0x8BAC0F),
        paper: c(0x9BBC0F), face: c(0x8BAC0F), tile: c(0x9BBC0F), highlight: c(0x6E9A28), hover: c(0xA8C830), disabled: c(0x7A9A2A),
        frame: c(0x0F380F), frame_inner: c(0x306230),
        sky_top: c(0x306230), sky_bottom: c(0x8BAC0F), stripe: 12,
        primary: c(0x306230), danger: c(0x0F380F), grey: c(0x507A20), track: c(0x0F380F),
        card: c(0x9BBC0F), card_ink: c(0x306230),
        field_top: c(0x9BBC0F), field_mid: c(0x8BAC0F), field_low: c(0x7A9C10), platform: c(0x306230), platform_light: c(0x5A8A28), grass: c(0x306230),
        mono: true,
    },
    Palette {
        key: "lava", name: "Lava",
        ink: c(0x4A2018), ink_shadow: c(0xF0D0B8),
        paper: c(0xFFF6EC), face: c(0xFBEADA), tile: c(0xFFFBF5), highlight: c(0xFFD8B0), hover: c(0xFFEEDD), disabled: c(0xE6D4C4),
        frame: c(0x5A1E1E), frame_inner: c(0xF0A040),
        sky_top: c(0xD8483A), sky_bottom: c(0xF8C890), stripe: 18,
        primary: c(0xE07818), danger: c(0xB8282E), grey: c(0xB0988C), track: c(0x5A2A26),
        card: c(0xFFE0A0), card_ink: c(0xA04818),
        field_top: c(0xFCE8C8), field_mid: c(0xF0D0A0), field_low: c(0xD8B078), platform: c(0xC09060), platform_light: c(0xE0B888), grass: c(0xA87848),
        mono: false,
    },
    Palette {
        key: "forest", name: "Forest",
        ink: c(0x203828), ink_shadow: c(0xC8DCC8),
        paper: c(0xF4FBF2), face: c(0xE8F4E4), tile: c(0xFBFFFA), highlight: c(0xCFEAC8), hover: c(0xE6F5E2), disabled: c(0xCCD8CA),
        frame: c(0x1E3A2A), frame_inner: c(0x6CC08A),
        sky_top: c(0x2F8A52), sky_bottom: c(0xBCE8A8), stripe: 14,
        primary: c(0x2E8A5A), danger: c(0xD0503C), grey: c(0x96A898), track: c(0x2A3A30),
        card: c(0xE8F0B0), card_ink: c(0x6A7A20),
        field_top: c(0xEEF6DC), field_mid: c(0xCDE8B0), field_low: c(0x9CCC80), platform: c(0x88B868), platform_light: c(0xA8D088), grass: c(0x5E9A48),
        mono: false,
    },
    Palette {
        key: "sakura", name: "Sakura",
        ink: c(0x5A3050), ink_shadow: c(0xF4D4E2),
        paper: c(0xFFF8FB), face: c(0xFCEEF4), tile: c(0xFFFFFF), highlight: c(0xFAD0E2), hover: c(0xFDEAF2), disabled: c(0xEAD8E0),
        frame: c(0x5A3050), frame_inner: c(0xF07AB0),
        sky_top: c(0xE87AAE), sky_bottom: c(0xFFE0EC), stripe: 20,
        primary: c(0xC05AA8), danger: c(0xE0506A), grey: c(0xBCA4B4), track: c(0x5A3A50),
        card: c(0xFFE4F0), card_ink: c(0xB04880),
        field_top: c(0xFFF2F6), field_mid: c(0xF4E2EE), field_low: c(0xD8E8C0), platform: c(0xC8D8A8), platform_light: c(0xE0ECC8), grass: c(0xE890B8),
        mono: false,
    },
    Palette {
        key: "ocean", name: "Ocean",
        ink: c(0x123048), ink_shadow: c(0xC4DCEA),
        paper: c(0xF2FAFE), face: c(0xE4F2F8), tile: c(0xFAFDFF), highlight: c(0xC8E6F4), hover: c(0xE2F2FA), disabled: c(0xCAD8E0),
        frame: c(0x0E2E48), frame_inner: c(0x30B0C8),
        sky_top: c(0x0E5A8A), sky_bottom: c(0x7ED8E0), stripe: 14,
        primary: c(0x1C88B8), danger: c(0xE0604A), grey: c(0x8CA4B4), track: c(0x10283C),
        card: c(0xD8F0F4), card_ink: c(0x1A6A88),
        field_top: c(0xE8F8FC), field_mid: c(0xC8EAF0), field_low: c(0xF0E2B8), platform: c(0xD8C890), platform_light: c(0xEADCA8), grass: c(0x58B0B8),
        mono: false,
    },
];

static CURRENT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// The active colour scheme.
pub fn pal() -> &'static Palette {
    &PALETTES[CURRENT.load(std::sync::atomic::Ordering::Relaxed).min(PALETTES.len() - 1)]
}

/// Switch scheme by key (unknown keys fall back to the default, "sky").
pub fn set_scheme(key: &str) {
    let i = PALETTES.iter().position(|p| p.key == key).unwrap_or(0);
    CURRENT.store(i, std::sync::atomic::Ordering::Relaxed);
}

pub const HP_GREEN: Color32 = Color32::from_rgb(0x58, 0xD0, 0x80);
pub const HP_YELLOW: Color32 = Color32::from_rgb(0xF8, 0xC0, 0x18);
pub const HP_RED: Color32 = Color32::from_rgb(0xF0, 0x58, 0x38);

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
    let shadow = if luminance(color) > 0.6 { pal().frame } else { pal().ink_shadow };
    p.text(pos + Vec2::splat(PX * 0.5), anchor, s, font.clone(), shadow);
    p.text(pos, anchor, s, font, color)
}

/// The classic white text-box frame: dark outline, light-blue inner rule, white paper.
pub fn frame(p: &egui::Painter, r: Rect) {
    p.rect_filled(r, CornerRadius::same((PX * 3.0) as u8), pal().frame);
    let inner = r.shrink(PX);
    p.rect_filled(inner, CornerRadius::same((PX * 2.0) as u8), pal().frame_inner);
    p.rect_filled(inner.shrink(PX * 1.5), CornerRadius::same((PX * 1.5) as u8), pal().paper);
}

/// A soft sky gradient with a faint dither so the backdrop never looks flat.
pub fn backdrop(p: &egui::Painter, r: Rect, t: f64) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(r.left_top(), pal().sky_top);
    mesh.colored_vertex(r.right_top(), pal().sky_top);
    mesh.colored_vertex(r.left_bottom(), pal().sky_bottom);
    mesh.colored_vertex(r.right_bottom(), pal().sky_bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 2, 3);
    p.add(Shape::mesh(mesh));
    // Drifting diagonal bands.
    let step = 48.0;
    let off = ((t * 12.0) % step as f64) as f32;
    let col = Color32::from_white_alpha(pal().stripe);
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
        (_, false) => (pal().disabled, pal().grey),
        (ButtonKind::Plain, _) => (pal().paper, pal().ink),
        (ButtonKind::Primary, _) => (pal().primary, ON_ACCENT),
        (ButtonKind::Danger, _) => (pal().danger, ON_ACCENT),
    };
    let face = lerp_color(face, Color32::WHITE, hover * 0.18);
    let cr = CornerRadius::same((PX * 2.5) as u8);
    // Shadow/base, outline, face, top highlight.
    p.rect_filled(rect.with_min_y(rect.min.y + lift), cr, pal().frame);
    p.rect_filled(face_rect, cr, pal().frame);
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
    p.rect_filled(r, CornerRadius::same(PX as u8 * 2), pal().frame);
    let track = r.shrink(PX);
    p.rect_filled(track, CornerRadius::same(PX as u8), pal().track);
    let col = match (pal().mono, frac > 0.5, frac > 0.2) {
        (true, true, _) => pal().hover,
        (true, false, true) => pal().paper,
        (true, false, false) => pal().highlight,
        (false, true, _) => HP_GREEN,
        (false, false, true) => HP_YELLOW,
        (false, false, false) => HP_RED,
    };
    if frac > 0.0 {
        let fill = Rect::from_min_size(track.min, Vec2::new(track.width() * frac, track.height()));
        p.rect_filled(fill, CornerRadius::same(PX as u8), col);
        p.rect_filled(Rect::from_min_size(fill.min, Vec2::new(fill.width(), PX)), 0.0, Color32::from_white_alpha(70));
    }
}

/// Perceived brightness 0..1.
pub fn luminance(c: Color32) -> f32 {
    (0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32) / 255.0
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
    v.panel_fill = pal().paper;
    v.window_fill = pal().paper;
    v.extreme_bg_color = pal().tile;
    v.override_text_color = Some(pal().ink);
    v.window_corner_radius = CornerRadius::same(6);
    v.menu_corner_radius = CornerRadius::same(6);
    v.window_stroke = Stroke::new(PX, pal().frame);
    v.selection.bg_fill = pal().frame_inner;
    v.selection.stroke = Stroke::new(1.0, pal().frame);
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open, &mut v.widgets.noninteractive] {
        w.corner_radius = CornerRadius::same(4);
        w.fg_stroke.color = pal().ink;
    }
    v.widgets.inactive.bg_fill = pal().paper;
    v.widgets.inactive.weak_bg_fill = pal().paper;
    v.widgets.inactive.bg_stroke = Stroke::new(PX * 0.75, pal().frame);
    v.widgets.hovered.weak_bg_fill = pal().hover;
    v.widgets.hovered.bg_stroke = Stroke::new(PX * 0.75, pal().frame);
    v.widgets.active.weak_bg_fill = pal().highlight;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, pal().frame_inner);
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
