//! Web build: no audio output yet (the game runs silently). Same interface as `audio.rs`.

use super::chiptune::Cue;
use crate::vcl::runtime::{Input, Media};
use std::sync::mpsc::Sender;

pub struct Audio;

impl Audio {
    pub fn new() -> Option<Audio> {
        None
    }
    pub fn sync_media(&mut self, _: &Media, _: &Sender<Input>) {}
    pub fn media_playing(&self) -> bool {
        false
    }
    pub fn set_cue(&mut self, _: Option<Cue>) {}
}
