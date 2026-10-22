//! System / SysUtils / Variants behaviour, as verified in recon/RTL_SEMANTICS.md.

use super::f80::F80;

/// System.RandSeed + Random(n). `Random(n) = hi32(n * (seed := seed * $08088405 + 1))`.
#[derive(Debug, Clone)]
pub struct DelphiRandom {
    pub seed: i32,
}

impl DelphiRandom {
    pub fn new(seed: i32) -> DelphiRandom {
        DelphiRandom { seed }
    }

    /// Randomize: the original uses QueryPerformanceCounter's low dword (GetTickCount fallback).
    pub fn randomize(&mut self) {
        let nanos = web_time::SystemTime::now()
            .duration_since(web_time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        self.seed = (nanos as u64 ^ (nanos >> 64) as u64) as i32;
    }

    /// System.Random(Range: Integer): Integer (0x4030f8).
    pub fn random(&mut self, range: i32) -> i32 {
        self.seed = self.seed.wrapping_mul(0x0808_8405).wrapping_add(1);
        ((range as u32 as u64 * self.seed as u32 as u64) >> 32) as i32
    }
}

/// SysUtils.IntToStr(Integer).
pub fn int_to_str(v: i32) -> String {
    v.to_string()
}

/// SysUtils.IntToStr(Int64) as the game uses it (Cardinals zero-extended).
pub fn int64_to_str(v: i64) -> String {
    v.to_string()
}

/// SysUtils.FloatToStr: ffGeneral, 15 significant digits, '.' separator.
/// Scientific notation when the decimal exponent is > 15 or < -3 (e.g. `1E-5`).
pub fn float_to_str(v: f64) -> String {
    if v == 0.0 {
        return "0".into();
    }
    // 15 significant digits, correctly rounded from the binary value.
    let s = format!("{:.14e}", v.abs());
    let (mant, exp) = s.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    let digits: String = mant.chars().filter(|c| c.is_ascii_digit()).collect();
    let digits = digits.trim_end_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };
    // Delphi's exponent E: value = 0.d1d2.. * 10^E, so E = exp + 1.
    let e = exp + 1;
    let mut out = String::new();
    if v < 0.0 {
        out.push('-');
    }
    if e > 15 || e < -3 {
        out.push_str(&digits[..1]);
        if digits.len() > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        out.push('E');
        out.push_str(&(e - 1).to_string());
    } else if e <= 0 {
        out.push_str("0.");
        out.push_str(&"0".repeat((-e) as usize));
        out.push_str(digits);
    } else {
        let e = e as usize;
        if digits.len() <= e {
            out.push_str(digits);
            out.push_str(&"0".repeat(e - digits.len()));
        } else {
            out.push_str(&digits[..e]);
            out.push('.');
            out.push_str(&digits[e..]);
        }
    }
    out
}

/// SysUtils.DateToStr(Date) / TimeToStr(Time) with en-US ShortDateFormat 'M/d/yyyy' and
/// LongTimeFormat 'h:mm:ss AMPM' (the original uses the Windows locale).
pub fn date_time_strings() -> (String, String) {
    use std::process::Command;
    // Local time via the platform `date` utility (avoids a timezone database dependency).
    let out = Command::new("date").arg("+%m %d %Y %H %M %S").output().ok();
    let parts: Vec<u32> = out
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.split_whitespace().filter_map(|x| x.parse().ok()).collect())
        .unwrap_or_default();
    if parts.len() != 6 {
        return (String::new(), String::new());
    }
    let (mo, d, y, h, mi, se) = (parts[0], parts[1], parts[2], parts[3], parts[4], parts[5]);
    let (h12, ampm) = match h {
        0 => (12, "AM"),
        1..=11 => (h, "AM"),
        12 => (12, "PM"),
        _ => (h - 12, "PM"),
    };
    (format!("{mo}/{d}/{y}"), format!("{h12}:{mi:02}:{se:02} {ampm}"))
}

/// ShellExecute(Handle, 'open', FileName, nil, nil, SW_SHOWNORMAL).
pub fn shell_open(path: &str) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(path).spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd").args(["/C", "start", "", path]).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    // Web: the exported file was handed to the browser as a download (see `store`).
    #[cfg(target_arch = "wasm32")]
    let _ = path;
}

/// The subset of Delphi Variants the game uses: ordinal classes and Double.
/// `Int` covers varSmallint/varInteger/varShortInt/varByte; `I64` covers varWord/varLongWord/varInt64.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Var {
    Empty,
    Int(i32),
    I64(i64),
    Dbl(f64),
}

impl Var {
    /// _VarFromInt(V, value, Range): Range 1 = Byte, 2 = Word, 4 = LongWord, -4 = Integer.
    pub fn from_int(value: i32, range: i8) -> Var {
        match range {
            1 | -1 | -2 | -4 => Var::Int(value),
            2 => Var::I64(value as u16 as i64),
            4 => Var::I64(value as u32 as i64),
            _ => panic!("unexpected VarFromInt range {range}"),
        }
    }
    pub fn byte(v: u8) -> Var {
        Var::Int(v as i32)
    }
    pub fn integer(v: i32) -> Var {
        Var::Int(v)
    }
    pub fn longword(v: u32) -> Var {
        Var::I64(v as i64)
    }
    pub fn word(v: u16) -> Var {
        Var::I64(v as i64)
    }
    /// _VarFromReal: the extended value on the FPU stack is stored as a Double (rounded).
    pub fn real(v: F80) -> Var {
        Var::Dbl(v.to_f64())
    }

    /// _VarToReal as an exact extended value.
    pub fn to_f80(self) -> F80 {
        match self {
            Var::Empty => F80::ZERO,
            Var::Int(i) => F80::from_i64(i as i64),
            Var::I64(i) => F80::from_i64(i),
            Var::Dbl(d) => F80::from_f64(d),
        }
    }

    /// _VarToInt: doubles are rounded half-to-even (FISTP), then range checked.
    pub fn to_int(self) -> i32 {
        let v = self.to_i64();
        i32::try_from(v).unwrap_or_else(|_| panic!("Variant to Integer range error ({v})"))
    }

    /// _VarToInt64.
    pub fn to_i64(self) -> i64 {
        match self {
            Var::Empty => 0,
            Var::Int(i) => i as i64,
            Var::I64(i) => i,
            Var::Dbl(d) => F80::from_f64(d).round_i64(),
        }
    }

    fn class(self) -> u8 {
        match self {
            Var::Empty => 1,
            Var::Int(_) => 3,
            Var::I64(_) => 9,
            Var::Dbl(_) => 4,
        }
    }

    fn op(self, o: Var, which: u8) -> Var {
        let (a, b) = (self.class(), o.class());
        if a == 4 || b == 4 {
            // Float op: both sides to Double temporaries, FLD/FADD in extended, FSTP qword.
            let x = F80::from_f64(self.to_f80().to_f64());
            let y = F80::from_f64(o.to_f80().to_f64());
            let r = match which {
                0 => x.add(y),
                1 => x.sub(y),
                _ => x.mul(y),
            };
            return Var::Dbl(r.to_f64());
        }
        if a == 9 || b == 9 {
            let (x, y) = (self.to_i64(), o.to_i64());
            let r = match which {
                0 => x.checked_add(y),
                1 => x.checked_sub(y),
                _ => x.checked_mul(y),
            };
            return match r {
                Some(v) => Var::I64(v),
                None => Var::Dbl(match which {
                    0 => x as f64 + y as f64,
                    1 => x as f64 - y as f64,
                    _ => x as f64 * y as f64,
                }),
            };
        }
        // int32 class (or Empty, which behaves as integer 0).
        let (x, y) = (self.to_int(), o.to_int());
        let r = match which {
            0 => x.checked_add(y),
            1 => x.checked_sub(y),
            _ => x.checked_mul(y),
        };
        match r {
            Some(v) => Var::Int(v),
            None => {
                let (fx, fy) = (F80::from_i64(x as i64), F80::from_i64(y as i64));
                Var::Dbl(match which {
                    0 => fx.add(fy),
                    1 => fx.sub(fy),
                    _ => fx.mul(fy),
                }
                .to_f64())
            }
        }
    }
    pub fn add(self, o: Var) -> Var {
        self.op(o, 0)
    }
    pub fn sub(self, o: Var) -> Var {
        self.op(o, 1)
    }
    pub fn mul(self, o: Var) -> Var {
        self.op(o, 2)
    }

    /// VarCmp: numeric comparison (int32 / int64 / extended class).
    pub fn cmp(self, o: Var) -> std::cmp::Ordering {
        let (a, b) = (self.class(), o.class());
        if a == 4 || b == 4 {
            self.to_f80().cmp(o.to_f80())
        } else {
            self.to_i64().cmp(&o.to_i64())
        }
    }
    pub fn eq(self, o: Var) -> bool {
        self.cmp(o).is_eq()
    }
    pub fn lt(self, o: Var) -> bool {
        self.cmp(o).is_lt()
    }
    pub fn le(self, o: Var) -> bool {
        self.cmp(o).is_le()
    }
    pub fn gt(self, o: Var) -> bool {
        self.cmp(o).is_gt()
    }
    pub fn ge(self, o: Var) -> bool {
        self.cmp(o).is_ge()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_matches_delphi_lcg() {
        // Delphi 7 with RandSeed = 0: Random(100) sequence.
        let mut r = DelphiRandom::new(0);
        let seq: Vec<i32> = (0..5).map(|_| r.random(100)).collect();
        // seed: 1, 0x08088406, ... ; hi32(100 * seed)
        let mut s: u32 = 0;
        let want: Vec<i32> = (0..5)
            .map(|_| {
                s = s.wrapping_mul(0x0808_8405).wrapping_add(1);
                ((100u64 * s as u64) >> 32) as i32
            })
            .collect();
        assert_eq!(seq, want);
        assert_eq!(seq[0], 0);
    }

    #[test]
    fn float_to_str_general() {
        assert_eq!(float_to_str(0.0), "0");
        assert_eq!(float_to_str(0.5), "0.5");
        assert_eq!(float_to_str(0.25), "0.25");
        assert_eq!(float_to_str(4.0), "4");
        assert_eq!(float_to_str(77.99), "77.99");
        assert_eq!(float_to_str(12.5), "12.5");
        assert_eq!(float_to_str(100.0), "100");
        assert_eq!(float_to_str(0.01), "0.01");
        assert_eq!(float_to_str(1234567.0), "1234567");
        assert_eq!(float_to_str(0.0001), "0.0001");
        assert_eq!(float_to_str(0.00001), "1E-5");
        assert_eq!(float_to_str(1e20), "1E20");
        assert_eq!(float_to_str(-2.5), "-2.5");
        assert_eq!(float_to_str(66.67), "66.67");
    }

    #[test]
    fn variant_rules() {
        assert_eq!(Var::byte(3).add(Var::integer(4)), Var::Int(7));
        assert_eq!(Var::longword(3).add(Var::integer(4)), Var::I64(7));
        assert_eq!(Var::Dbl(2.5).to_int(), 2);
        assert_eq!(Var::Dbl(3.5).to_int(), 4);
        assert_eq!(Var::integer(i32::MAX).add(Var::integer(1)), Var::Dbl(2147483648.0));
        assert!(Var::Dbl(2.0).eq(Var::byte(2)));
        assert!(Var::Empty.eq(Var::integer(0)));
    }
}
