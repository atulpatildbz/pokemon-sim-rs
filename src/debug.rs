//! Debug harness for the GUI (used to verify rendering without screen-capture permissions):
//! * `POKEGAME_SHOTS=<dir>` — save screenshots of the app's own frames.
//! * `POKEGAME_AUTOPLAY=<script>` — replay UI actions. One command per line:
//!   `wait <ms>` | `click <Form> <Control>` | `dblclick <Form> <Control>` | `menu <Form> <Item>` |
//!   `select <Form> <Control> <index>` | `text <Form> <Control> <text>` | `answer <modal result>` |
//!   `file <path>` (answers the next file dialog) | `shot <name>` | `quit`

use crate::vcl::runtime::{DialogAnswer, Event, Input, Snapshot};
use std::collections::VecDeque;
use std::io::Write;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

pub struct Harness {
    shots_dir: Option<std::path::PathBuf>,
    script: VecDeque<String>,
    next_at: Instant,
    pending_shot: Option<String>,
    pub pending_file: Option<String>,
    shot_count: usize,
}

impl Harness {
    pub fn from_env() -> Harness {
        let script = std::env::var("POKEGAME_AUTOPLAY")
            .ok()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|s| s.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty() && !l.starts_with('#')).collect())
            .unwrap_or_default();
        Harness {
            shots_dir: std::env::var_os("POKEGAME_SHOTS").map(Into::into),
            script,
            next_at: Instant::now() + Duration::from_millis(1500),
            pending_shot: None,
            pending_file: None,
            shot_count: 0,
        }
    }

    pub fn active(&self) -> bool {
        !self.script.is_empty() || self.pending_shot.is_some()
    }

    /// Run due script commands. Returns true if the app should quit.
    pub fn tick(&mut self, ctx: &egui::Context, snap: &Snapshot, tx: &Sender<Input>) -> bool {
        // Deliver screenshots requested earlier.
        let events = ctx.input(|i| i.raw.events.clone());
        for e in events {
            if let egui::Event::Screenshot { image, .. } = e {
                if let (Some(dir), Some(name)) = (&self.shots_dir, self.pending_shot.take()) {
                    let _ = std::fs::create_dir_all(dir);
                    self.shot_count += 1;
                    let path = dir.join(format!("{:02}_{name}.png", self.shot_count));
                    let _ = write_png(&path, image.width() as u32, image.height() as u32, image.as_raw());
                }
            }
        }
        if self.script.is_empty() || Instant::now() < self.next_at || self.pending_shot.is_some() {
            if self.active() {
                ctx.request_repaint_after(Duration::from_millis(50));
            }
            return false;
        }
        let line = self.script.pop_front().unwrap();
        let p: Vec<&str> = line.splitn(4, ' ').collect();
        let s = |i: usize| p.get(i).map(|x| x.to_string()).unwrap_or_default();
        self.next_at = Instant::now() + Duration::from_millis(250);
        match p[0] {
            "wait" => self.next_at = Instant::now() + Duration::from_millis(s(1).parse().unwrap_or(500)),
            "click" => {
                let _ = tx.send(Input::Event { form: s(1), control: s(2), event: Event::Click });
            }
            "dblclick" => {
                let _ = tx.send(Input::Event { form: s(1), control: s(2), event: Event::DblClick });
            }
            "menu" => {
                let _ = tx.send(Input::Menu { form: s(1), item: s(2) });
            }
            "select" => {
                let _ = tx.send(Input::SetItemIndex { form: s(1), control: s(2), index: s(3).parse().unwrap_or(-1) });
            }
            "text" => {
                let _ = tx.send(Input::SetText { form: s(1), control: s(2), text: s(3) });
            }
            "answer" => {
                if let Some((id, _)) = &snap.dialog {
                    let _ = tx.send(Input::Answer { id: *id, answer: DialogAnswer::Button(s(1).parse().unwrap_or(1)) });
                }
            }
            "file" => self.pending_file = Some(line[5..].to_string()),
            "shot" => {
                self.pending_shot = Some(s(1));
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
            "quit" => return true,
            _ => eprintln!("autoplay: unknown command {line}"),
        }
        ctx.request_repaint();
        false
    }
}

/// Minimal PNG writer (RGBA8, stored deflate blocks).
pub fn write_png(path: &std::path::Path, w: u32, h: u32, rgba: &[u8]) -> std::io::Result<()> {
    fn crc32(data: &[u8]) -> u32 {
        let mut c = 0xFFFF_FFFFu32;
        for &b in data {
            c ^= b as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
        }
        !c
    }
    fn chunk(out: &mut Vec<u8>, kind: &[u8], data: &[u8]) {
        out.extend((data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend(crc32(&body).to_be_bytes());
    }
    let mut raw = Vec::with_capacity((w * h * 4 + h) as usize);
    for y in 0..h {
        raw.push(0);
        let row = (y * w * 4) as usize;
        raw.extend_from_slice(&rgba[row..row + (w * 4) as usize]);
    }
    let mut z = vec![0x78, 0x01];
    for (i, block) in raw.chunks(65535).enumerate() {
        let last = (i + 1) * 65535 >= raw.len();
        z.push(last as u8);
        z.extend((block.len() as u16).to_le_bytes());
        z.extend((!(block.len() as u16)).to_le_bytes());
        z.extend_from_slice(block);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &raw {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    z.extend(((b << 16) | a).to_be_bytes());
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend(w.to_be_bytes());
    ihdr.extend(h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut png, b"IHDR", &ihdr);
    chunk(&mut png, b"IDAT", &z);
    chunk(&mut png, b"IEND", &[]);
    std::fs::File::create(path)?.write_all(&png)
}
