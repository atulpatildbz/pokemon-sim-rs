//! The app icon, drawn as pixel art in code: a capture ball on the game's sky-stripe
//! background. Used for the desktop window, and rendered to Android launcher PNGs by
//! `tests/icons.rs` (`GEN_ICONS=1`).

type Rgba = [u8; 4];

const OUTLINE: Rgba = [0x38, 0x40, 0x50, 255];
const RED: Rgba = [0xE0, 0x40, 0x38, 255];
const RED_DARK: Rgba = [0xB0, 0x28, 0x28, 255];
const WHITE: Rgba = [0xF8, 0xF8, 0xF8, 255];
const SHADE: Rgba = [0xC8, 0xD0, 0xD8, 255];
const SHINE: Rgba = [0xFF, 0xD8, 0xD0, 255];
const CLEAR: Rgba = [0, 0, 0, 0];

fn mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    [l(a[0], b[0]), l(a[1], b[1]), l(a[2], b[2]), l(a[3], b[3])]
}

/// Sky background cell (grid `g`): vertical gradient with lighter diagonal bands.
fn sky(x: i32, y: i32, g: i32) -> Rgba {
    let top = [0x58, 0x98, 0xE8, 255];
    let bottom = [0xB8, 0xE8, 0xF8, 255];
    let c = mix(top, bottom, y as f32 / (g - 1) as f32);
    if (x + y).rem_euclid(g / 4) < g / 12 { mix(c, [255, 255, 255, 255], 0.22) } else { c }
}

/// The ball, centred at (cx, cy) with radius r (grid cells). None = outside it.
fn ball(x: i32, y: i32, cx: f32, cy: f32, r: f32) -> Option<Rgba> {
    let (px, py) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
    let d = (px * px + py * py).sqrt();
    if d > r {
        return None;
    }
    if d > r - 1.0 {
        return Some(OUTLINE);
    }
    let band = r * 0.14;
    // Centre button: outline ring, white face.
    if d < r * 0.36 {
        return Some(if d < r * 0.22 { WHITE } else { OUTLINE });
    }
    if py.abs() < band.max(0.6) {
        return Some(OUTLINE);
    }
    if py < 0.0 {
        // Top: red, with a highlight up-left and a darker rim lower-right.
        if (px + r * 0.42).powi(2) + (py + r * 0.5).powi(2) < (r * 0.2).powi(2) {
            return Some(SHINE);
        }
        return Some(if px + py * 0.3 > r * 0.55 { RED_DARK } else { RED });
    }
    Some(if px - py * 0.3 > r * 0.45 || py > r * 0.72 { SHADE } else { WHITE })
}

/// Render a `g`×`g` pixel design to `size`×`size` RGBA (nearest neighbour).
fn render(size: u32, g: i32, cell: impl Fn(i32, i32) -> Rgba) -> Vec<u8> {
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let gx = (x as i64 * g as i64 / size as i64) as i32;
            let gy = (y as i64 * g as i64 / size as i64) as i32;
            out.extend_from_slice(&cell(gx, gy));
        }
    }
    out
}

/// Full icon (rounded square, sky, ball): desktop window and legacy launcher icon.
pub fn full(size: u32) -> Vec<u8> {
    let g = 24;
    render(size, g, |x, y| {
        // Rounded corners: cut a 2-cell staircase.
        let (ex, ey) = (x.min(g - 1 - x), y.min(g - 1 - y));
        if ex + ey < 2 {
            return CLEAR;
        }
        ball(x, y, 12.0, 12.5, 8.6).unwrap_or_else(|| sky(x, y, g))
    })
}

/// Adaptive icon foreground (108dp canvas, ball inside the 66dp safe zone).
pub fn adaptive_foreground(size: u32) -> Vec<u8> {
    let g = 36;
    render(size, g, |x, y| ball(x, y, 18.0, 18.0, 10.5).unwrap_or(CLEAR))
}

/// Adaptive icon background.
pub fn adaptive_background(size: u32) -> Vec<u8> {
    let g = 36;
    render(size, g, |x, y| sky(x, y, g))
}

pub fn window_icon() -> egui::IconData {
    egui::IconData { rgba: full(256), width: 256, height: 256 }
}
