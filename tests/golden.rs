//! Golden traces: long seeded "monkey" sessions that drive the game exactly like the UI does
//! (`World::handle_input`), fingerprinting the whole program state after every step. Any change
//! to game behaviour — RNG draws, rounding, captions, save bytes — shows up as the first step
//! whose fingerprint differs from `tests/golden/*.txt`.
//!
//! Regenerate (only when a behaviour change is intended!): `UPDATE_GOLDEN=1 cargo test --release --test golden`

use pokegame::assets::Assets;
use pokegame::game::units;
use pokegame::game::world::*;
use pokegame::vcl::runtime::{DialogAnswer, Event, Input};
use pokegame::vcl::{Form, Kind, MenuItem};
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn game_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("POKEGAME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("../downloads/pokegame/extracted"));
    dir.join("PokeGame.exe").exists().then_some(dir)
}

fn world(dir: &Path) -> World {
    let assets = Arc::new(Assets::load(dir).unwrap());
    let exe = std::fs::read(dir.join("PokeGame.exe")).unwrap();
    let pe = pokegame::assets::pe::Pe::parse(exe).unwrap();
    let data = pe.sections.iter().find(|s| s.name == "DATA").map(|s| pe.data[s.raw_ptr as usize..(s.raw_ptr + s.raw_size) as usize].to_vec()).unwrap();
    let mut w = World::new(assets, &data, None);
    w.show(FORM3).unwrap();
    units::dispatch(&mut w, FORM3, "TimerEventLoad").unwrap();
    w.rng.seed = 12345;
    w
}

/// Driver RNG (independent of the game's Delphi RNG).
struct Lcg(u64);
impl Lcg {
    fn next(&mut self, n: usize) -> usize {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as usize) % n.max(1)
    }
}

struct Action {
    desc: String,
    inputs: Vec<Input>,
    weight: usize,
}

fn ancestors_ok(form: &Form, i: usize) -> bool {
    let mut cur = Some(i);
    while let Some(c) = cur {
        if !form.controls[c].visible || !form.controls[c].enabled {
            return false;
        }
        cur = form.controls[c].parent;
    }
    true
}

fn menu_actions(form: &Form, items: &[MenuItem], out: &mut Vec<Action>) {
    for m in items.iter().filter(|m| m.visible && m.enabled) {
        if !m.children.is_empty() {
            menu_actions(form, &m.children, out);
        } else if m.on_click.is_some() && !m.name.starts_with("Quit") && !m.name.starts_with("Export") {
            // Quit ends the session; Export only writes a timestamped HTML file and opens it.
            let meta = ["Openfile1", "Contents1", "About1", "RepeatMusic1", "StopMusic1"].contains(&m.name.as_str());
            let weight = if meta { 1 } else { 4 };
            out.push(Action { desc: format!("menu {}.{}", form.var, m.name), inputs: vec![Input::Menu { form: form.var.clone(), item: m.name.clone() }], weight });
        }
    }
}

/// Everything a player could click/select right now.
fn actions(w: &World, rng: &mut Lcg) -> Vec<Action> {
    let mut out = Vec::new();
    for form in w.forms.iter().filter(|f| f.visible) {
        let fv = form.var.clone();
        menu_actions(form, &form.menu, &mut out);
        if form.close_box && !matches!(fv.as_str(), "Form3" | "Form4" | "Form1") {
            out.push(Action { desc: format!("close {fv}"), inputs: vec![Input::CloseForm { form: fv.clone() }], weight: 1 });
        }
        for (i, c) in form.controls.iter().enumerate() {
            if !ancestors_ok(form, i) {
                continue;
            }
            let ev = |e: Event| Input::Event { form: fv.clone(), control: c.name.clone(), event: e };
            let set = |k: i32| Input::SetItemIndex { form: fv.clone(), control: c.name.clone(), index: k };
            let name = format!("{fv}.{}", c.name);
            match c.kind {
                Kind::Button | Kind::Image if c.events.on_click.is_some() => {
                    if c.name.contains("Exit") || c.name.contains("Help") {
                        continue;
                    }
                    // Battle attacks are what a session is mostly made of.
                    let weight = if fv == "Form4" && c.name == "Button1" { 30 } else { 3 };
                    out.push(Action { desc: format!("click {name}"), inputs: vec![ev(Event::Click)], weight });
                }
                Kind::RadioGroup if !c.items.is_empty() => {
                    let k = rng.next(c.items.len()) as i32;
                    let mut inputs = vec![set(k)];
                    if c.events.on_click.is_some() {
                        inputs.push(ev(Event::Click));
                    }
                    out.push(Action { desc: format!("radio {name} {k}"), inputs, weight: 4 });
                }
                Kind::ListView if !c.list_items.is_empty() => {
                    let k = rng.next(c.list_items.len()) as i32;
                    if c.events.on_click.is_some() {
                        out.push(Action { desc: format!("lv {name} {k}"), inputs: vec![set(k), ev(Event::Click)], weight: 4 });
                    }
                    if c.events.on_dbl_click.is_some() {
                        out.push(Action { desc: format!("lvdbl {name} {k}"), inputs: vec![set(k), ev(Event::DblClick)], weight: 2 });
                    }
                }
                Kind::ComboBox if !c.items.is_empty() => {
                    let k = rng.next(c.items.len()) as i32;
                    let mut inputs = vec![set(k)];
                    if c.events.on_change.is_some() {
                        inputs.push(ev(Event::Change));
                    }
                    out.push(Action { desc: format!("combo {name} {k}"), inputs, weight: 1 });
                }
                Kind::SpinEdit => {
                    let lo = c.min_value;
                    let hi = if c.max_value > lo { c.max_value } else { lo + 100 };
                    let v = lo + rng.next((hi - lo + 1) as usize) as i32;
                    let mut inputs = vec![Input::SetValue { form: fv.clone(), control: c.name.clone(), value: v }];
                    if c.events.on_change.is_some() {
                        inputs.push(ev(Event::Change));
                    }
                    out.push(Action { desc: format!("spin {name} {v}"), inputs, weight: 1 });
                }
                Kind::Edit if !c.read_only => {
                    let t = ["Red", "", "Gary", "x"][rng.next(4)].to_string();
                    let mut inputs = vec![Input::SetText { form: fv.clone(), control: c.name.clone(), text: t.clone() }];
                    if c.events.on_change.is_some() {
                        inputs.push(ev(Event::Change));
                    }
                    out.push(Action { desc: format!("text {name} {t:?}"), inputs, weight: 1 });
                }
                Kind::Timer if c.enabled && c.events.on_timer.is_some() => {
                    out.push(Action { desc: "timers".into(), inputs: vec![], weight: 2 });
                }
                _ => {}
            }
        }
    }
    out
}

fn fnv(h: &mut u64, bytes: &[u8]) {
    for b in bytes {
        *h ^= *b as u64;
        *h = h.wrapping_mul(0x100000001b3);
    }
}

fn replace_all(hay: &[u8], needle: &[u8], with: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(hay.len());
    let mut i = 0;
    while i < hay.len() {
        if !needle.is_empty() && hay[i..].starts_with(needle) {
            out.extend_from_slice(with);
            i += needle.len();
        } else {
            out.push(hay[i]);
            i += 1;
        }
    }
    out
}

/// Hash of the whole observable program state. The save path is machine-specific, so it is
/// normalised out.
fn fingerprint(w: &World, save: &str) -> u64 {
    let mut buf = Vec::new();
    w.m.digest_into(&mut buf);
    buf.extend_from_slice(&w.rng.seed.to_le_bytes());
    for f in &w.forms {
        let s = format!("{}|{}|{}|{}|{}|{}|{:?}|{:?}", f.var, f.caption, f.visible, f.left, f.top, f.modal_result, f.controls, f.menu);
        buf.extend_from_slice(s.as_bytes());
    }
    buf.extend_from_slice(format!("{:?}|{:?}|{}", w.headless_log, w.media, w.terminated).as_bytes());
    let buf = replace_all(&buf, save.as_bytes(), b"<SAVE>");
    let mut h = 0xcbf29ce484222325;
    fnv(&mut h, &buf);
    if let Ok(bytes) = std::fs::read(save) {
        fnv(&mut h, &bytes);
    }
    h
}

fn run_trace(dir: &Path, seed: u64, steps: usize) -> Vec<String> {
    run_trace_with(dir, seed, steps, |_| {})
}

/// `each_step` runs before every step (test-only state pokes, e.g. topping up money).
fn run_trace_with(dir: &Path, seed: u64, steps: usize, each_step: impl Fn(&mut World)) -> Vec<String> {
    let save = std::env::temp_dir().join(format!("pg_golden_{seed}.pgs")).to_string_lossy().into_owned();
    let _ = std::fs::remove_file(&save);
    let mut w = world(dir);
    let mut rng = Lcg(seed);
    w.c(OK_RIGHT_DLG, "Edit1").text = "Red".into();
    let mut lines = Vec::new();
    for step in 0..steps {
        if w.terminated {
            break;
        }
        each_step(&mut w);
        // Scripted answers for any dialogs this step opens.
        w.headless_answers.clear();
        for _ in 0..8 {
            w.headless_answers.push(match rng.next(6) {
                0 | 1 => DialogAnswer::Button(6),
                2 => DialogAnswer::Button(7),
                3 => DialogAnswer::Button(2),
                4 => DialogAnswer::File(Some(save.clone())),
                _ => DialogAnswer::Button(1),
            });
        }
        let acts = actions(&w, &mut rng);
        let total: usize = acts.iter().map(|a| a.weight).sum();
        if total == 0 {
            lines.push(format!("{step} <no actions>"));
            break;
        }
        let mut pick = rng.next(total);
        let act = acts.iter().find(|a| {
            if pick < a.weight {
                true
            } else {
                pick -= a.weight;
                false
            }
        }).unwrap();
        if act.desc == "timers" {
            w.fire_timers_now();
        }
        for i in &act.inputs {
            w.handle_input(i.clone());
        }
        lines.push(format!("{step} {} {:016x}", act.desc, fingerprint(&w, &save)));
    }
    let _ = std::fs::remove_file(&save);
    lines
}

const SEEDS: [u64; 5] = [1, 2, 3, 4, 5];
const STEPS: usize = 1500;

#[test]
fn golden_traces() {
    let Some(dir) = game_dir() else {
        eprintln!("skipping: game files not found");
        return;
    };
    let golden_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let update = std::env::var_os("UPDATE_GOLDEN").is_some();
    for seed in SEEDS {
        // Seed 5 starts rich so the Mart and battle items get exercised.
        let lines = if seed == 5 { run_trace_with(&dir, seed, STEPS, |w| {
            if w.visible(FORM4) && w.m.money() < 50_000 {
                w.m.set_money(999_999);
            }
        }) } else { run_trace(&dir, seed, STEPS) };
        let path = golden_dir.join(format!("trace_{seed}.txt"));
        let text = lines.join("\n") + "\n";
        if update {
            std::fs::create_dir_all(&golden_dir).unwrap();
            std::fs::write(&path, &text).unwrap();
            continue;
        }
        let expected = std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("missing {}; run with UPDATE_GOLDEN=1", path.display()));
        for (i, (got, want)) in text.lines().zip(expected.lines()).enumerate() {
            assert_eq!(got, want, "trace {seed} diverges at line {i}");
        }
        assert_eq!(text.lines().count(), expected.lines().count(), "trace {seed} length differs");
    }
}

/// The trace itself must be reproducible, or the golden files are meaningless.
#[test]
fn golden_trace_is_deterministic() {
    let Some(dir) = game_dir() else { return };
    assert_eq!(run_trace(&dir, 99, 400), run_trace(&dir, 99, 400));
}
