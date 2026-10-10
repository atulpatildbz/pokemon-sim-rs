//! Renders the launcher icons into `android/res` (`GEN_ICONS=1`), or checks they are current.

use pokegame::ui::icon;
use std::path::Path;

/// (density, legacy px, adaptive layer px)
const DENSITIES: [(&str, u32, u32); 5] = [("mdpi", 48, 108), ("hdpi", 72, 162), ("xhdpi", 96, 216), ("xxhdpi", 144, 324), ("xxxhdpi", 192, 432)];

#[test]
fn launcher_icons() {
    let res = Path::new(env!("CARGO_MANIFEST_DIR")).join("android/res");
    let generate = std::env::var_os("GEN_ICONS").is_some();
    for (d, legacy, layer) in DENSITIES {
        let dir = res.join(format!("mipmap-{d}"));
        let files = [
            ("ic_launcher.png", legacy, icon::full(legacy)),
            ("ic_launcher_foreground.png", layer, icon::adaptive_foreground(layer)),
            ("ic_launcher_background.png", layer, icon::adaptive_background(layer)),
        ];
        for (name, px, rgba) in files {
            let path = dir.join(name);
            if generate {
                std::fs::create_dir_all(&dir).unwrap();
                pokegame::debug::write_png(&path, px, px, &rgba).unwrap();
            } else {
                assert!(path.exists(), "{} missing; run GEN_ICONS=1 cargo test --test icons", path.display());
            }
        }
    }
}
