//! Unit7 — TForm7 "Battle Info" log window. Port of recon/Unit7.pas.

use super::*;

pub fn dispatch(w: &mut World, h: &str) -> Option<R<()>> {
    Some(match h {
        "FormShow" => {
            w.memo_clear(FORM7, "Memo1"); // @0047d294 Memo1.Clear
            Ok(())
        }
        _ => return None,
    })
}
