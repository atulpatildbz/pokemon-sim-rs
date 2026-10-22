//! egui rendering of the VCL model. The root window plays the role of the Windows desktop;
//! each Delphi form is an egui window placed at its Left/Top (desktop coordinates).

use super::model::*;
use super::runtime::{Dialog, DialogAnswer, Event, Input, MsgDlgType, Snapshot, MB_CANCEL, MB_NO, MB_OK, MB_YES};
use crate::assets::{Assets, Rgba};
use egui::{Align2, Color32, FontFamily, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle, Vec2};
use std::collections::HashMap;
use std::sync::mpsc::Sender;

pub const MENU_HEIGHT: f32 = 20.0;
/// Height of the classic skin's own title bar (replaces egui's, which was oversized).
const TITLE_HEIGHT: f32 = 24.0;


/// System colours for D7's `$FF0000xx` encoding, mapped to the classic skin's cool-grey palette
/// (the original's beige D4D0C8 / navy highlight are what made it look like Windows 7).
pub fn tcolor(c: TColor) -> Color32 {
    let rgb = if c & 0xFF00_0000 != 0 {
        match c & 0xFF {
            0x05 => 0xFFFFFF, // clWindow
            0x08 => 0x30251E, // clWindowText (BGR of 1E2530)
            0x0F => 0xF3EFEC, // clBtnFace (BGR of ECEFF3)
            0x12 => 0x30251E, // clBtnText
            0x0D => 0xDE6F2F, // clHighlight (BGR of 2F6FDE)
            0x0E => 0xFFFFFF,
            0x11 => 0x808080,
            0x10 => 0xA3948A, // clBtnShadow
            0x14 => 0xFFFFFF, // clBtnHighlight
            0x04 => 0xFFFFFF,
            0x18 => 0xE1FFFF,
            _ => 0xF3EFEC,
        }
    } else {
        c & 0xFFFFFF
    };
    // TColor is $00BBGGRR.
    Color32::from_rgb((rgb & 0xFF) as u8, ((rgb >> 8) & 0xFF) as u8, ((rgb >> 16) & 0xFF) as u8)
}

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    add_system_fonts(&mut fonts);
    ctx.set_fonts(fonts);
    classic_visuals(ctx);
}

/// MS Sans Serif / Arial from the system (the original's fonts), as families "mssans" / "arial".
pub fn add_system_fonts(fonts: &mut egui::FontDefinitions) {
    let candidates = [
        ("mssans", "/System/Library/Fonts/Supplemental/Microsoft Sans Serif.ttf"),
        ("mssans", "C:\\Windows\\Fonts\\micross.ttf"),
        ("arial", "/System/Library/Fonts/Supplemental/Arial.ttf"),
        ("arial", "C:\\Windows\\Fonts\\arial.ttf"),
        ("arial", "/usr/share/fonts/truetype/msttcorefonts/Arial.ttf"),
    ];
    for (fam, path) in candidates {
        if fonts.families.contains_key(&FontFamily::Name(fam.into())) {
            continue;
        }
        if let Ok(bytes) = std::fs::read(path) {
            fonts.font_data.insert(fam.into(), egui::FontData::from_owned(bytes).into());
            let mut list = vec![fam.to_string()];
            list.extend(fonts.families[&FontFamily::Proportional].iter().cloned());
            fonts.families.insert(FontFamily::Name(fam.into()), list);
        }
    }
    for fam in ["mssans", "arial"] {
        fonts
            .families
            .entry(FontFamily::Name(fam.into()))
            .or_insert_with(|| vec!["Ubuntu-Light".into(), "NotoEmoji-Regular".into(), "emoji-icon-font".into()]);
    }
    if let Some(list) = fonts.families.get(&FontFamily::Name("mssans".into())).cloned() {
        fonts.families.insert(FontFamily::Proportional, list);
    }
}

pub const ACCENT: Color32 = Color32::from_rgb(0x2F, 0x6F, 0xDE);
const EDGE: Color32 = Color32::from_rgb(0xA9, 0xB2, 0xBF);
const INK: Color32 = Color32::from_rgb(0x1E, 0x25, 0x30);

/// The classic skin: the original forms and layout, drawn flat and cool-grey with an accent
/// colour for hover/selection instead of Windows 7's bevels.
pub fn classic_visuals(ctx: &egui::Context) {
    let mut v = egui::Visuals::light();
    let face = tcolor(CL_BTN_FACE);
    v.window_fill = face;
    v.panel_fill = face;
    v.window_corner_radius = egui::CornerRadius::same(3);
    v.menu_corner_radius = egui::CornerRadius::same(3);
    v.window_stroke = Stroke::new(1.0, Color32::from_rgb(0x6B, 0x77, 0x8A));
    v.window_shadow = egui::Shadow { offset: [0, 6], blur: 20, spread: 0, color: Color32::from_black_alpha(90) };
    v.popup_shadow = egui::Shadow { offset: [0, 3], blur: 10, spread: 0, color: Color32::from_black_alpha(60) };
    v.extreme_bg_color = Color32::WHITE;
    v.selection.bg_fill = ACCENT;
    v.selection.stroke = Stroke::new(1.5, ACCENT);
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open, &mut v.widgets.noninteractive] {
        w.corner_radius = egui::CornerRadius::same(3);
        w.fg_stroke.color = INK;
    }
    v.widgets.inactive.bg_fill = Color32::from_rgb(0xF8, 0xF9, 0xFB);
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(0xF8, 0xF9, 0xFB);
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, EDGE);
    v.widgets.hovered.bg_fill = Color32::from_rgb(0xE3, 0xEC, 0xFB);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0xE3, 0xEC, 0xFB);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
    v.widgets.active.bg_fill = Color32::from_rgb(0xCB, 0xDB, 0xF6);
    v.widgets.active.weak_bg_fill = Color32::from_rgb(0xCB, 0xDB, 0xF6);
    v.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, EDGE);
    v.override_text_color = None;
    ctx.set_visuals(v);
}

/// How controls are painted: the original Windows look, or the game skin.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Skin {
    #[default]
    Classic,
    Game,
}

fn font_id(f: &FontSpec) -> FontId {
    let fam = if f.name.eq_ignore_ascii_case("Arial") { "arial" } else { "mssans" };
    FontId::new(f.height.unsigned_abs().max(8) as f32, FontFamily::Name(fam.into()))
}

/// Strip the Delphi accelerator marker ('&X' -> 'X', '&&' -> '&').
pub fn strip_amp(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '&' {
            if it.peek() == Some(&'&') {
                out.push('&');
                it.next();
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[derive(Default)]
pub struct Renderer {
    pub skin: Skin,
    textures: HashMap<(String, i32), TextureHandle>,
    /// Text being typed into TEdits, until the logic thread's snapshot catches up.
    edit_buffers: HashMap<(String, String), String>,
    /// Last Left/Top seen per form (to detect moves made by game code vs. by the user).
    positions: HashMap<String, (i32, i32)>,
    raise_seq: u64,
    /// Game skin: the combo box whose drop-down list is open, and where its box was drawn.
    open_combo: Option<(String, String, Rect)>,
    /// Classic skin: keyboard shortcuts are live (no modal dialog, no text field focused).
    hotkeys: bool,
    /// Classic skin, per memo: (line count last seen, first line of the latest update, when it last grew).
    memo_marks: HashMap<(String, String), (usize, usize, f64)>,
}

fn to_color_image(img: &Rgba) -> egui::ColorImage {
    egui::ColorImage::from_rgba_unmultiplied([img.width, img.height], &img.pixels)
}

impl Renderer {
    fn font(&self, f: &FontSpec) -> FontId {
        match self.skin {
            Skin::Classic => font_id(f),
            Skin::Game => {
                let px = f.height.unsigned_abs().max(8) as f32;
                if f.bold { crate::ui::theme::bold((px * 1.3).round()) } else { crate::ui::theme::body((px * 1.3).round()) }
            }
        }
    }

    fn col(&self, c: TColor) -> Color32 {
        match self.skin {
            Skin::Game if c == CL_BTN_FACE => crate::ui::theme::pal().face,
            Skin::Game if c == 0xFF00_000D => crate::ui::theme::pal().frame_inner,
            Skin::Game if c == 0 || c == 0xFF00_0008 || c == 0xFF00_0012 => crate::ui::theme::pal().ink,
            _ => tcolor(c),
        }
    }

    fn bevel(&self, p: &egui::Painter, r: Rect, sunk: bool) {
        match self.skin {
            Skin::Classic if sunk => sunken(p, r),
            Skin::Classic => raised(p, r),
            Skin::Game => {
                p.rect_stroke(r, 4.0, Stroke::new(1.5, crate::ui::theme::pal().frame), StrokeKind::Inside);
            }
        }
    }

    fn group(&self, p: &egui::Painter, rect: Rect, caption: &str, fid: &FontId, col: Color32) {
        match self.skin {
            Skin::Classic => group_frame(p, rect, caption, fid, col),
            Skin::Game => {
                use crate::ui::theme as t;
                let galley_h = fid.size;
                p.rect_filled(rect, 0.0, crate::ui::theme::pal().face);
                let frame = Rect::from_min_max(rect.min + Vec2::new(0.0, galley_h * 0.5), rect.max);
                p.rect_filled(frame, 6.0, t::pal().tile);
                p.rect_stroke(frame, 6.0, Stroke::new(2.0, t::pal().frame_inner), StrokeKind::Inside);
                let cap = strip_amp(caption);
                if !cap.trim().is_empty() {
                    let galley = p.layout_no_wrap(cap, fid.clone(), t::ON_ACCENT);
                    let bg = Rect::from_min_size(rect.min + Vec2::new(6.0, -1.0), galley.size() + Vec2::new(10.0, 2.0));
                    p.rect_filled(bg, 4.0, t::pal().frame);
                    p.galley(bg.min + Vec2::new(5.0, 1.0), galley, t::ON_ACCENT);
                }
            }
        }
    }

    pub fn texture(&mut self, ctx: &egui::Context, assets: &Assets, key: &str, index: i32) -> Option<TextureHandle> {
        let k = (key.to_string(), index);
        if let Some(t) = self.textures.get(&k) {
            return Some(t.clone());
        }
        let img = if index < 0 {
            assets.pictures.get(key)?
        } else {
            assets.image_lists.get(key)?.images.get(index as usize)?
        };
        let t = ctx.load_texture(format!("{key}#{index}"), to_color_image(img), egui::TextureOptions::NEAREST);
        self.textures.insert(k, t.clone());
        Some(t)
    }

    /// Render the whole desktop. `blocked` forms (behind a modal) ignore input.
    pub fn show(&mut self, ctx: &egui::Context, snap: &Snapshot, assets: &Assets, tx: &Sender<Input>) {
        egui::CentralPanel::default().frame(egui::Frame::new()).show(ctx, |ui| {
            let r = ui.max_rect();
            let (top, bottom) = (Color32::from_rgb(0x1B, 0x26, 0x38), Color32::from_rgb(0x34, 0x4A, 0x6B));
            let mut mesh = egui::Mesh::default();
            mesh.colored_vertex(r.left_top(), top);
            mesh.colored_vertex(r.right_top(), top);
            mesh.colored_vertex(r.right_bottom(), bottom);
            mesh.colored_vertex(r.left_bottom(), bottom);
            mesh.add_triangle(0, 1, 2);
            mesh.add_triangle(0, 2, 3);
            ui.painter().add(mesh);
        });
        let modal_form = match &snap.dialog {
            Some((_, Dialog::ModalForm { var })) => Some(var.clone()),
            _ => None,
        };
        let any_modal = snap.dialog.is_some();
        self.hotkeys = self.skin == Skin::Classic && !any_modal && !ctx.wants_keyboard_input();
        // Draw back to front in the logic thread's z-order; raise the top form when it changes.
        let mut order: Vec<&Form> = snap.forms.iter().filter(|f| f.visible).collect();
        order.sort_by_key(|f| snap.z_order.iter().position(|v| *v == f.var).unwrap_or(0));
        let raised = snap.raise_seq != self.raise_seq;
        self.raise_seq = snap.raise_seq;
        for form in &order {
            let interactive = !any_modal || modal_form.as_deref() == Some(form.var.as_str());
            let active = snap.z_order.last().is_some_and(|v| *v == form.var);
            self.show_form(ctx, form, assets, tx, interactive, active);
        }
        if raised {
            if let Some(top) = snap.z_order.last() {
                ctx.move_to_top(egui::LayerId::new(egui::Order::Middle, egui::Id::new(("form", top))));
            }
        }
        if let Some(m) = &modal_form {
            ctx.move_to_top(egui::LayerId::new(egui::Order::Middle, egui::Id::new(("form", m))));
        }
        if let Some((id, dlg)) = &snap.dialog {
            self.show_dialog(ctx, *id, dlg, tx);
        }
    }

    fn show_form(&mut self, ctx: &egui::Context, form: &Form, assets: &Assets, tx: &Sender<Input>, interactive: bool, active: bool) {
        let menu_h = if form.menu.is_empty() { 0.0 } else { MENU_HEIGHT };
        let client = Vec2::new(form.client_width as f32, form.client_height as f32 + menu_h);
        let size = client + Vec2::new(0.0, TITLE_HEIGHT);
        let mut closed = false;
        let pos = (form.left, form.top);
        // Always place the window where the model says; a user drag shows up as a rect that
        // differs from the model in the same frame and is reported back as Moved.
        let top_radius = egui::CornerRadius { nw: 3, ne: 3, sw: 0, se: 0 };
        let win = egui::Window::new(strip_amp(&form.caption))
            .id(egui::Id::new(("form", &form.var)))
            .fixed_size(size)
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .interactable(interactive)
            .frame(egui::Frame::window(&ctx.style()).fill(if form.var == "Form1" { NAVY } else { tcolor(form.color) }).inner_margin(0.0))
            .current_pos(Pos2::new(form.left as f32, form.top as f32));
        let resp = win.show(ctx, |ui| {
            ui.set_min_size(size);
            ui.set_max_size(size);
            let origin = ui.min_rect().min;
            let bar = Rect::from_min_size(origin, Vec2::new(size.x, TITLE_HEIGHT));
            closed = title_bar(ui, bar, &strip_amp(&form.caption), form.close_box && interactive, active, top_radius);
            let body = Rect::from_min_size(origin + Vec2::new(0.0, TITLE_HEIGHT), client);
            ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| {
                self.form_body(ui, form, assets, tx, interactive, true);
            });
        });
        if let Some(r) = resp {
            let p = (r.response.rect.left().round() as i32, r.response.rect.top().round() as i32);
            let seen = self.positions.insert(form.var.clone(), p);
            // Ignore the first frames of a window (egui's sizing pass) and report real drags only.
            if seen.is_some() && p != pos && ctx.input(|i| i.pointer.any_down()) {
                let _ = tx.send(Input::Moved { form: form.var.clone(), left: p.0, top: p.1 });
            }
        }
        if closed {
            let _ = tx.send(Input::CloseForm { form: form.var.clone() });
        }
    }

    /// The form's client area (and menu bar if `with_menu`), drawn at the ui's top-left.
    pub fn form_body(&mut self, ui: &mut egui::Ui, form: &Form, assets: &Assets, tx: &Sender<Input>, interactive: bool, with_menu: bool) {
        let origin = ui.min_rect().min;
        let menu_h = if with_menu && !form.menu.is_empty() { MENU_HEIGHT } else { 0.0 };
        if menu_h > 0.0 {
            self.menu_bar(ui, form, tx, interactive);
        }
        let client = origin + Vec2::new(0.0, menu_h);
        ui.add_enabled_ui(interactive, |ui| {
            for i in paint_order(form) {
                if form.controls[i].kind.is_visual() && form.showing(i) {
                    self.control(ui, form, i, client, assets, tx);
                }
            }
            self.combo_list(ui, form, tx);
        });
    }

    /// Game skin: the open combo box's list, drawn in the form's own layer (egui popups would
    /// not follow the scaled layer).
    fn combo_list(&mut self, ui: &mut egui::Ui, form: &Form, tx: &Sender<Input>) {
        use crate::ui::theme as t;
        let Some((fv, name, anchor)) = self.open_combo.clone() else { return };
        if fv != form.var {
            return;
        }
        if !form.has(&name) {
            self.open_combo = None;
            return;
        }
        let c = form.c(&name);
        let fid = self.font(&c.font);
        let row_h = (fid.size + 8.0).max(anchor.height());
        let rows = c.items.len().min(8) as f32;
        let list = Rect::from_min_size(anchor.left_bottom() + Vec2::new(0.0, 2.0), Vec2::new(anchor.width().max(120.0), row_h * rows + 4.0));
        let painter = ui.painter_at(ui.clip_rect());
        painter.rect_filled(list.translate(Vec2::splat(3.0)), 6.0, Color32::from_black_alpha(50));
        painter.rect_filled(list, 6.0, t::pal().paper);
        painter.rect_stroke(list, 6.0, Stroke::new(2.0, t::pal().frame), StrokeKind::Inside);
        let mut picked = None;
        ui.scope_builder(egui::UiBuilder::new().max_rect(list.shrink(2.0)).layer_id(ui.layer_id()), |ui| {
            egui::ScrollArea::vertical().id_salt(("gcl", &form.var, &name)).show(ui, |ui| {
                for (k, item) in c.items.iter().enumerate() {
                    let (r, resp) = ui.allocate_exact_size(Vec2::new(list.width() - 6.0, row_h), Sense::click());
                    let sel = c.item_index == k as i32;
                    if sel || resp.hovered() {
                        ui.painter().rect_filled(r, 4.0, if sel { t::pal().frame_inner } else { t::pal().hover });
                    }
                    ui.painter().text(r.left_center() + Vec2::new(6.0, 0.0), Align2::LEFT_CENTER, item, fid.clone(), t::pal().ink);
                    if resp.clicked() {
                        picked = Some(k as i32);
                    }
                }
            });
        });
        let clicked_outside = ui.input(|i| i.pointer.any_pressed())
            && ui.ctx().pointer_interact_pos().is_some_and(|p| {
                let local = ui.ctx().layer_transform_from_global(ui.layer_id()).map_or(p, |t| t * p);
                !list.contains(local) && !anchor.contains(local)
            });
        if let Some(k) = picked {
            self.open_combo = None;
            if k != c.item_index {
                let _ = tx.send(Input::SetItemIndex { form: form.var.clone(), control: c.name.clone(), index: k });
                if c.events.on_change.is_some() {
                    let _ = tx.send(Input::Event { form: form.var.clone(), control: c.name.clone(), event: Event::Change });
                }
            }
        } else if clicked_outside {
            self.open_combo = None;
        }
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui, form: &Form, tx: &Sender<Input>, interactive: bool) {
        let top = ui.min_rect().min;
        egui::menu::bar(ui, |ui| {
            ui.set_height(MENU_HEIGHT);
            for item in form.menu.iter().filter(|m| m.visible) {
                menu_item(ui, form, item, tx, interactive, true);
            }
        });
        if self.skin == Skin::Classic {
            let y = top.y + MENU_HEIGHT - 0.5;
            ui.painter().hline(top.x..=top.x + form.client_width as f32, y, Stroke::new(1.0, Color32::from_rgb(0xD3, 0xD9, 0xE1)));
        }
    }

    fn control(&mut self, ui: &mut egui::Ui, form: &Form, i: usize, client: Pos2, assets: &Assets, tx: &Sender<Input>) {
        let c = &form.controls[i];
        let (x, y) = form.abs_pos(i);
        let mut rect = Rect::from_min_size(client + Vec2::new(x as f32, y as f32), Vec2::new(c.width as f32, c.height as f32));
        // Classic skin: controls the original placed flush against the window edge get a gutter.
        if self.skin == Skin::Classic && x == 0 && matches!(c.kind, Kind::Label | Kind::Button) {
            rect.min.x += 5.0;
        }
        let enabled = c.enabled && ancestors_enabled(form, i);
        let send = |e: Event, h: &Option<String>| {
            if h.is_some() {
                let _ = tx.send(Input::Event { form: form.var.clone(), control: c.name.clone(), event: e });
            }
        };
        let painter = ui.painter_at(ui.clip_rect());
        let fid = self.font(&c.font);
        let fcol = if enabled { self.col(c.font.color) } else { Color32::GRAY };
        match c.kind {
            Kind::Label => {
                if !c.parent_color && !c.transparent {
                    painter.rect_filled(rect, 0.0, self.col(c.color));
                }
                let (anchor, pos) = match c.alignment {
                    1 => (Align2::RIGHT_TOP, rect.right_top()),
                    2 => (Align2::CENTER_TOP, rect.center_top()),
                    _ => (Align2::LEFT_TOP, rect.left_top()),
                };
                let caption = strip_amp(&c.caption);
                // Classic skin: "Name:"-style captions recede so the values next to them read first;
                // the title screen's labels go white on navy.
                let is_text_color = matches!(c.font.color, 0 | 0xFF00_0008 | 0xFF00_0012);
                let fcol = match self.skin {
                    Skin::Classic if form.var == "Form1" => Color32::WHITE,
                    Skin::Classic if enabled && is_text_color && caption.trim_end().ends_with(':') => MUTED,
                    _ => fcol,
                };
                painter.text(pos, anchor, caption, fid, fcol);
            }
            Kind::Button if self.skin == Skin::Game => {
                let kind = if c.name.contains("End") || c.caption.contains("Cancel") {
                    crate::ui::theme::ButtonKind::Danger
                } else if c.caption.contains("Go!") {
                    crate::ui::theme::ButtonKind::Primary
                } else {
                    crate::ui::theme::ButtonKind::Plain
                };
                if crate::ui::theme::button(ui, rect, &strip_amp(&c.caption), kind, enabled).clicked() {
                    send(Event::Click, &c.events.on_click);
                }
            }
            Kind::Button => {
                let resp = ui.interact(rect, ui.id().with(("btn", &c.name)), Sense::click());
                let hot = enabled && resp.hovered();
                let down = hot && resp.is_pointer_button_down_on();
                let primary = c.caption.contains("Go!");
                let danger = c.name.contains("End") || c.caption.contains("Cancel");
                let title_screen = form.var == "Form1";
                let (fill, edge, text) = match (primary, danger, enabled) {
                    (_, _, false) => (tcolor(CL_BTN_FACE), Color32::from_rgb(0xDD, 0xE2, 0xE8), Color32::from_rgb(0x8A, 0x95, 0xA5)),
                    _ if title_screen => (
                        if down { Color32::from_white_alpha(40) } else if hot { Color32::from_rgba_unmultiplied(0x2F, 0x6F, 0xDE, 200) } else { Color32::from_white_alpha(14) },
                        if hot { ACCENT } else { Color32::from_white_alpha(40) },
                        Color32::WHITE,
                    ),
                    (true, _, _) => (if down { Color32::from_rgb(0x1E, 0x55, 0xB8) } else if hot { Color32::from_rgb(0x4A, 0x86, 0xEE) } else { ACCENT }, Color32::from_rgb(0x1E, 0x55, 0xB8), Color32::WHITE),
                    (_, true, _) => (if down { Color32::from_rgb(0xF2, 0xC4, 0xC4) } else if hot { Color32::from_rgb(0xFB, 0xDD, 0xDD) } else { Color32::from_rgb(0xFD, 0xEE, 0xEE) }, Color32::from_rgb(0xD0, 0x78, 0x78), Color32::from_rgb(0x9A, 0x1F, 0x1F)),
                    _ => (if down { Color32::from_rgb(0xCB, 0xDB, 0xF6) } else if hot { Color32::from_rgb(0xE3, 0xEC, 0xFB) } else { Color32::from_rgb(0xF8, 0xF9, 0xFB) }, if hot { ACCENT } else { EDGE }, INK),
                };
                // The title screen's buttons are stacked flush; inset the drawing (not the hit area)
                // so they read as separate choices.
                let face = if title_screen { rect.shrink2(Vec2::new(0.0, 3.0)) } else { rect };
                painter.rect_filled(face, 4.0, fill);
                painter.rect_stroke(face, 4.0, Stroke::new(1.0, edge), StrokeKind::Inside);
                if resp.has_focus() {
                    painter.rect_stroke(face.shrink(2.0), 2.0, Stroke::new(1.0, ACCENT), StrokeKind::Inside);
                }
                // Captions padded with trailing spaces ("POKéBalls:      ") were the original's way
                // of pushing text left; honour that intent with real left alignment.
                let caption = strip_amp(&c.caption);
                let (anchor, at) = if caption.ends_with(' ') {
                    (Align2::LEFT_CENTER, face.left_center() + Vec2::new(6.0, 0.0))
                } else {
                    (Align2::CENTER_CENTER, face.center())
                };
                painter.text(at + Vec2::new(0.0, if down { 1.0 } else { 0.0 }), anchor, caption.trim_end(), fid, text);
                if hot {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if enabled && resp.clicked() {
                    send(Event::Click, &c.events.on_click);
                }
            }
            Kind::Edit | Kind::ComboBox | Kind::SpinEdit if c.kind == Kind::Edit => {
                let key = (form.var.clone(), c.name.clone());
                let buf = self.edit_buffers.entry(key.clone()).or_insert_with(|| c.text.clone());
                let mut edit = egui::TextEdit::singleline(buf).font(fid);
                if c.max_length > 0 {
                    edit = edit.char_limit(c.max_length as usize);
                }
                let resp = ui.put(rect, edit.interactive(!c.read_only));
                if resp.changed() {
                    let _ = tx.send(Input::SetText { form: form.var.clone(), control: c.name.clone(), text: buf.clone() });
                    send(Event::Change, &c.events.on_change);
                } else if !resp.has_focus() && *buf != c.text {
                    *buf = c.text.clone();
                }
            }
            Kind::Memo => {
                painter.rect_filled(rect, 0.0, self.col(c.color));
                self.bevel(&painter, rect, true);
                // Classic skin: the lines added by the latest update stay in full ink and older ones
                // fade, so what just happened in a battle is visible without re-reading the log.
                // Lines arriving within 1.5 s of each other count as one update.
                let n = c.items.len();
                let now = ui.input(|inp| inp.time);
                let mark = self.memo_marks.entry((form.var.clone(), c.name.clone())).or_insert((n, 0, f64::MIN));
                if mark.0 != n {
                    let from = if n < mark.0 { 0 } else if now - mark.2 < 1.5 { mark.1 } else { mark.0 };
                    *mark = (n, from, now);
                }
                let fresh_from = if self.skin == Skin::Classic { mark.1 } else { 0 };
                let mut job = egui::text::LayoutJob::default();
                for (k, line) in c.items.iter().enumerate() {
                    let fresh = k >= fresh_from;
                    let color = if fresh { fcol } else { MUTED };
                    let text = if k + 1 < n { format!("{line}\n") } else { line.clone() };
                    let line_height = (self.skin == Skin::Classic).then_some(fid.size + 4.0);
                    job.append(&text, 0.0, egui::TextFormat { font_id: fid.clone(), color, line_height, ..Default::default() });
                }
                job.wrap.max_width = rect.width() - 14.0;
                ui.scope_builder(egui::UiBuilder::new().max_rect(rect.shrink2(Vec2::new(5.0, 4.0))), |ui| {
                    egui::ScrollArea::vertical().id_salt(("memo", &form.var, &c.name)).stick_to_bottom(true).show(ui, |ui| {
                        ui.set_width(rect.width() - 14.0);
                        ui.label(job);
                    });
                });
            }
            Kind::ListView => self.list_view(ui, form, c, rect, assets, tx),
            Kind::RadioGroup => {
                self.group(&painter, rect, &c.caption, &fid, fcol);
                // Classic skin: a visible, enabled "Go!" next to this group makes it the battle's
                // attack list, with 1-9 / Enter / double-click shortcuts.
                let go = if self.skin == Skin::Classic { go_button(form, i) } else { None };
                let mut fire = false;
                let n = c.items.len().max(1);
                let cols = c.columns.max(1) as usize;
                let rows = n.div_ceil(cols);
                let inner = Rect::from_min_max(rect.min + Vec2::new(8.0, 16.0), rect.max - Vec2::new(4.0, 4.0));
                let col_w = inner.width() / cols as f32;
                let row_h = (inner.height() / rows as f32).max(14.0);
                for (k, item) in c.items.iter().enumerate() {
                    let (col, row) = (k / rows, k % rows);
                    let r = Rect::from_min_size(
                        inner.min + Vec2::new(col as f32 * col_w, row as f32 * row_h),
                        Vec2::new(col_w, row_h),
                    );
                    let mut sel = c.item_index == k as i32;
                    let resp = if self.skin == Skin::Game {
                        use crate::ui::theme as t;
                        let resp = ui.interact(r, ui.id().with(("radio", &c.name, k)), Sense::click());
                        if sel || resp.hovered() {
                            painter.rect_filled(r.shrink2(Vec2::new(0.0, 1.0)), 4.0, if sel { t::pal().highlight } else { t::pal().hover });
                        }
                        let dot = Pos2::new(r.left() + r.height() * 0.45, r.center().y);
                        painter.circle_filled(dot, r.height() * 0.3, t::pal().frame);
                        painter.circle_filled(dot, r.height() * 0.3 - 1.5, Color32::WHITE);
                        if sel {
                            painter.circle_filled(dot, r.height() * 0.16, t::pal().danger);
                        }
                        painter.text(Pos2::new(r.left() + r.height() * 0.95, r.center().y), Align2::LEFT_CENTER, strip_amp(item), fid.clone(), fcol);
                        resp
                    } else {
                        let resp = ui.interact(r, ui.id().with(("radio", &c.name, k)), Sense::click());
                        let row = r.shrink2(Vec2::new(0.0, 1.0));
                        if sel {
                            painter.rect_filled(row, 3.0, Color32::from_rgb(0xDC, 0xE7, 0xFB));
                            painter.rect_filled(Rect::from_min_size(row.min, Vec2::new(3.0, row.height())), 1.5, ACCENT);
                        } else if enabled && resp.hovered() {
                            painter.rect_filled(row, 3.0, Color32::from_rgb(0xEE, 0xF3, 0xFC));
                        }
                        if enabled && resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        let dot = Pos2::new(row.left() + 12.0, row.center().y);
                        painter.circle_stroke(dot, 5.0, Stroke::new(1.2, if sel { ACCENT } else { EDGE }));
                        if sel {
                            painter.circle_filled(dot, 2.6, ACCENT);
                        }
                        painter.text(Pos2::new(row.left() + 23.0, row.center().y), Align2::LEFT_CENTER, strip_amp(item), fid.clone(), fcol);
                        // The number key that picks this row (see the hotkeys below).
                        if go.is_some() && k < 9 {
                            painter.text(row.right_center() - Vec2::new(6.0, 0.0), Align2::RIGHT_CENTER, format!("{}", k + 1), fid.clone(), Color32::from_rgb(0xA3, 0xAC, 0xB9));
                        }
                        resp
                    };
                    let key = self.hotkeys && go.is_some() && k < 9 && ui.input(|inp| inp.key_pressed(NUM_KEYS[k]));
                    if enabled && (resp.clicked() || resp.double_clicked() || key) && !sel {
                        sel = true;
                        let _ = tx.send(Input::SetItemIndex { form: form.var.clone(), control: c.name.clone(), index: k as i32 });
                        send(Event::Click, &c.events.on_click);
                    }
                    // Double-click an attack = pick it and press Go!.
                    if enabled && resp.double_clicked() {
                        fire = true;
                    }
                    let _ = sel;
                }
                if self.hotkeys && ui.input(|inp| inp.key_pressed(egui::Key::Enter)) {
                    fire = true;
                }
                if let (true, Some(g)) = (enabled && fire, go) {
                    if form.controls[g].events.on_click.is_some() {
                        let _ = tx.send(Input::Event { form: form.var.clone(), control: form.controls[g].name.clone(), event: Event::Click });
                    }
                }
            }
            Kind::GroupBox => self.group(&painter, rect, &c.caption, &fid, fcol),
            Kind::Panel => {
                painter.rect_filled(rect, if self.skin == Skin::Classic { 3.0 } else { 0.0 }, self.col(c.color));
                self.bevel(&painter, rect, false);
                painter.text(rect.center(), Align2::CENTER_CENTER, strip_amp(&c.caption), fid, fcol);
            }
            Kind::Bevel => self.bevel(&painter, rect, true),
            Kind::Image => {
                let tex = match &c.picture {
                    Some(PictureRef::Design(k)) => self.texture(ui.ctx(), assets, k, -1),
                    Some(PictureRef::FromList { list, index }) => self.texture(ui.ctx(), assets, list, *index),
                    _ => None,
                };
                if let Some(t) = tex {
                    let size = if c.stretch { rect.size() } else { t.size_vec2() };
                    painter.image(t.id(), Rect::from_min_size(rect.min, size), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                }
                let resp = ui.interact(rect, ui.id().with(("img", &c.name)), Sense::click());
                if resp.clicked() {
                    send(Event::Click, &c.events.on_click);
                }
            }
            Kind::Gauge if self.skin == Skin::Game => {
                let span = (c.max_value - c.min_value).max(1) as f32;
                let frac = ((c.progress - c.min_value) as f32 / span).clamp(0.0, 1.0);
                crate::ui::theme::hp_bar(&painter, rect, frac);
            }
            Kind::Gauge => {
                painter.rect_filled(rect, 3.0, tcolor(c.back_color));
                let span = (c.max_value - c.min_value).max(1) as f32;
                let frac = ((c.progress - c.min_value) as f32 / span).clamp(0.0, 1.0);
                let mut fill = rect.shrink(1.0);
                fill.set_width((rect.width() - 2.0) * frac);
                // The battle's HP bars turn amber, then red, as HP drops.
                let fore = if form.var == "Form4" {
                    if frac > 0.5 { Color32::from_rgb(0x4C, 0xC2, 0x7E) } else if frac > 0.2 { Color32::from_rgb(0xF2, 0xB4, 0x41) } else { Color32::from_rgb(0xEE, 0x60, 0x55) }
                } else {
                    tcolor(c.fore_color)
                };
                painter.rect_filled(rect, 3.0, Color32::from_rgb(0xDD, 0xE2, 0xE9));
                painter.rect_filled(fill, 2.0, fore);
                // TGauge ShowText: percentage, Delphi Trunc((Progress-Min)*100/(Max-Min)).
                let pct = ((c.progress - c.min_value) as i64 * 100 / (c.max_value - c.min_value).max(1) as i64) as i32;
                painter.text(rect.center(), Align2::CENTER_CENTER, format!("{pct}%"), FontId::new(fid.size.min(rect.height() - 1.0), fid.family.clone()), INK);
            }
            Kind::ComboBox if self.skin == Skin::Game => {
                use crate::ui::theme as t;
                let text = if c.item_index >= 0 { c.items.get(c.item_index as usize).cloned().unwrap_or_default() } else { c.text.clone() };
                let resp = ui.interact(rect, ui.id().with(("gcombo", &c.name)), Sense::click());
                painter.rect_filled(rect, 4.0, t::pal().tile);
                painter.rect_stroke(rect, 4.0, Stroke::new(1.5, t::pal().frame), StrokeKind::Inside);
                painter.text(rect.left_center() + Vec2::new(4.0, 0.0), Align2::LEFT_CENTER, text, fid.clone(), fcol);
                t::icon(&painter, rect.right_center() - Vec2::new(rect.height() * 0.4, 0.0), rect.height() * 0.45, t::Icon::Down, t::pal().frame);
                if enabled && resp.clicked() {
                    let is_open = self.open_combo.as_ref().is_some_and(|(f, n, _)| *f == form.var && *n == c.name);
                    self.open_combo = if is_open { None } else { Some((form.var.clone(), c.name.clone(), rect)) };
                }
            }
            Kind::ComboBox => {
                let mut sel = c.item_index;
                let text = if sel >= 0 { c.items.get(sel as usize).cloned().unwrap_or_default() } else { c.text.clone() };
                ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                    egui::ComboBox::from_id_salt(("combo", &form.var, &c.name))
                        .width(rect.width() - 8.0)
                        .selected_text(egui::RichText::new(text).font(fid.clone()))
                        .height(300.0)
                        .show_ui(ui, |ui| {
                            for (k, item) in c.items.iter().enumerate() {
                                if ui.selectable_label(sel == k as i32, egui::RichText::new(item).font(fid.clone())).clicked() {
                                    sel = k as i32;
                                }
                            }
                        });
                });
                if sel != c.item_index {
                    let _ = tx.send(Input::SetItemIndex { form: form.var.clone(), control: c.name.clone(), index: sel });
                    send(Event::Change, &c.events.on_change);
                }
            }
            Kind::SpinEdit => {
                let mut v = c.value;
                let mut dv = egui::DragValue::new(&mut v).speed(c.increment.max(1) as f64 * 0.1);
                if c.max_value != 0 || c.min_value != 0 {
                    dv = dv.range(c.min_value..=c.max_value);
                }
                let r = ui.put(rect, dv);
                if r.changed() && v != c.value {
                    let _ = tx.send(Input::SetValue { form: form.var.clone(), control: c.name.clone(), value: v });
                    send(Event::Change, &c.events.on_change);
                }
            }
            _ => {}
        }
    }

    fn list_view(&mut self, ui: &mut egui::Ui, form: &Form, c: &Control, rect: Rect, assets: &Assets, tx: &Sender<Input>) {
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, self.col(c.color));
        self.bevel(&ui.painter_at(ui.clip_rect()), rect, true);
        let fid = self.font(&c.font);
        let icons = if c.view_style == "vsIcon" { c.large_images.clone() } else { c.small_images.clone() };
        let (iw, ih) = icons
            .as_ref()
            .and_then(|k| assets.image_lists.get(k))
            .map(|l| (l.width as f32, l.height as f32))
            .unwrap_or((0.0, 0.0));
        // vsList: items flow top-to-bottom, then into the next column (horizontal scrolling).
        let item_h = ih.max(14.0) + 2.0;
        let item_w = (c.list_items.iter().map(|it| it.caption.len()).max().unwrap_or(4) as f32 * 6.0 + iw + 12.0).max(60.0);
        let inner = rect.shrink(2.0);
        ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
            egui::ScrollArea::horizontal().id_salt(("lv", &form.var, &c.name)).show(ui, |ui| {
                let mut avail_h = inner.height();
                let mut per_col = (avail_h / item_h).floor().max(1.0) as usize;
                if c.list_items.len() > per_col {
                    avail_h -= 16.0; // horizontal scroll bar
                    per_col = (avail_h / item_h).floor().max(1.0) as usize;
                }
                let cols = c.list_items.len().div_ceil(per_col).max(1);
                let (area, _) = ui.allocate_exact_size(Vec2::new(cols as f32 * item_w, avail_h), Sense::hover());
                for (k, it) in c.list_items.iter().enumerate() {
                    let (col, row) = (k / per_col, k % per_col);
                    let r = Rect::from_min_size(area.min + Vec2::new(col as f32 * item_w, row as f32 * item_h), Vec2::new(item_w, item_h));
                    if !ui.is_rect_visible(r) {
                        continue;
                    }
                    let resp = ui.interact(r, ui.id().with(("lvi", &c.name, k)), Sense::click());
                    let selected = c.item_index == k as i32;
                    let row = Rect::from_min_size(r.min + Vec2::new(iw + 2.0, 0.0), Vec2::new(item_w - iw - 2.0, item_h));
                    if selected && self.skin == Skin::Classic {
                        ui.painter().rect_filled(row, 3.0, Color32::from_rgb(0xDC, 0xE7, 0xFB));
                        ui.painter().rect_filled(Rect::from_min_size(row.min, Vec2::new(3.0, row.height())), 1.5, ACCENT);
                    } else if selected {
                        ui.painter().rect_filled(row, 2.0, self.col(0xFF00_000D));
                    } else if self.skin == Skin::Classic && resp.hovered() {
                        ui.painter().rect_filled(row, 2.0, Color32::from_rgb(0xE3, 0xEC, 0xFB));
                    }
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if let Some(key) = &icons {
                        if let Some(t) = self.texture(ui.ctx(), assets, key, it.image_index) {
                            ui.painter().image(t.id(), Rect::from_min_size(r.min, Vec2::new(iw, ih)), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                        }
                    }
                    let tcol = if selected && self.skin == Skin::Classic { INK } else if selected { Color32::WHITE } else { self.col(c.font.color) };
                    ui.painter().text(r.left_center() + Vec2::new(iw + 4.0, 0.0), Align2::LEFT_CENTER, &it.caption, fid.clone(), tcol);
                    if resp.clicked() || resp.double_clicked() {
                        if !selected {
                            let _ = tx.send(Input::SetItemIndex { form: form.var.clone(), control: c.name.clone(), index: k as i32 });
                        }
                        if resp.double_clicked() {
                            if c.events.on_dbl_click.is_some() {
                                let _ = tx.send(Input::Event { form: form.var.clone(), control: c.name.clone(), event: Event::DblClick });
                            }
                        } else if c.events.on_click.is_some() {
                            let _ = tx.send(Input::Event { form: form.var.clone(), control: c.name.clone(), event: Event::Click });
                        }
                    }
                }
            });
        });
    }

    fn show_dialog(&mut self, ctx: &egui::Context, id: u64, dlg: &Dialog, tx: &Sender<Input>) {
        match dlg {
            Dialog::Message { text, kind, buttons } => {
                // Enter takes the affirmative button, Esc the negative one (or OK if that is all there is).
                let enter = if buttons & MB_YES != 0 { Some(6) } else if buttons & MB_OK != 0 { Some(1) } else { None };
                let esc = if buttons & MB_NO != 0 { Some(7) } else if buttons & MB_CANCEL != 0 { Some(2) } else { enter };
                let key = ctx.input(|i| {
                    if i.key_pressed(egui::Key::Enter) { enter } else if i.key_pressed(egui::Key::Escape) { esc } else { None }
                });
                if let Some(result) = key {
                    let _ = tx.send(Input::Answer { id, answer: DialogAnswer::Button(result) });
                }
                egui::Modal::new(egui::Id::new(("msgdlg", id))).frame(egui::Frame::window(&ctx.style()).inner_margin(0.0)).show(ctx, |ui| {
                    ui.set_width(420.0);
                    let (bar, _) = ui.allocate_exact_size(Vec2::new(420.0, TITLE_HEIGHT), Sense::hover());
                    let _ = title_bar(ui, bar, kind.title(), false, true, egui::CornerRadius { nw: 3, ne: 3, sw: 0, se: 0 });
                    egui::Frame::new().inner_margin(egui::Margin::same(14)).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let (glyph, col) = match kind {
                            MsgDlgType::Warning => ("!", Color32::from_rgb(0xE0, 0x9A, 0x16)),
                            MsgDlgType::Error => ("×", BALL_RED),
                            MsgDlgType::Information => ("i", ACCENT),
                            MsgDlgType::Confirmation => ("?", ACCENT),
                            MsgDlgType::Custom => ("", ACCENT),
                        };
                        if !glyph.is_empty() {
                            let (r, _) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::hover());
                            ui.painter().circle_filled(r.center(), 14.0, col);
                            ui.painter().text(r.center(), Align2::CENTER_CENTER, glyph, FontId::new(20.0, FontFamily::Name("arial".into())), Color32::WHITE);
                        }
                        ui.add(egui::Label::new(egui::RichText::new(text).font(FontId::new(12.0, FontFamily::Name("mssans".into())))).wrap());
                    });
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        for (bit, label, result) in
                            [(MB_YES, "Yes", 6), (MB_NO, "No", 7), (MB_OK, "OK", 1), (MB_CANCEL, "Cancel", 2)]
                        {
                            if buttons & bit != 0 && ui.add(egui::Button::new(label).min_size(Vec2::new(75.0, 23.0))).clicked() {
                                let _ = tx.send(Input::Answer { id, answer: DialogAnswer::Button(result) });
                            }
                        }
                    });
                    });
                });
            }
            Dialog::File { save, title, initial, .. } => {
                let key = ("file".to_string(), id.to_string());
                let buf = self.edit_buffers.entry(key.clone()).or_insert_with(|| initial.clone());
                let mut answer = None;
                egui::Modal::new(egui::Id::new(("filedlg", id))).show(ctx, |ui| {
                    ui.label(egui::RichText::new(if title.is_empty() { if *save { "Save As" } else { "Open" } } else { title }).strong());
                    ui.label("File name:");
                    ui.add(egui::TextEdit::singleline(buf).desired_width(360.0));
                    ui.horizontal(|ui| {
                        if ui.button(if *save { "Save" } else { "Open" }).clicked() && !buf.is_empty() {
                            answer = Some(Some(buf.clone()));
                        }
                        if ui.button("Cancel").clicked() {
                            answer = Some(None);
                        }
                    });
                });
                if let Some(a) = answer {
                    self.edit_buffers.remove(&key);
                    let _ = tx.send(Input::Answer { id, answer: DialogAnswer::File(a) });
                }
            }
            Dialog::ModalForm { .. } => {}
        }
    }
}

/// VCL paint order: within each parent, graphic controls (TLabel, TImage, TBevel, TGauge) are
/// painted on the parent's canvas first, then windowed controls on top, in creation order.
fn paint_order(form: &Form) -> Vec<usize> {
    let windowed = |k: Kind| !matches!(k, Kind::Label | Kind::Image | Kind::Bevel | Kind::Gauge);
    let key = |i: usize| {
        let mut path = Vec::new();
        let mut cur = Some(i);
        while let Some(c) = cur {
            path.push((windowed(form.controls[c].kind), c));
            cur = form.controls[c].parent;
        }
        path.reverse();
        path
    };
    let mut order: Vec<usize> = (0..form.controls.len()).collect();
    order.sort_by_cached_key(|&i| key(i));
    order
}

fn ancestors_enabled(form: &Form, i: usize) -> bool {
    let mut cur = form.controls[i].parent;
    while let Some(p) = cur {
        if !form.controls[p].enabled {
            return false;
        }
        cur = form.controls[p].parent;
    }
    true
}

fn menu_item(ui: &mut egui::Ui, form: &Form, item: &MenuItem, tx: &Sender<Input>, interactive: bool, top: bool) {
    let caption = strip_amp(&item.caption);
    if caption == "-" {
        ui.separator();
        return;
    }
    let visible: Vec<&MenuItem> = item.children.iter().filter(|m| m.visible).collect();
    if !visible.is_empty() {
        ui.add_enabled_ui(item.enabled && interactive, |ui| {
            ui.menu_button(egui::RichText::new(caption).font(menu_font()), |ui| {
                for ch in visible {
                    menu_item(ui, form, ch, tx, interactive, false);
                }
            });
        });
    } else {
        let label = if item.checked { format!("✔ {caption}") } else if top { caption } else { format!("   {caption}") };
        if ui.add_enabled(item.enabled && interactive, egui::Button::new(egui::RichText::new(label).font(menu_font())).frame(false)).clicked() {
            if item.on_click.is_some() {
                let _ = tx.send(Input::Menu { form: form.var.clone(), item: item.name.clone() });
            }
            ui.close_menu();
        }
    }
}

const MUTED: Color32 = Color32::from_rgb(0x5D, 0x6B, 0x80);
const NUM_KEYS: [egui::Key; 9] = [
    egui::Key::Num1, egui::Key::Num2, egui::Key::Num3, egui::Key::Num4, egui::Key::Num5,
    egui::Key::Num6, egui::Key::Num7, egui::Key::Num8, egui::Key::Num9,
];

fn menu_font() -> FontId {
    FontId::new(12.0, FontFamily::Name("mssans".into()))
}

/// The visible, enabled "Go!" button sharing control `i`'s parent, if any.
fn go_button(form: &Form, i: usize) -> Option<usize> {
    let parent = form.controls[i].parent;
    (0..form.controls.len()).find(|&j| {
        let b = &form.controls[j];
        b.kind == Kind::Button && b.parent == parent && strip_amp(&b.caption).trim() == "Go!" && b.enabled && form.showing(j) && ancestors_enabled(form, j)
    })
}

const NAVY: Color32 = Color32::from_rgb(0x16, 0x23, 0x3B);
const SLATE: Color32 = Color32::from_rgb(0x6B, 0x77, 0x8A);
const BALL_RED: Color32 = Color32::from_rgb(0xE2, 0x3B, 0x47);

/// A small Pokéball: red top, white bottom, dark band and button.
fn pokeball(p: &egui::Painter, c: Pos2, r: f32, red: Color32, dark: Color32) {
    p.circle_filled(c, r, Color32::from_rgb(0xF4, 0xF6, 0xFA));
    let steps = 14;
    let mut pts = vec![Pos2::new(c.x - r, c.y)];
    for i in 0..=steps {
        let a = std::f32::consts::PI * (1.0 + i as f32 / steps as f32);
        pts.push(Pos2::new(c.x + r * a.cos(), c.y + r * a.sin()));
    }
    p.add(egui::Shape::convex_polygon(pts, red, Stroke::NONE));
    p.line_segment([Pos2::new(c.x - r, c.y), Pos2::new(c.x + r, c.y)], Stroke::new(1.5, dark));
    p.circle_stroke(c, r, Stroke::new(1.2, dark));
    p.circle_filled(c, r * 0.38, Color32::from_rgb(0xF4, 0xF6, 0xFA));
    p.circle_stroke(c, r * 0.38, Stroke::new(1.2, dark));
}

/// Compact title bar: Pokéball, left-aligned title, close button. The focused window is navy
/// with a red edge; the others fall back to slate so the active window is obvious at a glance.
/// Returns true if the close button was clicked.
fn title_bar(ui: &mut egui::Ui, bar: Rect, title: &str, closable: bool, active: bool, radius: egui::CornerRadius) -> bool {
    let p = ui.painter();
    p.rect_filled(bar, radius, if active { NAVY } else { SLATE });
    if active {
        p.rect_filled(Rect::from_min_size(bar.left_bottom() - Vec2::new(0.0, 2.0), Vec2::new(bar.width(), 2.0)), 0.0, BALL_RED);
    }
    pokeball(p, bar.left_center() + Vec2::new(13.0, -1.0), 6.0, if active { BALL_RED } else { Color32::from_rgb(0xC4, 0xCB, 0xD6) }, if active { NAVY } else { SLATE });
    let font = FontId::new(13.0, FontFamily::Name("mssans".into()));
    p.text(bar.left_center() + Vec2::new(27.0, -1.0), Align2::LEFT_CENTER, title, font, Color32::WHITE);
    if !closable {
        return false;
    }
    let x = Rect::from_min_size(bar.right_top() + Vec2::new(-TITLE_HEIGHT, 0.0), Vec2::splat(TITLE_HEIGHT));
    let resp = ui.interact(x, ui.id().with(("close", title)), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(x.shrink(2.0), 3.0, BALL_RED);
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let c = x.center() - Vec2::new(0.0, 1.0);
    let d = 4.0;
    let st = Stroke::new(1.6, Color32::WHITE);
    p.line_segment([c + Vec2::new(-d, -d), c + Vec2::new(d, d)], st);
    p.line_segment([c + Vec2::new(-d, d), c + Vec2::new(d, -d)], st);
    resp.clicked()
}

/// Flat inset field: hairline edge, 3px corners.
fn sunken(p: &egui::Painter, r: Rect) {
    p.rect_stroke(r, 3.0, Stroke::new(1.0, EDGE), StrokeKind::Inside);
}

/// Flat raised panel: a slightly lighter hairline than the inset field.
fn raised(p: &egui::Painter, r: Rect) {
    p.rect_stroke(r, 3.0, Stroke::new(1.0, Color32::from_rgb(0xC9, 0xD0, 0xDA)), StrokeKind::Inside);
}

fn group_frame(p: &egui::Painter, rect: Rect, caption: &str, fid: &FontId, col: Color32) {
    let frame = Rect::from_min_max(rect.min + Vec2::new(0.0, 6.0), rect.max);
    p.rect_stroke(frame, 4.0, Stroke::new(1.0, EDGE), StrokeKind::Inside);
    if !caption.is_empty() {
        let galley = p.layout_no_wrap(strip_amp(caption), fid.clone(), col);
        let bg = Rect::from_min_size(rect.min + Vec2::new(6.0, 0.0), galley.size() + Vec2::new(4.0, 0.0));
        p.rect_filled(bg, 0.0, tcolor(CL_BTN_FACE));
        p.galley(rect.min + Vec2::new(8.0, 0.0), galley, col);
    }
}
