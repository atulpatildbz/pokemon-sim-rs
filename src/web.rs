//! Web build: entry point and browser glue. The page (web/index.html) hands over the player's
//! own PokeGame.exe / POKéGAME.dat; saves and settings live in the browser's storage
//! (`store`), with download / upload for backups. The program's Save/Open dialogs become a
//! save-slot picker.

use crate::store;
use crate::ui::slots;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

/// Start the game on the canvas `canvas_id` with the contents of the two original files.
#[wasm_bindgen]
pub async fn start(canvas_id: String, exe: Vec<u8>, dat: Vec<u8>) -> Result<(), JsValue> {
    console_panics();
    store::load();
    let canvas = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id(&canvas_id))
        .ok_or_else(|| JsValue::from_str("canvas not found"))?
        .dyn_into::<web_sys::HtmlCanvasElement>()?;
    eframe::WebRunner::new()
        .start(canvas, eframe::WebOptions::default(), Box::new(move |cc| Ok(Box::new(crate::app::App::new(cc, &exe, &dat, Vec::new())?))))
        .await
}

/// Panics go to the browser console (on any thread).
fn console_panics() {
    std::panic::set_hook(Box::new(|info| web_sys::console::error_1(&format!("pokegame: {info}").into())));
}

/// Hand `bytes` to the browser as a file download.
pub fn download(name: &str, bytes: &[u8]) {
    // Copy out of the (shared) wasm memory: a Blob cannot be made from a SharedArrayBuffer view.
    let arr = js_sys::Uint8Array::new_with_length(bytes.len() as u32);
    arr.copy_from(bytes);
    let Ok(blob) = web_sys::Blob::new_with_u8_array_sequence(&js_sys::Array::of1(&arr)) else { return };
    let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) else { return };
    if let Some(a) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.create_element("a").ok())
        .and_then(|e| e.dyn_into::<web_sys::HtmlAnchorElement>().ok())
    {
        a.set_href(&url);
        a.set_download(name);
        a.click();
    }
    let _ = web_sys::Url::revoke_object_url(&url);
}

thread_local! {
    /// Options › Saves: the slot whose Upload was pressed once (it holds a save) and when.
    static CONFIRM: RefCell<Option<(usize, f64)>> = const { RefCell::new(None) };
    /// A file the player picked: (tag passed to `pick_file`, file name, contents).
    static UPLOAD: RefCell<Option<(String, String, Vec<u8>)>> = const { RefCell::new(None) };
}

/// Ask the browser for a file; it arrives later through `take_upload(tag)`.
pub fn pick_file(tag: &str, accept: &str) {
    let Some(input) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.create_element("input").ok())
        .and_then(|e| e.dyn_into::<web_sys::HtmlInputElement>().ok())
    else {
        return;
    };
    input.set_type("file");
    input.set_accept(accept);
    let tag = tag.to_string();
    let target = input.clone();
    let on_change = Closure::once(move || {
        let Some(file) = target.files().and_then(|f| f.get(0)) else { return };
        wasm_bindgen_futures::spawn_local(async move {
            let Ok(buf) = wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await else { return };
            let bytes = js_sys::Uint8Array::new(&buf).to_vec();
            UPLOAD.with(|u| *u.borrow_mut() = Some((tag, file.name(), bytes)));
        });
    });
    input.set_onchange(Some(on_change.as_ref().unchecked_ref()));
    on_change.forget();
    input.click();
}

/// The file picked for `tag`, once it has been read.
pub fn take_upload(tag: &str) -> Option<(String, Vec<u8>)> {
    UPLOAD.with(|u| {
        let mut u = u.borrow_mut();
        if u.as_ref().is_some_and(|(t, _, _)| t == tag) {
            u.take().map(|(_, name, bytes)| (name, bytes))
        } else {
            None
        }
    })
}

/// Start over (the program quit: Exit, or closing the main window).
pub fn reload() {
    if let Some(w) = web_sys::window() {
        let _ = w.location().reload();
    }
}

fn slot_label(slot: usize, sum: &Option<slots::Summary>) -> String {
    match sum {
        Some(m) => format!("Slot {}: {}  ·  {} badges  ·  Dex {}", slot + 1, m.name, m.badges, m.dex),
        None => format!("Slot {}: empty", slot + 1),
    }
}

/// Stand-ins for the program's file dialogs.
#[derive(Default)]
pub struct Dialogs {}

impl Dialogs {
    /// Draw the stand-in for a TOpenDialog / TSaveDialog. Returns the answer once there is one:
    /// a save slot for saved games, a download for other saves (HTML export), and nothing for
    /// opening other files (music: the web build has no audio output).
    pub fn file_dialog(&mut self, ctx: &egui::Context, save: bool, filter: &str, initial: &str) -> Option<Option<String>> {
        if !filter.to_lowercase().contains("pgs") {
            if !save {
                return Some(None);
            }
            let name = std::path::Path::new(initial).file_name().map(|n| n.to_string_lossy().into_owned()).filter(|n| !n.is_empty());
            return Some(Some(format!("{}{}", store::DOWNLOADS, name.unwrap_or_else(|| "pokemon.htm".into()))));
        }
        let mut answer = None;
        egui::Modal::new(egui::Id::new("web_file_dialog")).show(ctx, |ui| {
            ui.set_width(360.0);
            ui.label(egui::RichText::new(if save { "Save to which slot?" } else { "Open which save?" }).strong().size(15.0));
            ui.add_space(8.0);
            for slot in 0..slots::COUNT {
                let sum = slots::summary(slot);
                let mut label = slot_label(slot, &sum);
                if save && sum.is_some() {
                    label.push_str("  (replaces it)");
                }
                let button = egui::Button::new(label).min_size(egui::vec2(360.0, 30.0));
                if ui.add_enabled(save || sum.is_some(), button).clicked() {
                    slots::ensure_dir();
                    answer = Some(slots::path(slot).map(|p| p.to_string_lossy().into_owned()));
                }
            }
            ui.add_space(6.0);
            ui.label(egui::RichText::new("Back up or restore saves under Options (⚙).").size(11.0).weak());
            ui.add_space(4.0);
            if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                answer = Some(None);
            }
        });
        answer
    }
}

/// Options rows: download each slot as a .pgs file, or replace it with an uploaded one.
pub fn save_backups(ui: &mut egui::Ui, width: f32, message: &mut Option<String>) {
    use crate::ui::theme as t;
    for slot in 0..slots::COUNT {
        let tag = format!("slot{slot}");
        if let Some((_, bytes)) = take_upload(&tag) {
            *message = Some(match slots::import_bytes(slot, &bytes) {
                Ok(()) => format!("Slot {} restored.", slot + 1),
                Err(e) => e,
            });
        }
        let sum = slots::summary(slot);
        let (r, _) = ui.allocate_exact_size(egui::vec2(width, 44.0), egui::Sense::hover());
        let bw = 96.0;
        let label = match &sum {
            Some(m) => format!("{}. {}", slot + 1, m.name),
            None => format!("{}. empty", slot + 1),
        };
        t::text(ui.painter(), r.left_center(), egui::Align2::LEFT_CENTER, &label, t::body(17.0), t::pal().ink);
        let up = egui::Rect::from_min_size(egui::pos2(r.right() - bw, r.top() + 2.0), egui::vec2(bw, 38.0));
        let down = up.translate(egui::vec2(-(bw + 6.0), 0.0));
        if t::button(ui, down, "Download", t::ButtonKind::Plain, sum.is_some()).clicked() {
            if let (Some(p), Some(m)) = (slots::path(slot), &sum) {
                if let Ok(b) = store::read(p) {
                    download(&format!("{}.pgs", m.name), &b);
                }
            }
        }
        // Replacing a save takes a second press within 3 s.
        let now = ui.input(|i| i.time);
        let confirming = CONFIRM.with(|c| c.borrow().is_some_and(|(s, at)| s == slot && now - at < 3.0));
        let (label, kind) = if confirming { ("Replace?", t::ButtonKind::Danger) } else { ("Upload", t::ButtonKind::Plain) };
        if t::button(ui, up, label, kind, true).clicked() {
            if sum.is_some() && !confirming {
                CONFIRM.with(|c| *c.borrow_mut() = Some((slot, now)));
            } else {
                CONFIRM.with(|c| *c.borrow_mut() = None);
                pick_file(&tag, ".pgs");
            }
        }
    }
    if let Some(m) = message {
        let (r, _) = ui.allocate_exact_size(egui::vec2(width, 22.0), egui::Sense::hover());
        t::text(ui.painter(), r.left_center(), egui::Align2::LEFT_CENTER, m, t::body(15.0), t::pal().grey);
    }
}
