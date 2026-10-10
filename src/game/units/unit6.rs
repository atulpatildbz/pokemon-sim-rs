//! Unit6 — TOKRightDlg "Enter your name:" (modal). Port of recon/Unit6.pas.

use super::*;

pub fn dispatch(w: &mut World, h: &str) -> Option<R<()>> {
    Some(match h {
        "OKBtnClick" => ok_btn_click(w),
        _ => return None,
    })
}

/// @0048bfe4 (OKBtn.ModalResult = mrOk closes the dialog after this handler)
fn ok_btn_click(w: &mut World) -> R<()> {
    let mut name = w.cr(OK_RIGHT_DLG, "Edit1").text.clone();
    if name.is_empty() {
        name = "Ash".into();
    }
    w.m.set_player_name(&name);
    let fname = w.cr(FORM4, "SaveDialog1").file_name.clone();
    let cap = format!("{} - {} [{}]", w.forms[FORM1].caption, name, title_suffix(&fname));
    w.set_form_caption(FORM4, &cap);
    w.set_caption(FORM5, "PlayerNameLbl", &name);
    Ok(())
}
