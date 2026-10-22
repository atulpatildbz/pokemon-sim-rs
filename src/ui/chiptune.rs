//! A tiny GBA/NES-style synthesizer and the game's original soundtrack (composed for this port;
//! no melodies from any Pokémon game). Two pulse channels, a triangle bass and a noise/kick
//! drum channel, rendered on the fly as a rodio `Source`. Songs are note lists: `C5:4` = C5 for
//! four 16th-steps, `r:2` = rest; drums use `k` (kick), `s` (snare), `h` (hi-hat).

#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

const RATE: u32 = 44_100;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cue {
    Title,
    Hub,
    Battle,
    Victory,
}

impl Cue {
    pub fn name(self) -> &'static str {
        match self {
            Cue::Title => "title",
            Cue::Hub => "hub",
            Cue::Battle => "battle",
            Cue::Victory => "victory",
        }
    }
}

struct SongDef {
    bpm: f32,
    looped: bool,
    lead: &'static str,
    /// Lead's pulse duty.
    lead_duty: f32,
    harmony: &'static str,
    bass: &'static str,
    drums: &'static str,
}

// ---------------------------------------------------------------- the soundtrack

const TITLE: SongDef = SongDef {
    bpm: 120.0,
    looped: true,
    lead_duty: 0.25,
    lead: "C5:4 G4:2 C5:2 E5:4 D5:2 C5:2  D5:4 G4:2 B4:2 D5:6 r:2  E5:4 C5:2 E5:2 G5:4 F5:2 E5:2  D5:2 E5:2 F5:2 D5:2 G5:8 \
           A5:4 G5:2 F5:2 E5:4 D5:2 C5:2  F5:4 E5:2 D5:2 C5:4 A4:4  B4:2 C5:2 D5:2 E5:2 F5:2 E5:2 D5:2 B4:2  C5:8 G4:4 r:4",
    harmony: "@C4E4G4 @B3D4G4 @C4E4G4 @B3D4G4 @A3C4F4 @D4F4A4 @B3D4G4 @C4E4G4",
    bass: "C C G G C C G G F F D D G G C C",
    drums: "k:2 h:2 s:2 h:2 k:2 k:2 s:2 h:2",
};

const HUB: SongDef = SongDef {
    bpm: 96.0,
    looped: true,
    lead_duty: 0.5,
    lead: "F5:3 A5:1 C6:4 A5:4 F5:4  G5:3 A5:1 G5:2 F5:2 E5:4 C5:4  D5:3 F5:1 A5:4 G5:2 F5:2 E5:4  F5:12 r:4 \
           A5:3 G5:1 F5:4 E5:4 D5:4  C5:3 D5:1 E5:2 F5:2 G5:8  A5:3 A#5:1 A5:2 G5:2 F5:4 E5:4  F5:12 r:4",
    harmony: "%F4A4C5 %E4G4C5 %D4F4A4 %F4A4C5 %D4F4A4 %C4E4G4 %A#3D4F4 %F4A4C5",
    bass: "F F C C D D F F D D C C A# A# F F",
    drums: "k:4 h:4 h:4 h:4",
};

const BATTLE: SongDef = SongDef {
    bpm: 150.0,
    looped: true,
    lead_duty: 0.25,
    lead: "A4:2 C5:2 E5:2 A5:4 G5:2 E5:2 C5:2  D5:2 E5:2 C5:2 A4:6 r:4  F5:2 A5:2 C6:4 A5:2 F5:2 C5:4  D5:2 G5:2 B5:4 A5:2 G5:2 D5:4 \
           E5:2 A5:2 E5:2 C5:2 E5:4 A4:4  C5:2 B4:2 A4:2 C5:2 E5:8  F5:4 E5:2 D5:2 C5:4 A4:4  B4:4 D5:2 E5:2 G#5:8",
    harmony: "@A3C4E4 @A3C4E4 @F3A3C4 @G3B3D4 @A3C4E4 @A3C4E4 @F3A3C4 @G#3B3E4",
    bass: "!A !A !F !G !A !A !F !E",
    drums: "k:2 h:2 s:2 h:2 k:2 h:2 s:2 h:1 h:1",
};

const VICTORY: SongDef = SongDef {
    bpm: 140.0,
    looped: false,
    lead_duty: 0.25,
    lead: "C5:2 E5:2 G5:2 C6:6 B5:2 C6:2 D6:4 C6:12",
    harmony: "E4:8 A4:8 B4:8 C5:8",
    bass: "C3:8 F3:8 G3:8 C3:8",
    drums: "k:8 s:8 k:8 s:4 s:2 s:2",
};

fn def(cue: Cue) -> &'static SongDef {
    match cue {
        Cue::Title => &TITLE,
        Cue::Hub => &HUB,
        Cue::Battle => &BATTLE,
        Cue::Victory => &VICTORY,
    }
}

// ---------------------------------------------------------------- notation

/// MIDI note number of "C#5" etc.
fn midi(s: &str) -> Option<i32> {
    let (name, oct) = s.split_at(s.find(|c: char| c.is_ascii_digit())?);
    let base = match name {
        "C" => 0,
        "C#" => 1,
        "D" => 2,
        "D#" => 3,
        "E" => 4,
        "F" => 5,
        "F#" => 6,
        "G" => 7,
        "G#" => 8,
        "A" => 9,
        "A#" => 10,
        "B" => 11,
        _ => return None,
    };
    Some((oct.parse::<i32>().ok()? + 1) * 12 + base)
}

fn freq(m: i32) -> f32 {
    440.0 * 2f32.powf((m - 69) as f32 / 12.0)
}

/// One note event: start step, length in steps, pitch (Hz) or drum kind.
#[derive(Clone, Copy)]
enum Ev {
    Tone(f32),
    Kick,
    Snare,
    Hat,
}

#[derive(Default)]
struct Track {
    /// For every step: index of the event covering it (and whether this step starts it).
    events: Vec<(usize, usize, Ev)>,
    steps: usize,
}

impl Track {
    fn push(&mut self, len: usize, ev: Option<Ev>) {
        if let Some(ev) = ev {
            self.events.push((self.steps, len, ev));
        }
        self.steps += len;
    }

    /// Plain list: `C5:4 r:2 k:2 …`
    fn parse(src: &str) -> Track {
        let mut t = Track::default();
        for tok in src.split_whitespace() {
            let (n, len) = tok.split_once(':').unwrap_or((tok, "1"));
            let len: usize = len.parse().unwrap_or(1);
            let ev = match n {
                "r" => None,
                "k" => Some(Ev::Kick),
                "s" => Some(Ev::Snare),
                "h" => Some(Ev::Hat),
                _ => midi(n).map(|m| Ev::Tone(freq(m))),
            };
            t.push(len, ev);
        }
        t
    }

    /// Chord bars: `@C4E4G4` = one bar of 16th arpeggio (1-2-3-2), `%…` = 8th arpeggio.
    fn chords(src: &str) -> Track {
        if !src.contains(['@', '%']) {
            return Track::parse(src);
        }
        let mut t = Track::default();
        for tok in src.split_whitespace() {
            let (fast, body) = (tok.starts_with('@'), &tok[1..]);
            let mut notes = Vec::new();
            let mut rest = body;
            while !rest.is_empty() {
                let end = rest.find(|c: char| c.is_ascii_digit()).map_or(rest.len(), |i| i + 1);
                if let Some(m) = midi(&rest[..end]) {
                    notes.push(freq(m));
                }
                rest = &rest[end..];
            }
            if notes.len() < 3 {
                continue;
            }
            let order = [0, 1, 2, 1];
            let (count, len) = if fast { (16, 1) } else { (8, 2) };
            for k in 0..count {
                t.push(len, Some(Ev::Tone(notes[order[k % 4]])));
            }
        }
        t
    }

    /// Bass: a root per half bar (`C`, `A#`) as octave-jumping 8ths, or `!A` = a whole bar of
    /// driving 16ths. Plain note lists also work.
    fn bass(src: &str) -> Track {
        if src.contains(':') {
            return Track::parse(src);
        }
        let mut t = Track::default();
        for tok in src.split_whitespace() {
            let (drive, root) = match tok.strip_prefix('!') {
                Some(r) => (true, r),
                None => (false, tok),
            };
            let Some(lo) = midi(&format!("{root}2")) else { continue };
            if drive {
                for k in 0..16 {
                    t.push(1, Some(Ev::Tone(freq(if k % 2 == 0 { lo } else { lo + 12 }))));
                }
            } else {
                for k in 0..4 {
                    t.push(2, Some(Ev::Tone(freq(if k % 2 == 0 { lo } else { lo + 12 }))));
                }
            }
        }
        t
    }

    /// Repeat a short pattern to fill `steps`.
    fn fill(mut self, steps: usize) -> Track {
        if self.steps == 0 || self.steps >= steps {
            return self;
        }
        let pat = self.events.clone();
        let plen = self.steps;
        while self.steps < steps {
            for (s, l, e) in &pat {
                if self.steps + s < steps {
                    self.events.push((self.steps + s, *l, *e));
                }
            }
            self.steps += plen;
        }
        self.steps = steps;
        self
    }

    /// Event sounding at `step` (with its start step).
    fn at(&self, step: usize) -> Option<(usize, usize, Ev)> {
        // Events are sorted by start; binary search the last one starting at or before `step`.
        let i = self.events.partition_point(|(s, _, _)| *s <= step);
        let e = *self.events.get(i.checked_sub(1)?)?;
        (step < e.0 + e.1).then_some(e)
    }
}

// ---------------------------------------------------------------- synth

pub struct Song {
    tracks: [Track; 4],
    duty: f32,
    steps: usize,
    samples_per_step: f32,
    looped: bool,
    t: u64,
    phase: [f32; 3],
    lfsr: u16,
    noise: f32,
}

impl Song {
    pub fn new(cue: Cue) -> Song {
        let d = def(cue);
        let lead = Track::parse(d.lead);
        let steps = lead.steps;
        Song {
            tracks: [lead, Track::chords(d.harmony).fill(steps), Track::bass(d.bass).fill(steps), Track::parse(d.drums).fill(steps)],
            duty: d.lead_duty,
            steps,
            samples_per_step: RATE as f32 * 60.0 / d.bpm / 4.0,
            looped: d.looped,
            t: 0,
            phase: [0.0; 3],
            lfsr: 1,
            noise: 0.0,
        }
    }

    /// Envelope of a note `age` samples in, `len` samples long.
    fn env(age: f32, len: f32, decay: f32, sustain: f32) -> f32 {
        let a = (age / (0.002 * RATE as f32)).min(1.0);
        let d = sustain + (1.0 - sustain) * (-age / (decay * RATE as f32)).exp();
        let release = ((len - age) / (0.012 * RATE as f32)).clamp(0.0, 1.0);
        a * d * release
    }

    fn sample(&mut self) -> Option<f32> {
        let total = (self.steps as f32 * self.samples_per_step) as u64;
        if !self.looped && self.t >= total + RATE as u64 / 2 {
            return None;
        }
        let pos = if self.looped { self.t % total } else { self.t.min(total.saturating_sub(1)) } as f32;
        let ended = !self.looped && self.t >= total;
        self.t += 1;
        if ended {
            return Some(0.0);
        }
        let step = (pos / self.samples_per_step) as usize;
        let mut out = 0.0;
        // Pulse lead, pulse harmony, triangle bass.
        for ch in 0..3 {
            let Some((start, len, Ev::Tone(f))) = self.tracks[ch].at(step) else { continue };
            let age = pos - start as f32 * self.samples_per_step;
            let len = len as f32 * self.samples_per_step;
            self.phase[ch] = (self.phase[ch] + f / RATE as f32).fract();
            let p = self.phase[ch];
            let (wave, vol, decay, sus) = match ch {
                0 => (if p < self.duty { 1.0 } else { -1.0 }, 0.16, 0.25, 0.55),
                1 => (if p < 0.125 { 1.0 } else { -1.0 }, 0.07, 0.08, 0.35),
                _ => (((4.0 * (p - 0.5).abs() - 1.0) * 8.0).round() / 8.0, 0.30, 0.3, 0.8),
            };
            out += wave * vol * Self::env(age, len, decay, sus);
        }
        // Drums.
        if let Some((start, _, ev)) = self.tracks[3].at(step) {
            let age = pos - start as f32 * self.samples_per_step;
            let secs = age / RATE as f32;
            // 15-bit LFSR noise, clocked every few samples.
            if self.t % 3 == 0 {
                let bit = (self.lfsr ^ (self.lfsr >> 1)) & 1;
                self.lfsr = (self.lfsr >> 1) | (bit << 14);
                self.noise = if self.lfsr & 1 == 1 { 1.0 } else { -1.0 };
            }
            out += match ev {
                Ev::Kick => {
                    let f = 50.0 + 110.0 * (-secs * 30.0).exp();
                    (secs * f * std::f32::consts::TAU).sin() * 0.35 * (-secs * 18.0).exp()
                }
                Ev::Snare => self.noise * 0.10 * (-secs * 22.0).exp(),
                Ev::Hat => self.noise * 0.05 * (-secs * 70.0).exp(),
                Ev::Tone(_) => 0.0,
            };
        }
        Some(out * 0.8)
    }
}

impl Iterator for Song {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        self.sample()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl rodio::Source for Song {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        1
    }
    fn sample_rate(&self) -> u32 {
        RATE
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn songs_have_whole_bars_and_valid_notes() {
        for cue in [Cue::Title, Cue::Hub, Cue::Battle, Cue::Victory] {
            let s = Song::new(cue);
            assert_eq!(s.steps % 16, 0, "{cue:?}: lead is {} steps", s.steps);
            for (k, t) in s.tracks.iter().enumerate() {
                assert!(t.steps >= s.steps, "{cue:?} track {k}: {} < {}", t.steps, s.steps);
                assert!(!t.events.is_empty(), "{cue:?} track {k} is empty");
            }
            let peak = Song::new(cue).take(RATE as usize * 4).fold(0f32, |m, x| m.max(x.abs()));
            assert!(peak > 0.05 && peak <= 1.0, "{cue:?}: peak {peak}");
        }
    }
}
