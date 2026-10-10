//! A small model of the Delphi 7 VCL components this program uses.
//!
//! Ported event handlers mutate these structs exactly like the original code mutated VCL
//! controls; the egui front-end only renders them and feeds user input back.

use std::collections::HashMap;

/// Delphi TColor. System colours use the D7 encoding `$FF0000xx` (clBtnFace = $FF00000F).
pub type TColor = u32;
pub const CL_BTN_FACE: TColor = 0xFF00_000F;
pub const CL_WINDOW: TColor = 0xFF00_0005;
pub const CL_WINDOW_TEXT: TColor = 0xFF00_0008;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Label,
    Button,
    Edit,
    Memo,
    ListView,
    RadioGroup,
    GroupBox,
    Panel,
    Bevel,
    Image,
    Gauge,
    ComboBox,
    SpinEdit,
    Timer,
    MediaPlayer,
    ImageList,
    MainMenu,
    OpenDialog,
    SaveDialog,
    Other,
}

impl Kind {
    pub fn from_class(c: &str) -> Kind {
        match c {
            "TLabel" => Kind::Label,
            "TButton" | "TBitBtn" => Kind::Button,
            "TEdit" => Kind::Edit,
            "TMemo" => Kind::Memo,
            "TListView" => Kind::ListView,
            "TRadioGroup" => Kind::RadioGroup,
            "TGroupBox" => Kind::GroupBox,
            "TPanel" => Kind::Panel,
            "TBevel" => Kind::Bevel,
            "TImage" => Kind::Image,
            "TGauge" => Kind::Gauge,
            "TComboBox" => Kind::ComboBox,
            "TSpinEdit" => Kind::SpinEdit,
            "TTimer" => Kind::Timer,
            "TMediaPlayer" => Kind::MediaPlayer,
            "TImageList" => Kind::ImageList,
            "TMainMenu" => Kind::MainMenu,
            "TOpenDialog" => Kind::OpenDialog,
            "TSaveDialog" => Kind::SaveDialog,
            _ => Kind::Other,
        }
    }
    /// Non-visual components are never drawn.
    pub fn is_visual(self) -> bool {
        !matches!(
            self,
            Kind::Timer | Kind::ImageList | Kind::MainMenu | Kind::OpenDialog | Kind::SaveDialog | Kind::Other
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FontSpec {
    pub name: String,
    /// Delphi Font.Height: negative = character height in pixels.
    pub height: i32,
    pub color: TColor,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

impl Default for FontSpec {
    fn default() -> Self {
        FontSpec { name: "MS Sans Serif".into(), height: -11, color: CL_WINDOW_TEXT, bold: false, italic: false, underline: false }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ListItem {
    pub caption: String,
    pub image_index: i32,
    pub sub_items: Vec<String>,
}

/// Which handler (method name of the owning form) each event is bound to.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Events {
    pub on_click: Option<String>,
    pub on_dbl_click: Option<String>,
    pub on_change: Option<String>,
    pub on_timer: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Control {
    pub name: String,
    pub class: String,
    pub kind: Kind,
    /// Index of the parent control within the form, or None for the form itself.
    pub parent: Option<usize>,
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
    pub visible: bool,
    pub enabled: bool,
    pub caption: String,
    /// TEdit.Text / TComboBox.Text.
    pub text: String,
    pub color: TColor,
    pub parent_color: bool,
    pub transparent: bool,
    pub font: FontSpec,
    pub alignment: i32,
    pub auto_size: bool,
    /// TRadioGroup / TComboBox items, TMemo lines.
    pub items: Vec<String>,
    pub item_index: i32,
    pub columns: i32,
    pub list_items: Vec<ListItem>,
    /// "Form.ImageList" key used by a TListView (SmallImages / LargeImages).
    pub small_images: Option<String>,
    pub large_images: Option<String>,
    pub view_style: String,
    /// TGauge.
    pub progress: i32,
    pub min_value: i32,
    pub max_value: i32,
    pub fore_color: TColor,
    pub back_color: TColor,
    /// TSpinEdit: the design-time Value (runtime state is `text`, Value = StrToInt(Text)).
    pub value: i32,
    pub increment: i32,
    /// TTimer.
    pub interval: u32,
    /// TImage picture key into Assets::pictures, or an ImageList image set at runtime.
    pub picture: Option<PictureRef>,
    pub stretch: bool,
    /// TOpenDialog / TSaveDialog.
    pub file_name: String,
    pub filter: String,
    pub default_ext: String,
    pub read_only: bool,
    pub max_length: i32,
    pub modal_result: i32,
    pub events: Events,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PictureRef {
    /// Design-time Picture.Data ("Form4.Image1").
    Design(String),
    /// Copied from an ImageList at runtime (ImageList.GetBitmap).
    FromList { list: String, index: i32 },
    None,
}

impl Control {
    pub fn new(name: &str, class: &str) -> Control {
        Control {
            name: name.into(),
            class: class.into(),
            kind: Kind::from_class(class),
            parent: None,
            left: 0,
            top: 0,
            width: 0,
            height: 0,
            visible: true,
            enabled: true,
            caption: String::new(),
            text: String::new(),
            color: CL_BTN_FACE,
            parent_color: true,
            transparent: false,
            font: FontSpec::default(),
            alignment: 0,
            auto_size: true,
            items: Vec::new(),
            item_index: -1,
            columns: 1,
            list_items: Vec::new(),
            small_images: None,
            large_images: None,
            view_style: "vsIcon".into(),
            progress: 0,
            min_value: 0,
            max_value: 100,
            fore_color: 0x000000,
            back_color: 0xFFFFFF,
            value: 0,
            increment: 1,
            interval: 1000,
            picture: None,
            stretch: false,
            file_name: String::new(),
            filter: String::new(),
            default_ext: String::new(),
            read_only: false,
            max_length: 0,
            modal_result: 0,
            events: Events::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MenuItem {
    pub name: String,
    pub caption: String,
    pub enabled: bool,
    pub visible: bool,
    pub checked: bool,
    pub on_click: Option<String>,
    pub children: Vec<MenuItem>,
}

impl MenuItem {
    pub fn find_mut(&mut self, name: &str) -> Option<&mut MenuItem> {
        if self.name.eq_ignore_ascii_case(name) {
            return Some(self);
        }
        self.children.iter_mut().find_map(|c| c.find_mut(name))
    }
    pub fn find(&self, name: &str) -> Option<&MenuItem> {
        if self.name.eq_ignore_ascii_case(name) {
            return Some(self);
        }
        self.children.iter().find_map(|c| c.find(name))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Form {
    /// Global variable name ("Form4").
    pub var: String,
    pub class: String,
    pub caption: String,
    pub left: i32,
    pub top: i32,
    pub client_width: i32,
    pub client_height: i32,
    pub visible: bool,
    pub color: TColor,
    pub font: FontSpec,
    pub border_style: String,
    pub position: String,
    pub controls: Vec<Control>,
    pub by_name: HashMap<String, usize>,
    /// Top-level items of the form's TMainMenu (if any).
    pub menu: Vec<MenuItem>,
    pub events: FormEvents,
    /// BorderIcons contains biSystemMenu (window has a close box).
    pub close_box: bool,
    /// Set while shown with ShowModal.
    pub modal: bool,
    pub modal_result: i32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FormEvents {
    pub on_show: Option<String>,
    pub on_hide: Option<String>,
    pub on_close: Option<String>,
    pub on_deactivate: Option<String>,
}

impl Form {
    pub fn idx(&self, name: &str) -> usize {
        *self
            .by_name
            .get(&name.to_ascii_lowercase())
            .unwrap_or_else(|| panic!("{}: no component named {name}", self.var))
    }
    pub fn c(&self, name: &str) -> &Control {
        &self.controls[self.idx(name)]
    }
    pub fn cm(&mut self, name: &str) -> &mut Control {
        let i = self.idx(name);
        &mut self.controls[i]
    }
    pub fn menu_item(&mut self, name: &str) -> &mut MenuItem {
        let var = self.var.clone();
        self.menu
            .iter_mut()
            .find_map(|m| m.find_mut(name))
            .unwrap_or_else(|| panic!("{var}: no menu item named {name}"))
    }
    pub fn menu_ref(&self, name: &str) -> &MenuItem {
        self.menu.iter().find_map(|m| m.find(name)).unwrap_or_else(|| panic!("{}: no menu item {name}", self.var))
    }
    pub fn has(&self, name: &str) -> bool {
        self.by_name.contains_key(&name.to_ascii_lowercase())
    }
    /// Effective visibility (a control is shown only if all its parents are visible).
    pub fn showing(&self, i: usize) -> bool {
        let mut cur = Some(i);
        while let Some(c) = cur {
            if !self.controls[c].visible {
                return false;
            }
            cur = self.controls[c].parent;
        }
        true
    }
    /// Position relative to the form's client area.
    pub fn abs_pos(&self, i: usize) -> (i32, i32) {
        let (mut x, mut y) = (0, 0);
        let mut cur = Some(i);
        while let Some(c) = cur {
            x += self.controls[c].left;
            y += self.controls[c].top;
            // Group boxes offset their children's client area by nothing extra in VCL coordinates.
            cur = self.controls[c].parent;
        }
        (x, y)
    }
}
