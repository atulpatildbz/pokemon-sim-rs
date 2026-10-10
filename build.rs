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
    // Android: the audio backend (Oboe) is C++; link the NDK's C++ runtime statically so the
    // APK needs no libc++_shared.so.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android") {
        println!("cargo:rerun-if-env-changed=ANDROID_NDK_HOME");
        let ndk = std::env::var_os("ANDROID_NDK_HOME").map(PathBuf::from).expect("ANDROID_NDK_HOME");
        let triple = match std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
            Ok("aarch64") => "aarch64-linux-android",
            Ok("arm") => "arm-linux-androideabi",
            Ok("x86_64") => "x86_64-linux-android",
            _ => "i686-linux-android",
        };
        // Copy just the two C++ archives into their own folder: putting the NDK's sysroot lib
        // folder on the search path would also make `-lc` pick up the static libc.a.
        let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("cxx");
        std::fs::create_dir_all(&out).unwrap();
        let prebuilt = ndk.join("toolchains/llvm/prebuilt");
        let host = std::fs::read_dir(&prebuilt).unwrap().flatten().next().expect("NDK prebuilt toolchain").path();
        let lib = host.join("sysroot/usr/lib").join(triple);
        for a in ["libc++_static.a", "libc++abi.a"] {
            std::fs::copy(lib.join(a), out.join(a)).unwrap();
        }
        println!("cargo:rustc-link-search=native={}", out.display());
        println!("cargo:rustc-link-lib=static=c++_static");
        println!("cargo:rustc-link-lib=static=c++abi");
    }
    // Windows: the .exe's own icon (Explorer, taskbar) from packaging/pokegame.ico.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let ico = Path::new(env!("CARGO_MANIFEST_DIR")).join("packaging/pokegame.ico");
        println!("cargo:rerun-if-changed={}", ico.display());
        let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
        let rc = out.join("icon.rc");
        std::fs::write(&rc, format!("1 ICON \"{}\"\n", ico.display().to_string().replace('\\', "/"))).unwrap();
        let obj = out.join("icon.o");
        let windres = std::env::var("WINDRES").unwrap_or_else(|_| "x86_64-w64-mingw32-windres".into());
        let ok = std::process::Command::new(&windres).arg(&rc).arg("-O").arg("coff").arg("-o").arg(&obj).status().is_ok_and(|s| s.success());
        if ok {
            println!("cargo:rustc-link-arg-bins={}", obj.display());
        } else {
            println!("cargo:warning=windres not available; the .exe will have no icon");
        }
    }
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
