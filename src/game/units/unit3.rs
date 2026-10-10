//! Unit3 — TForm3 loader/splash, the application's main form. Port of recon/Unit3.pas.

use super::*;

pub fn dispatch(w: &mut World, h: &str) -> Option<R<()>> {
    Some(match h {
        "TimerEventLoad" => timer_event_load(w),
        "FormHide" => form_hide(w),
        _ => return None,
    })
}

/// @00481aec
fn incre(w: &mut World) {
    let p1 = w.gauge_progress(FORM3, "Gauge1");
    w.set_gauge_progress(FORM3, "Gauge1", p1 + 1);
    let p2 = w.gauge_progress(FORM3, "Gauge2");
    w.set_gauge_progress(FORM3, "Gauge2", p2 + 1);
}

/// @00481b18
fn r(w: &mut World, max: i32, caption: &str) {
    w.set_gauge_progress(FORM3, "Gauge1", 0);
    w.set_gauge_max(FORM3, "Gauge1", max);
    w.set_caption(FORM3, "Label2", caption);
    w.update();
}

/// @00481b90 — reads <exe dir>\POKéGAME.dat. The file was parsed (same order and widths, see
/// assets::datfile) when the program started; here the tables are stored and the progress shown.
fn timer_event_load(w: &mut World) -> R<()> {
    w.set_enabled(FORM3, "Timer1", false);
    w.rng.randomize();
    let sections: [(i32, &str, usize); 13] = [
        (306, "Declaring type relations", 17 * 18),
        (386, "Declaring POKéMON names", 386),
        (24, "Declaring badge names", 24),
        (772, "Declaring POKéMON Types", 772),
        (386, "Declaring evolution levels", 386),
        (386, "Declaring evolutions", 386),
        (119, "Declaring attack names", 119),
        (119, "Declaring attack powers", 119),
        (119, "Declaring attack types", 119),
        (17, "Declaring type names", 17),
        (1544, "Declaring POKéMON attacks", 1544),
        (18, "Initializing tournament prizes", 18),
        (64, "Initializing tournament trainers", 64),
    ];
    for (max, caption, increments) in sections {
        r(w, max, caption);
        for _ in 0..increments {
            incre(w);
        }
    }
    let assets = w.assets.clone();
    w.m.load_tables(&assets.data);
    w.hide(FORM3)?; // -> FormHide
    Ok(()) // CloseFile
}

/// @00482258
fn form_hide(w: &mut World) -> R<()> {
    let mut found = 0usize;
    for (i, p) in w.params.clone().iter().enumerate() {
        if std::path::Path::new(p).is_file() {
            found = i + 1;
        }
    }
    if found > 0 {
        let p = w.params[found - 1].clone();
        super::unit1::load_game(w, &p)?;
    }
    if found == 0 {
        w.set_visible(FORM1, true)?;
    }
    Ok(())
}
