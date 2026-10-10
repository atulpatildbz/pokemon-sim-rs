//! Unit9 — TForm9 POKéMON Mart. Port of recon/Unit9.pas.

use super::*;

pub fn dispatch(w: &mut World, h: &str) -> Option<R<()>> {
    Some(match h {
        "RadioGroup1Click" => radio_group1_click(w),
        "FormShow" => form_show(w),
        "Button1Click" => button1_click(w),
        "Button2Click" => w.hide(FORM9), // @00481738
        "SpinEdit1Change" => spin_edit1_change(w),
        _ => return None,
    })
}

/// Inlined in all three handlers: 32-bit IMUL of SpinEdit1.Value and the unit price, as Cardinal.
fn cost(w: &World) -> u32 {
    let mut c = 0u32;
    for (idx, price) in [(0, 1000), (1, 3000), (2, 6000), (3, 2000), (4, 5000)] {
        if w.item_index(FORM9, "RadioGroup1") == idx {
            c = w.spin_value(FORM9, "SpinEdit1").wrapping_mul(price) as u32;
        }
    }
    c
}

/// @0048122c
fn radio_group1_click(w: &mut World) -> R<()> {
    let c = cost(w);
    w.set_edit_text(FORM9, "Edit1", &format!("{c}¥"))?;
    w.set_enabled(FORM9, "Button1", true);
    if c > w.m.money() || c == 0 {
        w.set_enabled(FORM9, "Button1", false);
    }
    w.set_caption(FORM9, "Label2", "");
    w.set_caption(FORM9, "Label3", "");
    let i = w.item_index(FORM9, "RadioGroup1");
    if i == 0 {
        w.set_caption(FORM9, "Label2", "A standard POKéBALL.");
    }
    if i == 0 {
        w.set_caption(FORM9, "Label3", "Used for catching POKéMON.");
    }
    if i == 1 {
        w.set_caption(FORM9, "Label2", "This Ball has good chances");
    }
    if i == 1 {
        w.set_caption(FORM9, "Label3", "of capturing a POKéMON.");
    }
    if i == 2 {
        w.set_caption(FORM9, "Label2", "Has a very high chance of");
    }
    if i == 2 {
        w.set_caption(FORM9, "Label3", "capturing a POKéMON.");
    }
    if i == 3 {
        w.set_caption(FORM9, "Label2", "Restores 50HP in battle.");
    }
    if i == 4 {
        w.set_caption(FORM9, "Label2", "Restores 100HP in battle.");
    }
    Ok(())
}

/// @0048158c
fn form_show(w: &mut World) -> R<()> {
    w.set_spin_value(FORM9, "SpinEdit1", 0)?; // may fire SpinEdit1Change (text changed)
    w.set_enabled(FORM9, "Button1", false);
    w.set_item_index(FORM9, "RadioGroup1", -1);
    w.set_caption(FORM9, "Label2", "");
    w.set_caption(FORM9, "Label3", "");
    Ok(())
}

/// @004815d4
fn button1_click(w: &mut World) -> R<()> {
    let c = cost(w);
    w.m.set_money(w.m.money().wrapping_sub(c));
    let i = w.item_index(FORM9, "RadioGroup1");
    if i == 0 {
        w.m.set_poke_balls(w.m.poke_balls().wrapping_add(c / 1000));
    }
    if i == 1 {
        w.m.set_great_balls(w.m.great_balls().wrapping_add(c / 3000));
    }
    if i == 2 {
        w.m.set_ultra_balls(w.m.ultra_balls().wrapping_add(c / 6000));
    }
    if i == 3 {
        w.m.set_potions(w.m.potions().wrapping_add(c / 2000));
    }
    if i == 4 {
        w.m.set_super_potions(w.m.super_potions().wrapping_add(c / 5000));
    }
    w.hide(FORM9)
}

/// @00481744
fn spin_edit1_change(w: &mut World) -> R<()> {
    let c = cost(w);
    w.set_edit_text(FORM9, "Edit1", &format!("{c}¥"))?;
    w.set_enabled(FORM9, "Button1", true);
    if c > w.m.money() || c == 0 {
        w.set_enabled(FORM9, "Button1", false);
    }
    Ok(())
}
