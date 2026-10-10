//! Feature `bundled` (on by default): copy the original game files into OUT_DIR so they are
//! compiled into the binary — one self-contained executable for personal use. Source folder:
//! $POKEGAME_DIR, or ../downloads/pokegame/extracted next to this crate. If the files are not
//! there, the build continues without them (the binary then asks for the folder at start-up).

use std::path::{Path, PathBuf};

fn find(dir: &Path, name: &str) -> Option<PathBuf> {
    let norm = |s: &str| s.to_lowercase().replace("e\u{301}", "é").replace('\u{fffd}', "é");
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| p.file_name().is_some_and(|f| norm(&f.to_string_lossy()) == norm(name)))
}

fn main() {
    println!("cargo:rerun-if-env-changed=POKEGAME_DIR");
    println!("cargo:rustc-check-cfg=cfg(has_bundle)");
    if std::env::var_os("CARGO_FEATURE_BUNDLED").is_none() {
        return;
    }
    let dir = std::env::var_os("POKEGAME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../downloads/pokegame/extracted"));
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    println!("cargo:rerun-if-changed={}", dir.display());
    let files = [("PokeGame.exe", "PokeGame.exe"), ("POKéGAME.dat", "POKEGAME.dat")].map(|(name, dest)| (find(&dir, name), dest));
    if files.iter().any(|(src, _)| src.is_none()) {
        println!("cargo:warning=game files not found in {} — building without them (set POKEGAME_DIR to bundle)", dir.display());
        return;
    }
    for (src, dest) in files {
        let src = src.unwrap();
        println!("cargo:rerun-if-changed={}", src.display());
        std::fs::copy(&src, out.join(dest)).unwrap();
    }
    println!("cargo:rustc-cfg=has_bundle");
}
