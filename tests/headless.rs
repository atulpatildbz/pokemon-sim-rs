//! Headless runs of the ported game (no UI thread): scripted dialog answers, fixed RandSeed.
use pokegame::assets::Assets;
use pokegame::game::units;
use pokegame::game::world::*;
use pokegame::vcl::runtime::DialogAnswer;
use std::path::PathBuf;
use std::sync::Arc;

fn world() -> Option<World> {
    let dir = std::env::var_os("POKEGAME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("../downloads/pokegame/extracted"));
    if !dir.join("PokeGame.exe").exists() {
        eprintln!("skipping: game files not found");
        return None;
    }
    let assets = Arc::new(Assets::load(&dir).unwrap());
    let exe = std::fs::read(dir.join("PokeGame.exe")).unwrap();
    let pe = pokegame::assets::pe::Pe::parse(exe).unwrap();
    let data = pe.sections.iter().find(|s| s.name == "DATA").map(|s| pe.data[s.raw_ptr as usize..(s.raw_ptr + s.raw_size) as usize].to_vec()).unwrap();
    let mut w = World::new(assets, &data, None);
    // Application.Run -> Form3 timer -> TimerEventLoad
    w.show(FORM3).unwrap();
    units::dispatch(&mut w, FORM3, "TimerEventLoad").unwrap();
    w.rng.seed = 12345; // deterministic after Randomize
    Some(w)
}

fn tmp(name: &str) -> String {
    std::env::temp_dir().join(name).to_string_lossy().into_owned()
}

/// New game -> name dialog -> pick Bulbasaur.
fn new_game(w: &mut World, save: &str) {
    w.headless_answers.push(DialogAnswer::File(Some(save.to_string())));
    w.c(OK_RIGHT_DLG, "Edit1").text = "Red".into();
    units::dispatch(w, FORM1, "NewButtonClick").unwrap();
    assert!(w.visible(FORM4));
    assert!(w.ctl_visible(FORM4, "ListView1"));
    assert_eq!(w.forms[FORM4].caption, format!("POKéMON Simulator V4.5 - Red [{}]", std::path::Path::new(save).file_name().unwrap().to_string_lossy()));
    w.c(FORM4, "ListView1").item_index = 0;
    units::dispatch(w, FORM4, "ListView1Click").unwrap();
    assert_eq!(w.m.pc_species(1), 1);
    assert_eq!(w.m.pc_level(1), 5);
    assert_eq!(w.m.pc_exp(1), 125);
    assert_eq!(w.lv_caption(FORM5, "ListView1", 0), "Bulbasaur");
    assert!(w.m.owned(1));
}

#[test]
fn startup_loads_tables_and_shows_title() {
    let Some(w) = world() else { return };
    assert!(w.visible(FORM1));
    assert!(!w.visible(FORM3));
    assert_eq!(w.m.pkmn_name(25), pokegame::game::mem::DStr::S("Pikachu".into()));
    assert_eq!(w.cr(FORM3, "Label2").caption, "Initializing tournament trainers");
}

#[test]
fn wild_battle_round() {
    let Some(mut w) = world() else { return };
    let save = tmp("pg_wild.pgs");
    new_game(&mut w, &save);
    units::dispatch(&mut w, FORM4, "FindPKMN1Click").unwrap();
    assert!(w.m.in_wild_battle());
    let enemy = w.m.enemy_species();
    assert!((1..=386).contains(&enemy));
    // choose Bulbasaur (info + confirm Yes)
    w.c(FORM5, "ListView1").item_index = 0;
    w.headless_answers.push(DialogAnswer::Button(1));
    w.headless_answers.push(DialogAnswer::Button(6));
    units::dispatch(&mut w, FORM5, "ListView1Click").unwrap();
    assert_eq!(w.m.cur_pkmn(), 1);
    assert_eq!(w.m.own_hp(), 50);
    assert_eq!(w.item(FORM4, "RadioGroup1", 0), "Tackle (Normal)");
    w.c(FORM4, "RadioGroup1").item_index = 0;
    units::dispatch(&mut w, FORM4, "Button1Click").unwrap();
    let log = w.cr(FORM7, "Memo1").items.clone();
    eprintln!("{log:#?}");
    assert!(log.iter().any(|l| l.starts_with("Bulbasaur used Tackle!")));
    assert!(log.iter().any(|l| l.contains("points of damage to enemy")));
}

#[test]
fn tournament_species_387_access_violation() {
    let Some(mut w) = world() else { return };
    let save = tmp("pg_av.pgs");
    new_game(&mut w, &save);
    let tn = w.m.trainer_name(1);
    let r = units::unit4::start_trainer_battle(&mut w, 387, 5, &tn);
    let e = r.unwrap_err();
    assert_eq!(e.message(), "Access violation at address 00404E03 in module 'PokeGame.exe'. Read of address 040301FD.");
    // state left behind exactly as in the original: battle flags set, first log line written
    assert!(w.m.in_trainer_battle());
    assert_eq!(w.cr(FORM7, "Memo1").items, vec!["Tracey wants to fight!".to_string()]);
}

#[test]
fn save_and_load_roundtrip() {
    let Some(mut w) = world() else { return };
    let save = tmp("pg_rt.pgs");
    new_game(&mut w, &save);
    w.m.set_money(12345);
    units::dispatch(&mut w, FORM4, "FormClose").unwrap();
    assert!(w.terminated);
    let text = std::fs::read(&save).unwrap();
    assert!(text.starts_with(b"VERSION2SAVEFILEIDENTIFIER\r\nRed\r\nFALSE\r\n"));
    let Some(mut w2) = world() else { return };
    units::unit1::load_game(&mut w2, &save).unwrap();
    assert_eq!(w2.m.money(), 12345);
    assert_eq!(w2.m.pc_species(1), 1);
    assert_eq!(w2.lv_count(FORM5, "ListView1"), 1);
    assert_eq!(w2.cr(FORM5, "MoneyLbl").caption, "12345");
}

/// Auto-save mid-tournament writes exactly the file the original's quit (Form4.FormClose)
/// writes at that moment: fee paid, quit penalty applied — and changes nothing in the game.
#[test]
fn tournament_autosave_matches_quit() {
    let Some(mut w) = world() else { return };
    let save = tmp("pg_tour_autosave.pgs");
    new_game(&mut w, &save);
    // Give the starter some experience so the penalty is visible, and money for the fee.
    w.m.set_pc_exp(1, 300);
    w.m.set_money(5000);
    units::dispatch(&mut w, FORM4, "VeryEasyLevels2101Click").unwrap();
    assert!(w.m.in_tournament());
    assert_eq!(w.m.money(), 4000, "fee paid");
    // Send out the starter (info + confirm Yes), then fight one turn.
    w.c(FORM5, "ListView1").item_index = 0;
    w.headless_answers.push(DialogAnswer::Button(1));
    w.headless_answers.push(DialogAnswer::Button(6));
    units::dispatch(&mut w, FORM5, "ListView1Click").unwrap();
    w.c(FORM4, "RadioGroup1").item_index = 0;
    units::dispatch(&mut w, FORM4, "Button1Click").unwrap();
    assert!(w.m.in_tournament());

    let before = (w.m.pc_exp(1), w.m.pc_level(1), w.m.money(), w.rng.seed);
    w.autosave = true;
    w.autosave_if_safe();
    let auto = std::fs::read(&save).expect("auto-save written");
    assert_eq!((w.m.pc_exp(1), w.m.pc_level(1), w.m.money(), w.rng.seed), before, "auto-save changed the game");

    units::dispatch(&mut w, FORM4, "FormClose").unwrap();
    let quit = std::fs::read(&save).unwrap();
    assert_eq!(auto, quit, "auto-save differs from what quitting writes");
    // And the penalty really is in there: experience went down from 300.
    let text: String = quit.iter().map(|&b| b as char).collect();
    let exp: u32 = text.lines().nth(19 + 386 + 2).unwrap().parse().unwrap();
    assert!(exp < 300, "penalty applied (exp {exp})");
}
