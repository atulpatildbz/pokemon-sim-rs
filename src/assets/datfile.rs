//! POKéGAME.dat loader — mirrors TForm3.TimerEventLoad (@00481b90) read order and storage widths.
//!
//! Every array keeps the original Pascal 1-based indexing: element 0 is unused padding,
//! so ported code can index exactly as the Delphi source did.

pub const NUM_PKMN: usize = 386;
pub const NUM_MOVES: usize = 119;
pub const NUM_TYPES: usize = 17;

#[derive(Debug, Clone)]
pub struct GameData {
    /// TypeEff[row 1..17][col 0..17] as Single; col 0 is forced to 1.0 by the loader.
    pub type_eff: [[f32; 18]; NUM_TYPES + 1],
    pub pkmn_name: Vec<String>,
    pub badge_name: Vec<String>,
    pub pkmn_type1: Vec<u8>,
    pub pkmn_type2: Vec<u8>,
    pub evolve_level: Vec<u8>,
    pub evolve_to: Vec<i32>,
    pub move_name: Vec<String>,
    pub move_power: Vec<u8>,
    pub move_type: Vec<u8>,
    pub type_name: Vec<String>,
    /// Learnset[pkmn 1..386][slot 1..4] (move ids).
    pub learnset: Vec<[u8; 5]>,
    /// PrizeText[1..6][1..3].
    pub prize_text: Vec<[String; 4]>,
    pub trainer_name: Vec<String>,
}

struct Lines<'a> {
    it: std::slice::Iter<'a, String>,
    line: usize,
}

impl Lines<'_> {
    fn next(&mut self) -> Result<&str, String> {
        self.line += 1;
        self.it.next().map(String::as_str).ok_or_else(|| format!("POKéGAME.dat ended early at line {}", self.line))
    }
    /// Readln(f, s: string): the whole line.
    fn string(&mut self) -> Result<String, String> {
        Ok(self.next()?.to_string())
    }
    /// Read(f, i: Integer); Readln(f) — first whitespace-delimited token.
    fn int(&mut self) -> Result<i32, String> {
        let line = self.line + 1;
        let tok = self.next()?.split_whitespace().next().unwrap_or("");
        tok.parse::<i64>().map(|v| v as i32).map_err(|_| format!("POKéGAME.dat line {line}: bad integer {tok:?}"))
    }
    /// Read(f, x: Extended); Readln(f).
    fn real(&mut self) -> Result<f64, String> {
        let line = self.line + 1;
        let tok = self.next()?.split_whitespace().next().unwrap_or("");
        tok.parse::<f64>().map_err(|_| format!("POKéGAME.dat line {line}: bad number {tok:?}"))
    }
}

impl GameData {
    pub fn parse(raw: &[u8]) -> Result<GameData, String> {
        let text = super::dfm::latin1(raw);
        let lines: Vec<String> = text.lines().map(|l| l.trim_end_matches('\r').to_string()).collect();
        let mut r = Lines { it: lines.iter(), line: 0 };

        let mut type_eff = [[0f32; 18]; NUM_TYPES + 1];
        for row in type_eff.iter_mut().skip(1) {
            row[0] = 1.0;
            for cell in row.iter_mut().skip(1) {
                *cell = r.real()? as f32;
            }
        }
        let strings = |r: &mut Lines, n: usize| -> Result<Vec<String>, String> {
            std::iter::once(Ok(String::new())).chain((0..n).map(|_| r.string())).collect()
        };
        let pkmn_name = strings(&mut r, NUM_PKMN)?;
        let badge_name = strings(&mut r, 24)?;
        let mut pkmn_type1 = vec![0u8; NUM_PKMN + 1];
        let mut pkmn_type2 = vec![0u8; NUM_PKMN + 1];
        for i in 1..=NUM_PKMN {
            pkmn_type1[i] = r.int()? as u8;
            pkmn_type2[i] = r.int()? as u8;
        }
        let mut evolve_level = vec![0u8; NUM_PKMN + 1];
        for v in evolve_level.iter_mut().skip(1) {
            *v = r.int()? as u8;
        }
        let mut evolve_to = vec![0i32; NUM_PKMN + 1];
        for v in evolve_to.iter_mut().skip(1) {
            *v = r.int()?;
        }
        let move_name = strings(&mut r, NUM_MOVES)?;
        let mut move_power = vec![0u8; NUM_MOVES + 1];
        for v in move_power.iter_mut().skip(1) {
            *v = r.int()? as u8;
        }
        let mut move_type = vec![0u8; NUM_MOVES + 1];
        for v in move_type.iter_mut().skip(1) {
            *v = r.int()? as u8;
        }
        let type_name = strings(&mut r, NUM_TYPES)?;
        let mut learnset = vec![[0u8; 5]; NUM_PKMN + 1];
        for set in learnset.iter_mut().skip(1) {
            for slot in set.iter_mut().skip(1) {
                *slot = r.int()? as u8;
            }
        }
        let mut prize_text = vec![Default::default(); 7];
        for row in prize_text.iter_mut().skip(1) {
            let row: &mut [String; 4] = row;
            for cell in row.iter_mut().skip(1) {
                *cell = r.string()?;
            }
        }
        let trainer_name = strings(&mut r, 64)?;
        Ok(GameData {
            type_eff,
            pkmn_name,
            badge_name,
            pkmn_type1,
            pkmn_type2,
            evolve_level,
            evolve_to,
            move_name,
            move_power,
            move_type,
            type_name,
            learnset,
            prize_text,
            trainer_name,
        })
    }
}
