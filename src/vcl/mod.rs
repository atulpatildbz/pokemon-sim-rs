//! Minimal VCL emulation: component model built from DFMs, egui renderer, and the
//! logic/UI threading bridge.

pub mod build;
pub mod model;
pub mod render;
pub mod runtime;

pub use model::{Control, Form, Kind, ListItem, MenuItem, PictureRef};
