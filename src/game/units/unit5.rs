//! Unit5 — TForm5 "Statistics": player stats, battle item buttons and the PC box (ListView1).
//! Port of recon/Unit5.pas.

use super::*;
use crate::delphi::{float_to_str, F80};
use crate::game::mem::fault;
use crate::vcl::model::{CL_BTN_FACE, CL_WINDOW};

pub fn dispatch(w: &mut World, h: &str) -> Option<R<()>> {
    Some(match h {
        "Timer1Timer" => timer1_timer(w),
        "ListView1Click" => list_view1_click(w),
        "Button1Click" => ball_click(w, "You threw a POKéBall.", 50, Ball::Poke),
        "Button2Click" => ball_click(w, "You threw a Great Ball.", 100, Ball::Great),
        "Button3Click" => ball_click(w, "You threw an Ultra Ball.", 250, Ball::Ultra),
        "Button4Click" => button4_click(w),
        "Button5Click" => potion_click(w, 50, false),
        "Button6Click" => potion_click(w, 100, true),
        "FormClick" => Ok(()), // @00480f7c Form4.SetFocus (focus is not modelled)
        _ => return None,
    })
}

/// @0047dffc
pub fn hide_buttons(w: &mut World) {
    for b in ["Button1", "Button2", "Button3", "Button4", "Button5", "Button6"] {
        w.hide_c(FORM5, b);
    }
}

/// @0047e044
pub fn show_buttons(w: &mut World) {
    enable_buttons(w);
    for b in ["Button1", "Button2", "Button3", "Button4", "Button5", "Button6"] {
        w.show_c(FORM5, b);
    }
}

/// @0047e094
pub fn enable_buttons(w: &mut World) {
    let mut owns_mewtwo = false;
    for b in ["Button1", "Button2", "Button3", "Button4", "Button5", "Button6"] {
        w.set_enabled(FORM5, b, false);
    }
    let counts = [
        w.m.poke_balls(),
        w.m.great_balls(),
        w.m.ultra_balls(),
        w.m.master_balls(),
        w.m.potions(),
        w.m.super_potions(),
    ];
    for (k, n) in counts.into_iter().enumerate() {
        if n > 0 {
            w.set_enabled(FORM5, ["Button1", "Button2", "Button3", "Button4", "Button5", "Button6"][k], true);
        }
    }
    for i in 1..=w.lv_count(FORM5, "ListView1") {
        if w.m.pc_species(i) == 150 {
            owns_mewtwo = true;
        }
    }
    let balls = ["Button1", "Button2", "Button3", "Button4"];
    if owns_mewtwo && w.lv_image(FORM4, "ListView3", 0) == 149 {
        for b in balls {
            w.set_enabled(FORM5, b, false);
        }
    }
    if w.m.in_trainer_battle() || (w.m.in_wild_battle() && w.lv_count(FORM5, "ListView1") == 10000) {
        for b in balls {
            w.set_enabled(FORM5, b, false);
        }
    }
    if w.m.in_tournament() {
        w.set_enabled(FORM5, "Button5", false);
        w.set_enabled(FORM5, "Button6", false);
    }
}

/// @0047e280
pub fn freshen(w: &mut World) -> R<()> {
    let labels = [
        ("PokeBallsLbl", w.m.poke_balls()),
        ("GreatBallsLbl", w.m.great_balls()),
        ("UltraBallsLbl", w.m.ultra_balls()),
        ("MasterBallsLbl", w.m.master_balls()),
        ("PotionsLbl", w.m.potions()),
        ("SuperPotionsLbl", w.m.super_potions()),
        ("WinsLbl", w.m.wins()),
        ("LossesLbl", w.m.losses()),
        ("FightsLbl", w.m.fights()),
        ("CapturesLbl", w.m.captures()),
        ("MoneyLbl", w.m.money()),
    ];
    for (l, v) in labels {
        w.set_caption(FORM5, l, &v.to_string()); // IntToStr(Int64(Cardinal))
    }
    let sum = w.m.wins().wrapping_add(w.m.losses());
    if sum > 0 {
        // FILD Wins; FILD Sum; FDIVP; FMUL 10000.0; Int; FDIV 100.0; FSTP Double
        let d = F80::from_u32(w.m.wins())
            .div(F80::from_u32(sum))
            .mul(F80::from_f32(10000.0))
            .int()
            .div(F80::from_f32(100.0))
            .to_f64();
        w.set_caption(FORM5, "Percent", &float_to_str(d));
    }
    let mut n = 0;
    for i in 1..=251 {
        if w.m.owned(i) {
            n += 1;
        }
    }
    w.set_caption(FORM5, "OwnedPKMNLbl", &n.to_string());
    // if Form5.Focused and Form4.Visible then Form4.SetFocus — focus is not modelled.
    Ok(())
}

/// @0047e518
fn timer1_timer(w: &mut World) -> R<()> {
    if w.visible(FORM5) {
        freshen(w)?;
    }
    Ok(())
}

/// @0047e52c AddPKMN(Species, Name, Level)
pub fn add_pkmn(w: &mut World, species: i32, name: &str, level: u8) -> R<()> {
    if species == 150 {
        w.info(
            "NOTE: You can only catch one Mewtwo at a time. You must release the one(s) you already have if you want to catch another.",
        )?;
    }
    hide_buttons(w);
    w.hide_c(FORM4, "Button1");
    let mut idx = 1;
    if !w.ctl_visible(FORM4, "ListView1") {
        w.lv_add(FORM5, "ListView1");
        idx = w.lv_count(FORM5, "ListView1");
    }
    if !w.m.owned(species) {
        let s = crate::cat!(&w.m.pkmn_name(species), " has been added to the POKéDEX!")?;
        w.log(&s);
        w.show(FORM8)?;
        w.m.set_owned(species, true);
        w.set_item_index(FORM8, "ComboBox1", species - 1);
        super::unit8::update_display(w)?;
    }
    w.m.set_pc_species(idx, species);
    w.lv_set_caption(FORM5, "ListView1", idx - 1, name);
    w.lv_set_image(FORM5, "ListView1", idx - 1, species - 1);
    w.m.set_pc_level(idx, level);
    let l = level as i32;
    w.m.set_pc_exp(idx, l.wrapping_mul(l).wrapping_mul(l) as u32);
    Ok(())
}

/// Shared by the wild and trainer "I choose you!" blocks.
fn set_attack_list(w: &mut World, species: i32) -> R<()> {
    let s = crate::cat!(&w.m.pkmn_name(species), "'s attacks:")?;
    w.set_caption(FORM4, "RadioGroup1", &s);
    for i in 0..4 {
        let mv = w.m.learnset(species, i + 1) as i32;
        let s = crate::cat!(&w.m.move_name(mv), " (", &w.m.type_name(w.m.move_type(mv) as i32), ")")?;
        w.items_put(FORM4, "RadioGroup1", i, &s);
    }
    Ok(())
}

fn set_own_hp_display(w: &mut World, species: i32, idx: i32) -> R<()> {
    let s = crate::cat!(&w.m.pkmn_name(species), " L.", &w.m.pc_level(idx).to_string())?;
    w.set_caption(FORM4, "Label2", &s);
    let left = w.cr(FORM4, "Label2").width + 80;
    w.c(FORM4, "Gauge1").left = left;
    w.c(FORM4, "Gauge1").width = 289 - left;
    Ok(())
}

/// @0047e780 (bound to ListView1.OnDblClick)
fn list_view1_click(w: &mut World) -> R<()> {
    if w.item_index(FORM5, "ListView1") + 1 <= 0 {
        return Ok(());
    }
    let idx = w.item_index(FORM5, "ListView1") + 1;
    let mut rel_idx = 0;
    let mut to_next = 0i32;
    let species = w.lv_image(FORM5, "ListView1", idx - 1) + 1;
    let mut s = String::new();
    if species < 1000 {
        s = format!("# {species} ");
    }
    if species < 100 {
        s = format!("# 0{species} ");
    }
    if species < 10 {
        s = format!("# 00{species} ");
    }
    let lvl = w.m.pc_level(idx) as i32;
    if lvl < 100 {
        to_next = (lvl + 1).wrapping_mul(lvl + 1).wrapping_mul(lvl + 1).wrapping_sub(w.m.pc_exp(idx) as i32);
    }
    let msg = crate::cat!(
        &s,
        &w.m.pkmn_name(species),
        ", L.",
        &lvl.to_string(),
        ", XP:",
        &w.m.pc_exp(idx).to_string(),
        ", XP to next level:",
        &to_next.to_string()
    )?;
    w.info(&msg)?;

    // ---- choose a POKéMON for a WILD battle
    if w.m.in_wild_battle() {
        let q = crate::cat!("Choose ", &w.m.pkmn_name(species), "?")?;
        if confirm(w, &q)? {
            show_buttons(w);
            let s = crate::cat!(&w.m.pkmn_name(species), ", I choose you!")?;
            w.log(&s);
            w.set_enabled(FORM5, "ListView1", false);
            w.c(FORM5, "ListView1").color = CL_BTN_FACE;
            w.show_c(FORM4, "Button1");
            w.show_c(FORM4, "Gauge1");
            w.lv_set_image(FORM4, "ListView2", 0, species - 1);
            w.show_c(FORM4, "RadioGroup1");
            w.menu_enabled(FORM4, "File1", false);
            set_attack_list(w, species)?;
            set_own_hp_display(w, species, idx)?;
            let hp = (w.m.pc_level(idx) as i32) * 10;
            w.m.set_own_hp(hp);
            w.m.set_cur_pkmn(idx);
            w.set_caption(FORM4, "Label8", &format!("{hp}/{hp}"));
            w.set_gauge_max(FORM4, "Gauge1", hp);
            w.set_gauge_progress(FORM4, "Gauge1", hp);
            w.fx(Fx::SendOut { species, hp }); // presentation hook (no state)
            w.show_c(FORM4, "Label2");
            w.show_c(FORM4, "Label8");
            w.m.set_fights(w.m.fights().wrapping_add(1));
        }
    }

    // ---- choose a POKéMON for a TRAINER battle (level must be in [MinLevel..MaxLevel])
    let lv = w.m.pc_level(idx);
    if w.m.in_trainer_battle() && lv >= w.m.min_wild_level() && lv <= w.m.max_wild_level() {
        let q = crate::cat!("Choose ", &w.m.pkmn_name(species), "?")?;
        if confirm(w, &q)? {
            show_buttons(w);
            w.m.set_fights(w.m.fights().wrapping_add(1));
            w.menu_enabled(FORM4, "File1", false);
            let s = crate::cat!(&w.m.pkmn_name(species), ", I choose you!")?;
            w.log(&s);
            w.set_enabled(FORM5, "ListView1", false);
            w.c(FORM5, "ListView1").color = CL_BTN_FACE;
            w.show_c(FORM4, "Button1");
            w.lv_set_image(FORM4, "ListView2", 0, species - 1);
            w.show_c(FORM4, "RadioGroup1");
            w.show_c(FORM4, "Gauge1");
            set_attack_list(w, species)?;
            set_own_hp_display(w, species, idx)?;
            let hp = (w.m.pc_level(idx) as i32) * 10;
            w.m.set_own_hp(hp);
            w.set_caption(FORM4, "Label8", &format!("{hp}/{hp}"));
            w.set_gauge_max(FORM4, "Gauge1", hp);
            w.m.set_cur_pkmn(idx);
            w.set_gauge_progress(FORM4, "Gauge1", hp);
            w.fx(Fx::SendOut { species, hp }); // presentation hook (no state)
            w.show_c(FORM4, "Label2");
            w.show_c(FORM4, "Label8");
        }
    }

    // ---- EVOLUTION MODE
    if w.m.evolve_mode() {
        let mut handled = false;
        let ev = w.m.evolve_level(species);
        if ev == 0 {
            let m = crate::cat!(&w.m.pkmn_name(species), " can't evolve.")?;
            w.info(&m)?;
        } else if ev > w.m.pc_level(idx) {
            let m = crate::cat!(
                "Your ",
                &w.m.pkmn_name(species),
                " needs to grow ",
                &(ev as i32 - w.m.pc_level(idx) as i32).to_string(),
                " more level(s) before it can evolve."
            )?;
            w.info(&m)?;
        } else {
            let branches: &[(i32, &str, &[&str])] = &[
                (44, "Gloom", &["Vileplume", "Bellossom"]),
                (61, "Poliwhirl", &["Poliwrath", "Politoed"]),
                (79, "Slowpoke", &["Slowbro", "Slowking"]),
                (133, "Eevee", &["Vaporeon", "Jolteon", "Flareon", "Espeon", "Umbreon"]),
                (236, "Tyrouge", &["Hitmonlee", "Hitmonchan", "Hitmontop"]),
                (265, "Wurmple", &["Silcoon", "Cascoon"]),
                (366, "Clamperl", &["Huntail", "Gorebyss"]),
            ];
            if let Some((_, pre, choices)) = branches.iter().find(|(sp, _, _)| *sp == species) {
                w.info(&format!("Please select what you would like to evolve your {pre} to."))?;
                w.items_add(FORM4, "EvolveChoice", "Don't evolve");
                for c in choices.iter() {
                    w.items_add(FORM4, "EvolveChoice", c);
                }
                w.set_item_index(FORM4, "EvolveChoice", 0);
                w.show_c(FORM4, "EvolveChoice");
                w.show_c(FORM4, "EvolveButton");
                w.m.set_evolve_slot(idx);
                w.m.set_g_4a8c70(rel_idx);
                w.set_enabled(FORM5, "ListView1", false);
                w.c(FORM5, "ListView1").color = CL_BTN_FACE;
                handled = true;
            } else if species == 290 {
                let q = crate::cat!(
                    "Your ",
                    &w.m.pkmn_name(species),
                    " can evolve into ",
                    &w.m.pkmn_name(w.m.evolve_to(species)),
                    ". Do you want to evolve it? NOTE: Nincada will evolve into Ninjask AND Shedinja every 3 of 4 times."
                )?;
                if confirm(w, &q)? {
                    w.m.set_pc_species(idx, 291);
                    let n = w.m.pkmn_name(291).refd(fault::LSTR_ASG)?.to_string();
                    w.lv_set_caption(FORM5, "ListView1", idx - 1, &n);
                    w.lv_set_image(FORM5, "ListView1", idx - 1, 290);
                    w.m.set_owned(291, true);
                    let m = crate::cat!(&w.m.pkmn_name(species), " evolved into ", &w.m.pkmn_name(w.m.pc_species(idx)), "!")?;
                    w.info(&m)?;
                    handled = true;
                    let r = w.rng.random(4);
                    if F80::from_i64(r as i64).int().is_zero() {
                        w.info("Shedinja didn't appear.")?;
                    } else {
                        w.info("Nincada also evolved into Shedinja!")?;
                        let l = w.m.pc_level(idx);
                        add_pkmn(w, 292, "Shedinja", l)?;
                    }
                }
            }
            if !handled {
                let to = w.m.evolve_to(species);
                let q = crate::cat!(
                    "Your ",
                    &w.m.pkmn_name(species),
                    " can evolve into ",
                    &w.m.pkmn_name(to),
                    ". Do you want to evolve it?"
                )?;
                if confirm(w, &q)? {
                    w.m.set_pc_species(idx, to);
                    let n = w.m.pkmn_name(to).refd(fault::LSTR_ASG)?.to_string();
                    w.lv_set_caption(FORM5, "ListView1", idx - 1, &n);
                    w.lv_set_image(FORM5, "ListView1", idx - 1, to - 1);
                    w.m.set_owned(to, true);
                    let m = crate::cat!(&w.m.pkmn_name(species), " evolved into ", &w.m.pkmn_name(w.m.pc_species(idx)), "!")?;
                    w.info(&m)?;
                }
            }
        }
        if rel_idx == 0 {
            w.menu_checked(FORM4, "Evolve1", false);
            w.menu_enabled(FORM4, "Tournaments1", true);
            w.m.set_evolve_mode(false);
            for m in ["ReleasePKMN1", "FightTrainer1", "Buy1", "FightWild1"] {
                w.menu_enabled(FORM4, m, true);
            }
        }
    }

    // ---- RELEASE MODE
    if w.m.release_mode() {
        let q = crate::cat!(
            "Once released, ",
            &w.m.pkmn_name(species),
            " will not come back. Release ",
            &w.m.pkmn_name(species),
            "?"
        )?;
        if confirm(w, &q)? {
            rel_idx = w.item_index(FORM5, "ListView1");
            w.lv_delete(FORM5, "ListView1", rel_idx);
            w.m.set_pc_species(idx, 0);
            w.m.set_pc_level(idx, 0);
            w.m.set_pc_exp(idx, 0);
            if w.m.pc_species(rel_idx + 2) > 0 {
                let count = w.lv_count(FORM5, "ListView1");
                for i in (rel_idx + 1)..=(count + 1) {
                    w.m.set_pc_species(i, w.m.pc_species(i + 1));
                    w.m.set_pc_level(i, w.m.pc_level(i + 1));
                    w.m.set_pc_exp(i, w.m.pc_exp(i + 1));
                }
            }
            let m = crate::cat!(&w.m.pkmn_name(species), " was released. Bye, ", &w.m.pkmn_name(species), "!")?;
            w.info(&m)?;
        }
        w.menu_checked(FORM4, "ReleasePKMN1", false);
        w.m.set_release_mode(false);
        for m in ["Tournaments1", "Evolve1", "FightTrainer1", "Buy1", "FightWild1"] {
            w.menu_enabled(FORM4, m, true);
        }
    }
    Ok(())
}

/// @00480510 Potion (+50) / @004806bc Super Potion (+100)
fn potion_click(w: &mut World, amount: i32, sup: bool) -> R<()> {
    let old = w.m.own_hp();
    let cur = w.m.cur_pkmn();
    let max = (w.m.pc_level(cur) as i32) * 10;
    if max == w.m.own_hp() {
        w.log("It won't have any effect.");
    }
    if max > w.m.own_hp() {
        if sup {
            w.m.set_super_potions(w.m.super_potions().wrapping_sub(1));
        } else {
            w.m.set_potions(w.m.potions().wrapping_sub(1));
        }
        w.m.set_own_hp(w.m.own_hp().wrapping_add(amount));
        enable_buttons(w);
        if max < w.m.own_hp() {
            w.m.set_own_hp(max);
        }
        let s = crate::cat!(&w.m.pkmn_name(w.m.pc_species(cur)), " recovered ", &(w.m.own_hp() - old).to_string(), " HP!")?;
        w.log(&s);
        let hp = w.m.own_hp();
        w.fx(Fx::Hp { side: Side::Own, hp, max }); // presentation hook (no state)
        super::unit4::do_opp_attack(w)?;
    }
    Ok(())
}

/// @00480868 Master Ball: always catches.
fn button4_click(w: &mut World) -> R<()> {
    w.log("You threw a Master Ball.");
    w.fx(Fx::BallThrow); // presentation hook (no state)
    w.fx(Fx::Caught(true));
    let sp = w.m.enemy_species();
    let s = crate::cat!("You caught ", &w.m.pkmn_name(sp), "!")?;
    w.log(&s);
    let name = w.m.pkmn_name(sp).refd(fault::LSTR_ADDREF)?.to_string();
    let lvl = w.m.enemy_level();
    add_pkmn(w, sp, &name, lvl)?;
    w.m.set_master_balls(w.m.master_balls().wrapping_sub(1));
    w.m.set_battle_code(1);
    w.show_c(FORM4, "EndButton");
    Ok(())
}

enum Ball {
    Poke,
    Great,
    Ultra,
}

/// @00480990 / @00480b88 / @00480d80
fn ball_click(w: &mut World, throw: &str, n: i32, ball: Ball) -> R<()> {
    w.log(throw);
    w.fx(Fx::BallThrow); // presentation hook (no state)
    match ball {
        Ball::Poke => w.m.set_poke_balls(w.m.poke_balls().wrapping_sub(1)),
        Ball::Great => w.m.set_great_balls(w.m.great_balls().wrapping_sub(1)),
        Ball::Ultra => w.m.set_ultra_balls(w.m.ultra_balls().wrapping_sub(1)),
    }
    let r = w.rng.random(n);
    // FILD r; FILD CatchValue; FADD 0.5; FSUBP; Int; compare with 0.0
    let v = F80::from_i64(r as i64)
        .sub(F80::from_i64(w.m.enemy_hp() as i64).add(F80::from_f32(0.5)))
        .int();
    let sp = w.m.enemy_species();
    w.fx(Fx::Caught(v.cmp(F80::ZERO).is_gt())); // presentation hook (no state)
    if v.cmp(F80::ZERO).is_gt() {
        let s = crate::cat!("You caught ", &w.m.pkmn_name(sp), "!")?;
        w.log(&s);
        let name = w.m.pkmn_name(sp).refd(fault::LSTR_ADDREF)?.to_string();
        let lvl = w.m.enemy_level();
        add_pkmn(w, sp, &name, lvl)?;
        w.m.set_battle_code(1);
        w.show_c(FORM4, "EndButton");
    }
    if !w.ctl_visible(FORM4, "EndButton") {
        let s = crate::cat!("Sorry, ", &w.m.pkmn_name(sp), " wasn't caught.")?;
        w.log(&s);
        super::unit4::do_opp_attack(w)?;
    }
    enable_buttons(w);
    Ok(())
}

pub fn reset_list_color(w: &mut World) {
    w.c(FORM5, "ListView1").color = CL_WINDOW;
}
