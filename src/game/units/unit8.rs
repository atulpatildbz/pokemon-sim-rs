//! Unit8 — TForm8 POKéDEX viewer. Port of recon/Unit8.pas.

use super::*;

pub fn dispatch(w: &mut World, h: &str) -> Option<R<()>> {
    Some(match h {
        "Button1Click" => w.hide(FORM8), // @0047d5c4
        "ComboBox1Change" => update_display(w), // @0047d5d0
        "UpdateDisplay" => update_display(w),
        "FormShow" => form_show(w),
        _ => return None,
    })
}

/// @0047d5d8
pub fn update_display(w: &mut World) -> R<()> {
    if w.item_index(FORM8, "ComboBox1") == -1 {
        return Ok(());
    }
    let i = w.item_index(FORM8, "ComboBox1") + 1;
    if w.m.pkmn_type2(i) == 0 {
        w.hide_c(FORM8, "Label3");
    } else {
        w.show_c(FORM8, "Label3");
    }
    let t1 = w.m.type_name(w.m.pkmn_type1(i) as i32);
    w.set_caption(FORM8, "Label2", t1.refd(crate::game::mem::fault::LSTR_ASG)?);
    let left = w.cr(FORM8, "Label2").left + w.cr(FORM8, "Label2").width;
    w.c(FORM8, "Label3").left = left;
    if w.m.pkmn_type2(i) > 0 {
        let s = crate::cat!("/", &w.m.type_name(w.m.pkmn_type2(i) as i32))?;
        w.set_caption(FORM8, "Label3", &s);
    }
    w.lv_set_image(FORM8, "ListView1", 0, i - 1);
    let cap = w.item(FORM8, "ComboBox1", i - 1);
    w.set_caption(FORM8, "Label1", &cap);
    let site = crate::game::mem::fault::LSTR_ASG;
    for (k, label) in [(1, "Label6"), (2, "Label10"), (3, "Label11"), (4, "Label12")] {
        let s = w.m.move_name(w.m.learnset(i, k) as i32);
        w.set_caption(FORM8, label, s.refd(site)?);
    }
    for (k, label) in [(1, "Label13"), (2, "Label14"), (3, "Label15"), (4, "Label16")] {
        let s = w.m.type_name(w.m.move_type(w.m.learnset(i, k) as i32) as i32);
        w.set_caption(FORM8, label, s.refd(site)?);
    }
    if !w.m.owned(i) && !w.is_menu_checked(FORM4, "AlwaysshowattacksinPOKDEX1") {
        for label in ["Label6", "Label10", "Label11", "Label12", "Label13", "Label14", "Label15", "Label16"] {
            w.set_caption(FORM8, label, "???");
        }
    }
    let ev = w.m.evolve_level(i);
    if ev == 0 {
        w.set_caption(FORM8, "Label4", "Does not evolve.");
    } else {
        w.set_caption(FORM8, "Label4", &format!("Can evolve at level {ev}."));
    }
    w.show_c(FORM8, "Panel1");
    Ok(())
}

/// "Name (#NNN)" combo text used by Form4/Form8.
pub fn numbered_name(w: &World, i: i32) -> R<String> {
    let mut s = crate::cat!(&w.m.pkmn_name(i), " (#")?;
    if i < 100 {
        s.push('0');
    }
    if i < 10 {
        s.push('0');
    }
    Ok(format!("{s}{i})"))
}

/// @0047d9a8
fn form_show(w: &mut World) -> R<()> {
    let saved = w.item_index(FORM8, "ComboBox1");
    w.set_item_index(FORM8, "ComboBox1", -1);
    w.items_clear(FORM8, "ComboBox1");
    for i in 1..=386 {
        let s = numbered_name(w, i)?;
        w.items_add(FORM8, "ComboBox1", &s);
    }
    w.set_item_index(FORM8, "ComboBox1", saved); // no OnChange
    Ok(())
}
