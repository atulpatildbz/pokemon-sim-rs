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

## Releases

`scripts/release.sh` builds everything into `dist/`: a universal macOS `.app` (zipped), a Windows
x64 `.exe` (zipped, cross-compiled with `brew install mingw-w64` + `rustup target add
x86_64-pc-windows-gnu`) and an Android arm64 `.apk` signed with the release key
(`~/.android/pokemon-sim-release.{jks,pass}`, not in the repo — back it up: Android only accepts
updates signed with the same key, and updating in place is what keeps players' saves; the
APK's versionCode is derived from the Cargo version, so bump that for every release). These binaries contain the original game files: keep them
private.

## Android

    scripts/build-android.sh          # builds target/android/pokegame.apk, installs + starts it via adb

Needs the Android SDK (platform + build-tools), the NDK, `rustup target add aarch64-linux-android`
and `cargo install cargo-ndk`; the script finds them under `$ANDROID_HOME` (default: Homebrew's
`android-commandlinetools`). The APK is a NativeActivity (no Java), signed with
`~/.android/debug.keystore`, with the game files compiled in. Saves, settings and custom music
live in the app's internal storage. To bring a save onto a phone, put it at
`Android/data/com.atulpatildbz.pokegame/files/import/slotN.pgs` (e.g. `adb push`); it moves into
that slot on the next launch if the slot is empty. The launcher icon is drawn in code (`src/ui/icon.rs`;
regenerate with `GEN_ICONS=1 cargo test --test icons`). Two vendored one-line patches make egui
behave on phones (see `[patch.crates-io]` in `Cargo.toml`).

## Web

Play at https://atulpatildbz.github.io/pokemon-sim-rs/. Sign in with Google (invited accounts
only); saves sync to the account, so the same game continues on any browser. The page holds no game
files: the first time, it asks for `PokeGame.exe` and `POKéGAME.dat` from your own copy (extract
`PokeGame32.exe` with 7-Zip), keeps them in the browser (IndexedDB) and in the account's private
storage, so other devices download them instead of asking. Options (⚙) shows the account (Sign out)
and still downloads a slot as a `.pgs` file or replaces it with an uploaded one. No audio on the web
yet.

How sync works (`convex/`, `web/cloud.js`): before the game starts the newest version of every save
slot is merged into the browser; every save the game writes is pushed (`store::flush` calls
`window.pokegameFileChanged`); when another device saves while this one is open, the page asks
whether to load that save or keep this one. A browser that has signed in before plays offline and
syncs later. Settings stay per device. Sign-in is [Convex Auth](https://labs.convex.dev/auth) with
Google; `beforeSessionCreation` in `convex/auth.ts` refuses emails missing from the `allowedEmails`
table, and every query checks it again.

### What is deployed where

The web version has two halves, deployed separately:

| Part | Where | Deployed by |
|---|---|---|
| Page + game (wasm) | GitHub Pages | **automatic**: every push to `main` (`.github/workflows/pages.yml`) |
| Backend: sign-in, saves, game files (`convex/`) | Convex project `pokegame`, prod deployment `amicable-mouse-185` | **by hand**: `npx convex deploy` (pushing to `main` does *not* do it) |

- Convex dev deployment (for local testing): `charming-gazelle-410`. Dashboard:
  https://dashboard.convex.dev (team `atulpatildbz`, project `pokegame`).
- The Pages build points the page at the prod backend through the GitHub repository variable
  `CONVEX_URL` (= `https://amicable-mouse-185.convex.cloud`; Settings › Secrets and variables ›
  Actions › Variables). Without it the page runs with browser-only saves and no sign-in.
- Google sign-in uses the OAuth client named **Habitool** in Google Cloud project **"first"**
  (`gen-lang-client-0415070736`), Google Auth Platform › Clients. Its *Authorised redirect URIs*
  list both deployments' `https://<deployment>.convex.site/api/auth/callback/google`.
- Each Convex deployment holds its own environment variables (`npx convex env list [--prod]`) and
  its own data (users, saves, allowlist):

  | Variable | Value |
  |---|---|
  | `SITE_URL` | prod: `https://atulpatildbz.github.io/pokemon-sim-rs`; dev: `http://localhost:8000` (localhost is always accepted too) |
  | `AUTH_GOOGLE_ID`, `AUTH_GOOGLE_SECRET` | the Habitool OAuth client's ID and secret (same values as in the Habitool Convex project) |
  | `JWT_PRIVATE_KEY`, `JWKS` | session-token signing keys, generated by `node scripts/convex-keys.mjs [--prod]` |

### Redeploying

- **Changed Rust, `web/` or the build:** push to `main`. The Pages workflow builds and publishes
  (watch it under the repository's Actions tab).
- **Changed anything in `convex/`:** run `npx convex deploy` (asks for confirmation; `-y` skips it).
  Do this *before* pushing a page that relies on the new functions. Try it on dev first with
  `npx convex dev`.
- **Both:** `npx convex deploy`, then push.
- **Invite / remove someone:**
  `npx convex run --prod allowlist:grant '{"email":"someone@gmail.com"}'`
  (`allowlist:revoke`, `allowlist:list`). Takes effect immediately, no redeploy.

### Testing locally

    npm ci                                    # Convex CLI + Convex's browser bundle
    npx convex dev --once                     # push convex/ to the dev deployment
    POKEGAME_CONVEX_URL=https://charming-gazelle-410.convex.cloud scripts/build-web.sh
    cd dist/web && python3 -m http.server 8000          # open http://localhost:8000

The dev deployment has its own allowlist and data, so testing never touches real saves.
`scripts/build-web.sh` alone (no `POKEGAME_CONVEX_URL`) builds the browser-only version.

Building needs nightly Rust (`rustup target add wasm32-unknown-unknown --toolchain nightly`,
`rustup component add rust-src --toolchain nightly`) and `wasm-bindgen-cli` at the version in
`Cargo.lock`. The program's logic thread runs in a Web Worker (wasm threads), which needs
cross-origin isolation; `web/coi.js` is a service worker that adds those headers, since GitHub
Pages cannot. `build.rs` never bundles the game files into a web build.

### Setting up a new Convex deployment (if one is ever lost or replaced)

1. `npx convex deploy` (prod) or `npx convex dev --configure` (dev) to create it and push `convex/`.
2. `node scripts/convex-keys.mjs --prod` (signing keys; omit `--prod` for dev).
3. `npx convex env set --prod SITE_URL https://atulpatildbz.github.io/pokemon-sim-rs`
4. Google keys, copied from the Habitool Convex project (run from this folder; prints only
   "Successfully set"):
   `for k in AUTH_GOOGLE_ID AUTH_GOOGLE_SECRET; do npx convex env set --prod $k -- "$(cd ../Habitool && npx convex env get $k)"; done`
5. Add `https://<new-deployment>.convex.site/api/auth/callback/google` to the Habitool OAuth
   client's redirect URIs (see above) and Save; it can take a few minutes to apply.
6. Invite the players again (`allowlist:grant`, one per email).
7. Prod only: set the repository variable `CONVEX_URL` to `https://<new-deployment>.convex.cloud`
   (`gh variable set CONVEX_URL --body …`) and re-run the Pages workflow (or push).

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
folder, never in the save file. Fonts: Figtree (OFL), Press Start 2P (OFL) and Pixel Operator (CC0), in `fonts/`.

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

**Auto-save.** The original saves only when you quit the main window. The game UI also saves
after every action, writing exactly what quitting at that moment would write (TForm4.FormClose
with the original serializer, atomically): mid-battle that is the state as it stands, and in a
tournament the quit penalty is included (applied to a copy, never to the running game). An app
the system kills therefore behaves like a player who quit. `tests/golden.rs` checks auto-save
never changes the game; `tests/headless.rs` checks the tournament case byte for byte.

**Colour schemes.** Options › Color scheme: Sky (default), Pocket (four-shade handheld green), Lava,
Forest, Sakura and Ocean, plus four dark ones for playing at night: Twilight (dusky indigo),
Midnight (neutral slate), Ember (warm amber, little blue light) and Abyss (true black for OLED
screens). Defined in `src/ui/theme.rs` (`PALETTES`).

**Text.** Options › Text: Clear (default; Figtree, OFL) or Pixel (the GBA pixel fonts). The logo
keeps its pixel font either way.

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
