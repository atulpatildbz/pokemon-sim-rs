//! Front-end preferences. Kept in their own file in the platform config folder — never in the
//! `.pgs` save, whose format stays exactly the original's.

use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextSpeed {
    Slow,
    Normal,
    Fast,
    Instant,
}

impl TextSpeed {
    pub const ALL: [TextSpeed; 4] = [TextSpeed::Slow, TextSpeed::Normal, TextSpeed::Fast, TextSpeed::Instant];
    pub fn label(self) -> &'static str {
        match self {
            TextSpeed::Slow => "Slow",
            TextSpeed::Normal => "Normal",
            TextSpeed::Fast => "Fast",
            TextSpeed::Instant => "Instant",
        }
    }
    /// Typewriter speed in characters per second (`None` = whole text at once).
    pub fn chars_per_sec(self) -> Option<f32> {
        match self {
            TextSpeed::Slow => Some(25.0),
            TextSpeed::Normal => Some(50.0),
            TextSpeed::Fast => Some(110.0),
            TextSpeed::Instant => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub text_speed: TextSpeed,
    /// Draw the original Windows forms instead of the game UI.
    pub classic: bool,
    pub animations: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { text_speed: TextSpeed::Normal, classic: false, animations: true }
    }
}

/// Per-platform app data folder: %APPDATA%\PokeGame, ~/Library/Application Support/PokeGame,
/// $XDG_CONFIG_HOME/pokegame (or ~/.config/pokegame).
/// `POKEGAME_DATA_DIR` overrides it (tests, portable installs).
pub fn app_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("POKEGAME_DATA_DIR") {
        return Some(PathBuf::from(d));
    }
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("PokeGame"));
    #[cfg(target_os = "macos")]
    let base = std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support/PokeGame"));
    #[cfg(all(unix, not(target_os = "macos")))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .map(|d| d.join("pokegame"));
    base
}

fn settings_path() -> Option<PathBuf> {
    app_dir().map(|d| d.join("settings.txt"))
}

impl Settings {
    pub fn load() -> Settings {
        let mut s = Settings::default();
        let Some(text) = settings_path().and_then(|p| std::fs::read_to_string(p).ok()) else { return s };
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            match (k.trim(), v.trim()) {
                ("text_speed", v) => {
                    if let Some(t) = TextSpeed::ALL.into_iter().find(|t| t.label().eq_ignore_ascii_case(v)) {
                        s.text_speed = t;
                    }
                }
                ("classic", v) => s.classic = v == "true",
                ("animations", v) => s.animations = v != "false",
                _ => {}
            }
        }
        s
    }

    pub fn save(&self) {
        let Some(path) = settings_path() else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let text = format!("text_speed={}\nclassic={}\nanimations={}\n", self.text_speed.label(), self.classic, self.animations);
        let _ = std::fs::write(path, text);
    }
}
