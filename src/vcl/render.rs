//! egui rendering of the VCL model. The root window plays the role of the Windows desktop;
//! each Delphi form is an egui window placed at its Left/Top (desktop coordinates).

use super::model::*;
use super::runtime::{Dialog, DialogAnswer, Event, Input, MsgDlgType, Snapshot, MB_CANCEL, MB_NO, MB_OK, MB_YES};
use crate::assets::{Assets, Rgba};
use egui::{Align2, Color32, FontFamily, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle, Vec2};
use std::collections::HashMap;
use std::sync::mpsc::Sender;

pub const MENU_HEIGHT: f32 = 20.0;


/// Windows XP-era system colours for D7's `$FF0000xx` encoding.
pub fn tcolor(c: TColor) -> Color32 {
    let rgb = if c & 0xFF00_0000 != 0 {
        match c & 0xFF {
            0x05 => 0xFFFFFF, // clWindow
            0x08 => 0x000000, // clWindowText
            0x0F => 0xC8D0D4, // clBtnFace (BGR of D4D0C8)
            0x12 => 0x000000, // clBtnText
            0x0D => 0x6A240A, // clHighlight (BGR)
            0x0E => 0xFFFFFF,
            0x11 => 0x808080,
            0x10 => 0x808080, // clBtnShadow
            0x14 => 0xFFFFFF, // clBtnHighlight
            0x04 => 0xFFFFFF,
            0x18 => 0xE1FFFF,
            _ => 0xC8D0D4,
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

/// Windows classic look.
pub fn classic_visuals(ctx: &egui::Context) {
    // Windows classic look: light theme, square corners, btnface buttons.
    let mut v = egui::Visuals::light();
    let face = tcolor(CL_BTN_FACE);
    v.window_fill = face;
    v.panel_fill = face;
    v.window_corner_radius = egui::CornerRadius::ZERO;
    v.menu_corner_radius = egui::CornerRadius::ZERO;
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open, &mut v.widgets.noninteractive] {
        w.corner_radius = egui::CornerRadius::ZERO;
        w.fg_stroke.color = Color32::BLACK;
    }
    v.widgets.inactive.bg_fill = face;
    v.widgets.inactive.weak_bg_fill = face;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::from_gray(64));
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0xE0, 0xDD, 0xD6);
    v.widgets.active.weak_bg_fill = Color32::from_rgb(0xB8, 0xB4, 0xAC);
    v.widgets.noninteractive.fg_stroke.color = Color32::BLACK;
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
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(Color32::from_rgb(0x3A, 0x6E, 0xA5)))
            .show(ctx, |_ui| {});
        let modal_form = match &snap.dialog {
            Some((_, Dialog::ModalForm { var })) => Some(var.clone()),
            _ => None,
        };
        let any_modal = snap.dialog.is_some();
        // Draw back to front in the logic thread's z-order; raise the top form when it changes.
        let mut order: Vec<&Form> = snap.forms.iter().filter(|f| f.visible).collect();
        order.sort_by_key(|f| snap.z_order.iter().position(|v| *v == f.var).unwrap_or(0));
        let raised = snap.raise_seq != self.raise_seq;
        self.raise_seq = snap.raise_seq;
        for form in &order {
            let interactive = !any_modal || modal_form.as_deref() == Some(form.var.as_str());
            self.show_form(ctx, form, assets, tx, interactive);
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

    fn show_form(&mut self, ctx: &egui::Context, form: &Form, assets: &Assets, tx: &Sender<Input>, interactive: bool) {
        let menu_h = if form.menu.is_empty() { 0.0 } else { MENU_HEIGHT };
        let size = Vec2::new(form.client_width as f32, form.client_height as f32 + menu_h);
        let mut open = true;
        let pos = (form.left, form.top);
        // Always place the window where the model says; a user drag shows up as a rect that
        // differs from the model in the same frame and is reported back as Moved.
        let moved_by_code = true;
        let mut win = egui::Window::new(strip_amp(&form.caption))
            .id(egui::Id::new(("form", &form.var)))
            .fixed_size(size)
            .collapsible(false)
            .resizable(false)
            .interactable(interactive)
            .frame(egui::Frame::window(&ctx.style()).fill(tcolor(form.color)).inner_margin(0.0));
        if moved_by_code {
            win = win.current_pos(Pos2::new(form.left as f32, form.top as f32));
        }
        if form.close_box {
            win = win.open(&mut open);
        }
        let resp = win.show(ctx, |ui| {
            ui.set_min_size(size);
            ui.set_max_size(size);
            self.form_body(ui, form, assets, tx, interactive, true);
        });
        if let Some(r) = resp {
            let p = (r.response.rect.left().round() as i32, r.response.rect.top().round() as i32);
            let seen = self.positions.insert(form.var.clone(), p);
            // Ignore the first frames of a window (egui's sizing pass) and report real drags only.
            if seen.is_some() && p != pos && ctx.input(|i| i.pointer.any_down()) {
                let _ = tx.send(Input::Moved { form: form.var.clone(), left: p.0, top: p.1 });
            }
        }
        if !open {
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
        egui::menu::bar(ui, |ui| {
            ui.set_height(MENU_HEIGHT);
            for item in form.menu.iter().filter(|m| m.visible) {
                menu_item(ui, form, item, tx, interactive, true);
            }
        });
    }

    fn control(&mut self, ui: &mut egui::Ui, form: &Form, i: usize, client: Pos2, assets: &Assets, tx: &Sender<Input>) {
        let c = &form.controls[i];
        let (x, y) = form.abs_pos(i);
        let rect = Rect::from_min_size(client + Vec2::new(x as f32, y as f32), Vec2::new(c.width as f32, c.height as f32));
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
                painter.text(pos, anchor, strip_amp(&c.caption), fid, fcol);
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
                let resp = ui.put(
                    rect,
                    egui::Button::new(egui::RichText::new(strip_amp(&c.caption)).font(fid)).min_size(rect.size()),
                );
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
                let text = c.items.join("\n");
                ui.scope_builder(egui::UiBuilder::new().max_rect(rect.shrink(3.0)), |ui| {
                    egui::ScrollArea::vertical().id_salt(("memo", &form.var, &c.name)).stick_to_bottom(true).show(ui, |ui| {
                        ui.set_width(rect.width() - 8.0);
                        ui.label(egui::RichText::new(text).font(fid).color(fcol));
                    });
                });
            }
            Kind::ListView => self.list_view(ui, form, c, rect, assets, tx),
            Kind::RadioGroup => {
                self.group(&painter, rect, &c.caption, &fid, fcol);
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
                        ui.put(r, egui::RadioButton::new(sel, egui::RichText::new(strip_amp(item)).font(fid.clone())))
                    };
                    if enabled && resp.clicked() && !sel {
                        sel = true;
                        let _ = tx.send(Input::SetItemIndex { form: form.var.clone(), control: c.name.clone(), index: k as i32 });
                        send(Event::Click, &c.events.on_click);
                    }
                    let _ = sel;
                }
            }
            Kind::GroupBox => self.group(&painter, rect, &c.caption, &fid, fcol),
            Kind::Panel => {
                painter.rect_filled(rect, 0.0, self.col(c.color));
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
                painter.rect_filled(rect, 0.0, tcolor(c.back_color));
                let span = (c.max_value - c.min_value).max(1) as f32;
                let frac = ((c.progress - c.min_value) as f32 / span).clamp(0.0, 1.0);
                let mut fill = rect;
                fill.set_width(rect.width() * frac);
                painter.rect_filled(fill, 0.0, tcolor(c.fore_color));
                sunken(&painter, rect);
                // TGauge ShowText: percentage, Delphi Trunc((Progress-Min)*100/(Max-Min)).
                let pct = ((c.progress - c.min_value) as i64 * 100 / (c.max_value - c.min_value).max(1) as i64) as i32;
                painter.text(rect.center(), Align2::CENTER_CENTER, format!("{pct}%"), fid, Color32::BLACK);
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
                    if selected {
                        ui.painter().rect_filled(Rect::from_min_size(r.min + Vec2::new(iw + 2.0, 0.0), Vec2::new(item_w - iw - 2.0, item_h)), 0.0, self.col(0xFF00_000D));
                    }
                    if let Some(key) = &icons {
                        if let Some(t) = self.texture(ui.ctx(), assets, key, it.image_index) {
                            ui.painter().image(t.id(), Rect::from_min_size(r.min, Vec2::new(iw, ih)), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                        }
                    }
                    let tcol = if selected { Color32::WHITE } else { self.col(c.font.color) };
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
                egui::Modal::new(egui::Id::new(("msgdlg", id))).show(ctx, |ui| {
                    ui.set_max_width(420.0);
                    ui.label(egui::RichText::new(kind.title()).strong());
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        let icon = match kind {
                            MsgDlgType::Warning => "⚠",
                            MsgDlgType::Error => "⛔",
                            MsgDlgType::Information => "ℹ",
                            MsgDlgType::Confirmation => "?",
                            MsgDlgType::Custom => "",
                        };
                        ui.label(egui::RichText::new(icon).size(24.0));
                        ui.label(egui::RichText::new(text).font(FontId::new(11.0, FontFamily::Name("mssans".into()))));
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
            ui.menu_button(caption, |ui| {
                for ch in visible {
                    menu_item(ui, form, ch, tx, interactive, false);
                }
            });
        });
    } else {
        let label = if item.checked { format!("✔ {caption}") } else if top { caption } else { format!("   {caption}") };
        if ui.add_enabled(item.enabled && interactive, egui::Button::new(label).frame(false)).clicked() {
            if item.on_click.is_some() {
                let _ = tx.send(Input::Menu { form: form.var.clone(), item: item.name.clone() });
            }
            ui.close_menu();
        }
    }
}

fn sunken(p: &egui::Painter, r: Rect) {
    p.line_segment([r.left_top(), r.right_top()], Stroke::new(1.0, Color32::from_gray(128)));
    p.line_segment([r.left_top(), r.left_bottom()], Stroke::new(1.0, Color32::from_gray(128)));
    p.line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(1.0, Color32::WHITE));
    p.line_segment([r.right_top(), r.right_bottom()], Stroke::new(1.0, Color32::WHITE));
}

fn raised(p: &egui::Painter, r: Rect) {
    p.line_segment([r.left_top(), r.right_top()], Stroke::new(1.0, Color32::WHITE));
    p.line_segment([r.left_top(), r.left_bottom()], Stroke::new(1.0, Color32::WHITE));
    p.line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(1.0, Color32::from_gray(128)));
    p.line_segment([r.right_top(), r.right_bottom()], Stroke::new(1.0, Color32::from_gray(128)));
}

fn group_frame(p: &egui::Painter, rect: Rect, caption: &str, fid: &FontId, col: Color32) {
    let frame = Rect::from_min_max(rect.min + Vec2::new(0.0, 6.0), rect.max);
    p.rect_stroke(frame, 2.0, Stroke::new(1.0, Color32::from_gray(150)), StrokeKind::Inside);
    if !caption.is_empty() {
        let galley = p.layout_no_wrap(strip_amp(caption), fid.clone(), col);
        let bg = Rect::from_min_size(rect.min + Vec2::new(6.0, 0.0), galley.size() + Vec2::new(4.0, 0.0));
        p.rect_filled(bg, 0.0, tcolor(CL_BTN_FACE));
        p.galley(rect.min + Vec2::new(8.0, 0.0), galley, col);
    }
}
