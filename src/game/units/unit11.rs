//! Unit11 — TFormB "Game Corner" lottery. Port of recon/Unit11.pas.

use super::*;

pub fn dispatch(w: &mut World, h: &str) -> Option<R<()>> {
    Some(match h {
        "FormShow" => form_show(w),
        "Button2Click" => w.close(FORM_B), // @00482bfc
        "Button1Click" => button1_click(w),
        _ => return None,
    })
}

/// @00482bc8
fn form_show(w: &mut World) -> R<()> {
    w.set_spin_value(FORM_B, "SpinEdit1", 0)?;
    w.set_item_index(FORM_B, "RadioGroup1", -1);
    let money = w.m.money();
    w.c(FORM_B, "SpinEdit1").max_value = money as i32; // direct FMaxValue write (Cardinal -> Integer)
    Ok(())
}

/// @00482c04
fn button1_click(w: &mut World) -> R<()> {
    let bet = w.spin_value(FORM_B, "SpinEdit1");
    if !(bet >= 0 && (bet as u32) <= w.m.money()) {
        return Ok(());
    }
    if w.item_index(FORM_B, "RadioGroup1") == -1 {
        return warning(w, "Please select something to play for.");
    }
    if w.item_index(FORM_B, "RadioGroup2") == -1 {
        return warning(w, "Please select a number to bet on.");
    }
    w.m.set_money(w.m.money().wrapping_sub(bet as u32));
    super::unit5::freshen(w)?;
    w.update(); // Form5.Update
    for _ in 1..=10000 {
        let r = w.rng.random(10);
        w.set_item_index(FORM_B, "RadioGroup3", r);
        w.update(); // FormB.Update
    }
    if w.item_index(FORM_B, "RadioGroup2") != w.item_index(FORM_B, "RadioGroup3") {
        return Ok(());
    }
    w.info("Congratulations! You won! And your prize is...")?;
    let mut rest: u32 = 0;
    let mut s = String::new();
    let items: [(&str, i32); 6] = [
        (" POKéBall", 100),
        (" Great Ball", 300),
        (" Ultra Ball", 600),
        (" Master Ball", 2500),
        (" Potion", 200),
        (" Super Potion", 500),
    ];
    match w.item_index(FORM_B, "RadioGroup1") {
        0 => {
            rest = bet.wrapping_mul(10) as u32;
            s = format!("{rest}¥!");
        }
        k @ 1..=6 => {
            let (label, per) = items[(k - 1) as usize];
            let n = (bet / per) as u32;
            rest = ((bet % per) * 10) as u32;
            s = format!("{n}{label}");
            if n > 1 {
                s.push('s');
            }
            if rest > 0 {
                s = format!("{s} and {rest}¥");
            }
            s.push('!');
            match k {
                1 => w.m.set_poke_balls(w.m.poke_balls().wrapping_add(n)),
                2 => w.m.set_great_balls(w.m.great_balls().wrapping_add(n)),
                3 => w.m.set_ultra_balls(w.m.ultra_balls().wrapping_add(n)),
                4 => w.m.set_master_balls(w.m.master_balls().wrapping_add(n)),
                5 => w.m.set_potions(w.m.potions().wrapping_add(n)),
                _ => w.m.set_super_potions(w.m.super_potions().wrapping_add(n)),
            }
        }
        _ => {}
    }
    w.m.set_money(w.m.money().wrapping_add(rest));
    w.info(&s)
}
