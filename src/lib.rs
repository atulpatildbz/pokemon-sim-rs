//! Faithful Rust port of BirdieSoft's POKéMON Simulator V4.5 (Delphi 7, 2004).
//!
//! Game logic mirrors the original Delphi units 1:1 (see `units`), reconstructed from the
//! decompiled executable. Assets are loaded at runtime from the user's own copy.

pub mod app;
pub mod assets;
pub mod vcl;
pub mod delphi;
pub mod game;
pub mod debug;
pub mod ui;

/// Android entry point (NativeActivity). The game files are compiled in (feature `bundled`);
/// the app's data folder (settings, save slots, music) is its internal storage.
#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    // winit's event loop exists once per process. If Android re-creates the activity in a
    // process that already ran (Back then reopen, or a relaunch), start over in a fresh
    // process; progress is already auto-saved.
    static STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if STARTED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        std::process::exit(0);
    }
    if let Some(p) = app.internal_data_path() {
        std::env::set_var("POKEGAME_DATA_DIR", p);
    }
    let Some((exe, dat)) = assets::bundled() else {
        eprintln!("pokegame: built without the game files (feature `bundled`)");
        return;
    };
    let options = eframe::NativeOptions { android_app: Some(app), ..Default::default() };
    if let Err(e) = app::run(options, exe, dat, Vec::new()) {
        eprintln!("pokegame: {e}");
    }
    // The activity is gone (Back or system): end the process so the next launch is clean.
    std::process::exit(0);
}
