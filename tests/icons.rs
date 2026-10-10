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

/// Desktop packaging icons: `packaging/pokegame.ico` (Windows, PNG entries) and
/// `packaging/icon.iconset` (macOS, turned into .icns by `iconutil` at release time).
#[test]
fn desktop_icons() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("packaging");
    let ico = dir.join("pokegame.ico");
    let iconset = dir.join("icon.iconset");
    if std::env::var_os("GEN_ICONS").is_none() {
        assert!(ico.exists(), "packaging/pokegame.ico missing; run GEN_ICONS=1 cargo test --test icons");
        return;
    }
    std::fs::create_dir_all(&iconset).unwrap();
    let png = |px: u32| {
        let path = std::env::temp_dir().join(format!("pg_icon_{px}.png"));
        pokegame::debug::write_png(&path, px, px, &icon::full(px)).unwrap();
        std::fs::read(path).unwrap()
    };
    // .ico with PNG-compressed entries (Windows Vista+).
    let sizes = [16u32, 24, 32, 48, 64, 128, 256];
    let images: Vec<Vec<u8>> = sizes.iter().map(|&s| png(s)).collect();
    let mut out = vec![0, 0, 1, 0, sizes.len() as u8, 0];
    let mut offset = 6 + 16 * sizes.len() as u32;
    for (s, img) in sizes.iter().zip(&images) {
        let b = if *s >= 256 { 0 } else { *s as u8 };
        out.extend_from_slice(&[b, b, 0, 0, 1, 0, 32, 0]);
        out.extend_from_slice(&(img.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += img.len() as u32;
    }
    for img in &images {
        out.extend_from_slice(img);
    }
    std::fs::write(&ico, out).unwrap();
    for (name, px) in [("16x16", 16), ("16x16@2x", 32), ("32x32", 32), ("32x32@2x", 64), ("128x128", 128), ("128x128@2x", 256), ("256x256", 256), ("256x256@2x", 512), ("512x512", 512), ("512x512@2x", 1024)] {
        std::fs::write(iconset.join(format!("icon_{name}.png")), png(px)).unwrap();
    }
}
