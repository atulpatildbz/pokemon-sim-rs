//! Unit4 — TForm4, the main game window: battles, trainers, gyms, tournaments, evolution, export,
//! music. Port of recon/Unit4_a.pas, Unit4_b.pas, Unit4_c.pas and Unit4_d.pas.

use super::*;
use crate::delphi::{Var, F80};
use crate::game::mem::{fault, DStr};
use crate::game::textfile::TextWriter;
use crate::vcl::model::CL_WINDOW;

const CL_GREEN: u32 = 0x8000;
const CL_YELLOW: u32 = 0xFFFF;
const CL_RED: u32 = 0xFF;

pub fn dispatch(w: &mut World, h: &str) -> Option<R<()>> {
    if let Some(t) = TRAINERS.iter().find(|t| t.handler == h) {
        return Some(fixed_trainer(w, t));
    }
    if let Some(&(_, lo, hi, fee, tier)) = TIERS.iter().find(|t| t.0 == h) {
        return Some(tier_click(w, lo, hi, fee, tier));
    }
    Some(match h {
        "FormClose" => form_close(w),
        "About1Click" => w.show_modal(ABOUT_BOX).map(|_| ()),
        "Quit1Click" => w.close(FORM4),
        "Contents1Click" => super::unit1::help_button_click(w),
        "ListView1Click" => list_view1_click(w),
        "ReleasePKMN1Click" => release_pkmn1_click(w),
        "ViewDEX1Click" => w.show(FORM8),
        "Evolve1Click" => evolve1_click(w),
        "EvolveButtonClick" => evolve_button_click(w),
        "Buy1Click" => w.show(FORM9),
        "FindPKMN1Click" => find_pkmn1_click(w),
        "EndButtonClick" => end_battle(w),
        "FormShow" => form_show(w),
        "Button4Click" => button4_click(w),
        "Button3Click" => button3_click(w),
        "Button2Click" => button2_click(w),
        "Mewtwo1Click" => start_wild_battle(w, 150, 100),
        "Openfile1Click" => openfile1_click(w),
        "RepeatMusic1Click" => toggle_check(w, "RepeatMusic1"),
        "StopMusic1Click" => {
            media_stop(w);
            media_rewind(w);
            Ok(())
        }
        "Timer1Timer" => timer1_timer(w),
        "MakeVisible1Click" => {
            w.menu_enabled(FORM4, "MakeVisible1", false);
            Ok(())
        }
        "AlwaysshowattacksinPOKDEX1Click" => toggle_check(w, "AlwaysshowattacksinPOKDEX1"),
        "ExportPOKMONinformation1Click" => export_click(w),
        "TypeComparisons1Click" => w.show(FORM_A),
        "GameCorner1Click" => w.show(FORM_B),
        "Button1Click" => button1_click(w),
        "DoOppAttack" => do_opp_attack(w),
        "DoOwnAttack" => do_own_attack(w),
        "EndBattle" => end_battle(w),
        "HandleTournament" => handle_tournament(w),
        "EnableGymLeaders" => {
            enable_gym_leaders(w);
            Ok(())
        }
        _ => return None,
    })
}

fn toggle_check(w: &mut World, item: &str) -> R<()> {
    let c = w.is_menu_checked(FORM4, item);
    w.menu_checked(FORM4, item, !c);
    Ok(())
}

/// Shared by FormClose and Button1Click (Run in a tournament): lose Int(exp/20 + 0.5) exp and
/// level down while level^3 > exp.
fn quit_tournament_penalty(w: &mut World, slot: i32) -> R<()> {
    let lost = Var::real(
        F80::from_u32(w.m.pc_exp(slot)).div(F80::from_f32(20.0)).add(F80::from_f32(0.5)).int(),
    );
    let sp = w.m.pc_species(slot);
    let s = crate::cat!(
        "Quit the tournament! ",
        &w.m.pkmn_name(sp),
        " lost ",
        &lost.to_int().to_string(),
        " experience points!"
    )?;
    w.log(&s);
    let new_exp = Var::longword(w.m.pc_exp(slot)).sub(lost);
    w.m.set_pc_exp(slot, new_exp.to_i64() as u32);
    let cube = |l: u8| (l as u32).wrapping_mul(l as u32).wrapping_mul(l as u32);
    let mut done = w.m.pc_exp(slot) >= cube(w.m.pc_level(slot));
    while !done {
        w.m.set_pc_level(slot, w.m.pc_level(slot).wrapping_sub(1));
        if cube(w.m.pc_level(slot)) <= w.m.pc_exp(slot) {
            done = true;
        }
        let s = crate::cat!(&w.m.pkmn_name(w.m.pc_species(slot)), " lost a level!")?;
        w.log(&s);
    }
    Ok(())
}

// ===================================================================================== part A

/// @0048435c (OnClose)
fn form_close(w: &mut World) -> R<()> {
    if w.m.in_tournament() {
        let slot = w.m.cur_pkmn();
        quit_tournament_penalty(w, slot)?;
        w.m.set_battle_code(0);
        w.m.set_in_tournament(false);
        super::unit5::hide_buttons(w);
        w.show_c(FORM4, "EndButton");
        w.hide_c(FORM4, "Button1");
    }
    if !w.ctl_visible(FORM4, "ListView1") {
        save(w)?;
    }
    w.close(FORM3)
}

/// @004845e8
fn save(w: &mut World) -> R<()> {
    let file = w.cr(FORM4, "SaveDialog1").file_name.clone();
    let mut f = TextWriter::create(&file)?;
    f.write_bytes(&save_contents(w));
    f.close()?;
    w.c(FORM4, "SaveDialog1").file_name.clear(); // yes, the original clears it
    Ok(())
}

/// The save file's bytes, exactly as TForm4.Save writes them. Reads state only (`&World`), so
/// the front-end's auto-save can call it at any safe point without affecting the game.
pub fn save_contents(w: &World) -> Vec<u8> {
    let mut f = TextWriter::memory();
    f.writeln("VERSION2SAVEFILEIDENTIFIER");
    f.writeln(&w.m.player_name());
    for b in [w.m.beat_lance(), w.m.beat_ash(), w.m.beat_me(), w.m.beat_rby_champion(), w.m.beat_rusa_champion()] {
        f.writeln_bool(b);
    }
    f.writeln(&w.m.badges().to_string());
    for v in [
        w.m.money(),
        w.m.wins(),
        w.m.losses(),
        w.m.fights(),
        w.m.captures(),
        w.m.poke_balls(),
        w.m.great_balls(),
        w.m.ultra_balls(),
        w.m.master_balls(),
        w.m.potions(),
        w.m.super_potions(),
    ] {
        f.writeln(&v.to_string());
    }
    for i in 1..=386 {
        f.writeln_bool(w.m.owned(i));
    }
    for i in 1..=10000 {
        f.writeln(&w.m.pc_species(i).to_string());
        f.writeln(&w.m.pc_level(i).to_string());
        f.writeln(&w.m.pc_exp(i).to_string());
    }
    f.into_bytes()
}

/// @00484a04 — starter selection.
fn list_view1_click(w: &mut World) -> R<()> {
    let ii = w.item_index(FORM4, "ListView1");
    if ii >= 0 {
        let species = w.lv_image(FORM4, "ListView1", ii) + 1;
        let cap = w.lv_caption(FORM4, "ListView1", ii);
        super::unit5::add_pkmn(w, species, &cap, 5)?;
        w.hide_c(FORM4, "Label1");
        w.hide_c(FORM4, "ListView1");
        w.menu_enabled(FORM4, "File1", true);
        w.menu_enabled(FORM4, "Action1", true);
        w.show(FORM5)?;
        w.show(FORM4)?;
    }
    Ok(())
}

const MODE_ITEMS: [&str; 5] = ["Tournaments1", "FightTrainer1", "Buy1", "FightWild1", ""];

/// @00484abc
fn release_pkmn1_click(w: &mut World) -> R<()> {
    if w.lv_count(FORM5, "ListView1") <= 1 {
        return warning(w, "You can't release your last POKéMON!");
    }
    let on = !w.m.release_mode();
    w.menu_checked(FORM4, "ReleasePKMN1", on);
    w.m.set_release_mode(on);
    w.menu_enabled(FORM4, "Evolve1", !on);
    for m in MODE_ITEMS.iter().filter(|m| !m.is_empty()) {
        w.menu_enabled(FORM4, m, !on);
    }
    if on {
        w.info("Doubleclick a POKéMON in the Statistics window to release it. Once you have doubleclicked on a POKéMON, Release Mode will be disabled.")
    } else {
        w.info("Release Mode deactivated. Statistics window will now function normally.")
    }
}

/// @00484cf8
fn evolve1_click(w: &mut World) -> R<()> {
    let on = !w.m.evolve_mode();
    w.menu_checked(FORM4, "Evolve1", on);
    w.m.set_evolve_mode(on);
    w.menu_enabled(FORM4, "ReleasePKMN1", !on);
    for m in MODE_ITEMS.iter().filter(|m| !m.is_empty()) {
        w.menu_enabled(FORM4, m, !on);
    }
    if on {
        w.info("Doubleclick a POKéMON in the Statistics window to evolve it. Once you have doubleclicked on a POKéMON, Evolution Mode will be disabled.")
    } else {
        w.info("Evolution Mode cancelled.")
    }
}

/// @00484e94 — confirm a branched evolution chosen in EvolveChoice.
fn evolve_button_click(w: &mut World) -> R<()> {
    let mut new_species = 0;
    let mut prev = String::new();
    w.set_enabled(FORM5, "ListView1", true);
    if w.item_index(FORM4, "EvolveChoice") > 0 {
        let first = w.item(FORM4, "EvolveChoice", 1);
        for (choice, pre) in [
            ("Vileplume", "Gloom"),
            ("Poliwrath", "Poliwhirl"),
            ("Slowbro", "Slowpoke"),
            ("Vaporeon", "Eevee"),
            ("Hitmonlee", "Tyrouge"),
        ] {
            if first == choice {
                prev = pre.to_string();
            }
        }
        let chosen = w.item(FORM4, "EvolveChoice", w.item_index(FORM4, "EvolveChoice"));
        for i in 1..=386 {
            if w.m.pkmn_name(i) == DStr::S(chosen.clone()) {
                new_species = i;
            }
        }
        let slot = w.m.evolve_slot();
        w.m.set_pc_species(slot, new_species);
        w.m.set_owned(new_species, true);
        let n = w.m.pkmn_name(new_species).refd(fault::LSTR_ASG)?.to_string();
        w.lv_set_caption(FORM5, "ListView1", slot - 1, &n);
        w.lv_set_image(FORM5, "ListView1", slot - 1, new_species - 1);
        let m = crate::cat!(&prev, " evolved into ", &w.m.pkmn_name(w.m.pc_species(slot)), "!")?;
        w.info(&m)?;
    }
    if w.item_index(FORM4, "EvolveChoice") == 0 {
        w.info("Evolution cancelled.")?;
    }
    w.hide_c(FORM4, "EvolveChoice");
    w.hide_c(FORM4, "EvolveButton");
    w.items_clear(FORM4, "EvolveChoice");
    w.c(FORM5, "ListView1").color = CL_WINDOW;
    Ok(())
}

/// Labels 4..7 = the enemy's four moves; Gauge2 placed after Label3.
fn show_enemy_moves(w: &mut World, species: i32) -> R<()> {
    for (k, l) in [(1, "Label4"), (2, "Label5"), (3, "Label6"), (4, "Label7")] {
        let s = w.m.move_name(w.m.learnset(species, k) as i32);
        w.set_caption(FORM4, l, s.refd(fault::LSTR_ASG)?);
    }
    let left = w.cr(FORM4, "Label3").width + 80;
    w.c(FORM4, "Gauge2").left = left;
    w.c(FORM4, "Gauge2").width = 289 - left;
    Ok(())
}

/// @00485280 StartWildBattle(Species, Level)
pub fn start_wild_battle(w: &mut World, species: i32, level: i32) -> R<()> {
    for c in ["ComboBox1", "SpinEdit1", "Button2", "Button3", "Button4", "Label10", "Label11"] {
        w.hide_c(FORM4, c);
    }
    w.show_c(FORM4, "GroupBox1");
    w.m.set_in_wild_battle(true);
    w.show(FORM7)?;
    w.menu_enabled(FORM4, "Action1", false);
    let hp = level.wrapping_mul(10);
    w.m.set_enemy_hp(hp);
    w.set_caption(FORM4, "Label9", &format!("{hp}/{hp}"));
    w.set_gauge_max(FORM4, "Gauge2", hp);
    w.m.set_enemy_level(level as u8);
    w.set_gauge_progress(FORM4, "Gauge2", hp);
    w.fx(Fx::BattleStart { species, hp }); // presentation hook (no state)
    let s = crate::cat!("Wild ", &w.m.pkmn_name(species), " appeared!")?;
    w.log(&s);
    let s = crate::cat!("Enemy ", &w.m.pkmn_name(species), " L.", &level.to_string())?;
    w.set_caption(FORM4, "Label3", &s);
    w.lv_set_image(FORM4, "ListView3", 0, species - 1);
    let s = crate::cat!("Enemy ", &w.m.pkmn_name(species), "'s Attacks:")?;
    w.set_caption(FORM4, "GroupBox2", &s);
    w.info("Select a POKéMON from the Statistics Window. You'll be asked to confirm your choice.")?;
    w.hide_c(FORM4, "RadioGroup1");
    show_enemy_moves(w, species)?;
    w.m.set_enemy_species(species);
    if !w.m.owned(species) {
        w.hide_c(FORM4, "Image1");
    } else {
        w.show_c(FORM4, "Image1");
    }
    Ok(())
}

/// @0048561c StartTrainerBattle(Species, Level, TrainerName)
pub fn start_trainer_battle(w: &mut World, species: i32, level: i32, trainer: &DStr) -> R<()> {
    w.menu_enabled(FORM4, "Action1", false);
    w.m.set_enemy_level(level as u8);
    w.m.set_enemy_species(species);
    w.show_c(FORM4, "GroupBox1");
    w.m.set_in_trainer_battle(true);
    let hp = level.wrapping_mul(10);
    w.m.set_enemy_hp(hp);
    w.set_caption(FORM4, "Label9", &format!("{hp}/{hp}"));
    w.set_gauge_max(FORM4, "Gauge2", hp);
    w.set_gauge_progress(FORM4, "Gauge2", hp);
    w.fx(Fx::BattleStart { species, hp }); // presentation hook (no state)
    w.show(FORM7)?;
    let s = crate::cat!(trainer, " wants to fight!")?;
    w.log(&s);
    let s = crate::cat!(trainer, " sent out ", &w.m.pkmn_name(species), "!")?;
    w.log(&s);
    let s = crate::cat!("Enemy ", &w.m.pkmn_name(species), " L.", &level.to_string())?;
    w.set_caption(FORM4, "Label3", &s);
    w.lv_set_image(FORM4, "ListView3", 0, species - 1);
    let s = crate::cat!("Enemy ", &w.m.pkmn_name(species), "'s Attacks:")?;
    w.set_caption(FORM4, "GroupBox2", &s);
    w.hide_c(FORM4, "RadioGroup1");
    w.info("Select a POKéMON from the Statistics Window. You'll be asked to confirm your choice.")?;
    show_enemy_moves(w, species)
}

/// @004859a4 "Search for a POKéMON"
fn find_pkmn1_click(w: &mut World) -> R<()> {
    let mut has_mewtwo = false;
    for i in 1..=w.lv_count(FORM5, "ListView1") {
        if w.m.pc_species(i) == 150 {
            has_mewtwo = true;
        }
    }
    if !has_mewtwo {
        let mut ok = false;
        let mut sp = Var::Empty;
        let badges = w.m.badges();
        while !ok {
            sp = rand_plus(w, 386, 1.0);
            ok = true;
            let is = |v: i32| sp.eq(Var::integer(v));
            if badges <= 3 {
                for i in 1..=386 {
                    if is(w.m.evolve_to(i)) {
                        ok = false;
                    }
                }
            }
            if badges <= 3 && [45, 62, 80, 182, 186, 199, 266, 268, 292, 367, 368].iter().any(|&v| is(v)) {
                ok = false;
            }
            if badges <= 4 && [106, 107, 237].iter().any(|&v| is(v)) {
                ok = false;
            }
            if badges <= 8 && is(143) {
                ok = false;
            }
            let between = |a: i32, b: i32| sp.ge(Var::integer(a)) && sp.le(Var::integer(b));
            if badges <= 10 && between(243, 245) {
                ok = false;
            }
            if badges <= 12 && between(144, 146) {
                ok = false;
            }
            if badges <= 13 && (is(249) || is(250)) {
                ok = false;
            }
            if badges <= 19 && between(382, 384) {
                ok = false;
            }
            if badges <= 21 && (is(380) || is(381)) {
                ok = false;
            }
            if [134, 135, 136, 150, 151, 196, 197, 251, 385, 386].iter().any(|&v| is(v)) {
                ok = false;
            }
        }
        // Level cascade: every executed Random() consumes the RNG, later ones overwrite.
        let mut lv = rand_plus(w, 30, 71.0);
        if !w.m.owned(150) {
            lv = rand_plus(w, 30, 61.0);
        }
        if !w.m.beat_ash() {
            lv = rand_plus(w, 20, 61.0);
        }
        if badges <= 14 {
            lv = rand_plus(w, 30, 40.0);
        }
        if badges <= 12 {
            lv = rand_plus(w, 20, 40.0);
        }
        if badges <= 10 {
            lv = rand_plus(w, 20, 30.0);
        }
        if badges <= 8 {
            lv = rand_plus(w, 10, 30.0);
        }
        if badges <= 6 {
            lv = rand_plus(w, 10, 20.0);
        }
        if badges <= 4 {
            lv = rand_plus(w, 19, 2.0);
        }
        if badges <= 2 {
            lv = rand_plus(w, 9, 2.0);
        }
        w.m.set_battle_code(2);
        let level = lv.to_int();
        start_wild_battle(w, sp.to_int(), level)?;
    }
    if has_mewtwo {
        w.menu_enabled(FORM4, "Action1", false);
        for c in ["ComboBox1", "SpinEdit1", "Button2", "Button3", "Button4", "Label10", "Label11"] {
            w.show_c(FORM4, c);
        }
        w.set_item_index(FORM4, "ComboBox1", -1);
        w.set_spin_value(FORM4, "SpinEdit1", 100)?;
        w.info("Using Mewtwo's psychic powers, you can fight any POKéMON you want to, and at any level. Please select which POKéMON to fight against, or click Random to face a random POKéMON.")?;
    }
    Ok(())
}

// ===================================================================================== part B

/// Level normalisation loop shared by HandleTournament and EndBattle.
fn normalise_levels(w: &mut World) {
    for i in 1..=w.lv_count(FORM5, "ListView1") {
        loop {
            let l = w.m.pc_level(i).wrapping_add(1) as u32;
            if l.wrapping_mul(l).wrapping_mul(l) <= w.m.pc_exp(i) {
                w.m.set_pc_level(i, w.m.pc_level(i).wrapping_add(1));
            } else {
                break;
            }
        }
        if w.m.pc_exp(i) > 1_000_000 {
            w.m.set_pc_exp(i, 1_000_000);
            w.m.set_pc_level(i, 100);
        }
    }
}

/// @0048633c — a tournament round was won.
fn handle_tournament(w: &mut World) -> R<()> {
    let code = w.m.battle_code() as i32;
    let round = code % 10;
    if round < 8 {
        let s = crate::cat!(
            "Congratulations! You beat ",
            &w.m.trainer_name(w.m.tour_trainer(round) as i32),
            "! Next up is trainer #",
            &(round + 1).to_string(),
            ", ",
            &w.m.trainer_name(w.m.tour_trainer(round + 1) as i32),
            " with a L.",
            &w.m.tour_level(round + 1).to_string(),
            " ",
            &w.m.pkmn_name(w.m.tour_pkmn(round + 1)),
            "."
        )?;
        w.info(&s)?;
        w.m.set_battle_code(w.m.battle_code().wrapping_add(1));
        let (sp, lv) = (w.m.tour_pkmn(round + 1), w.m.tour_level(round + 1) as i32);
        let tn = w.m.trainer_name(w.m.tour_trainer(round + 1) as i32);
        start_trainer_battle(w, sp, lv, &tn)?;
        w.m.set_in_tournament(true);
    }
    if round == 8 {
        let s = crate::cat!(
            "Congratulations! You beat ",
            &w.m.trainer_name(w.m.tour_trainer(round) as i32),
            "! You won the tournament! Now for the unveiling of your prize..."
        )?;
        w.info(&s)?;
        normalise_levels(w);
        w.m.set_min_wild_level(2);
        w.m.set_max_wild_level(100);
        w.m.set_in_tournament(false);
        let mut choice = rand_plus(w, 3, 1.0);
        let code = w.m.battle_code() as i64;
        let mut tier = Var::real(F80::from_i64(code - code % 10).div(F80::from_f32(10.0)));
        if w.lv_count(FORM5, "ListView1") == 10000 && tier.eq(Var::byte(6)) && choice.eq(Var::byte(3)) {
            choice = rand_plus(w, 2, 1.0);
        }
        let ci = choice.to_int();
        let prize = w.m.prize_text(tier.to_int(), ci);
        let s = crate::cat!("You won ", &prize, "!")?;
        w.info(&s)?;
        tier = tier.sub(Var::byte(1)).mul(Var::byte(3));
        tier = tier.add(choice);
        let n = (1..=18).find(|&k| tier.eq(Var::byte(k as u8))).unwrap_or(0);
        let m = &mut w.m;
        match n {
            1 => m.set_poke_balls(m.poke_balls().wrapping_add(3)),
            2 => m.set_potions(m.potions().wrapping_add(2)),
            3 => m.set_money(m.money().wrapping_add(2500)),
            4 => m.set_potions(m.potions().wrapping_add(3)),
            5 => m.set_poke_balls(m.poke_balls().wrapping_add(5)),
            6 => m.set_money(m.money().wrapping_add(4000)),
            7 => m.set_great_balls(m.great_balls().wrapping_add(3)),
            8 => m.set_ultra_balls(m.ultra_balls().wrapping_add(1)),
            9 => m.set_super_potions(m.super_potions().wrapping_add(1)),
            10 => m.set_super_potions(m.super_potions().wrapping_add(3)),
            11 => m.set_great_balls(m.great_balls().wrapping_add(5)),
            12 => m.set_money(m.money().wrapping_add(10000)),
            13 => m.set_ultra_balls(m.ultra_balls().wrapping_add(3)),
            14 => m.set_money(m.money().wrapping_add(15000)),
            15 => m.set_great_balls(m.great_balls().wrapping_add(10)),
            16 => m.set_master_balls(m.master_balls().wrapping_add(1)),
            17 => m.set_money(m.money().wrapping_add(30000)),
            _ => {}
        }
        if n == 18 {
            let species = rand_plus(w, 386, 1.0);
            let level = rand_plus(w, 99, 2.0);
            let lv = level.to_int();
            let name = w.m.pkmn_name(species.to_int()).refd(fault::LSTR_ADDREF)?.to_string();
            super::unit5::add_pkmn(w, species.to_int(), &name, lv as u8)?;
            let s = crate::cat!("You won a L.", &lv.to_string(), " ", &w.m.pkmn_name(species.to_int()), "!")?;
            w.info(&s)?;
        }
    }
    Ok(())
}

/// @00486be4 (also EndButtonClick @00489cc4)
fn end_battle(w: &mut World) -> R<()> {
    super::unit5::hide_buttons(w);
    let code = w.m.battle_code();
    if code == 0 {
        normalise_levels(w);
        if w.m.in_tournament() {
            w.m.set_losses(w.m.losses().wrapping_add(1));
        }
        w.m.set_min_wild_level(2);
        w.m.set_max_wild_level(100);
    }
    if code == 1 {
        w.m.set_captures(w.m.captures().wrapping_add(1));
    }
    if code >= 2 {
        w.m.set_wins(w.m.wins().wrapping_add(1));
    }
    if code == 3 {
        w.m.set_badges(w.m.badges().wrapping_add(1));
        let s = crate::cat!("You earned the ", &w.m.badge_name(w.m.badges() as i32), "!")?;
        w.info(&s)?;
        enable_gym_leaders(w);
        let b = w.m.badges();
        if b == 8 {
            w.info("You beat Gary, the final Kanto leader! You can now battle the Elite Four! Also, you get a 2500¥ bonus!")?;
            w.m.set_money(w.m.money().wrapping_add(2500));
        }
        if b == 16 {
            w.info("You beat Clair, the final Johto leader! You can now battle Ash! Also, you get a 3000¥ bonus!")?;
            w.m.set_money(w.m.money().wrapping_add(3000));
        }
        if b == 24 {
            w.info("You beat Wallace, the final Hoenn leader! You can now battle the R/B/Y and Ru/Sa Elites! Also, you get a 5000¥ bonus!")?;
            w.m.set_money(w.m.money().wrapping_add(5000));
        }
    }
    if w.m.battle_code() == 8 && !w.m.beat_lance() {
        w.m.set_beat_lance(true);
        w.info("You beat Lance, the Elite Four champion! You can now battle the Johto Gym Leaders! Also, you get a Master Ball!")?;
        w.m.set_master_balls(w.m.master_balls().wrapping_add(1));
        w.info("This Ball will catch any POKéMON at any HP. Use it wisely.")?;
        w.menu_enabled(FORM4, "JohtoGymLeaders1", true);
        w.m.set_beat_lance(true);
    }
    if w.m.battle_code() == 9 {
        w.info("You beat Ash! You can now battle me! Also, you get a Celebi at L.5!")?;
        super::unit5::add_pkmn(w, 251, "Celebi", 5)?;
        w.menu_enabled(FORM4, "Me1", true);
        w.m.set_beat_ash(true);
    }
    if w.m.battle_code() == 10 {
        w.info("You beat me! You can now battle and/or catch Mewtwo! You can also fight the Hoenn Gym Leaders! And as if that wasn't enough, you also get a Mew at L.10!")?;
        super::unit5::add_pkmn(w, 151, "Mew", 10)?;
        w.menu_enabled(FORM4, "Mewtwo1", true);
        w.menu_enabled(FORM4, "HoennGymLeaders1", true);
        w.m.set_beat_me(true);
    }

    // common battle teardown
    w.hide_c(FORM4, "EndButton");
    w.set_item_index(FORM4, "RadioGroup1", -1);
    w.m.set_in_trainer_battle(false);
    w.m.set_in_tournament(false);
    w.m.set_in_wild_battle(false);
    w.menu_enabled(FORM4, "File1", true);
    w.menu_enabled(FORM4, "Action1", true);
    w.hide_c(FORM4, "Gauge1");
    w.hide_c(FORM4, "Label8");
    w.hide(FORM7)?;
    w.hide_c(FORM4, "Label2");
    w.hide_c(FORM4, "GroupBox1");
    w.set_enabled(FORM5, "ListView1", true);
    w.set_gauge_fore(FORM4, "Gauge1", CL_GREEN);
    w.set_gauge_fore(FORM4, "Gauge2", CL_GREEN);
    w.lv_set_image(FORM4, "ListView2", 0, -1);
    w.hide_c(FORM4, "RadioGroup1");
    w.c(FORM5, "ListView1").color = CL_WINDOW;

    // Elite Four chains, checked top-down.
    if w.m.battle_code() == 0x55 {
        w.info("You beat Steven, the Ru/Sa Elite Four champion! You get a bonus of 7500¥!")?;
        w.m.set_money(w.m.money().wrapping_add(7500));
        if !w.m.beat_rusa_champion() {
            w.m.set_beat_rusa_champion(true);
            w.info("Also, you get a Deoxys at L.5!")?;
            super::unit5::add_pkmn(w, 386, "Deoxys", 5)?;
        }
    }
    let chain: [(u8, &str, i32, i32, &str, u8); 4] = [
        (0x54, "You have proven your worth against the Ru/Sa Elite Four. Now, it is time for you to fight the Ru/Sa Elite Four Champion: Steven!", 376, 100, "Steven", 0x55),
        (0x53, "Congratulations! You beat Glacia! Your next opponent is Drake.", 330, 100, "Drake", 0x54),
        (0x52, "Congratulations! You beat Phoebe! Your next opponent is Glacia.", 365, 100, "Glacia", 0x53),
        (0x51, "Congratulations! You beat Sidney! Your next opponent is Phoebe.", 354, 100, "Phoebe", 0x52),
    ];
    for (code, msg, sp, lv, name, next) in chain {
        if w.m.battle_code() == code {
            w.info(msg)?;
            start_trainer_battle(w, sp, lv, &DStr::from(name))?;
            w.m.set_battle_code(next);
        }
    }
    if w.m.battle_code() == 0x4B {
        w.info("You beat Gary, the R/B/Y Elite Four champion! You get a bonus of 7500¥!")?;
        w.m.set_money(w.m.money().wrapping_add(7500));
        if !w.m.beat_rby_champion() {
            w.m.set_beat_rby_champion(true);
            w.info("Also, you get a Jirachi at L.5!")?;
            super::unit5::add_pkmn(w, 385, "Jirachi", 5)?;
        }
    }
    let chain: [(u8, &str, i32, i32, &str, u8); 4] = [
        (0x4A, "You have proven your worth against the R/B/Y Elite Four. Now, it is time for you to fight the R/B/Y Elite Four Champion: Gary!", 65, 100, "Gary", 0x4B),
        (0x49, "Congratulations! You beat Agatha! Your next opponent is Lance.", 149, 100, "Lance", 0x4A),
        (0x48, "Congratulations! You beat Bruno! Your next opponent is Agatha.", 94, 100, "Agatha", 0x49),
        (0x47, "Congratulations! You beat Lorelei! Your next opponent is Bruno.", 95, 100, "Bruno", 0x48),
    ];
    for (code, msg, sp, lv, name, next) in chain {
        if w.m.battle_code() == code {
            w.info(msg)?;
            start_trainer_battle(w, sp, lv, &DStr::from(name))?;
            w.m.set_battle_code(next);
        }
    }
    if w.m.battle_code() == 7 {
        w.info("You have proven your worth against the G/S/C Elite Four. Now, it is time for you to fight the G/S/C Elite Four Champion: Lance!")?;
        start_trainer_battle(w, 149, 50, &DStr::from("Lance"))?;
        w.m.set_battle_code(8);
        if w.is_menu_enabled(FORM4, "JohtoGymLeaders1") {
            w.m.set_battle_code(2);
        }
    }
    let chain: [(u8, &str, i32, i32, &str, u8); 3] = [
        (6, "Congratulations! You beat Bruno! Your next opponent is Karen.", 94, 45, "Karen", 7),
        (5, "Congratulations! You beat Koga! Your next opponent is Bruno.", 95, 43, "Bruno", 6),
        (4, "Congratulations! You beat Will! Your next opponent is Koga.", 205, 43, "Koga", 5),
    ];
    for (code, msg, sp, lv, name, next) in chain {
        if w.m.battle_code() == code {
            w.info(msg)?;
            start_trainer_battle(w, sp, lv, &DStr::from(name))?;
            w.m.set_battle_code(next);
        }
    }
    let c = w.m.battle_code();
    if c > 10 && c < 70 {
        handle_tournament(w)?;
    }
    Ok(())
}

/// @00487c9c — case Badges of 1..24 (jump table @0x487cb4).
fn enable_gym_leaders(w: &mut World) {
    const ITEMS: [&str; 24] = [
        "Misty1", "LtSurge1", "Erika1", "Sabrina1", "Janine1", "Blaine1", "Gary1", "EliteFour1", "Bugsy1",
        "Whitney1", "Morty1", "Chuck1", "Jasmine1", "Pryce1", "Clair1", "Ash1", "Brawly1", "Wattson1",
        "Flannery1", "Norman1", "Winona1", "TateLiza1", "Wallace1", "Elites1",
    ];
    let b = w.m.badges() as usize;
    if (1..=24).contains(&b) {
        w.menu_enabled(FORM4, ITEMS[b - 1], true);
    }
}

/// The six effectiveness tests (FCOMP on Singles), in the original order.
fn effectiveness_messages(w: &mut World, eff1: f32, eff2: f32) {
    if eff1 == 2.0 && !(eff2 < 1.0) {
        w.log("It's super effective!");
    }
    if eff2 == 2.0 && eff1 == 1.0 {
        w.log("It's super effective!");
    }
    if eff1 == 0.5 && eff2 == 1.0 {
        w.log("It's not very effective...");
    }
    if eff1 == 0.5 && eff2 == 0.5 {
        w.log("It's not very effective...");
    }
    if eff1 == 1.0 && eff2 == 0.5 {
        w.log("It's not very effective...");
    }
}

/// HP / (Level*10) compared on the x87 (FILD; FILD; FDIVP; FCOMP const).
fn hp_ratio_gt(hp: i32, max: i32, c: f32) -> bool {
    F80::from_i64(hp as i64).div(F80::from_i64(max as i64)).cmp(F80::from_f32(c)).is_gt()
}

/// @00487e68 — the player's POKéMON attacks with the move selected in RadioGroup1.
pub fn do_own_attack(w: &mut World) -> R<()> {
    let cur = w.m.cur_pkmn();
    let own = w.m.pc_species(cur);
    let ii = w.item_index(FORM4, "RadioGroup1");
    let mv = w.m.learnset(own, ii + 1) as i32;
    let enemy = w.m.enemy_species();
    let atk = w.m.move_type(mv) as i32;
    let eff1 = w.m.type_eff(atk, w.m.pkmn_type1(enemy) as i32);
    let eff2 = w.m.type_eff(atk, w.m.pkmn_type2(enemy) as i32);
    let my_name = w.m.pkmn_name(own);
    // [RNG #1] crit roll, [RNG #2] spread — both before the "used" log line.
    let power = w.m.move_power(mv);
    let level = w.m.pc_level(cur);
    let crit_roll = rand_plus(w, 10, 1.0);
    let is_crit = crit_roll.eq(Var::byte(10));
    let r = w.rng.random(20) - 10;
    let mut dmg = Var::real(F80::from_i64(r as i64).add(
        F80::from_i64(power as i64).div(F80::from_f32(100.0)).mul(F80::from_i64(level as i64)),
    ));
    w.fx(Fx::Attack(Side::Own)); // presentation hook (no state)
    let s = crate::cat!(&my_name, " used ", &w.m.move_name(mv), "!")?;
    w.log(&s);
    if is_crit {
        w.log("Critical hit!");
        dmg = dmg.mul(Var::byte(2));
    }
    let t = dmg.mul(Var::real(F80::from_f32(eff1))).mul(Var::real(F80::from_f32(eff2)));
    dmg = Var::real(t.to_f80().int());
    if dmg.lt(Var::byte(1)) {
        dmg = Var::byte(1);
    }
    effectiveness_messages(w, eff1, eff2);
    if eff1 == 0.0 || eff2 == 0.0 {
        let s = crate::cat!("It doesn't affect ", &w.m.pkmn_name(enemy), "!")?;
        w.log(&s);
        dmg = Var::byte(0);
    }
    let s = crate::cat!(
        &my_name,
        " did ",
        &dmg.to_int().to_string(),
        " points of damage to enemy ",
        &w.m.pkmn_name(enemy),
        "."
    )?;
    w.log(&s);
    let hp = Var::integer(w.m.enemy_hp()).sub(dmg).to_int();
    w.m.set_enemy_hp(hp);
    w.set_gauge_progress(FORM4, "Gauge2", hp);
    w.set_gauge_fore(FORM4, "Gauge2", CL_GREEN);
    let max = (w.m.enemy_level() as i32) * 10;
    w.fx(Fx::Hp { side: Side::Enemy, hp, max }); // presentation hook (no state)
    if !hp_ratio_gt(hp, max, 0.5) {
        w.set_gauge_fore(FORM4, "Gauge2", CL_YELLOW);
    }
    if !hp_ratio_gt(hp, max, 0.25) {
        w.set_gauge_fore(FORM4, "Gauge2", CL_RED);
    }
    if w.m.enemy_hp() < 0 {
        w.m.set_enemy_hp(0);
    }
    let cap = format!("{}/{}0", w.m.enemy_hp(), w.m.enemy_level());
    w.set_caption(FORM4, "Label9", &cap);
    if w.m.enemy_hp() > 0 {
        return Ok(());
    }

    // ---------------- enemy fainted ----------------
    w.show_c(FORM4, "EndButton");
    super::unit5::hide_buttons(w);
    w.hide_c(FORM4, "Button1");
    w.fx(Fx::Faint(Side::Enemy)); // presentation hook (no state)
    let s = crate::cat!("Enemy ", &w.m.pkmn_name(enemy), " fainted!")?;
    w.log(&s);
    let el = w.m.enemy_level() as u32;
    let old = Var::integer(w.m.enemy_level() as i32 - 1);
    let cube_old = old.mul(old).mul(old);
    let gained = Var::longword(el.wrapping_mul(el).wrapping_mul(el)).sub(cube_old);
    let s = crate::cat!(&my_name, " gained ", &gained.to_int().to_string(), " experience points!")?;
    w.log(&s);
    let sum = el.wrapping_mul(el).wrapping_mul(el).wrapping_add(w.m.pc_exp(cur));
    let new_exp = Var::longword(sum).sub(old.mul(old).mul(old));
    w.m.set_pc_exp(cur, new_exp.to_i64() as u32);
    let old_level = Var::byte(w.m.pc_level(cur));
    let code = w.m.battle_code();
    if code <= 10 || code % 10 >= 8 {
        loop {
            let l = w.m.pc_level(cur).wrapping_add(1) as u32;
            if l.wrapping_mul(l).wrapping_mul(l) <= w.m.pc_exp(cur) {
                w.m.set_pc_level(cur, w.m.pc_level(cur).wrapping_add(1));
            } else {
                break;
            }
        }
    }
    if w.m.pc_exp(cur) > 1_000_000 {
        w.m.set_pc_level(cur, 100);
        w.m.set_pc_exp(cur, 1_000_000);
    }
    let new_level = w.m.pc_level(cur);
    if Var::byte(new_level).gt(old_level) {
        w.fx(Fx::LevelUp); // presentation hook (no state)
        let s = crate::cat!(&my_name, " grew to level ", &new_level.to_string(), "!")?;
        w.log(&s);
    }

    // ---------------- prize money ----------------
    let r = w.rng.random(3) + 1; // INC EAX before FILD
    let mut prize = Var::real(F80::from_i64(r as i64).int());
    if prize.eq(Var::byte(3)) {
        prize = Var::byte(4);
    }
    prize = prize.mul(Var::byte(250));
    let b = w.m.badges();
    if (4..8).contains(&b) {
        prize = prize.mul(Var::byte(2));
    }
    if (8..12).contains(&b) {
        prize = prize.mul(Var::byte(4));
    }
    if b >= 12 && !w.m.owned(150) {
        prize = prize.mul(Var::byte(6));
    }
    if w.m.owned(150) {
        prize = prize.mul(Var::byte(8));
    }
    let s = crate::cat!("You got ", &prize.to_int().to_string(), "¥!")?;
    w.log(&s);
    w.m.set_money(Var::longword(w.m.money()).add(prize).to_i64() as u32);
    Ok(())
}

/// @00488e68 — the enemy's turn.
pub fn do_opp_attack(w: &mut World) -> R<()> {
    let slot = rand_plus(w, 4, 0.0).add(Var::byte(1)).to_int();
    let enemy = w.m.enemy_species();
    let mv = w.m.learnset(enemy, slot) as i32;
    let cur = w.m.cur_pkmn();
    let own = w.m.pc_species(cur);
    let atk = w.m.move_type(mv) as i32;
    let eff1 = w.m.type_eff(atk, w.m.pkmn_type1(own) as i32);
    let eff2 = w.m.type_eff(atk, w.m.pkmn_type2(own) as i32);
    let power = w.m.move_power(mv);
    let level = w.m.enemy_level();
    let crit_roll = rand_plus(w, 10, 1.0);
    let is_crit = crit_roll.eq(Var::byte(10));
    let r = w.rng.random(20) - 10;
    let mut dmg = Var::real(F80::from_i64(r as i64).add(
        F80::from_i64(power as i64).div(F80::from_f32(100.0)).mul(F80::from_i64(level as i64)),
    ));
    w.fx(Fx::Attack(Side::Enemy)); // presentation hook (no state)
    let s = crate::cat!("Enemy ", &w.m.pkmn_name(enemy), " used ", &w.m.move_name(mv), "!")?;
    w.log(&s);
    if is_crit {
        w.log("Critical hit!");
        dmg = dmg.mul(Var::byte(2));
    }
    let t = dmg.mul(Var::real(F80::from_f32(eff1))).mul(Var::real(F80::from_f32(eff2)));
    dmg = Var::real(t.to_f80().int());
    if dmg.lt(Var::byte(1)) {
        dmg = Var::byte(1);
    }
    effectiveness_messages(w, eff1, eff2);
    if eff1 == 0.0 || eff2 == 0.0 {
        let s = crate::cat!("It doesn't affect ", &w.m.pkmn_name(own), "!")?;
        w.log(&s);
        dmg = Var::byte(0);
    }
    let s = crate::cat!(
        "Enemy ",
        &w.m.pkmn_name(enemy),
        " did ",
        &dmg.to_int().to_string(),
        " points of damage to ",
        &w.m.pkmn_name(own),
        "."
    )?;
    w.log(&s);
    let hp = Var::integer(w.m.own_hp()).sub(dmg).to_int();
    w.m.set_own_hp(hp);
    w.set_gauge_progress(FORM4, "Gauge1", hp);
    w.set_gauge_fore(FORM4, "Gauge1", CL_GREEN);
    let max = (w.m.pc_level(cur) as i32) * 10;
    w.fx(Fx::Hp { side: Side::Own, hp, max }); // presentation hook (no state)
    if !hp_ratio_gt(hp, max, 0.5) {
        w.set_gauge_fore(FORM4, "Gauge1", CL_YELLOW);
    }
    if !hp_ratio_gt(hp, max, 0.25) {
        w.set_gauge_fore(FORM4, "Gauge1", CL_RED);
    }
    if w.m.own_hp() < 0 {
        w.m.set_own_hp(0);
    }
    let cap = format!("{}/{}0", w.m.own_hp(), w.m.pc_level(cur));
    w.set_caption(FORM4, "Label8", &cap);
    if w.m.own_hp() > 0 {
        return Ok(());
    }

    // ---------------- own POKéMON fainted ----------------
    w.m.set_battle_code(0);
    super::unit5::hide_buttons(w);
    w.show_c(FORM4, "EndButton");
    w.hide_c(FORM4, "Button1");
    w.m.set_fights(w.m.fights().wrapping_add(1));
    w.m.set_losses(w.m.losses().wrapping_add(1));
    w.fx(Fx::Faint(Side::Own)); // presentation hook (no state)
    let s = crate::cat!(&w.m.pkmn_name(own), " fainted!")?;
    w.log(&s);
    let money = w.m.money();
    w.log(&format!("You lost {}¥!", money - (money >> 1)));
    w.m.set_money(money >> 1);
    Ok(())
}

/// @00489954 — "Attack" button (RadioGroup1: 0..3 attacks, 4 = Run).
fn button1_click(w: &mut World) -> R<()> {
    let i = w.item_index(FORM4, "RadioGroup1");
    if i < 4 && i + 1 > 0 {
        do_own_attack(w)?;
        if w.m.enemy_hp() > 0 {
            do_opp_attack(w)?;
        }
    }
    if w.item_index(FORM4, "RadioGroup1") == 4 {
        if w.m.in_trainer_battle() && !w.m.in_tournament() {
            w.log("No! There's no running from a trainer battle!");
        }
        if w.m.in_wild_battle() {
            w.log("Got away safely!");
            w.m.set_battle_code(0);
            super::unit5::hide_buttons(w);
            w.show_c(FORM4, "EndButton");
            w.hide_c(FORM4, "Button1");
        }
        if w.m.in_tournament() {
            let slot = w.m.cur_pkmn();
            quit_tournament_penalty(w, slot)?;
            w.m.set_battle_code(0);
            w.m.set_in_tournament(false);
            super::unit5::hide_buttons(w);
            w.show_c(FORM4, "EndButton");
            w.hide_c(FORM4, "Button1");
        }
    }
    Ok(())
}

// ===================================================================================== part C

/// @00489ccc (OnShow) — appends 386 entries every time (no Clear), like the original.
fn form_show(w: &mut World) -> R<()> {
    for i in 1..=386 {
        let s = super::unit8::numbered_name(w, i)?;
        w.items_add(FORM4, "ComboBox1", &s);
    }
    Ok(())
}

/// @00489db4 'Random'
fn button4_click(w: &mut World) -> R<()> {
    w.menu_enabled(FORM4, "Action1", true);
    let sp = rand_plus(w, 386, 1.0);
    let lv = rand_plus(w, 11, 90.0);
    w.m.set_battle_code(2);
    let level = lv.to_int();
    start_wild_battle(w, sp.to_int(), level)
}

/// @00489e80 'Cancel'
fn button3_click(w: &mut World) -> R<()> {
    w.menu_enabled(FORM4, "Action1", true);
    for c in ["ComboBox1", "SpinEdit1", "Button2", "Button3", "Button4", "Label10", "Label11"] {
        w.hide_c(FORM4, c);
    }
    Ok(())
}

/// @00489ee0 'Go!' (no ItemIndex = -1 guard: species 0, like the original)
fn button2_click(w: &mut World) -> R<()> {
    let sp = w.item_index(FORM4, "ComboBox1") + 1;
    let lv = w.spin_value(FORM4, "SpinEdit1");
    start_wild_battle(w, sp, lv)
}

/// Fixed opponents (recon/Unit4_c.pas table, verified entry-by-entry against the asm).
pub struct Trainer {
    pub handler: &'static str,
    pub name: &'static str,
    pub species: i32,
    pub level: i32,
    pub mode: u8,
    pub upgrade: Upgrade,
}

#[derive(Clone, Copy)]
pub enum Upgrade {
    None,
    /// BattleCode := 3 if Badges = n (gym that awards the next badge).
    Badge(u8),
    /// BattleCode := 2 if the flag is set (repeat win gives nothing).
    IfBeatAsh,
    IfBeatMe,
}

const fn t(handler: &'static str, name: &'static str, species: i32, level: i32, mode: u8, upgrade: Upgrade) -> Trainer {
    Trainer { handler, name, species, level, mode, upgrade }
}

pub const TRAINERS: [Trainer; 45] = [
    t("EliteFour1Click", "Will", 178, 40, 4, Upgrade::None),
    t("Ash1Click", "Ash", 25, 95, 9, Upgrade::IfBeatAsh),
    t("Falkner1Click", "Falkner", 17, 50, 2, Upgrade::Badge(8)),
    t("Bugsy1Click", "Bugsy", 123, 53, 2, Upgrade::Badge(9)),
    t("Whitney1Click", "Whitney", 241, 57, 2, Upgrade::Badge(10)),
    t("Morty1Click", "Morty", 94, 61, 2, Upgrade::Badge(11)),
    t("Chuck1Click", "Chuck", 62, 64, 2, Upgrade::Badge(12)),
    t("Jasmine1Click", "Jasmine", 208, 70, 2, Upgrade::Badge(13)),
    t("Pryce1Click", "Pryce", 87, 75, 2, Upgrade::Badge(14)),
    t("Clair1Click", "Clair", 230, 82, 2, Upgrade::Badge(15)),
    t("Brock1Click", "Brock", 74, 12, 2, Upgrade::Badge(0)),
    t("Misty1Click", "Misty", 120, 18, 2, Upgrade::Badge(1)),
    t("LtSurge1Click", "Lt. Surge", 26, 22, 2, Upgrade::Badge(2)),
    t("Erika1Click", "Erika", 45, 25, 2, Upgrade::Badge(3)),
    t("Sabrina1Click", "Sabrina", 49, 28, 2, Upgrade::Badge(4)),
    t("Janine1Click", "Janine", 89, 32, 2, Upgrade::Badge(5)),
    t("Blaine1Click", "Blaine", 126, 35, 2, Upgrade::Badge(6)),
    t("Gary1Click", "Gary", 34, 40, 2, Upgrade::Badge(7)),
    t("Red1Click", "Red", 4, 5, 2, Upgrade::None),
    t("Blue1Click", "Blue", 7, 5, 2, Upgrade::None),
    t("Green1Click", "Green", 1, 5, 2, Upgrade::None),
    t("Yellow1Click", "Yellow", 25, 5, 2, Upgrade::None),
    t("Gold1Click", "Gold", 152, 10, 2, Upgrade::None),
    t("Silver1Click", "Silver", 155, 10, 2, Upgrade::None),
    t("Crystal1Click", "Crystal", 158, 10, 2, Upgrade::None),
    t("ProfOak1Click", "Prof. Oak", 99, 50, 2, Upgrade::None),
    t("ProfElm1Click", "Prof. Elm", 172, 50, 2, Upgrade::None),
    t("Jesse1Click", "Jesse", 24, 75, 2, Upgrade::None),
    t("James1Click", "James", 110, 75, 2, Upgrade::None),
    t("Giovanni1Click", "Giovanni", 150, 100, 2, Upgrade::None),
    t("Me1Click", "Link", 151, 100, 10, Upgrade::IfBeatMe),
    t("RBYEliteFour1Click", "Lorelei", 131, 100, 0x47, Upgrade::None),
    t("RuSaEliteFour1Click", "Sidney", 262, 100, 0x51, Upgrade::None),
    t("Roxanne1Click", "Roxanne", 299, 100, 2, Upgrade::Badge(16)),
    t("Brawly1Click", "Brawly", 296, 100, 2, Upgrade::Badge(17)),
    t("Wattson1Click", "Wattson", 82, 100, 2, Upgrade::Badge(18)),
    t("Flannery1Click", "Flannery", 324, 100, 2, Upgrade::Badge(19)),
    t("Norman1Click", "Norman", 289, 100, 2, Upgrade::Badge(20)),
    t("Winona1Click", "Winona", 334, 100, 2, Upgrade::Badge(21)),
    t("TateLiza1Click", "Tate & Liza", 337, 100, 2, Upgrade::Badge(22)),
    t("Wallace1Click", "Wallace", 350, 100, 2, Upgrade::Badge(23)),
    t("Ruby1Click", "Ruby", 256, 25, 2, Upgrade::None),
    t("Sapphire1Click", "Sapphire", 259, 25, 2, Upgrade::None),
    t("Emerald1Click", "Emerald", 253, 25, 2, Upgrade::None),
    t("ProfBirch1Click", "Prof. Birch", 350, 50, 2, Upgrade::None),
];

fn fixed_trainer(w: &mut World, t: &Trainer) -> R<()> {
    start_trainer_battle(w, t.species, t.level, &DStr::from(t.name))?;
    w.m.set_battle_code(t.mode);
    match t.upgrade {
        Upgrade::None => {}
        Upgrade::Badge(n) => {
            if w.m.badges() == n {
                w.m.set_battle_code(3);
            }
        }
        Upgrade::IfBeatAsh => {
            if w.m.beat_ash() {
                w.m.set_battle_code(2);
            }
        }
        Upgrade::IfBeatMe => {
            if w.m.beat_me() {
                w.m.set_battle_code(2);
            }
        }
    }
    Ok(())
}


// ---- music (TMediaPlayer; audio output is done by the front-end)

fn media_stop(w: &mut World) {
    w.media.playing = false;
    w.media.command_seq += 1;
}
fn media_rewind(w: &mut World) {
    w.media.position = 0;
    w.media.command_seq += 1;
}
fn media_play(w: &mut World) {
    w.media.playing = true;
    w.media.command_seq += 1;
}

/// @0048a598
fn openfile1_click(w: &mut World) -> R<()> {
    if w.execute_file_dialog(FORM4, "OpenDialog1", false)? {
        if w.media.playing {
            media_stop(w);
        }
        w.media.file_name = w.cr(FORM4, "OpenDialog1").file_name.clone();
        w.media.position = 0;
        w.media.length = 0;
        w.media.command_seq += 1; // Open
        media_play(w);
    }
    Ok(())
}

/// @0048a678 (Timer1, 1 ms)
fn timer1_timer(w: &mut World) -> R<()> {
    // Length is only known once the front-end has opened the file (MCI reports it after Open).
    if !w.media.file_name.is_empty()
        && w.media.length > 0
        && w.media.position >= w.media.length
        && w.is_menu_checked(FORM4, "RepeatMusic1")
    {
        media_stop(w);
        media_rewind(w);
        media_play(w);
    }
    Ok(())
}

// ---- HTML export

/// @0048a6e4
fn export_click(w: &mut World) -> R<()> {
    if !w.execute_file_dialog(FORM4, "HTMLExport", true)? {
        return Ok(());
    }
    let mut white = false;
    let file = w.cr(FORM4, "HTMLExport").file_name.clone();
    let player = w.caption(FORM5, "PlayerNameLbl");
    let mut f = TextWriter::create(&file)?;
    f.writeln(&format!("<html><head><meta name=\"generator\" content=\"POKéMON Simulator\"><style>body {{ background-color: white; background-image: none; font-family: Arial,sans-serif; color: black; font-size: 10pt;}} table{{ background-image: none; font-family: Arial,sans-serif; color: black; font-size: 10pt;}}</style><meta http-equiv=\"content-type\" content=\"text/html; charset=iso-8859-1\"><title>POKéMON Information for {player}</title></head><body>"));
    let (date, time) = crate::delphi::rtl::date_time_strings();
    f.writeln(&format!("<h1>POKéMON Information for {player}</h1><p>Generated by POKéMON Simulator {date} {time}"));
    f.writeln("<p><table border><tr><td><b>Index</b></td><td><b>Number/Name</b></td><td><b>Level</b></td><td><b>Experience Points</b></td><td><b>...to next level</b></td></tr>");
    let n = w.lv_count(FORM5, "ListView1");
    for i in 1..=n {
        let mut row = String::from("<tr bgcolor=");
        row.push_str(if white { "#FFFFFF" } else { "#C0C0C0" });
        white = !white;
        row.push_str(&format!("><td>{i}</td><td># "));
        let sp = w.m.pc_species(i);
        if sp < 100 {
            row.push('0');
        }
        if sp < 10 {
            row.push('0');
        }
        let name = w.m.pkmn_name(sp);
        row = crate::cat!(&row, &sp.to_string(), " ", &name, "</td><td>")?;
        let lv = w.m.pc_level(i);
        row.push_str(&format!("{lv}</td><td>{}</td><td>", w.m.pc_exp(i)));
        if lv == 100 {
            row.push('0');
        } else {
            let l = lv as u32 + 1;
            row.push_str(&l.wrapping_mul(l).wrapping_mul(l).wrapping_sub(w.m.pc_exp(i)).to_string());
        }
        row.push_str("</td></tr>");
        f.writeln(&row);
    }
    let owned_lbl = w.caption(FORM5, "OwnedPKMNLbl");
    f.writeln(&format!("</table><p><table border><tr><td><b>In POKéDEX ({owned_lbl}/386)</b></td></tr>"));
    white = false;
    for i in 1..=386 {
        if w.m.owned(i) {
            let mut row = String::from("<tr bgcolor=");
            row.push_str(if white { "#FFFFFF" } else { "#C0C0C0" });
            white = !white;
            row.push_str("><td># ");
            if i < 100 {
                row.push('0');
            }
            if i < 10 {
                row.push('0');
            }
            row = crate::cat!(&row, &i.to_string(), " ", &w.m.pkmn_name(i))?;
            f.writeln(&row);
        }
    }
    f.writeln("</table></body></html>");
    f.close()?;
    let q = format!("POKéMON Information exported to {file}. View file now?");
    if w.message_dlg(&q, MsgDlgType::Information, MB_YES_NO)? == MR_YES {
        crate::delphi::rtl::shell_open(&file);
    }
    Ok(())
}

// ---- tournaments

/// (handler, lo, hi, fee, tier). PrimeCup tests "Level = 100" (lo = hi = 100).
const TIERS: [(&str, u8, u8, u32, u8); 6] = [
    ("VeryEasyLevels2101Click", 2, 10, 1000, 1),
    ("EasyLevels10301Click", 10, 30, 2000, 2),
    ("IntermediateLevels30501Click", 30, 50, 3000, 3),
    ("HardLevels50701Click", 50, 70, 5000, 4),
    ("VeryHardLevels70991Click", 70, 99, 7000, 5),
    ("PrimeCupLevel1001Click", 100, 100, 10000, 6),
];

/// @0048b0c0 .. @0048b5d4
fn tier_click(w: &mut World, lo: u8, hi: u8, fee: u32, tier: u8) -> R<()> {
    let mut found = false;
    for k in 1..=w.lv_count(FORM5, "ListView1") {
        let l = w.m.pc_level(k);
        if l >= lo && l <= hi {
            found = true;
        }
    }
    if !found {
        warning(w, "None of your POKéMON meet the level requirements.")?;
    }
    if w.m.money() < fee && found {
        w.info("You don't have enough money!")?;
    } else if found {
        set_up_tournament(w, tier)?;
        w.m.set_money(w.m.money().wrapping_sub(fee));
    }
    Ok(())
}

/// @0048b6cc
fn set_up_tournament(w: &mut World, tier: u8) -> R<()> {
    let range = match tier {
        1 => Some((2, 10)),
        2 => Some((10, 30)),
        3 => Some((30, 50)),
        4 => Some((50, 70)),
        5 => Some((70, 99)),
        6 => Some((100, 100)),
        _ => None,
    };
    if let Some((lo, hi)) = range {
        w.m.set_min_wild_level(lo);
        w.m.set_max_wild_level(hi);
    }
    loop {
        let mut ok = true;
        for i in 1..=8 {
            let v = rand_plus(w, 64, 1.0);
            w.m.set_tour_trainer(i, v.to_int() as u8);
        }
        for i in 1..=7 {
            for j in (i + 1)..=8 {
                if w.m.tour_trainer(i) == w.m.tour_trainer(j) {
                    ok = false;
                }
            }
        }
        if ok {
            for i in 1..=8 {
                let v = rand_plus(w, 386, 4.0); // 4..389: original bug (species past the tables)
                w.m.set_tour_pkmn(i, v.to_int());
                if [129, 132, 201, 235, 349].contains(&w.m.tour_pkmn(i)) {
                    ok = false;
                }
            }
        }
        if ok {
            let (lo, hi) = (w.m.min_wild_level(), w.m.max_wild_level());
            for i in 1..=8 {
                let r = w.rng.random(hi as i32 - lo as i32 + 5);
                let mut v = Var::real(F80::from_i64(r as i64).int().add(F80::from_i64(lo as i64)));
                if v.gt(Var::byte(hi)) {
                    v = Var::byte(hi);
                }
                if v.lt(Var::byte(lo)) {
                    v = Var::byte(lo);
                }
                w.m.set_tour_level(i, v.to_int() as u8);
            }
        }
        if ok && tier < 3 {
            for i in 1..=8 {
                let p = w.m.tour_pkmn(i);
                for k in 1..=386 {
                    if w.m.evolve_to(k) == p {
                        ok = false;
                    }
                }
                if [134, 135, 136, 196, 197, 45, 182, 186, 62, 80, 199, 237, 106, 107].contains(&p) {
                    ok = false;
                }
            }
        }
        if ok {
            break;
        }
    }
    w.m.set_battle_code(tier.wrapping_mul(10).wrapping_add(1));
    let (sp, lv) = (w.m.tour_pkmn(1), w.m.tour_level(1) as i32);
    let tn = w.m.trainer_name(w.m.tour_trainer(1) as i32);
    start_trainer_battle(w, sp, lv, &tn)?;
    w.m.set_in_tournament(true);
    Ok(())
}
