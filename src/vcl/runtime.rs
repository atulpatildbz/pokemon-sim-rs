//! Threading model that preserves Delphi's sequential, re-entrant event semantics.
//!
//! * The **logic thread** owns the world (forms + game state) and runs ported handlers.
//! * The **UI thread** (egui) renders the latest published [`Snapshot`] and sends [`Input`]s.
//! * Blocking VCL calls (MessageDlg, ShowModal, Execute of file dialogs, ProcessMessages)
//!   publish a snapshot, then pump inputs/timers until they can return — exactly like the
//!   VCL modal message loop, including timers firing re-entrantly while a dialog is open.

use super::model::Form;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Delphi TMsgDlgType.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsgDlgType {
    Warning = 0,
    Error = 1,
    Information = 2,
    Confirmation = 3,
    Custom = 4,
}

impl MsgDlgType {
    pub fn from_ord(v: u8) -> MsgDlgType {
        match v {
            0 => MsgDlgType::Warning,
            1 => MsgDlgType::Error,
            2 => MsgDlgType::Information,
            3 => MsgDlgType::Confirmation,
            _ => MsgDlgType::Custom,
        }
    }
    pub fn title(self) -> &'static str {
        // Dialogs.pas captions (SMsgDlgWarning etc.).
        match self {
            MsgDlgType::Warning => "Warning",
            MsgDlgType::Error => "Error",
            MsgDlgType::Information => "Information",
            MsgDlgType::Confirmation => "Confirm",
            MsgDlgType::Custom => "",
        }
    }
}

/// Delphi TMsgDlgButtons bit set: mbYes=0, mbNo=1, mbOK=2, mbCancel=3, mbAbort=4, mbRetry=5,
/// mbIgnore=6, mbAll=7, mbNoToAll=8, mbYesToAll=9, mbHelp=10.
pub const MB_YES: u16 = 1 << 0;
pub const MB_NO: u16 = 1 << 1;
pub const MB_OK: u16 = 1 << 2;
pub const MB_CANCEL: u16 = 1 << 3;

/// Modal results (Controls.pas).
pub const MR_NONE: i32 = 0;
pub const MR_OK: i32 = 1;
pub const MR_CANCEL: i32 = 2;
pub const MR_YES: i32 = 6;
pub const MR_NO: i32 = 7;

#[derive(Debug, Clone, PartialEq)]
pub enum Dialog {
    Message { text: String, kind: MsgDlgType, buttons: u16 },
    /// TOpenDialog / TSaveDialog .Execute
    File { save: bool, title: String, filter: String, default_ext: String, initial: String },
    /// A form shown with ShowModal (rendered from the forms list).
    ModalForm { var: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum DialogAnswer {
    Button(i32),
    File(Option<String>),
    /// ModalForm finished: the modal result is in the form model.
    Closed,
}

/// TMediaPlayer state (actual audio output is done by the front-end).
#[derive(Default, Debug, Clone, PartialEq)]
pub struct Media {
    pub file_name: String,
    pub playing: bool,
    /// Bumped on every Open/Play/Stop/Rewind so the front-end can react.
    pub command_seq: u64,
    /// Milliseconds, as reported back by the front-end.
    pub position: i64,
    pub length: i64,
}

/// Which POKéMON an effect concerns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Own,
    Enemy,
}

/// Presentation events emitted by the ported handlers at the moment something happens, so the
/// front-end can animate a turn step by step. Purely one-way: emitting never reads the RNG,
/// branches, or touches game state, and nothing is recorded when running headless.
#[derive(Debug, Clone, PartialEq)]
pub enum Fx {
    /// A battle-log line (Form7.Memo1.Lines.Add).
    Log(String),
    /// A wild or trainer battle begins against `species` (1-based).
    BattleStart { species: i32, hp: i32 },
    /// The player's POKéMON enters the battle with full `hp`.
    SendOut { species: i32, hp: i32 },
    Attack(Side),
    /// HP after a hit or a potion (not yet clamped at 0, like the program's own variable).
    Hp { side: Side, hp: i32, max: i32 },
    Faint(Side),
    BallThrow,
    Caught(bool),
    LevelUp,
}

#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    /// Effects published since the front-end last drained them.
    pub fx: Vec<Fx>,
    /// Auto-saves written so far.
    pub autosaves: u64,
    pub forms: Vec<Form>,
    pub dialog: Option<(u64, Dialog)>,
    /// Form variables, back to front.
    pub z_order: Vec<String>,
    pub raise_seq: u64,
    pub media: Media,
    pub quit: bool,
    pub generation: u64,
}

/// UI -> logic.
#[derive(Debug, Clone)]
pub enum Input {
    /// Fire an event handler bound on a control: ("Form4", "Button2", Event::Click).
    Event { form: String, control: String, event: Event },
    Menu { form: String, item: String },
    SetItemIndex { form: String, control: String, index: i32 },
    SetText { form: String, control: String, text: String },
    SetValue { form: String, control: String, value: i32 },
    /// User clicked the window close box.
    CloseForm { form: String },
    /// User dragged a window (Left/Top are readable by code).
    Moved { form: String, left: i32, top: i32 },
    /// Desktop (root window) size, reported as Screen.Width/Height.
    ScreenSize { width: i32, height: i32 },
    /// Playback progress from the front-end's audio output.
    MediaStatus { position: i64, length: i64 },
    Answer { id: u64, answer: DialogAnswer },
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Click,
    DblClick,
    Change,
}

/// Everything the logic thread needs to talk to the UI.
pub struct Bridge {
    pub snapshot: Arc<Mutex<Snapshot>>,
    pub inputs: Receiver<Input>,
    pub repaint: Box<dyn Fn() + Send>,
    next_dialog: u64,
    /// Inputs received while waiting for something specific; replayed in order later.
    pub deferred: Vec<Input>,
    pub quit: bool,
}

impl Bridge {
    pub fn new(snapshot: Arc<Mutex<Snapshot>>, inputs: Receiver<Input>, repaint: Box<dyn Fn() + Send>) -> Bridge {
        Bridge { snapshot, inputs, repaint, next_dialog: 1, deferred: Vec::new(), quit: false }
    }

    pub fn new_dialog_id(&mut self) -> u64 {
        self.next_dialog += 1;
        self.next_dialog
    }

    /// Wait for the next input or until `deadline` (for timers).
    pub fn recv(&mut self, deadline: Option<Instant>) -> Option<Input> {
        if !self.deferred.is_empty() {
            return Some(self.deferred.remove(0));
        }
        let res = match deadline {
            Some(d) => {
                let now = Instant::now();
                if d <= now {
                    return None;
                }
                self.inputs.recv_timeout(d - now)
            }
            None => self.inputs.recv().map_err(|_| RecvTimeoutError::Disconnected),
        };
        match res {
            Ok(i) => Some(i),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => {
                self.quit = true;
                Some(Input::Quit)
            }
        }
    }

    pub fn try_recv(&mut self) -> Option<Input> {
        if !self.deferred.is_empty() {
            return Some(self.deferred.remove(0));
        }
        self.inputs.try_recv().ok()
    }
}

/// Timer bookkeeping: when each enabled TTimer is next due.
#[derive(Default)]
pub struct Timers {
    pub due: Vec<((String, String), Instant)>,
}

impl Timers {
    /// Reconcile with the current forms: new/enabled timers get a fresh due time, disabled ones are dropped.
    /// Like Windows timers, (re)enabling or changing Interval restarts the countdown.
    pub fn sync(&mut self, forms: &[Form], now: Instant) {
        let mut next = Vec::new();
        for f in forms {
            for c in &f.controls {
                if c.kind == super::model::Kind::Timer && c.enabled && c.interval > 0 && c.events.on_timer.is_some() {
                    let key = (f.var.clone(), c.name.clone());
                    let due = self
                        .due
                        .iter()
                        .find(|(k, _)| *k == key)
                        .map(|(_, d)| *d)
                        .unwrap_or(now + Duration::from_millis(c.interval as u64));
                    next.push((key, due));
                }
            }
        }
        self.due = next;
    }
    pub fn next_deadline(&self) -> Option<Instant> {
        self.due.iter().map(|(_, d)| *d).min()
    }
    /// Pop one due timer (re-arming it).
    pub fn take_due(&mut self, forms: &[Form], now: Instant) -> Option<(String, String)> {
        let i = self.due.iter().position(|(_, d)| *d <= now)?;
        let key = self.due[i].0.clone();
        let interval = forms
            .iter()
            .find(|f| f.var == key.0)
            .map(|f| f.c(&key.1).interval)
            .unwrap_or(1000);
        self.due[i].1 = now + Duration::from_millis(interval as u64);
        Some(key)
    }
    /// Call when a handler changed a timer's Enabled/Interval so the countdown restarts.
    pub fn restart(&mut self, form: &str, timer: &str) {
        self.due.retain(|((f, t), _)| !(f == form && t == timer));
    }
}
