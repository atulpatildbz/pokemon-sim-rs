//! Bespoke sheets for Statistics/PC (Form5), the Mart (Form9) and the POKéDEX (Form8), and the
//! branched-evolution choice on Form4. All read the forms' controls and send the inputs the
//! original controls would.

use super::battle::{item_icon, type_color};
use super::theme as t;
use crate::assets::Assets;
use crate::vcl::model::Form;
use crate::vcl::render::{strip_amp, Renderer};
use crate::vcl::runtime::{Event, Input, Snapshot};
use egui::{Align2, Color32, Pos2, Rect, Sense, Vec2};
use std::sync::mpsc::Sender;

fn form<'a>(snap: &'a Snapshot, var: &str) -> Option<&'a Form> {
    snap.forms.iter().find(|f| f.var == var)
}

fn cap(f: &Form, name: &str) -> String {
    if f.has(name) { strip_amp(&f.c(name).caption).trim().to_string() } else { String::new() }
}

fn send(tx: &Sender<Input>, form: &str, control: &str, ev: Event) {
    let _ = tx.send(Input::Event { form: form.into(), control: control.into(), event: ev });
}

fn select(tx: &Sender<Input>, form: &str, control: &str, index: i32) {
    let _ = tx.send(Input::SetItemIndex { form: form.into(), control: control.into(), index });
}

fn heading(ui: &mut egui::Ui, text: &str) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::hover());
    t::text(ui.painter(), r.left_bottom() + Vec2::new(2.0, -4.0), Align2::LEFT_BOTTOM, text, t::bold(18.0), t::INK);
}

fn type_pill(p: &egui::Painter, at: Pos2, ty: &str, size: f32) -> f32 {
    let g = p.layout_no_wrap(ty.to_uppercase(), t::bold(size), Color32::WHITE);
    let r = Rect::from_min_size(at, g.size() + Vec2::new(10.0, 4.0));
    p.rect_filled(r, 5.0, type_color(ty));
    p.galley(r.min + Vec2::new(5.0, 2.0), g, Color32::WHITE);
    r.width()
}

const ITEMS: [(&str, &str); 6] = [
    ("POKé BALL", "PokeBallsLbl"),
    ("GREAT BALL", "GreatBallsLbl"),
    ("ULTRA BALL", "UltraBallsLbl"),
    ("MASTER BALL", "MasterBallsLbl"),
    ("POTION", "PotionsLbl"),
    ("SUPER POTION", "SuperPotionsLbl"),
];

// ===================================================================================== PC

#[allow(clippy::too_many_arguments)]
pub fn pc(ui: &mut egui::Ui, snap: &Snapshot, renderer: &mut Renderer, assets: &Assets, tx: &Sender<Input>) {
    let (Some(f5), Some(f4)) = (form(snap, "Form5"), form(snap, "Form4")) else { return };
    let free = snap.dialog.is_none();
    let width = ui.available_width();

    // Stats.
    let stats = [
        ("MONEY", format!("¥{}", cap(f5, "MoneyLbl"))),
        ("FIGHTS", cap(f5, "FightsLbl")),
        ("WINS", cap(f5, "WinsLbl")),
        ("LOSSES", cap(f5, "LossesLbl")),
        ("WIN %", cap(f5, "Percent")),
        ("CATCHES", cap(f5, "CapturesLbl")),
        ("DEX", format!("{} / 386", cap(f5, "OwnedPKMNLbl"))),
    ];
    let cols = if width > 520.0 { 4 } else { 3 };
    let cw = width / cols as f32;
    let rows = stats.len().div_ceil(cols);
    let (r, _) = ui.allocate_exact_size(Vec2::new(width, rows as f32 * 46.0 + 8.0), Sense::hover());
    let p = ui.painter().clone();
    p.rect_filled(r, 10.0, Color32::from_rgb(0xF8, 0xE8, 0xB0));
    for (k, (label, v)) in stats.iter().enumerate() {
        let pos = r.min + Vec2::new(12.0 + (k % cols) as f32 * cw, 6.0 + (k / cols) as f32 * 46.0);
        t::text(&p, pos, Align2::LEFT_TOP, label, t::bold(13.0), Color32::from_rgb(0xA0, 0x70, 0x20));
        t::text(&p, pos + Vec2::new(0.0, 15.0), Align2::LEFT_TOP, v, t::bold(19.0), t::INK);
    }
    ui.add_space(6.0);

    // Bag.
    heading(ui, "Bag");
    let icols = if width > 520.0 { 6 } else { 3 };
    let iw = (width - 6.0 * (icols as f32 - 1.0)) / icols as f32;
    let irows = ITEMS.len().div_ceil(icols);
    let (r, _) = ui.allocate_exact_size(Vec2::new(width, irows as f32 * 50.0), Sense::hover());
    for (k, (name, lbl)) in ITEMS.iter().enumerate() {
        let cell = Rect::from_min_size(r.min + Vec2::new((k % icols) as f32 * (iw + 6.0), (k / icols) as f32 * 50.0), Vec2::new(iw, 44.0));
        p.rect_filled(cell, 8.0, Color32::WHITE);
        p.rect_stroke(cell, 8.0, egui::Stroke::new(1.5, t::FRAME_INNER), egui::StrokeKind::Inside);
        item_icon(&p, cell.left_center() + Vec2::new(16.0, 0.0), name);
        t::text(&p, cell.left_center() + Vec2::new(32.0, -8.0), Align2::LEFT_CENTER, name, t::bold(12.0), t::INK);
        t::text(&p, cell.left_center() + Vec2::new(32.0, 9.0), Align2::LEFT_CENTER, &format!("×{}", cap(f5, lbl)), t::bold(16.0), t::INK);
    }
    ui.add_space(6.0);

    // PC box.
    let lv = f5.c("ListView1");
    let checked = |item: &str| super::hub::find_item(f4, &["Action1", item]).is_some_and(|(m, _)| m.checked);
    let in_battle = f4.c("GroupBox1").visible;
    let hint = if checked("Evolve1") {
        "Tap the POKéMON to evolve"
    } else if checked("ReleasePKMN1") {
        "Tap the POKéMON to release"
    } else if in_battle && lv.enabled {
        "Tap a POKéMON to send it out"
    } else {
        "Tap a POKéMON for its details"
    };
    heading(ui, &format!("PC Box · {}", lv.list_items.len()));
    let (r, _) = ui.allocate_exact_size(Vec2::new(width, 22.0), Sense::hover());
    t::text(ui.painter(), r.left_center() + Vec2::new(2.0, 0.0), Align2::LEFT_CENTER, hint, t::body(16.0), if lv.enabled { t::BLUE } else { t::GREY });
    let tile = 84.0;
    let cols = ((width + 6.0) / (tile + 6.0)).floor().max(1.0) as usize;
    let tw = (width - 6.0 * (cols as f32 - 1.0)) / cols as f32;
    let rows = lv.list_items.len().div_ceil(cols);
    let key = lv.small_images.clone().unwrap_or_else(|| "Form4.SmallPKMNImg".into());
    let enabled = lv.enabled && free && lv.events.on_dbl_click.is_some();
    egui::ScrollArea::vertical().id_salt("pc_box").auto_shrink([false, false]).show_rows(ui, tile + 6.0, rows, |ui, range| {
        for row in range {
            let (r, _) = ui.allocate_exact_size(Vec2::new(width, tile + 6.0), Sense::hover());
            for c in 0..cols {
                let k = row * cols + c;
                let Some(it) = lv.list_items.get(k) else { break };
                let cell = Rect::from_min_size(r.min + Vec2::new(c as f32 * (tw + 6.0), 0.0), Vec2::new(tw, tile));
                let resp = ui.interact(cell, ui.id().with(("pc", k)), if enabled { Sense::click() } else { Sense::hover() });
                let p = ui.painter();
                let sel = lv.item_index == k as i32;
                let bg = if sel { Color32::from_rgb(0xD8, 0xE8, 0xF8) } else if resp.hovered() && enabled { Color32::from_rgb(0xF0, 0xF6, 0xFC) } else { Color32::WHITE };
                p.rect_filled(cell, 10.0, bg);
                p.rect_stroke(cell, 10.0, egui::Stroke::new(if sel { 2.5 } else { 1.5 }, if sel { t::BLUE } else { t::FRAME_INNER }), egui::StrokeKind::Inside);
                if let Some(tex) = renderer.texture(ui.ctx(), assets, &key, it.image_index) {
                    let s = if tex.size()[0] <= 32 { 64.0 } else { 56.0 };
                    let img = Rect::from_center_size(cell.center() - Vec2::new(0.0, 9.0), Vec2::splat(s));
                    p.image(tex.id(), img, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), if enabled || !in_battle { Color32::WHITE } else { Color32::from_white_alpha(140) });
                }
                let name: String = it.caption.chars().take(10).collect();
                p.text(cell.center_bottom() - Vec2::new(0.0, 10.0), Align2::CENTER_CENTER, name, t::bold(13.0), t::INK);
                if resp.clicked() {
                    // The original reacts to a double-click; one tap does the same here.
                    select(tx, "Form5", "ListView1", k as i32);
                    send(tx, "Form5", "ListView1", Event::DblClick);
                }
            }
        }
    });
}

// ===================================================================================== Mart

pub fn mart(ui: &mut egui::Ui, snap: &Snapshot, tx: &Sender<Input>) {
    let Some(f9) = form(snap, "Form9") else { return };
    let money = form(snap, "Form5").map(|f| cap(f, "MoneyLbl")).unwrap_or_default();
    let free = snap.dialog.is_none();
    let width = ui.available_width();
    let p = ui.painter().clone();

    let (r, _) = ui.allocate_exact_size(Vec2::new(width, 40.0), Sense::hover());
    p.rect_filled(r, 10.0, Color32::from_rgb(0xF8, 0xE8, 0xB0));
    t::text(&p, r.left_center() + Vec2::new(14.0, 0.0), Align2::LEFT_CENTER, "YOUR MONEY", t::bold(14.0), Color32::from_rgb(0xA0, 0x70, 0x20));
    t::text(&p, r.right_center() - Vec2::new(14.0, 0.0), Align2::RIGHT_CENTER, &format!("¥{money}"), t::bold(22.0), t::INK);
    ui.add_space(8.0);

    // Shelf.
    let rg = f9.c("RadioGroup1");
    let names = ["POKé BALL", "GREAT BALL", "ULTRA BALL", "POTION", "SUPER POTION"];
    for (k, item) in rg.items.iter().enumerate() {
        let (r, _) = ui.allocate_exact_size(Vec2::new(width, 54.0), Sense::hover());
        let row = r.shrink2(Vec2::new(0.0, 3.0));
        let sel = rg.item_index == k as i32;
        let resp = t::button(ui, row, "", if sel { t::ButtonKind::Primary } else { t::ButtonKind::Plain }, free);
        let p = ui.painter();
        item_icon(p, row.left_center() + Vec2::new(22.0, -2.0), names.get(k).copied().unwrap_or("POKé BALL"));
        t::text(p, row.left_center() + Vec2::new(44.0, -2.0), Align2::LEFT_CENTER, &strip_amp(item), t::bold(19.0), if sel { t::PAPER } else { t::INK });
        if resp.clicked() && !sel {
            select(tx, "Form9", "RadioGroup1", k as i32);
            send(tx, "Form9", "RadioGroup1", Event::Click);
        }
    }
    ui.add_space(6.0);

    // Description (the clerk's two lines).
    let desc = [cap(f9, "Label2"), cap(f9, "Label3")].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ");
    let (r, _) = ui.allocate_exact_size(Vec2::new(width, 64.0), Sense::hover());
    t::frame(&p, r);
    let text = if desc.is_empty() { "What would you like to buy?".to_string() } else { desc };
    let g = p.layout(text, t::body(18.0), t::INK, r.width() - 32.0);
    p.galley(r.min + Vec2::new(16.0, 12.0), g, t::INK);
    ui.add_space(8.0);

    // Quantity stepper (SpinEdit1) and total (Edit1).
    let qty: i32 = f9.c("SpinEdit1").text.trim().parse().unwrap_or(f9.c("SpinEdit1").value);
    let (r, _) = ui.allocate_exact_size(Vec2::new(width, 52.0), Sense::hover());
    let set_qty = |v: i32| {
        let _ = tx.send(Input::SetValue { form: "Form9".into(), control: "SpinEdit1".into(), value: v.max(0) });
        send(tx, "Form9", "SpinEdit1", Event::Change);
    };
    let b = |x: f32, w: f32| Rect::from_min_size(r.min + Vec2::new(x, 0.0), Vec2::new(w, 48.0));
    if t::button(ui, b(0.0, 52.0), "−", t::ButtonKind::Plain, free && qty > 0).clicked() {
        set_qty(qty - 1);
    }
    t::text(ui.painter(), r.min + Vec2::new(92.0, 22.0), Align2::CENTER_CENTER, &format!("×{qty}"), t::bold(24.0), t::INK);
    if t::button(ui, b(132.0, 52.0), "+", t::ButtonKind::Plain, free).clicked() {
        set_qty(qty + 1);
    }
    if t::button(ui, b(190.0, 64.0), "+10", t::ButtonKind::Plain, free).clicked() {
        set_qty(qty + 10);
    }
    let total = f9.c("Edit1").text.clone();
    t::text(ui.painter(), r.right_center() - Vec2::new(4.0, 2.0), Align2::RIGHT_CENTER, if total.is_empty() { "—" } else { &total }, t::bold(24.0), t::INK);
    ui.add_space(10.0);

    let (r, _) = ui.allocate_exact_size(Vec2::new(width, 56.0), Sense::hover());
    let half = (width - 8.0) / 2.0;
    if t::button(ui, Rect::from_min_size(r.min, Vec2::new(half, 52.0)), "LEAVE", t::ButtonKind::Plain, free).clicked() {
        send(tx, "Form9", "Button2", Event::Click);
    }
    let buy = f9.c("Button1");
    if t::button(ui, Rect::from_min_size(r.min + Vec2::new(half + 8.0, 0.0), Vec2::new(half, 52.0)), "BUY", t::ButtonKind::Primary, free && buy.enabled).clicked() {
        send(tx, "Form9", "Button1", Event::Click);
    }
}

// ===================================================================================== POKéDEX

#[derive(Default)]
pub struct Dex {
    search: String,
}

impl Dex {
    #[allow(clippy::too_many_arguments)]
    pub fn show(&mut self, ui: &mut egui::Ui, snap: &Snapshot, renderer: &mut Renderer, assets: &Assets, tx: &Sender<Input>) {
        let Some(f8) = form(snap, "Form8") else { return };
        let free = snap.dialog.is_none();
        let width = ui.available_width();
        let combo = f8.c("ComboBox1");

        // Entry card.
        if f8.c("Panel1").visible {
            let (r, _) = ui.allocate_exact_size(Vec2::new(width, 196.0), Sense::hover());
            let p = ui.painter().clone();
            p.rect_filled(r, 12.0, Color32::from_rgb(0xD0, 0x38, 0x38));
            let screen = Rect::from_min_size(r.min + Vec2::new(10.0, 10.0), Vec2::new(150.0, r.height() - 20.0));
            p.rect_filled(screen, 8.0, Color32::from_rgb(0xD8, 0xF0, 0xD0));
            if let Some(it) = f8.c("ListView1").list_items.first() {
                if let Some(tex) = renderer.texture(ui.ctx(), assets, "Form4.PKMNImages", it.image_index) {
                    p.image(tex.id(), Rect::from_center_size(screen.center(), Vec2::splat(128.0)), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                }
            }
            let info = Rect::from_min_max(Pos2::new(screen.right() + 10.0, r.top() + 10.0), r.max - Vec2::new(10.0, 10.0));
            p.rect_filled(info, 8.0, t::PAPER);
            let mut y = info.top() + 8.0;
            t::text(&p, Pos2::new(info.left() + 10.0, y), Align2::LEFT_TOP, &cap(f8, "Label1"), t::bold(19.0), t::INK);
            y += 26.0;
            let mut x = info.left() + 10.0;
            x += type_pill(&p, Pos2::new(x, y), &cap(f8, "Label2"), 13.0) + 6.0;
            if f8.c("Label3").visible {
                type_pill(&p, Pos2::new(x, y), cap(f8, "Label3").trim_start_matches('/'), 13.0);
            }
            y += 26.0;
            t::text(&p, Pos2::new(info.left() + 10.0, y), Align2::LEFT_TOP, &cap(f8, "Label4"), t::body(15.0), t::INK);
            y += 22.0;
            for (m, ty) in [("Label6", "Label13"), ("Label10", "Label14"), ("Label11", "Label15"), ("Label12", "Label16")] {
                let mv = cap(f8, m);
                t::text(&p, Pos2::new(info.left() + 10.0, y), Align2::LEFT_TOP, &mv, t::body(16.0), t::INK);
                let tyn = cap(f8, ty);
                if tyn != "???" && !tyn.is_empty() {
                    let g = p.layout_no_wrap(tyn.to_uppercase(), t::bold(11.0), Color32::WHITE);
                    let pr = Rect::from_min_size(Pos2::new(info.right() - g.size().x - 18.0, y + 1.0), g.size() + Vec2::new(8.0, 2.0));
                    p.rect_filled(pr, 4.0, type_color(&tyn));
                    p.galley(pr.min + Vec2::new(4.0, 1.0), g, Color32::WHITE);
                }
                y += 19.0;
            }
            ui.add_space(8.0);
        }

        // Search + list.
        let (r, _) = ui.allocate_exact_size(Vec2::new(width, 40.0), Sense::hover());
        let edit = egui::TextEdit::singleline(&mut self.search).hint_text("Search name or number").font(t::body(18.0)).margin(Vec2::new(10.0, 8.0));
        ui.put(r, edit);
        let q = self.search.trim().to_lowercase();
        let entries: Vec<(usize, &String)> = combo.items.iter().enumerate().filter(|(_, s)| q.is_empty() || s.to_lowercase().contains(&q)).collect();
        let row_h = 40.0;
        egui::ScrollArea::vertical().id_salt("dex_list").auto_shrink([false, false]).show_rows(ui, row_h, entries.len(), |ui, range| {
            for (k, s) in &entries[range] {
                let (r, resp) = ui.allocate_exact_size(Vec2::new(width, row_h), if free { Sense::click() } else { Sense::hover() });
                let p = ui.painter();
                let sel = combo.item_index == *k as i32;
                if sel || resp.hovered() {
                    p.rect_filled(r.shrink(1.0), 6.0, if sel { Color32::from_rgb(0xD8, 0xE8, 0xF8) } else { Color32::from_rgb(0xF0, 0xF6, 0xFC) });
                }
                if let Some(tex) = renderer.texture(ui.ctx(), assets, "Form4.SmallPKMNImg", *k as i32) {
                    p.image(tex.id(), Rect::from_min_size(r.min + Vec2::new(4.0, 4.0), Vec2::splat(32.0)), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                }
                // "Bulbasaur (#001)" -> "#001  Bulbasaur"
                let label = match s.rsplit_once(" (") {
                    Some((n, num)) => format!("{}  {n}", num.trim_end_matches(')')),
                    None => s.to_string(),
                };
                p.text(r.left_center() + Vec2::new(46.0, 0.0), Align2::LEFT_CENTER, label, t::body(18.0), t::INK);
                if resp.clicked() && !sel {
                    select(tx, "Form8", "ComboBox1", *k as i32);
                    send(tx, "Form8", "ComboBox1", Event::Change);
                }
            }
        });
    }
}

// ===================================================================================== evolution choice

/// Form4 is showing EvolveChoice (a branched evolution): one button per choice.
pub fn evolve_choice_active(snap: &Snapshot) -> bool {
    form(snap, "Form4").is_some_and(|f| f.visible && f.has("EvolveChoice") && f.showing(f.idx("EvolveChoice")))
}

pub fn evolve_choice(ui: &mut egui::Ui, area: Rect, snap: &Snapshot, renderer: &mut Renderer, assets: &Assets, tx: &Sender<Input>) {
    let Some(f4) = form(snap, "Form4") else { return };
    let free = snap.dialog.is_none();
    let rg = f4.c("EvolveChoice");
    let col = Rect::from_center_size(area.center(), Vec2::new(area.width().min(520.0), area.height())).shrink(10.0);
    let p = ui.painter().clone();
    let head = Rect::from_min_size(col.min + Vec2::new(0.0, 10.0), Vec2::new(col.width(), 70.0));
    t::frame(&p, head);
    t::text(&p, head.center(), Align2::CENTER_CENTER, "EVOLVE INTO…", t::title(15.0), t::INK);
    let mut y = head.bottom() + 12.0;
    for (k, item) in rg.items.iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(col.left(), y), Vec2::new(col.width(), 72.0));
        y += 80.0;
        let dont = k == 0;
        let resp = t::button(ui, r, "", if dont { t::ButtonKind::Plain } else { t::ButtonKind::Plain }, free);
        let p = ui.painter();
        if !dont {
            let species = assets.data.pkmn_name.iter().position(|n| n == item).unwrap_or(0) as i32;
            if species > 0 {
                if let Some(tex) = renderer.texture(ui.ctx(), assets, "Form4.PKMNImages", species - 1) {
                    p.image(tex.id(), Rect::from_min_size(r.left_center() + Vec2::new(8.0, -34.0), Vec2::splat(64.0)), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                }
            }
        }
        t::text(p, r.left_center() + Vec2::new(84.0, -2.0), Align2::LEFT_CENTER, item, t::bold(22.0), if dont { t::GREY } else { t::INK });
        if resp.clicked() {
            select(tx, "Form4", "EvolveChoice", k as i32);
            send(tx, "Form4", "EvolveButton", Event::Click);
        }
    }
}
