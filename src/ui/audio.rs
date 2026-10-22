//! Audio output: the original's TMediaPlayer (Music > Open file…, driven by the program's
//! `Media` state) and the front-end soundtrack. A player-chosen file always wins: while it
//! plays, the soundtrack is silent — the original only ever played that one file.
//!
//! Soundtrack cues use `<app data>/music/<cue>.{ogg,mp3,flac,wav}` when present (your own
//! tracks), else the built-in chiptune.

use super::chiptune::{Cue, Song};
use crate::vcl::runtime::{Input, Media};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::time::Duration;
use web_time::Instant;

pub struct Audio {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    /// The program's media file.
    media: Option<Sink>,
    media_seq: u64,
    media_file: String,
    media_len: Option<Duration>,
    last_report: Instant,
    /// Soundtrack.
    bgm: Option<(Cue, Sink)>,
}

fn custom_track(cue: Cue) -> Option<PathBuf> {
    let dir = super::settings::app_dir()?.join("music");
    ["ogg", "mp3", "flac", "wav"].iter().map(|e| dir.join(format!("{}.{e}", cue.name()))).find(|p| p.exists())
}

impl Audio {
    /// None when there is no audio device (the game runs silently).
    pub fn new() -> Option<Audio> {
        let (stream, handle) = OutputStream::try_default().ok()?;
        Some(Audio { _stream: stream, handle, media: None, media_seq: 0, media_file: String::new(), media_len: None, last_report: Instant::now(), bgm: None })
    }

    /// Follow the program's TMediaPlayer: Open (file change), Play, Stop, Rewind are all
    /// published as a new `command_seq`. Position/length go back as MediaStatus so Repeat works.
    pub fn sync_media(&mut self, m: &Media, tx: &Sender<Input>) {
        if m.command_seq != self.media_seq {
            self.media_seq = m.command_seq;
            let restart = m.file_name != self.media_file || m.position == 0;
            if restart {
                self.media = None;
                self.media_len = None;
                self.media_file = m.file_name.clone();
                if !m.file_name.is_empty() {
                    if let Ok(f) = File::open(&m.file_name) {
                        if let Ok(dec) = Decoder::new(BufReader::new(f)) {
                            self.media_len = dec.total_duration();
                            if let Ok(sink) = Sink::try_new(&self.handle) {
                                sink.append(dec);
                                sink.pause();
                                self.media = Some(sink);
                            }
                        }
                    }
                }
            }
            if let Some(s) = &self.media {
                if m.playing {
                    s.play();
                } else {
                    s.pause();
                }
            }
        }
        // Report progress ~5x a second while a file is loaded.
        if let Some(s) = &self.media {
            if self.last_report.elapsed() > Duration::from_millis(200) {
                self.last_report = Instant::now();
                let mut pos = s.get_pos();
                // Unknown length (some MP3s): once playback has ended, the end is where we are.
                let len = self.media_len.or(s.empty().then_some(pos));
                // The decoder's last frame can stop a hair short of its stated length; once the
                // sink has run dry the file is over, so report the end (the program's Repeat
                // check is Position >= Length).
                if s.empty() {
                    pos = pos.max(len.unwrap_or(pos));
                }
                let _ = tx.send(Input::MediaStatus { position: pos.as_millis() as i64, length: len.map_or(0, |d| d.as_millis() as i64) });
            }
        }
    }

    /// Whether the program's own music is audible right now.
    pub fn media_playing(&self) -> bool {
        self.media.as_ref().is_some_and(|s| !s.is_paused() && !s.empty())
    }

    /// Switch the soundtrack to `cue` (None = silence).
    pub fn set_cue(&mut self, cue: Option<Cue>) {
        if self.bgm.as_ref().map(|(c, _)| *c) == cue {
            return;
        }
        if let Some((_, s)) = self.bgm.take() {
            s.stop();
        }
        let Some(cue) = cue else { return };
        let Ok(sink) = Sink::try_new(&self.handle) else { return };
        let fade = Duration::from_millis(400);
        let custom = custom_track(cue).and_then(|p| File::open(p).ok());
        match custom {
            Some(f) if cue == Cue::Victory => match Decoder::new(BufReader::new(f)) {
                Ok(d) => sink.append(d),
                Err(_) => sink.append(Song::new(cue)),
            },
            Some(f) => match Decoder::new_looped(BufReader::new(f)) {
                Ok(d) => sink.append(d.fade_in(fade)),
                Err(_) => sink.append(Song::new(cue).fade_in(fade)),
            },
            // The jingle starts at full volume (rodio's fade_in asserts a non-zero duration).
            None if cue == Cue::Victory => sink.append(Song::new(cue)),
            None => sink.append(Song::new(cue).fade_in(fade)),
        }
        sink.set_volume(0.7);
        self.bgm = Some((cue, sink));
    }
}
