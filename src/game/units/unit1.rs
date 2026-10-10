//! Unit1 — TForm1 title screen (New / Load / Help / About / Exit) and the save-game loader.
//! Port of recon/Unit1.pas.

use super::*;
use crate::delphi::{Var, F80};
use crate::game::textfile::TextReader;

pub fn dispatch(w: &mut World, h: &str) -> Option<R<()>> {
    Some(match h {
        "ExitButtonClick" => exit_button_click(w),
        "AboutButtonClick" => about_button_click(w),
        "HelpButtonClick" => help_button_click(w),
        "LoadButtonClick" => load_button_click(w),
        "NewButtonClick" => new_button_click(w),
        "FormDeactivate" => form_deactivate(w),
        "FormClose" => form_close(w),
        _ => return None,
    })
}

/// @0048c484
fn exit_button_click(w: &mut World) -> R<()> {
    w.close(FORM3) // Form3 is the main form -> terminates the application
}

/// @0048c494
fn about_button_click(w: &mut World) -> R<()> {
    w.show_modal(ABOUT_BOX).map(|_| ())
}

/// @0048c4a4 — Application.HelpFile := <exe dir> + 'PokéGame.hlp'; HelpJump('Index').
/// WinHelp (.hlp) has no viewer on current systems (also unsupported on Windows Vista+); no-op.
pub fn help_button_click(_w: &mut World) -> R<()> {
    Ok(())
}

/// @0048c54c
fn load_button_click(w: &mut World) -> R<()> {
    if w.execute_file_dialog(FORM1, "OpenDialog1", false)? {
        let f = w.cr(FORM1, "OpenDialog1").file_name.clone();
        load_game(w, &f)?;
    }
    Ok(())
}

/// Window placement shared by LoadGame / NewButtonClick:
/// L := Int(Screen.Width / 2) - 305; T := Int(Screen.Height / 2) - top_off  (x87, then Variants).
fn place_windows(w: &mut World, top_off: f32) {
    let half = |v: i32| F80::from_i64(v as i64).div(F80::from_f32(2.0)).int();
    let v_l = Var::real(half(w.screen.0).sub(F80::from_f32(305.0)));
    let v_l428 = v_l.add(Var::integer(428));
    let v_l7 = v_l;
    let v_t = Var::real(half(w.screen.1).sub(F80::from_f32(top_off)));
    let v_t5 = v_t;
    let v_t314 = v_t.add(Var::integer(314));
    w.set_left(FORM4, v_l.to_int());
    w.set_left(FORM5, v_l428.to_int());
    w.set_left(FORM7, v_l7.to_int());
    w.set_top(FORM4, v_t.to_int());
    w.set_top(FORM5, v_t5.to_int());
    w.set_top(FORM7, v_t314.to_int());
}

/// @0048c5ac
pub fn load_game(w: &mut World, file_name: &str) -> R<()> {
    w.m.set_min_wild_level(2);
    w.m.set_max_wild_level(100);
    let mut v2 = false;
    w.c(FORM4, "SaveDialog1").file_name = file_name.to_string();
    let mut f = TextReader::open(file_name)?;
    let mut name = f.read_line();
    if name == "VERSION2SAVEFILEIDENTIFIER" {
        name = f.read_line();
        v2 = true;
    }
    w.m.set_player_name(&name);
    let mut s = f.read_line();
    w.m.set_beat_lance(s == "TRUE");
    s = f.read_line();
    w.m.set_beat_ash(s == "TRUE");
    s = f.read_line();
    w.m.set_beat_me(s == "TRUE");
    if v2 {
        s = f.read_line();
        w.m.set_beat_rby_champion(s == "TRUE");
        s = f.read_line();
        w.m.set_beat_rusa_champion(s == "TRUE");
    }
    let b = f.read_long()?;
    w.m.set_badges(b as u8);
    f.read_ln();
    macro_rules! card {
        ($set:ident) => {{
            let v = f.read_int64()?;
            w.m.$set(v as u32);
            f.read_ln();
        }};
    }
    card!(set_money);
    card!(set_wins);
    card!(set_losses);
    card!(set_fights);
    card!(set_captures);
    card!(set_poke_balls);
    card!(set_great_balls);
    card!(set_ultra_balls);
    card!(set_master_balls);
    card!(set_potions);
    card!(set_super_potions);
    for i in 1..=251 {
        s = f.read_line();
        w.m.set_owned(i, s == "TRUE");
    }
    if v2 {
        for i in 252..=386 {
            s = f.read_line();
            w.m.set_owned(i, s == "TRUE");
        }
    }
    for i in 1..=10000 {
        let sp = f.read_long()?;
        w.m.set_pc_species(i, sp);
        f.read_ln();
        let lv = f.read_long()?;
        w.m.set_pc_level(i, lv as u8);
        f.read_ln();
        let ex = f.read_int64()?;
        w.m.set_pc_exp(i, ex as u32);
        f.read_ln();
    }

    // rebuild the PC list view: remove the starter-list placeholder item 0, then add until species <= 0
    w.lv_delete(FORM5, "ListView1", 0);
    let mut i = 1;
    while w.m.pc_species(i) > 0 {
        w.lv_add(FORM5, "ListView1");
        let name = w.m.pkmn_name(w.m.pc_species(i));
        let name = name.refd(crate::game::mem::fault::LSTR_ASG)?.to_string();
        w.lv_set_caption(FORM5, "ListView1", i - 1, &name);
        let img = w.m.pc_species(i) - 1;
        w.lv_set_image(FORM5, "ListView1", i - 1, img);
        i += 1;
    }
    let pn = w.m.player_name();
    w.set_caption(FORM5, "PlayerNameLbl", &pn);
    super::unit5::freshen(w)?;

    // caption suffix: file name after the last '\' (the Variant flag starts Unassigned = 0)
    let fname = w.cr(FORM4, "SaveDialog1").file_name.clone();
    let suffix = if fname.is_empty() { s } else { title_suffix(&fname) };
    let cap = format!("{} - {} [{}]", w.forms[FORM1].caption, w.m.player_name(), suffix);
    w.set_form_caption(FORM4, &cap);

    // unlock menus according to badges / flags (unsigned byte compares)
    let badges = w.m.badges();
    let by_badge: [(&str, u8); 16] = [
        ("Misty1", 0),
        ("LtSurge1", 1),
        ("Erika1", 2),
        ("Sabrina1", 3),
        ("Janine1", 4),
        ("Blaine1", 5),
        ("Gary1", 6),
        ("EliteFour1", 7),
        ("Bugsy1", 8),
        ("Whitney1", 9),
        ("Morty1", 10),
        ("Chuck1", 11),
        ("Jasmine1", 12),
        ("Pryce1", 13),
        ("Clair1", 14),
        ("Ash1", 15),
    ];
    for (item, more_than) in by_badge {
        if badges > more_than {
            w.menu_enabled(FORM4, item, true);
        }
    }
    if w.m.beat_lance() {
        w.menu_enabled(FORM4, "JohtoGymLeaders1", true);
    }
    if w.m.beat_ash() {
        w.menu_enabled(FORM4, "Me1", true);
    }
    if w.m.beat_me() {
        w.menu_enabled(FORM4, "Mewtwo1", true);
    }
    if w.m.beat_me() {
        w.menu_enabled(FORM4, "HoennGymLeaders1", true);
    }
    let by_badge2: [(&str, u8); 8] = [
        ("Brawly1", 16),
        ("Wattson1", 17),
        ("Flannery1", 18),
        ("Norman1", 19),
        ("Winona1", 20),
        ("TateLiza1", 21),
        ("Wallace1", 22),
        ("Elites1", 23),
    ];
    for (item, more_than) in by_badge2 {
        if badges > more_than {
            w.menu_enabled(FORM4, item, true);
        }
    }
    drop(f); // CloseFile

    super::unit5::freshen(w)?;
    w.show(FORM5)?;
    w.show(FORM4)?;
    w.m.set_max_wild_level(100);
    w.m.set_min_wild_level(2);
    place_windows(w, 246.0);
    w.set_visible(FORM1, false)
}

/// @0048d2d8
fn new_button_click(w: &mut World) -> R<()> {
    if w.execute_file_dialog(FORM4, "SaveDialog1", true)? {
        w.m.set_min_wild_level(2);
        w.m.set_max_wild_level(100);
        w.m.set_poke_balls(3);
        w.m.set_great_balls(0);
        w.m.set_ultra_balls(0);
        w.m.set_master_balls(0);
        w.m.set_potions(0);
        w.m.set_super_potions(0);
        w.hide(FORM1)?;
        w.show_modal(OK_RIGHT_DLG)?;
        w.show(FORM4)?;
        for i in 1..=386 {
            w.m.set_owned(i, false);
        }
        w.show_c(FORM4, "ListView1");
        w.show_c(FORM4, "Label1");
        w.menu_enabled(FORM4, "File1", false);
        w.menu_enabled(FORM4, "Action1", false);
        place_windows(w, 236.0);
    }
    Ok(())
}

/// @0048d590 — OnDeactivate. The front-end has no activation events; the condition never holds
/// in the flows that reach it (both New and Load hide Form1 themselves).
fn form_deactivate(w: &mut World) -> R<()> {
    if w.visible(FORM1) && w.visible(FORM4) {
        w.hide(FORM1)?;
    }
    Ok(())
}

/// @0048d5b4
fn form_close(w: &mut World) -> R<()> {
    w.close(FORM3)
}
