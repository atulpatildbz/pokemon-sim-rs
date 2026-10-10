//! The running program: emulated memory, RNG, forms, and the VCL operations ported handlers use.
//!
//! Every method that can run an event handler (Show -> OnShow, Close -> OnClose, a modal loop
//! that fires timers, ...) returns `R<_>` so a Delphi exception unwinds exactly as far as it did
//! in the original: to the message dispatch, where Application.HandleException shows it.

use super::mem::{Exc, Mem, R};
use crate::assets::Assets;
use crate::delphi::DelphiRandom;
use crate::vcl::model::{Control, Form, Kind, ListItem, TColor};
use crate::vcl::runtime::{Bridge, Dialog, DialogAnswer, Event, Input, MsgDlgType, Timers, MB_OK, MR_CANCEL};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Form indices (creation order of the original program main block, Form3 first = MainForm).
pub type FormId = usize;
pub const FORM3: FormId = 0;
pub const FORM1: FormId = 1;
pub const ABOUT_BOX: FormId = 2;
pub const FORM4: FormId = 3;
pub const FORM7: FormId = 4;
pub const FORM8: FormId = 5;
pub const FORM9: FormId = 6;
pub const FORM5: FormId = 7;
pub const OK_RIGHT_DLG: FormId = 8;
pub const FORM_A: FormId = 9;
pub const FORM_B: FormId = 10;

/// (resource name, global variable) in creation order.
pub const CREATION_ORDER: [(&str, &str); 11] = [
    ("TFORM3", "Form3"),
    ("TFORM1", "Form1"),
    ("TABOUTBOX", "AboutBox"),
    ("TFORM4", "Form4"),
    ("TFORM7", "Form7"),
    ("TFORM8", "Form8"),
    ("TFORM9", "Form9"),
    ("TFORM5", "Form5"),
    ("TOKRIGHTDLG", "OKRightDlg"),
    ("TFORMA", "FormA"),
    ("TFORMB", "FormB"),
];

pub const APP_TITLE: &str = "POKéMON Simulator";

/// Pixel advance widths for text measurement (AutoSize labels: code reads Label.Width).
pub struct Metrics {
    fonts: HashMap<String, Vec<u8>>,
    cache: HashMap<(String, i32), [i32; 256]>,
}

impl Metrics {
    pub fn load() -> Metrics {
        let mut fonts = HashMap::new();
        for (fam, paths) in [
            ("MS Sans Serif", &["/System/Library/Fonts/Supplemental/Microsoft Sans Serif.ttf", "C:\\Windows\\Fonts\\micross.ttf"][..]),
            ("Arial", &["/System/Library/Fonts/Supplemental/Arial.ttf", "C:\\Windows\\Fonts\\arial.ttf"][..]),
        ] {
            if let Some(b) = paths.iter().find_map(|p| std::fs::read(p).ok()) {
                fonts.insert(fam.to_string(), b);
            }
        }
        Metrics { fonts, cache: HashMap::new() }
    }

    /// Width in pixels of `s` (Latin-1) in the given font, like TCanvas.TextWidth.
    pub fn text_width(&mut self, family: &str, height: i32, s: &str) -> i32 {
        let px = height.abs().max(1);
        let key = (family.to_string(), px);
        if !self.cache.contains_key(&key) {
            let mut table = [6i32; 256];
            let data = self.fonts.get(family).or_else(|| self.fonts.get("MS Sans Serif"));
            if let Some(face) = data.and_then(|d| ttf_parser::Face::parse(d, 0).ok()) {
                let upem = face.units_per_em() as f32;
                for (c, slot) in table.iter_mut().enumerate() {
                    let ch = char::from_u32(c as u32).unwrap_or(' ');
                    if let Some(g) = face.glyph_index(ch) {
                        let adv = face.glyph_hor_advance(g).unwrap_or(0) as f32;
                        *slot = (adv * px as f32 / upem).round() as i32;
                    }
                }
            }
            self.cache.insert(key.clone(), table);
        }
        let t = &self.cache[&key];
        s.chars().map(|c| t[(c as u32 & 0xFF) as usize]).sum()
    }
}

pub use crate::vcl::runtime::Media;

pub struct World {
    pub m: Mem,
    pub rng: DelphiRandom,
    pub forms: Vec<Form>,
    pub assets: Arc<Assets>,
    pub metrics: Metrics,
    pub media: Media,
    bridge: Option<Bridge>,
    timers: Timers,
    dialogs: Vec<(u64, Dialog)>,
    raise_seq: u64,
    shown_once: Vec<bool>,
    pub z_order: Vec<FormId>,
    pub terminated: bool,
    last_publish: Instant,
    /// "Desktop" size reported as Screen.Width/Height.
    pub screen: (i32, i32),
    /// Command-line parameters (ParamStr(1..)).
    pub params: Vec<String>,
    /// Messages shown via MessageDlg when running headless (tests).
    pub headless_log: Vec<String>,
    /// Scripted answers for headless runs (tests): button results / file names.
    pub headless_answers: Vec<DialogAnswer>,
}

impl World {
    pub fn new(assets: Arc<Assets>, data_section: &[u8], bridge: Option<Bridge>) -> World {
        let forms = CREATION_ORDER
            .iter()
            .map(|(res, var)| crate::vcl::build::build_form(assets.form(res), var))
            .collect();
        World {
            m: Mem::new(data_section),
            rng: DelphiRandom::new(0),
            forms,
            assets,
            metrics: Metrics::load(),
            media: Media::default(),
            bridge,
            timers: Timers::default(),
            dialogs: Vec::new(),
            raise_seq: 0,
            shown_once: vec![false; CREATION_ORDER.len()],
            z_order: Vec::new(),
            terminated: false,
            last_publish: Instant::now() - Duration::from_secs(1),
            screen: (1024, 740),
            params: Vec::new(),
            headless_log: Vec::new(),
            headless_answers: Vec::new(),
        }
    }

    // ------------------------------------------------------------------ component access
    pub fn form(&mut self, f: FormId) -> &mut Form {
        &mut self.forms[f]
    }
    pub fn c(&mut self, f: FormId, name: &str) -> &mut Control {
        self.forms[f].cm(name)
    }
    pub fn cr(&self, f: FormId, name: &str) -> &Control {
        self.forms[f].c(name)
    }
    pub fn visible(&self, f: FormId) -> bool {
        self.forms[f].visible
    }
    pub fn ctl_visible(&self, f: FormId, name: &str) -> bool {
        self.forms[f].c(name).visible
    }

    /// TControl.Show / Visible := True on a control (no events in this program).
    pub fn show_c(&mut self, f: FormId, name: &str) {
        self.c(f, name).visible = true;
    }
    pub fn hide_c(&mut self, f: FormId, name: &str) {
        self.c(f, name).visible = false;
    }
    pub fn set_enabled(&mut self, f: FormId, name: &str, v: bool) {
        let c = self.c(f, name);
        c.enabled = v;
        if c.kind == Kind::Timer {
            let var = self.forms[f].var.clone();
            self.timers.restart(&var, name);
        }
    }

    /// TControl.SetText (Caption). AutoSize labels resize to the text width.
    pub fn set_caption(&mut self, f: FormId, name: &str, s: &str) {
        let (kind, auto, font) = {
            let c = self.cr(f, name);
            (c.kind, c.auto_size, c.font.clone())
        };
        let w = if kind == Kind::Label && auto { Some(self.metrics.text_width(&font.name, font.height, s)) } else { None };
        let c = self.c(f, name);
        c.caption = s.to_string();
        if let Some(w) = w {
            c.width = w;
        }
    }
    pub fn caption(&self, f: FormId, name: &str) -> String {
        self.cr(f, name).caption.clone()
    }
    pub fn set_form_caption(&mut self, f: FormId, s: &str) {
        self.forms[f].caption = s.to_string();
    }

    // ---- menus
    pub fn menu_enabled(&mut self, f: FormId, item: &str, v: bool) {
        self.forms[f].menu_item(item).enabled = v;
    }
    pub fn menu_checked(&mut self, f: FormId, item: &str, v: bool) {
        self.forms[f].menu_item(item).checked = v;
    }
    pub fn is_menu_checked(&self, f: FormId, item: &str) -> bool {
        self.forms[f].menu_ref(item).checked
    }
    pub fn is_menu_enabled(&self, f: FormId, item: &str) -> bool {
        self.forms[f].menu_ref(item).enabled
    }

    // ---- TMemo / TStrings
    pub fn memo_add(&mut self, f: FormId, name: &str, s: &str) {
        self.c(f, name).items.push(s.to_string());
    }
    pub fn memo_clear(&mut self, f: FormId, name: &str) {
        self.c(f, name).items.clear();
    }
    /// Form7.Memo1.Lines.Add — the battle log.
    pub fn log(&mut self, s: &str) {
        self.memo_add(FORM7, "Memo1", s);
    }

    // ---- TGauge (Gauges.pas: SetProgress clamps to [MinValue, MaxValue]; SetMaxValue lowers Progress)
    pub fn gauge_progress(&self, f: FormId, name: &str) -> i32 {
        self.cr(f, name).progress
    }
    pub fn set_gauge_progress(&mut self, f: FormId, name: &str, v: i32) {
        let c = self.c(f, name);
        c.progress = v.clamp(c.min_value, c.max_value.max(c.min_value));
    }
    pub fn set_gauge_max(&mut self, f: FormId, name: &str, v: i32) {
        let c = self.c(f, name);
        c.max_value = v;
        if c.progress > v {
            c.progress = v;
        }
    }
    pub fn set_gauge_fore(&mut self, f: FormId, name: &str, color: TColor) {
        self.c(f, name).fore_color = color;
    }

    // ---- TListView
    pub fn lv_count(&self, f: FormId, name: &str) -> i32 {
        self.cr(f, name).list_items.len() as i32
    }
    pub fn lv_add(&mut self, f: FormId, name: &str) {
        self.c(f, name).list_items.push(ListItem { caption: String::new(), image_index: -1, sub_items: vec![] });
    }
    pub fn lv_delete(&mut self, f: FormId, name: &str, i: i32) {
        let c = self.c(f, name);
        if (0..c.list_items.len() as i32).contains(&i) {
            c.list_items.remove(i as usize);
            if c.item_index == i {
                c.item_index = -1;
            } else if c.item_index > i {
                c.item_index -= 1;
            }
        }
    }
    pub fn lv_set_caption(&mut self, f: FormId, name: &str, i: i32, s: &str) {
        if let Some(it) = self.c(f, name).list_items.get_mut(i as usize) {
            it.caption = s.to_string();
        }
    }
    pub fn lv_set_image(&mut self, f: FormId, name: &str, i: i32, img: i32) {
        if let Some(it) = self.c(f, name).list_items.get_mut(i as usize) {
            it.image_index = img;
        }
    }
    pub fn lv_image(&self, f: FormId, name: &str, i: i32) -> i32 {
        self.cr(f, name).list_items.get(i as usize).map(|it| it.image_index).unwrap_or(-1)
    }
    pub fn lv_caption(&self, f: FormId, name: &str, i: i32) -> String {
        self.cr(f, name).list_items.get(i as usize).map(|it| it.caption.clone()).unwrap_or_default()
    }

    // ---- TRadioGroup / TComboBox items
    pub fn item_index(&self, f: FormId, name: &str) -> i32 {
        self.cr(f, name).item_index
    }
    /// SetItemIndex: clamped to [-1, Count-1] (TCustomRadioGroup / TCustomComboBox).
    pub fn set_item_index(&mut self, f: FormId, name: &str, v: i32) {
        let c = self.c(f, name);
        let n = c.items.len() as i32;
        c.item_index = v.clamp(-1, n - 1);
    }
    pub fn items_add(&mut self, f: FormId, name: &str, s: &str) {
        self.c(f, name).items.push(s.to_string());
    }
    pub fn items_clear(&mut self, f: FormId, name: &str) {
        let c = self.c(f, name);
        c.items.clear();
        c.item_index = -1;
    }
    pub fn item(&self, f: FormId, name: &str, i: i32) -> String {
        self.cr(f, name).items.get(i as usize).cloned().unwrap_or_default()
    }
    /// TStrings.Put
    pub fn items_put(&mut self, f: FormId, name: &str, i: i32, s: &str) {
        if let Some(x) = self.c(f, name).items.get_mut(i as usize) {
            *x = s.to_string();
        }
    }

    // ---- TSpinEdit (Spin.pas): Value = StrToInt(Text) or MinValue on error; SetValue clamps unless Min = Max
    pub fn spin_value(&self, f: FormId, name: &str) -> i32 {
        let c = self.cr(f, name);
        c.text.trim().parse::<i32>().unwrap_or(c.min_value)
    }
    pub fn set_spin_value(&mut self, f: FormId, name: &str, v: i32) -> R<()> {
        let (min, max) = {
            let c = self.cr(f, name);
            (c.min_value, c.max_value)
        };
        let v = if max != min { v.clamp(min, max) } else { v };
        self.set_edit_text(f, name, &v.to_string())
    }
    /// TControl.SetText on an edit: OnChange fires when the text actually changes.
    pub fn set_edit_text(&mut self, f: FormId, name: &str, s: &str) -> R<()> {
        let changed = self.cr(f, name).text != s;
        self.c(f, name).text = s.to_string();
        if changed {
            if let Some(h) = self.cr(f, name).events.on_change.clone() {
                return self.dispatch(f, &h);
            }
        }
        Ok(())
    }

    pub fn set_left(&mut self, f: FormId, v: i32) {
        self.forms[f].left = v;
    }
    pub fn set_top(&mut self, f: FormId, v: i32) {
        self.forms[f].top = v;
    }

    // ------------------------------------------------------------------ form lifecycle
    fn raise(&mut self, f: FormId) {
        self.z_order.retain(|&x| x != f);
        self.z_order.push(f);
        self.raise_seq += 1;
    }

    /// TCustomForm.Show: Visible := True (OnShow on the False->True transition) and BringToFront.
    pub fn show(&mut self, f: FormId) -> R<()> {
        self.raise(f);
        if !self.shown_once[f] {
            // TCustomForm.Position is applied when the window is first shown.
            self.shown_once[f] = true;
            let form = &mut self.forms[f];
            if matches!(form.position.as_str(), "poScreenCenter" | "poDesktopCenter" | "poMainFormCenter") {
                let (w, h) = (form.client_width + 8, form.client_height + 30);
                form.left = (self.screen.0 - w) / 2;
                form.top = (self.screen.1 - h) / 2;
            }
        }
        if !self.forms[f].visible {
            self.forms[f].visible = true;
            if let Some(h) = self.forms[f].events.on_show.clone() {
                self.dispatch(f, &h)?;
            }
        }
        Ok(())
    }
    /// TCustomForm.Hide.
    pub fn hide(&mut self, f: FormId) -> R<()> {
        if self.forms[f].visible {
            self.forms[f].visible = false;
            if let Some(h) = self.forms[f].events.on_hide.clone() {
                self.dispatch(f, &h)?;
            }
        }
        Ok(())
    }
    /// Visible := value (TCustomForm.SetVisible).
    pub fn set_visible(&mut self, f: FormId, v: bool) -> R<()> {
        if v { self.show(f) } else { self.hide(f) }
    }

    /// TCustomForm.Close: OnClose (Action = caHide), then MainForm -> Application.Terminate, else Hide.
    pub fn close(&mut self, f: FormId) -> R<()> {
        if self.forms[f].modal {
            self.forms[f].modal_result = MR_CANCEL;
            return Ok(());
        }
        if let Some(h) = self.forms[f].events.on_close.clone() {
            self.dispatch(f, &h)?;
        }
        if f == FORM3 {
            self.terminated = true;
            Ok(())
        } else {
            self.hide(f)
        }
    }

    /// TCustomForm.ShowModal.
    pub fn show_modal(&mut self, f: FormId) -> R<i32> {
        self.forms[f].modal = true;
        self.forms[f].modal_result = 0;
        self.show(f)?;
        let id = self.new_dialog(Dialog::ModalForm { var: self.forms[f].var.clone() });
        let res = loop {
            if self.terminated {
                break MR_CANCEL;
            }
            if self.forms[f].modal_result != 0 {
                break self.forms[f].modal_result;
            }
            if self.bridge.is_none() {
                // headless: press the button that has a ModalResult (TButton.Click semantics)
                let btn = self.forms[f].controls.iter().find(|c| c.modal_result != 0).cloned();
                match btn {
                    Some(b) => {
                        self.forms[f].modal_result = b.modal_result;
                        if let Some(h) = b.events.on_click.clone() {
                            self.dispatch(f, &h)?;
                        }
                    }
                    None => self.forms[f].modal_result = 1,
                }
                continue;
            }
            self.pump(Some(id))?;
        };
        self.end_dialog(id);
        self.forms[f].modal = false;
        self.hide(f)?;
        Ok(res)
    }

    // ------------------------------------------------------------------ dialogs
    fn new_dialog(&mut self, d: Dialog) -> u64 {
        let id = match &mut self.bridge {
            Some(b) => b.new_dialog_id(),
            None => self.dialogs.len() as u64 + 1,
        };
        self.dialogs.push((id, d));
        self.publish(true);
        id
    }
    fn end_dialog(&mut self, id: u64) {
        self.dialogs.retain(|(i, _)| *i != id);
        self.publish(true);
    }

    /// Wait for the answer to dialog `id`, running timers and the modal form's events meanwhile.
    fn wait_answer(&mut self, id: u64) -> R<DialogAnswer> {
        if self.bridge.is_none() {
            let a = if self.headless_answers.is_empty() { DialogAnswer::Button(1) } else { self.headless_answers.remove(0) };
            return Ok(a);
        }
        loop {
            if let Some(a) = self.pump(Some(id))? {
                return Ok(a);
            }
            if self.terminated {
                return Ok(DialogAnswer::Button(MR_CANCEL));
            }
        }
    }

    /// Dialogs.MessageDlg(Msg, DlgType, Buttons, 0).
    pub fn message_dlg(&mut self, text: &str, kind: MsgDlgType, buttons: u16) -> R<i32> {
        if self.bridge.is_none() {
            self.headless_log.push(text.to_string());
        }
        let id = self.new_dialog(Dialog::Message { text: text.to_string(), kind, buttons });
        let a = self.wait_answer(id);
        self.end_dialog(id);
        Ok(match a? {
            DialogAnswer::Button(b) => b,
            _ => MR_CANCEL,
        })
    }
    pub fn info(&mut self, text: &str) -> R<()> {
        self.message_dlg(text, MsgDlgType::Information, MB_OK).map(|_| ())
    }

    /// TOpenDialog/TSaveDialog.Execute on component `name` of form `f`; sets FileName.
    pub fn execute_file_dialog(&mut self, f: FormId, name: &str, save: bool) -> R<bool> {
        let (filter, ext, initial) = {
            let c = self.cr(f, name);
            (c.filter.clone(), c.default_ext.clone(), c.file_name.clone())
        };
        let id = self.new_dialog(Dialog::File { save, title: String::new(), filter, default_ext: ext, initial });
        let a = self.wait_answer(id);
        self.end_dialog(id);
        match a? {
            DialogAnswer::File(Some(path)) => {
                self.c(f, name).file_name = path;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// Application.HandleException -> ShowException: MessageBox(MB_OK or MB_ICONSTOP), caption = Title.
    pub fn show_exception(&mut self, e: &Exc) {
        let text = e.message();
        if self.bridge.is_none() {
            self.headless_log.push(format!("[exception] {text}"));
            return;
        }
        let id = self.new_dialog(Dialog::Message { text, kind: MsgDlgType::Error, buttons: MB_OK });
        // A nested exception while the box is open is shown on top of it, like the VCL does.
        loop {
            match self.pump(Some(id)) {
                Ok(Some(_)) => break,
                Ok(None) if self.terminated => break,
                Ok(None) => {}
                Err(e2) => self.show_exception(&e2),
            }
        }
        self.end_dialog(id);
    }

    // ------------------------------------------------------------------ painting
    /// TWinControl.Update (UpdateWindow): synchronous repaint. Throttled to ~60 Hz because the
    /// Game Corner calls it 10,000 times per draw.
    pub fn update(&mut self) {
        self.publish(false);
    }

    pub fn publish(&mut self, force: bool) {
        let now = Instant::now();
        if !force && now.duration_since(self.last_publish) < Duration::from_millis(16) {
            return;
        }
        self.last_publish = now;
        if let Some(b) = &self.bridge {
            let mut s = b.snapshot.lock().unwrap();
            s.forms = self.forms.clone();
            s.dialog = self.dialogs.last().cloned();
            s.z_order = self.z_order.iter().map(|&f| self.forms[f].var.clone()).collect();
            s.raise_seq = self.raise_seq;
            s.quit = self.terminated;
            s.generation += 1;
            s.media = self.media.clone();
            drop(s);
            (b.repaint)();
        }
    }

    // ------------------------------------------------------------------ message loop
    fn form_by_var(&self, var: &str) -> Option<FormId> {
        self.forms.iter().position(|f| f.var == var)
    }

    /// Process one input (or due timer). Returns the answer if `waiting` dialog got answered.
    /// Inputs for forms other than the modal one are dropped while a dialog is open (they are
    /// disabled windows in the VCL).
    pub fn pump(&mut self, waiting: Option<u64>) -> R<Option<DialogAnswer>> {
        self.publish(true);
        let now = Instant::now();
        self.timers.sync(&self.forms, now);
        if let Some((fv, tname)) = self.timers.take_due(&self.forms, now) {
            if let Some(f) = self.form_by_var(&fv) {
                if let Some(h) = self.forms[f].c(&tname).events.on_timer.clone() {
                    self.run_handler(f, &h);
                }
            }
            return Ok(None);
        }
        let deadline = self.timers.next_deadline();
        let input = match &mut self.bridge {
            Some(b) => b.recv(deadline),
            None => return Ok(None),
        };
        let Some(input) = input else { return Ok(None) };
        let modal_form = match self.dialogs.last() {
            Some((_, Dialog::ModalForm { var })) => Some(var.clone()),
            Some(_) => Some(String::new()),
            None => None,
        };
        let allowed = |form: &str| modal_form.as_deref().is_none_or(|m| m == form);
        match input {
            Input::Answer { id, answer } => {
                if Some(id) == waiting {
                    return Ok(Some(answer));
                }
            }
            Input::Quit => {
                self.terminated = true;
            }
            Input::Event { form, control, event } if allowed(&form) => {
                if let Some(f) = self.form_by_var(&form) {
                    if self.forms[f].has(&control) {
                        let c = self.forms[f].c(&control);
                        if c.enabled && self.forms[f].showing(self.forms[f].idx(&control)) {
                            let h = match event {
                                Event::Click => c.events.on_click.clone(),
                                Event::DblClick => c.events.on_dbl_click.clone(),
                                Event::Change => c.events.on_change.clone(),
                            };
                            // TButton.Click: Form.ModalResult := ModalResult, then OnClick.
                            if event == Event::Click && c.kind == Kind::Button && c.modal_result != 0 {
                                let mr = c.modal_result;
                                self.forms[f].modal_result = mr;
                            }
                            if let Some(h) = h {
                                self.run_handler(f, &h);
                            }
                        }
                    }
                }
            }
            Input::Menu { form, item } if allowed(&form) => {
                if let Some(f) = self.form_by_var(&form) {
                    let it = self.forms[f].menu_ref(&item);
                    if it.enabled {
                        if let Some(h) = it.on_click.clone() {
                            self.run_handler(f, &h);
                        }
                    }
                }
            }
            Input::SetItemIndex { form, control, index } if allowed(&form) => {
                if let Some(f) = self.form_by_var(&form) {
                    self.forms[f].cm(&control).item_index = index;
                }
            }
            Input::SetText { form, control, text } if allowed(&form) => {
                if let Some(f) = self.form_by_var(&form) {
                    self.forms[f].cm(&control).text = text;
                }
            }
            Input::SetValue { form, control, value } if allowed(&form) => {
                if let Some(f) = self.form_by_var(&form) {
                    self.forms[f].cm(&control).text = value.to_string();
                }
            }
            Input::CloseForm { form } if allowed(&form) => {
                if let Some(f) = self.form_by_var(&form) {
                    if self.forms[f].close_box {
                        let r = self.close(f);
                        if let Err(e) = r {
                            self.show_exception(&e);
                        }
                    }
                }
            }
            Input::Moved { form, left, top } => {
                if let Some(f) = self.form_by_var(&form) {
                    self.forms[f].left = left;
                    self.forms[f].top = top;
                }
            }
            Input::ScreenSize { width, height } => self.screen = (width, height),
            Input::MediaStatus { position, length } => {
                self.media.position = position;
                self.media.length = length;
            }
            _ => {}
        }
        Ok(None)
    }

    /// Dispatch a message: run the handler, and show an exception like Application.HandleException.
    pub fn run_handler(&mut self, f: FormId, handler: &str) {
        if let Err(e) = self.dispatch(f, handler) {
            self.show_exception(&e);
        }
    }

    /// Call a published method of form `f` by name.
    pub fn dispatch(&mut self, f: FormId, handler: &str) -> R<()> {
        super::units::dispatch(self, f, handler)
    }

    /// Application.Run: MainForm (Form3) is shown, then the message loop runs until Terminate.
    pub fn run(&mut self) {
        // Forms streamed with Visible = True are shown at creation.
        for f in 0..self.forms.len() {
            if self.forms[f].visible {
                self.forms[f].visible = false;
                let r = self.show(f);
                if let Err(e) = r {
                    self.show_exception(&e);
                }
            }
        }
        let r = self.show(FORM3);
        if let Err(e) = r {
            self.show_exception(&e);
        }
        while !self.terminated {
            if let Err(e) = self.pump(None) {
                self.show_exception(&e);
            }
        }
        if let Some(b) = &mut self.bridge {
            b.quit = true;
        }
        self.publish(true);
    }

    /// Fire due timers once (headless tests drive the program without a UI thread).
    pub fn fire_timers_now(&mut self) {
        let snapshot: Vec<(FormId, String)> = self
            .forms
            .iter()
            .enumerate()
            .flat_map(|(i, f)| {
                f.controls
                    .iter()
                    .filter(|c| c.kind == Kind::Timer && c.enabled)
                    .filter_map(move |c| c.events.on_timer.clone().map(|h| (i, h)))
            })
            .collect();
        for (f, h) in snapshot {
            self.run_handler(f, &h);
        }
    }
}
