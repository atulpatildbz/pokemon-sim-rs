//! Faithful Rust port of BirdieSoft's POKéMON Simulator V4.5 (Delphi 7, 2004).
//!
//! Game logic mirrors the original Delphi units 1:1 (see `units`), reconstructed from the
//! decompiled executable. Assets are loaded at runtime from the user's own copy.

pub mod assets;
pub mod vcl;
pub mod delphi;
pub mod game;
pub mod debug;
