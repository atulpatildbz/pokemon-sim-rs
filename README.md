# POKéMON Simulator V4.5 — Rust port

Faithful Rust/egui port of BirdieSoft's *POKéMON Simulator V4.5* (Delphi 7, 2004), reconstructed
from the decompiled executable (see `../downloads/pokegame/recon/` for the Pascal reconstruction
and `../downloads/pokegame/PROGRESS.md` for the full log).

## Running

You need your own copy of the game: extract `PokeGame32.exe` (an NSIS installer), e.g. `7z x PokeGame32.exe`,
to get `PokeGame.exe` and `POKéGAME.dat`. Nothing copyrighted is embedded in this program — sprites,
form layouts and game data are read from those files at start-up.

    cargo run --release -- /path/to/extracted        # or set POKEGAME_DIR
    cargo run --release -- save.pgs                   # load a save (like passing it to the original)

## How it is faithful

- **Logic**: every event handler is a 1:1 port of the original (addresses cited in the source).
- **Arithmetic**: x87 80-bit extended math is emulated in software (`delphi::F80`, verified bit-exact
  against exact rational arithmetic); Delphi `Random` LCG, `Int`/`Round`, Variant promotion rules,
  Byte/Cardinal wraparound, `FloatToStr`.
- **Memory**: game globals live in an emulated data segment at their original addresses, so the
  original's out-of-bounds reads (tournament species 387–389) behave identically, including the
  access-violation error box and its exact text.
- **UI**: forms are built from the original DFM resources and rendered with egui; modal dialogs,
  timers and re-entrancy follow the VCL message loop.

## Known differences

- Help (`.hlp`, WinHelp) is not displayed — WinHelp no longer exists (also not on modern Windows).
- Music menu: file selection/repeat logic is ported; audio playback is not implemented yet.
- Window focus/activation side effects (Form5 stealing focus) are not modelled.
- Text width uses the TrueType *Microsoft Sans Serif* instead of the bitmap *MS Sans Serif*
  (affects label-relative positions by a pixel or two).

## Tests

    cargo test        # asset loading, F80 vectors, RTL, memory model, headless game flows

Debug harness: `POKEGAME_SHOTS=<dir>` saves screenshots; `POKEGAME_AUTOPLAY=<script>` replays UI actions
(see `src/debug.rs`).
