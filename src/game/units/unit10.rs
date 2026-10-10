//! Unit10 — TFormA "Type Comparisons". Port of recon/Unit10.pas.

use super::*;
use crate::delphi::{float_to_str, F80};

pub fn dispatch(w: &mut World, h: &str) -> Option<R<()>> {
    Some(match h {
        "FormShow" => form_show(w),
        "ComboBox1Change" => combo_box1_change(w),
        "CalculateMultipliers" => calculate_multipliers(w),
        "Button1Click" => w.hide(FORM_A), // @0048295c
        _ => return None,
    })
}

/// @00482598
fn form_show(w: &mut World) -> R<()> {
    let saved = w.item_index(FORM_A, "ComboBox1");
    w.set_item_index(FORM_A, "ComboBox1", -1);
    w.items_clear(FORM_A, "ComboBox1");
    for i in 1..=386 {
        let n = w.m.pkmn_name(i);
        let t1 = w.m.type_name(w.m.pkmn_type1(i) as i32);
        let s = if w.m.pkmn_type2(i) == 0 {
            crate::cat!(&n, ", ", &t1)?
        } else {
            let t2 = w.m.type_name(w.m.pkmn_type2(i) as i32);
            crate::cat!(&n, ", ", &t1, "/", &t2)?
        };
        w.items_add(FORM_A, "ComboBox1", &s);
    }
    w.set_item_index(FORM_A, "ComboBox1", saved);
    let saved = w.item_index(FORM_A, "ComboBox2");
    w.set_item_index(FORM_A, "ComboBox2", -1);
    w.items_clear(FORM_A, "ComboBox2");
    for i in 1..=119 {
        let s = crate::cat!(&w.m.move_name(i), ", ", &w.m.type_name(w.m.move_type(i) as i32))?;
        w.items_add(FORM_A, "ComboBox2", &s);
    }
    w.set_item_index(FORM_A, "ComboBox2", saved);
    Ok(())
}

/// @004827a0 (OnChange of both combo boxes)
fn combo_box1_change(w: &mut World) -> R<()> {
    if w.item_index(FORM_A, "ComboBox1") != -1 && w.item_index(FORM_A, "ComboBox2") != -1 {
        calculate_multipliers(w)?;
    }
    Ok(())
}

/// @004827d0
fn calculate_multipliers(w: &mut World) -> R<()> {
    w.hide_c(FORM_A, "Label2");
    w.hide_c(FORM_A, "Label4");
    let m = w.item_index(FORM_A, "ComboBox2") + 1;
    let p = w.item_index(FORM_A, "ComboBox1") + 1;
    let atk = w.m.move_type(m) as i32;
    let mult1 = w.m.type_eff(atk, w.m.pkmn_type1(p) as i32);
    let mut mult2 = 1.0f32;
    if w.m.pkmn_type2(p) > 0 {
        mult2 = w.m.type_eff(atk, w.m.pkmn_type2(p) as i32);
        w.show_c(FORM_A, "Label2");
        w.show_c(FORM_A, "Label4");
    }
    let total = F80::from_f32(mult1).mul(F80::from_f32(mult2)).to_f32(); // FLD/FMUL/FSTP single
    w.set_caption(FORM_A, "Label3", &float_to_str(mult1 as f64));
    w.set_caption(FORM_A, "Label4", &float_to_str(mult2 as f64));
    w.set_caption(FORM_A, "Label6", &float_to_str(total as f64));
    for l in ["Label1", "Label3", "Label5", "Label6"] {
        w.show_c(FORM_A, l);
    }
    Ok(())
}
