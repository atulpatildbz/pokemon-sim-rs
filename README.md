# POKéMON Simulator V4.5 — Rust port

Faithful Rust/egui port of BirdieSoft's *POKéMON Simulator V4.5* (Delphi 7, 2004), reconstructed
from the decompiled executable (see `../downloads/pokegame/recon/` for the Pascal reconstruction
and `../downloads/pokegame/PROGRESS.md` for the full log).

## Running

You need your own copy of the game: extract `PokeGame32.exe` (an NSIS installer), e.g. `7z x PokeGame32.exe`,
to get `PokeGame.exe` and `POKéGAME.dat`.

**Single binary (default).** `cargo build --release` compiles those two files into the executable
when it finds them (in `$POKEGAME_DIR`, or `../downloads/pokegame/extracted`):
`target/release/pokegame` then runs from anywhere with no other files. This is for personal use
only — the bundled files are copyrighted; don't publish such a binary.

**Folder-reading binary.** `cargo build --release --no-default-features` (or a build that could
not find the files) embeds nothing and reads the game folder at start-up: a folder argument,
`$POKEGAME_DIR`, the folder picked last time, the current/executable directory, else it asks once.

    cargo run --release                               # bundled build
    cargo run --release -- /path/to/extracted         # use this folder instead
    cargo run --release -- save.pgs                   # load a save (like passing it to the original)

Text measurement (the game positions HP bars from label widths) uses advance-width tables of
MS Sans Serif / Arial compiled into the program (`src/game/font_metrics.rs`), so the game behaves
identically on systems without those fonts.

## Android

    scripts/build-android.sh          # builds target/android/pokegame.apk, installs + starts it via adb

Needs the Android SDK (platform + build-tools), the NDK, `rustup target add aarch64-linux-android`
and `cargo install cargo-ndk`; the script finds them under `$ANDROID_HOME` (default: Homebrew's
`android-commandlinetools`). The APK is a NativeActivity (no Java), signed with
`~/.android/debug.keystore`, with the game files compiled in. Saves, settings and custom music
live in the app's internal storage. The launcher icon is drawn in code (`src/ui/icon.rs`;
regenerate with `GEN_ICONS=1 cargo test --test icons`). Two vendored one-line patches make egui
behave on phones (see `[patch.crates-io]` in `Cargo.toml`).

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

## Game UI

By default the game runs in a GBA-style skin: one full-screen scene (title / main window), other
windows as slide-up sheets (docked on the right on wide screens), a Pokémon-style text box for
messages, touch-sized buttons and a MENU sheet for the main menu. **F2** (or Options → Classic look)
switches back to the original Windows forms. Preferences live in `settings.txt` in the app data
folder, never in the save file. Fonts: Press Start 2P (OFL) and Pixel Operator (CC0), in `fonts/`.

**Screens.** Title, name entry, starter cards, hub, battle, PC/Statistics, Mart, POKéDEX, Type
Comparisons, Game Corner and the evolution sequence are drawn as game screens over the original
forms' controls; About and anything else fall back to the restyled original form.

**Music.** Music › Open file / Repeat / Stop work as in the original (your file always takes
priority). The game UI adds an original chiptune soundtrack (title, hub, battle, victory —
synthesized in `src/ui/chiptune.rs`, no audio files). To use your own tracks, put
`title`, `hub`, `battle` or `victory` `.ogg/.mp3/.flac/.wav` files in `<app data>/music/`.
Options › Soundtrack turns it off; Classic mode has none, like the original.

**Save slots.** The title screen offers Continue / New Game / Load with three slots in the app data
folder (`saves/slot1..3.pgs`, ordinary `.pgs` files). Load also has Import / Export to move a
`.pgs` between devices. Classic mode keeps the original file dialogs. `POKEGAME_DATA_DIR`
overrides the app data folder.

**Auto-save.** The original saves only when you quit. The game UI also saves automatically after
every action at safe points (main window up, no battle or tournament, no dialog), using the
original save format and serializer, written atomically. A mid-battle exit therefore loses only
that battle. `tests/golden.rs` checks that auto-saving never changes the game.

`POKEGAME_WINDOW=390x844` opens a phone-shaped window for trying the portrait layout.

## Known differences

- Help (`.hlp`, WinHelp) is not displayed — WinHelp no longer exists (also not on modern Windows).
- Music › Open file plays MP3, WAV, OGG and FLAC (the original also took MIDI and WMA, which are not supported).
- Window focus/activation side effects (Form5 stealing focus) are not modelled.
- Text width uses the TrueType *Microsoft Sans Serif* instead of the bitmap *MS Sans Serif*
  (affects label-relative positions by a pixel or two).

## Tests

    cargo test --release   # asset loading, F80 vectors, RTL, memory model, headless game flows

`tests/golden.rs` replays long seeded sessions through the same input path as the UI and compares a
fingerprint of the entire program state after every step with `tests/golden/*.txt`. It guards the
front-end work: any change in game behaviour fails it. Regenerate only for an intended behaviour
change: `UPDATE_GOLDEN=1 cargo test --release --test golden`.

Debug harness: `POKEGAME_SHOTS=<dir>` saves screenshots; `POKEGAME_AUTOPLAY=<script>` replays UI actions
(see `src/debug.rs`).
