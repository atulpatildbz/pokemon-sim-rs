//! Emulated data segment of the original program (DATA + BSS, 0x48e000..0x4a9000).
//!
//! Every game global lives at its original address with its original width, and table
//! accessors do the same unchecked `base + index * size` arithmetic as the Delphi code. That makes
//! the original's out-of-bounds reads (e.g. tournament species 387..389) return exactly the same
//! neighbouring bytes. String variables hold fake heap pointers; dereferencing an invalid one
//! raises the same EAccessViolation the original raised (see recon/AV_NOTES.md).

use std::collections::HashMap;

pub const MEM_BASE: u32 = 0x0048_e000;
pub const MEM_END: u32 = 0x004a_9000;

/// Delphi exceptions the game can raise. `message()` reproduces SysUtils' text.
#[derive(Debug, Clone, PartialEq)]
pub enum Exc {
    AccessViolation { at: u32, read: u32 },
    /// Variant -> Integer conversion out of range etc. (RunError / EVariantError).
    Variant(String),
    InOut(i32),
    Convert(String),
}

impl Exc {
    /// The text Application.ShowException displays (with its trailing '.' rule applied).
    pub fn message(&self) -> String {
        let mut m = match self {
            Exc::AccessViolation { at, read } => format!(
                "Access violation at address {at:08X} in module 'PokeGame.exe'. Read of address {read:08X}"
            ),
            Exc::Variant(s) | Exc::Convert(s) => s.clone(),
            Exc::InOut(code) => format!("I/O error {code}"),
        };
        if m.chars().last().is_some_and(|c| c > '.') {
            m.push('.');
        }
        m
    }
}

pub type R<T> = Result<T, Exc>;

/// RTL fault sites (first instruction that dereferences a bad string argument).
pub mod fault {
    /// `_LStrCatN`: `ADD EAX,[ECX-4]`.
    pub const LSTR_CATN: u32 = 0x0040_4E03;
    /// `_LStrLAsg`: `MOV ECX,[EDX-8]`.
    pub const LSTR_LASG: u32 = 0x0040_4AB4;
    /// `_LStrAsg`: `MOV ECX,[EDX-8]`.
    pub const LSTR_ASG: u32 = 0x0040_4AF8;
    /// `_LStrAddRef`: `MOV EDX,[EAX-8]`.
    pub const LSTR_ADDREF: u32 = 0x0040_4F10;
}

/// A Delphi AnsiString value as read from memory: either a real string, or a pointer that
/// does not point at one (only produced by out-of-bounds table reads).
#[derive(Debug, Clone, PartialEq)]
pub enum DStr {
    S(String),
    Bad(u32),
}

impl DStr {
    /// Use as an argument of `_LStrCatN` (offset -4 is read first).
    pub fn catn(&self) -> R<&str> {
        match self {
            DStr::S(s) => Ok(s),
            DStr::Bad(p) => Err(Exc::AccessViolation { at: fault::LSTR_CATN, read: p.wrapping_sub(4) }),
        }
    }
    /// Use where the RTL first touches the reference count (assignment, AddRef: offset -8).
    pub fn refd(&self, site: u32) -> R<&str> {
        match self {
            DStr::S(s) => Ok(s),
            DStr::Bad(p) => Err(Exc::AccessViolation { at: site, read: p.wrapping_sub(8) }),
        }
    }
}

impl From<String> for DStr {
    fn from(s: String) -> DStr {
        DStr::S(s)
    }
}
impl From<&str> for DStr {
    fn from(s: &str) -> DStr {
        DStr::S(s.to_string())
    }
}

/// One operand of a string concatenation.
pub enum Part<'a> {
    Str(&'a str),
    D(&'a DStr),
}
impl<'a> From<&'a str> for Part<'a> {
    fn from(s: &'a str) -> Part<'a> {
        Part::Str(s)
    }
}
impl<'a> From<&'a String> for Part<'a> {
    fn from(s: &'a String) -> Part<'a> {
        Part::Str(s.as_str())
    }
}
impl<'a> From<&'a DStr> for Part<'a> {
    fn from(s: &'a DStr) -> Part<'a> {
        Part::D(s)
    }
}

/// `_LStrCatN` semantics: operands are scanned first to last; a bad pointer faults on its length.
pub fn catn(parts: &[Part]) -> R<String> {
    let mut out = String::new();
    for p in parts {
        match p {
            Part::Str(s) => out.push_str(s),
            Part::D(d) => out.push_str(d.catn()?),
        }
    }
    Ok(out)
}

/// `cat![a, b, c]` -> `_LStrCatN` of the operands (each `&str`, `&String` or `&DStr`).
#[macro_export]
macro_rules! cat {
    ($($p:expr),+ $(,)?) => {
        $crate::game::mem::catn(&[$($crate::game::mem::Part::from($p)),+])
    };
}

pub struct Mem {
    bytes: Vec<u8>,
    strings: HashMap<u32, String>,
    next_heap: u32,
}

/// Original addresses of the globals (see downloads/pokegame/globals.txt).
pub mod addr {
    // POKéGAME.dat tables. `_B` = address of element [0] of a 1-based array.
    pub const TYPE_EFF: u32 = 0x4925d0; // array[1..17, 0..17] of Single, row stride 72
    pub const PKMN_NAME_B: u32 = 0x490c5c; // string[1..386]
    pub const BADGE_NAME_B: u32 = 0x492ad8; // string[1..24]
    pub const PKMN_TYPE1_B: u32 = 0x4922c7; // Byte
    pub const PKMN_TYPE2_B: u32 = 0x49244b; // Byte
    pub const EVOLVE_LEVEL_B: u32 = 0x49186f; // Byte
    pub const EVOLVE_TO_B: u32 = 0x4919f0; // Integer
    pub const MOVE_NAME_B: u32 = 0x492070; // string[1..119]
    pub const MOVE_POWER_B: u32 = 0x491ffb; // Byte
    pub const MOVE_TYPE_B: u32 = 0x49224f; // Byte
    pub const TYPE_NAME_B: u32 = 0x492a94; // string[1..17]
    pub const LEARNSET: u32 = 0x491268; // array[1..386, 1..4] of Byte
    pub const PRIZE_TEXT: u32 = 0x4a8cd0; // array[1..6, 1..3] of string, row stride 12
    pub const TRAINER_NAME_B: u32 = 0x4a8d14; // string[1..64]

    // game state
    pub const PLAYER_NAME: u32 = 0x4a8c68;
    pub const IN_WILD_BATTLE: u32 = 0x48f37c;
    pub const IN_TRAINER_BATTLE: u32 = 0x48f380;
    pub const EVOLVE_MODE: u32 = 0x48f384;
    pub const RELEASE_MODE: u32 = 0x48f388;
    pub const IN_TOURNAMENT: u32 = 0x48f38c;
    pub const BEAT_LANCE: u32 = 0x48f390;
    pub const BEAT_ASH: u32 = 0x48f394;
    pub const BEAT_ME: u32 = 0x48f398;
    pub const BEAT_RBY_CHAMPION: u32 = 0x48f39c;
    pub const BEAT_RUSA_CHAMPION: u32 = 0x48f3a0;
    pub const BATTLE_CODE: u32 = 0x4a8c74;
    pub const BADGES: u32 = 0x4a8c75;
    pub const MONEY: u32 = 0x4a8c78;
    pub const WINS: u32 = 0x4a8c7c;
    pub const LOSSES: u32 = 0x4a8c80;
    pub const FIGHTS: u32 = 0x4a8c84;
    pub const CAPTURES: u32 = 0x4a8c88;
    pub const POKE_BALLS: u32 = 0x4a8c50;
    pub const GREAT_BALLS: u32 = 0x4a8c54;
    pub const ULTRA_BALLS: u32 = 0x4a8c58;
    pub const MASTER_BALLS: u32 = 0x4a8c5c;
    pub const POTIONS: u32 = 0x4a8c60;
    pub const SUPER_POTIONS: u32 = 0x4a8c64;
    pub const EVOLVE_SLOT: u32 = 0x4a8c6c;
    pub const G_4A8C70: u32 = 0x4a8c70;
    pub const OWN_HP: u32 = 0x4a8c8c;
    pub const ENEMY_HP: u32 = 0x4a8c90;
    pub const ENEMY_LEVEL: u32 = 0x4a8c94;
    pub const CUR_PKMN: u32 = 0x4a8c98;
    pub const ENEMY_SPECIES: u32 = 0x4a8c9c;
    pub const TOUR_TRAINER_B: u32 = 0x4a8c9f; // Byte[1..8]
    pub const TOUR_PKMN_B: u32 = 0x4a8ca4; // Integer[1..8]
    pub const TOUR_LEVEL_B: u32 = 0x4a8cc7; // Byte[1..8]
    pub const MAX_WILD_LEVEL: u32 = 0x4a8e18;
    pub const MIN_WILD_LEVEL: u32 = 0x4a8e19;
    pub const OWNED_B: u32 = 0x492b3b; // Boolean[1..386]
    pub const PC_SPECIES_B: u32 = 0x492cbc; // Integer[1..10000]
    pub const PC_LEVEL_B: u32 = 0x49c8ff; // Byte[1..10000]
    pub const PC_EXP_B: u32 = 0x49f00c; // Cardinal[1..10000]

    /// Unit initialization counters (`Dec(InitCount)` in every UnitInit): -1 after start-up.
    /// 0x490c5c (Unit11's) sits right before PkmnName[1], so PkmnName[0] reads $FFFFFFFF.
    pub const INIT_COUNTERS: &[u32] = &[0x4905b8, 0x490668, 0x490670, 0x490674, 0x490678, 0x49067c, 0x490790, 0x4907f8, 0x49081c, 0x49083c, 0x490840, 0x490844, 0x490854, 0x490888, 0x490890, 0x4908ec, 0x4908f0, 0x4908f4, 0x490920, 0x490934, 0x490938, 0x490940, 0x490944, 0x490968, 0x49097c, 0x490a3c, 0x490a4c, 0x490a50, 0x490a54, 0x490a58, 0x490a5c, 0x490a60, 0x490a64, 0x490a68, 0x490a6c, 0x490a70, 0x490aa8, 0x490aac, 0x490ab4, 0x490abc, 0x490ac0, 0x490af0, 0x490af4, 0x490af8, 0x490afc, 0x490b00, 0x490b0c, 0x490b10, 0x490b28, 0x490b30, 0x490b34, 0x490b40, 0x490b54, 0x490bcc, 0x490bd0, 0x490be4, 0x490bf4, 0x490c10, 0x490c14, 0x490c18, 0x490c1c, 0x490c20, 0x490c24, 0x490c2c, 0x490c34, 0x490c3c, 0x490c44, 0x490c4c, 0x490c54, 0x490c5c, 0x4a8e20, 0x4a8e28, 0x4a8e30];

    // VCL/RTL globals that sit next to game tables (object pointers; values are fake heap addresses)
    pub const FORM_GLOBALS: &[(u32, &str)] = &[
        (0x490bec, "Application"),
        (0x490bf0, "Screen"),
        (0x490c48, "Form3"),
        (0x4a8e2c, "Form1"),
        (0x490c0c, "AboutBox"),
        (0x4a8e1c, "Form4"),
        (0x490c28, "Form7"),
        (0x490c30, "Form8"),
        (0x490c40, "Form9"),
        (0x490c38, "Form5"),
        (0x4a8e24, "OKRightDlg"),
        (0x490c50, "FormA"),
        (0x490c58, "FormB"),
    ];
}

macro_rules! scalar {
    ($($get:ident, $set:ident, $addr:expr, $t:ident;)*) => {
        $(
            pub fn $get(&self) -> $t { self.$t($addr) }
            pub fn $set(&mut self, v: $t) { paste_set!(self, $t, $addr, v) }
        )*
    };
}
macro_rules! paste_set {
    ($s:ident, u8, $a:expr, $v:ident) => { $s.set_u8($a, $v) };
    ($s:ident, u32, $a:expr, $v:ident) => { $s.set_u32($a, $v) };
    ($s:ident, i32, $a:expr, $v:ident) => { $s.set_i32($a, $v) };
    ($s:ident, bool, $a:expr, $v:ident) => { $s.set_bool($a, $v) };
}

impl Mem {
    /// `data_section`: the original DATA bytes (VA 0x48e000..), so constants read out of bounds match.
    pub fn new(data_section: &[u8]) -> Mem {
        let mut bytes = vec![0u8; (MEM_END - MEM_BASE) as usize];
        let n = data_section.len().min(bytes.len());
        bytes[..n].copy_from_slice(&data_section[..n]);
        let mut m = Mem { bytes, strings: HashMap::new(), next_heap: 0x00D2_1000 };
        for a in addr::INIT_COUNTERS {
            m.set_u32(*a, 0xFFFF_FFFF);
        }
        // VCL object globals point into the (fake) heap, like the real program's instances.
        for (a, _) in addr::FORM_GLOBALS {
            let p = m.alloc(0x400);
            m.set_u32(*a, p);
        }
        m
    }

    fn off(&self, a: u32) -> Option<usize> {
        (MEM_BASE..MEM_END).contains(&a).then(|| (a - MEM_BASE) as usize)
    }

    /// Reads outside the emulated window return 0 (never reached by the game's index ranges).
    pub fn u8(&self, a: u32) -> u8 {
        self.off(a).map(|o| self.bytes[o]).unwrap_or(0)
    }
    pub fn set_u8(&mut self, a: u32, v: u8) {
        let o = self.off(a).unwrap_or_else(|| panic!("write outside data segment: {a:#x}"));
        self.bytes[o] = v;
    }
    pub fn u32(&self, a: u32) -> u32 {
        u32::from_le_bytes([self.u8(a), self.u8(a + 1), self.u8(a + 2), self.u8(a + 3)])
    }
    pub fn set_u32(&mut self, a: u32, v: u32) {
        for (k, b) in v.to_le_bytes().into_iter().enumerate() {
            self.set_u8(a + k as u32, b);
        }
    }
    pub fn i32(&self, a: u32) -> i32 {
        self.u32(a) as i32
    }
    pub fn set_i32(&mut self, a: u32, v: i32) {
        self.set_u32(a, v as u32)
    }
    pub fn bool(&self, a: u32) -> bool {
        self.u8(a) != 0
    }
    pub fn set_bool(&mut self, a: u32, v: bool) {
        self.set_u8(a, v as u8)
    }
    pub fn f32(&self, a: u32) -> f32 {
        f32::from_bits(self.u32(a))
    }
    pub fn set_f32(&mut self, a: u32, v: f32) {
        self.set_u32(a, v.to_bits())
    }

    fn alloc(&mut self, len: usize) -> u32 {
        // Delphi 7 MM: 4-byte block header, 8-byte string header, 4-byte granularity.
        let p = self.next_heap + 4 + 8;
        self.next_heap += ((4 + 8 + len as u32 + 1) + 3) & !3;
        p
    }

    /// Read a string variable/slot.
    pub fn str(&self, a: u32) -> DStr {
        let p = self.u32(a);
        if p == 0 {
            return DStr::S(String::new());
        }
        match self.strings.get(&p) {
            Some(s) => DStr::S(s.clone()),
            // Points at one of our fake objects: valid memory in the original, contents unknowable;
            // treated as empty (documented approximation).
            None if (0x00D2_1000..self.next_heap).contains(&p) => DStr::S(String::new()),
            None => DStr::Bad(p),
        }
    }
    /// A string that is known to be valid (in-range slots).
    pub fn s(&self, a: u32) -> String {
        match self.str(a) {
            DStr::S(s) => s,
            DStr::Bad(p) => panic!("unexpected bad string pointer {p:#x} at {a:#x}"),
        }
    }
    pub fn set_str(&mut self, a: u32, s: &str) {
        if s.is_empty() {
            self.set_u32(a, 0);
            return;
        }
        let p = self.alloc(s.len());
        self.strings.insert(p, s.to_string());
        self.set_u32(a, p);
    }

    // ---- 1-based table accessors (unchecked index arithmetic, like the original)
    fn ix(base: u32, i: i32, size: u32) -> u32 {
        base.wrapping_add((i as u32).wrapping_mul(size))
    }
    pub fn pkmn_name(&self, i: i32) -> DStr {
        self.str(Self::ix(addr::PKMN_NAME_B, i, 4))
    }
    pub fn badge_name(&self, i: i32) -> DStr {
        self.str(Self::ix(addr::BADGE_NAME_B, i, 4))
    }
    pub fn move_name(&self, i: i32) -> DStr {
        self.str(Self::ix(addr::MOVE_NAME_B, i, 4))
    }
    pub fn type_name(&self, i: i32) -> DStr {
        self.str(Self::ix(addr::TYPE_NAME_B, i, 4))
    }
    pub fn trainer_name(&self, i: i32) -> DStr {
        self.str(Self::ix(addr::TRAINER_NAME_B, i, 4))
    }
    pub fn prize_text(&self, tier: i32, choice: i32) -> DStr {
        self.str(addr::PRIZE_TEXT.wrapping_add((tier as u32).wrapping_mul(12)).wrapping_add((choice as u32).wrapping_mul(4)).wrapping_sub(16))
    }
    pub fn pkmn_type1(&self, i: i32) -> u8 {
        self.u8(Self::ix(addr::PKMN_TYPE1_B, i, 1))
    }
    pub fn pkmn_type2(&self, i: i32) -> u8 {
        self.u8(Self::ix(addr::PKMN_TYPE2_B, i, 1))
    }
    pub fn evolve_level(&self, i: i32) -> u8 {
        self.u8(Self::ix(addr::EVOLVE_LEVEL_B, i, 1))
    }
    pub fn evolve_to(&self, i: i32) -> i32 {
        self.i32(Self::ix(addr::EVOLVE_TO_B, i, 4))
    }
    pub fn move_power(&self, i: i32) -> u8 {
        self.u8(Self::ix(addr::MOVE_POWER_B, i, 1))
    }
    pub fn move_type(&self, i: i32) -> u8 {
        self.u8(Self::ix(addr::MOVE_TYPE_B, i, 1))
    }
    /// Learnset[species, slot] (slot 1..4): byte at LEARNSET + 4*species + slot - 5.
    pub fn learnset(&self, species: i32, slot: i32) -> u8 {
        self.u8(addr::LEARNSET.wrapping_add((species as u32).wrapping_mul(4)).wrapping_add(slot as u32).wrapping_sub(5))
    }
    /// TypeEff[atk, def]: Single at TYPE_EFF + (atk-1)*72 + def*4.
    pub fn type_eff(&self, atk: i32, def: i32) -> f32 {
        self.f32(addr::TYPE_EFF.wrapping_add(((atk - 1) as u32).wrapping_mul(72)).wrapping_add((def as u32).wrapping_mul(4)))
    }
    pub fn owned(&self, i: i32) -> bool {
        self.bool(Self::ix(addr::OWNED_B, i, 1))
    }
    pub fn set_owned(&mut self, i: i32, v: bool) {
        self.set_bool(Self::ix(addr::OWNED_B, i, 1), v)
    }
    pub fn pc_species(&self, i: i32) -> i32 {
        self.i32(Self::ix(addr::PC_SPECIES_B, i, 4))
    }
    pub fn set_pc_species(&mut self, i: i32, v: i32) {
        self.set_i32(Self::ix(addr::PC_SPECIES_B, i, 4), v)
    }
    pub fn pc_level(&self, i: i32) -> u8 {
        self.u8(Self::ix(addr::PC_LEVEL_B, i, 1))
    }
    pub fn set_pc_level(&mut self, i: i32, v: u8) {
        self.set_u8(Self::ix(addr::PC_LEVEL_B, i, 1), v)
    }
    pub fn pc_exp(&self, i: i32) -> u32 {
        self.u32(Self::ix(addr::PC_EXP_B, i, 4))
    }
    pub fn set_pc_exp(&mut self, i: i32, v: u32) {
        self.set_u32(Self::ix(addr::PC_EXP_B, i, 4), v)
    }
    pub fn tour_trainer(&self, i: i32) -> u8 {
        self.u8(Self::ix(addr::TOUR_TRAINER_B, i, 1))
    }
    pub fn set_tour_trainer(&mut self, i: i32, v: u8) {
        self.set_u8(Self::ix(addr::TOUR_TRAINER_B, i, 1), v)
    }
    pub fn tour_pkmn(&self, i: i32) -> i32 {
        self.i32(Self::ix(addr::TOUR_PKMN_B, i, 4))
    }
    pub fn set_tour_pkmn(&mut self, i: i32, v: i32) {
        self.set_i32(Self::ix(addr::TOUR_PKMN_B, i, 4), v)
    }
    pub fn tour_level(&self, i: i32) -> u8 {
        self.u8(Self::ix(addr::TOUR_LEVEL_B, i, 1))
    }
    pub fn set_tour_level(&mut self, i: i32, v: u8) {
        self.set_u8(Self::ix(addr::TOUR_LEVEL_B, i, 1), v)
    }

    scalar! {
        in_wild_battle, set_in_wild_battle, addr::IN_WILD_BATTLE, bool;
        in_trainer_battle, set_in_trainer_battle, addr::IN_TRAINER_BATTLE, bool;
        evolve_mode, set_evolve_mode, addr::EVOLVE_MODE, bool;
        release_mode, set_release_mode, addr::RELEASE_MODE, bool;
        in_tournament, set_in_tournament, addr::IN_TOURNAMENT, bool;
        beat_lance, set_beat_lance, addr::BEAT_LANCE, bool;
        beat_ash, set_beat_ash, addr::BEAT_ASH, bool;
        beat_me, set_beat_me, addr::BEAT_ME, bool;
        beat_rby_champion, set_beat_rby_champion, addr::BEAT_RBY_CHAMPION, bool;
        beat_rusa_champion, set_beat_rusa_champion, addr::BEAT_RUSA_CHAMPION, bool;
        battle_code, set_battle_code, addr::BATTLE_CODE, u8;
        badges, set_badges, addr::BADGES, u8;
        money, set_money, addr::MONEY, u32;
        wins, set_wins, addr::WINS, u32;
        losses, set_losses, addr::LOSSES, u32;
        fights, set_fights, addr::FIGHTS, u32;
        captures, set_captures, addr::CAPTURES, u32;
        poke_balls, set_poke_balls, addr::POKE_BALLS, u32;
        great_balls, set_great_balls, addr::GREAT_BALLS, u32;
        ultra_balls, set_ultra_balls, addr::ULTRA_BALLS, u32;
        master_balls, set_master_balls, addr::MASTER_BALLS, u32;
        potions, set_potions, addr::POTIONS, u32;
        super_potions, set_super_potions, addr::SUPER_POTIONS, u32;
        evolve_slot, set_evolve_slot, addr::EVOLVE_SLOT, i32;
        g_4a8c70, set_g_4a8c70, addr::G_4A8C70, i32;
        own_hp, set_own_hp, addr::OWN_HP, i32;
        enemy_hp, set_enemy_hp, addr::ENEMY_HP, i32;
        enemy_level, set_enemy_level, addr::ENEMY_LEVEL, u8;
        cur_pkmn, set_cur_pkmn, addr::CUR_PKMN, i32;
        enemy_species, set_enemy_species, addr::ENEMY_SPECIES, i32;
        max_wild_level, set_max_wild_level, addr::MAX_WILD_LEVEL, u8;
        min_wild_level, set_min_wild_level, addr::MIN_WILD_LEVEL, u8;
    }

    pub fn player_name(&self) -> String {
        self.s(addr::PLAYER_NAME)
    }
    pub fn set_player_name(&mut self, s: &str) {
        self.set_str(addr::PLAYER_NAME, s)
    }

    /// Fill the POKéGAME.dat tables (TForm3.TimerEventLoad stores).
    pub fn load_tables(&mut self, d: &crate::assets::GameData) {
        for i in 1..=17usize {
            for j in 0..=17usize {
                self.set_f32(addr::TYPE_EFF + (i as u32 - 1) * 72 + j as u32 * 4, d.type_eff[i][j]);
            }
        }
        for i in 1..=386i32 {
            let u = i as usize;
            self.set_str(Self::ix(addr::PKMN_NAME_B, i, 4), &d.pkmn_name[u]);
            self.set_u8(Self::ix(addr::PKMN_TYPE1_B, i, 1), d.pkmn_type1[u]);
            self.set_u8(Self::ix(addr::PKMN_TYPE2_B, i, 1), d.pkmn_type2[u]);
            self.set_u8(Self::ix(addr::EVOLVE_LEVEL_B, i, 1), d.evolve_level[u]);
            self.set_i32(Self::ix(addr::EVOLVE_TO_B, i, 4), d.evolve_to[u]);
            for k in 1..=4i32 {
                self.set_u8(
                    addr::LEARNSET + (i as u32) * 4 + k as u32 - 5,
                    d.learnset[u][k as usize],
                );
            }
        }
        for i in 1..=24i32 {
            self.set_str(Self::ix(addr::BADGE_NAME_B, i, 4), &d.badge_name[i as usize]);
        }
        for i in 1..=119i32 {
            self.set_str(Self::ix(addr::MOVE_NAME_B, i, 4), &d.move_name[i as usize]);
            self.set_u8(Self::ix(addr::MOVE_POWER_B, i, 1), d.move_power[i as usize]);
            self.set_u8(Self::ix(addr::MOVE_TYPE_B, i, 1), d.move_type[i as usize]);
        }
        for i in 1..=17i32 {
            self.set_str(Self::ix(addr::TYPE_NAME_B, i, 4), &d.type_name[i as usize]);
        }
        for t in 1..=6i32 {
            for c in 1..=3i32 {
                let a = addr::PRIZE_TEXT + (t as u32) * 12 + (c as u32) * 4 - 16;
                self.set_str(a, &d.prize_text[t as usize][c as usize]);
            }
        }
        for i in 1..=64i32 {
            self.set_str(Self::ix(addr::TRAINER_NAME_B, i, 4), &d.trainer_name[i as usize]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn out_of_bounds_name_faults_like_the_original() {
        let mut m = Mem::new(&[]);
        // Learnset[1] = (1,2,3,4) sits right after PkmnName[386].
        for (k, b) in [1u8, 2, 3, 4].into_iter().enumerate() {
            m.set_u8(addr::LEARNSET + k as u32, b);
        }
        m.set_str(Mem::ix(addr::PKMN_NAME_B, 386, 4), "Deoxys");
        assert_eq!(m.pkmn_name(386), DStr::S("Deoxys".into()));
        assert_eq!(m.pkmn_name(387), DStr::Bad(0x0403_0201));
        let e = catn(&[Part::from("x sent out "), Part::from(&m.pkmn_name(387)), Part::from("!")]).unwrap_err();
        assert_eq!(
            e.message(),
            "Access violation at address 00404E03 in module 'PokeGame.exe'. Read of address 040301FD."
        );
    }
}
