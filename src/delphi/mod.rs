//! Re-implementations of the Delphi 7 RTL behaviour the game depends on
//! (see downloads/pokegame/recon/RTL_SEMANTICS.md for the verified semantics).

pub mod f80;
pub use f80::F80;
pub mod rtl;
pub use rtl::{float_to_str, int64_to_str, int_to_str, DelphiRandom, Var};
