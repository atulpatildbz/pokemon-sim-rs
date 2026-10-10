//! Build runtime `Form` models from parsed DFM component trees.

use super::model::*;
use crate::assets::dfm::{self, Component, Value};
use std::collections::HashMap;

fn color_ident(s: &str) -> Option<TColor> {
    Some(match s {
        "clBlack" => 0x000000,
        "clMaroon" => 0x000080,
        "clGreen" => 0x008000,
        "clOlive" => 0x008080,
        "clNavy" => 0x800000,
        "clPurple" => 0x800080,
        "clTeal" => 0x808000,
        "clGray" => 0x808080,
        "clSilver" => 0xC0C0C0,
        "clRed" => 0x0000FF,
        "clLime" => 0x00FF00,
        "clYellow" => 0x00FFFF,
        "clBlue" => 0xFF0000,
        "clFuchsia" => 0xFF00FF,
        "clAqua" => 0xFFFF00,
        "clWhite" => 0xFFFFFF,
        "clBtnFace" => CL_BTN_FACE,
        "clWindow" => CL_WINDOW,
        "clWindowText" => CL_WINDOW_TEXT,
        "clBtnText" => 0xFF00_0012,
        "clHighlight" => 0xFF00_000D,
        "clHighlightText" => 0xFF00_000E,
        "clGrayText" => 0xFF00_0011,
        "clInfoBk" => 0xFF00_0018,
        "clMenu" => 0xFF00_0004,
        "clCaptionText" => 0xFF00_0009,
        _ => return None,
    })
}

fn color(v: &Value) -> Option<TColor> {
    match v {
        Value::Int(i) => Some(*i as u32),
        Value::Ident(s) => color_ident(s),
        _ => None,
    }
}

fn strings(v: &Value) -> Vec<String> {
    match v {
        Value::List(items) => items.iter().filter_map(|i| i.as_str().map(str::to_string)).collect(),
        _ => Vec::new(),
    }
}

fn font_from(c: &Component, base: &FontSpec) -> FontSpec {
    let mut f = base.clone();
    if let Some(n) = c.str("Font.Name") {
        f.name = n.into();
    }
    if let Some(h) = c.int("Font.Height") {
        f.height = h as i32;
    }
    if let Some(col) = c.prop("Font.Color").and_then(color) {
        f.color = col;
    }
    if let Some(Value::Set(s)) = c.prop("Font.Style") {
        f.bold = s.iter().any(|x| x == "fsBold");
        f.italic = s.iter().any(|x| x == "fsItalic");
        f.underline = s.iter().any(|x| x == "fsUnderline");
    }
    f
}

fn menu_from(c: &Component) -> MenuItem {
    MenuItem {
        name: c.name.clone(),
        caption: c.str("Caption").unwrap_or("").to_string(),
        enabled: c.bool("Enabled").unwrap_or(true),
        visible: c.bool("Visible").unwrap_or(true),
        checked: c.bool("Checked").unwrap_or(false),
        on_click: c.str("OnClick").map(str::to_string),
        children: c.children.iter().filter(|k| k.class == "TMenuItem").map(menu_from).collect(),
    }
}

/// `var` is the form's global variable name ("Form4"); it qualifies image-list references.
pub fn build_form(root: &Component, var: &str) -> Form {
    let font = font_from(root, &FontSpec::default());
    let mut form = Form {
        var: var.into(),
        class: root.class.clone(),
        caption: root.str("Caption").unwrap_or("").into(),
        left: root.int("Left").unwrap_or(0) as i32,
        top: root.int("Top").unwrap_or(0) as i32,
        client_width: root.int("ClientWidth").or(root.int("Width")).unwrap_or(320) as i32,
        client_height: root.int("ClientHeight").or(root.int("Height")).unwrap_or(240) as i32,
        visible: root.bool("Visible").unwrap_or(false),
        color: root.prop("Color").and_then(color).unwrap_or(CL_BTN_FACE),
        font: font.clone(),
        border_style: root.str("BorderStyle").unwrap_or("bsSizeable").into(),
        position: root.str("Position").unwrap_or("poDesigned").into(),
        controls: Vec::new(),
        by_name: HashMap::new(),
        menu: Vec::new(),
        events: FormEvents {
            on_show: root.str("OnShow").map(str::to_string),
            on_hide: root.str("OnHide").map(str::to_string),
            on_close: root.str("OnClose").map(str::to_string),
            on_deactivate: root.str("OnDeactivate").map(str::to_string),
        },
        close_box: match root.prop("BorderIcons") {
            Some(Value::Set(s)) => s.iter().any(|x| x == "biSystemMenu"),
            _ => true,
        },
        modal: false,
        modal_result: 0,
    };
    for child in &root.children {
        add(&mut form, child, None, &font, var);
    }
    form
}

fn qualify(var: &str, r: &str) -> String {
    if r.contains('.') { r.to_string() } else { format!("{var}.{r}") }
}

fn add(form: &mut Form, c: &Component, parent: Option<usize>, parent_font: &FontSpec, var: &str) {
    if c.class == "TMainMenu" {
        form.menu = c.children.iter().map(menu_from).collect();
    }
    let mut k = Control::new(&c.name, &c.class);
    k.parent = parent;
    k.left = c.int("Left").unwrap_or(0) as i32;
    k.top = c.int("Top").unwrap_or(0) as i32;
    k.width = c.int("Width").unwrap_or(0) as i32;
    k.height = c.int("Height").unwrap_or(0) as i32;
    k.visible = c.bool("Visible").unwrap_or(true);
    k.enabled = c.bool("Enabled").unwrap_or(true);
    k.caption = c.str("Caption").unwrap_or("").into();
    k.text = c.str("Text").unwrap_or("").into();
    let parent_font_flag = c.bool("ParentFont").unwrap_or(true);
    k.font = if parent_font_flag || c.prop("Font.Name").is_some() { font_from(c, parent_font) } else { parent_font.clone() };
    if let Some(col) = c.prop("Color").and_then(color) {
        k.color = col;
        k.parent_color = false;
    } else if matches!(k.kind, Kind::Edit | Kind::Memo | Kind::ListView | Kind::ComboBox | Kind::SpinEdit) {
        k.color = CL_WINDOW;
        k.parent_color = false;
    }
    if c.bool("ParentColor") == Some(true) {
        k.parent_color = true;
    }
    k.transparent = c.bool("Transparent").unwrap_or(false);
    k.alignment = match c.str("Alignment") {
        Some("taRightJustify") => 1,
        Some("taCenter") => 2,
        _ => 0,
    };
    k.auto_size = c.bool("AutoSize").unwrap_or(k.kind == Kind::Label);
    if let Some(v) = c.prop("Items.Strings") {
        k.items = strings(v);
    }
    if let Some(v) = c.prop("Lines.Strings") {
        k.items = strings(v);
    }
    if let Some(i) = c.int("ItemIndex") {
        k.item_index = i as i32;
    }
    k.columns = c.int("Columns").unwrap_or(1) as i32;
    if let Some(Value::Binary(b)) = c.prop("Items.Data") {
        if let Ok(items) = dfm::parse_list_items(b) {
            k.list_items = items
                .into_iter()
                .map(|i| ListItem { caption: i.caption, image_index: i.image_index, sub_items: i.sub_items })
                .collect();
        }
    }
    k.small_images = c.str("SmallImages").map(|s| qualify(var, s));
    k.large_images = c.str("LargeImages").map(|s| qualify(var, s));
    k.view_style = c.str("ViewStyle").unwrap_or("vsIcon").into();
    k.progress = c.int("Progress").unwrap_or(0) as i32;
    k.min_value = c.int("MinValue").unwrap_or(0) as i32;
    k.max_value = c.int("MaxValue").unwrap_or(if k.kind == Kind::Gauge { 100 } else { 0 }) as i32;
    k.fore_color = c.prop("ForeColor").and_then(color).unwrap_or(0x000000);
    k.back_color = c.prop("BackColor").and_then(color).unwrap_or(0xFFFFFF);
    k.value = c.int("Value").unwrap_or(0) as i32;
    if k.kind == Kind::SpinEdit {
        k.text = k.value.to_string();
    }
    k.increment = c.int("Increment").unwrap_or(1) as i32;
    k.interval = c.int("Interval").unwrap_or(1000) as u32;
    if c.prop("Picture.Data").is_some() {
        k.picture = Some(PictureRef::Design(format!("{var}.{}", c.name)));
    }
    k.stretch = c.bool("Stretch").unwrap_or(false);
    k.filter = c.str("Filter").unwrap_or("").into();
    k.default_ext = c.str("DefaultExt").unwrap_or("").into();
    k.read_only = c.bool("ReadOnly").unwrap_or(false);
    k.max_length = c.int("MaxLength").unwrap_or(0) as i32;
    k.modal_result = match c.prop("ModalResult") {
        Some(Value::Int(i)) => *i as i32,
        Some(Value::Ident(s)) => match s.as_str() {
            "mrOk" => 1,
            "mrCancel" => 2,
            "mrAbort" => 3,
            "mrRetry" => 4,
            "mrIgnore" => 5,
            "mrYes" => 6,
            "mrNo" => 7,
            _ => 0,
        },
        _ => 0,
    };
    k.events = Events {
        on_click: c.str("OnClick").map(str::to_string),
        on_dbl_click: c.str("OnDblClick").map(str::to_string),
        on_change: c.str("OnChange").map(str::to_string),
        on_timer: c.str("OnTimer").map(str::to_string),
    };
    let font = k.font.clone();
    let idx = form.controls.len();
    form.by_name.insert(c.name.to_ascii_lowercase(), idx);
    form.controls.push(k);
    if c.class != "TMainMenu" && c.class != "TMenuItem" {
        for child in &c.children {
            add(form, child, Some(idx), &font, var);
        }
    }
}
