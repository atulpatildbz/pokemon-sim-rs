//! The original Delphi units, ported 1:1 from the reconstructions in
//! downloads/pokegame/recon/*.pas (each function cites the original address).

pub mod unit1;
pub mod unit10;
pub mod unit11;
pub mod unit3;
pub mod unit4;
pub mod unit5;
pub mod unit6;
pub mod unit7;
pub mod unit8;
pub mod unit9;

use super::mem::R;
use super::world::*;
use crate::delphi::{Var, F80};
pub use crate::vcl::runtime::{Fx, MsgDlgType, Side, MB_CANCEL, MB_NO, MB_OK, MB_YES, MR_YES};

pub const MB_YES_NO: u16 = MB_YES | MB_NO;

/// Call a published method by name (what the VCL does when it fires an event).
pub fn dispatch(w: &mut World, f: FormId, handler: &str) -> R<()> {
    let handled = match f {
        FORM1 => unit1::dispatch(w, handler),
        FORM3 => unit3::dispatch(w, handler),
        FORM4 => unit4::dispatch(w, handler),
        FORM5 => unit5::dispatch(w, handler),
        OK_RIGHT_DLG => unit6::dispatch(w, handler),
        FORM7 => unit7::dispatch(w, handler),
        FORM8 => unit8::dispatch(w, handler),
        FORM9 => unit9::dispatch(w, handler),
        FORM_A => unit10::dispatch(w, handler),
        FORM_B => unit11::dispatch(w, handler),
        _ => None,
    };
    handled.unwrap_or_else(|| panic!("no handler {handler} on form {}", w.forms[f].var))
}

/// `VReal(Int(Random(n)) + c)`: FILD; Int; FADD single c; _VarFromReal.
pub fn rand_plus(w: &mut World, n: i32, c: f32) -> Var {
    let r = w.rng.random(n);
    Var::real(F80::from_i64(r as i64).int().add(F80::from_f32(c)))
}

pub fn warning(w: &mut World, text: &str) -> R<()> {
    w.message_dlg(text, MsgDlgType::Warning, MB_OK).map(|_| ())
}

pub fn confirm(w: &mut World, text: &str) -> R<bool> {
    Ok(w.message_dlg(text, MsgDlgType::Confirmation, MB_YES_NO)? == MR_YES)
}

/// Position of the last path separator handling in the title-suffix loops (TForm1.LoadGame,
/// TOKRightDlg.OKBtnClick): the original scans for '\'. On non-Windows hosts the native
/// separator plays that role so the title still shows just the file name.
pub fn title_suffix(file_name: &str) -> String {
    let chars: Vec<char> = file_name.chars().collect();
    let sep = |c: char| c == '\\' || c == std::path::MAIN_SEPARATOR;
    let mut found = false;
    let mut s = String::new();
    for i in (1..=chars.len()).rev() {
        if sep(chars[i - 1]) {
            found = true;
        }
        if !found {
            s = chars[i - 1..].iter().collect();
        }
    }
    s
}
