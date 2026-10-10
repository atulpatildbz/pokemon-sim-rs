//! Software x87 80-bit extended arithmetic.
//!
//! Delphi 7 runs the FPU with control word $1332: 64-bit mantissa precision and
//! round-to-nearest-even. The original game's float math therefore rounds every intermediate
//! FADD/FSUB/FMUL/FDIV result to a 64-bit significand, and only rounds to Single/Double when a
//! value is stored. Plain f64 arithmetic gives different integers after `Int()` for some inputs
//! (e.g. `Int(39/50*10000)` = 7800 on x87 but 7799 in f64), so the port uses this type for every
//! FPU sequence in the original code.
//!
//! Only finite values are needed (the game never overflows or divides by zero); NaN/Inf inputs
//! are not supported. Denormals are flushed to zero (never reached by game values).

use std::cmp::Ordering;

/// Finite extended value: (-1)^neg * mant * 2^(exp-63), `mant` normalized (bit 63 set) unless zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct F80 {
    neg: bool,
    exp: i32,
    mant: u64,
}

impl F80 {
    pub const ZERO: F80 = F80 { neg: false, exp: 0, mant: 0 };

    pub fn is_zero(self) -> bool {
        self.mant == 0
    }

    /// FILD: exact for any i64.
    pub fn from_i64(v: i64) -> F80 {
        if v == 0 {
            return F80::ZERO;
        }
        let neg = v < 0;
        let m = v.unsigned_abs();
        let lz = m.leading_zeros() as i32;
        F80 { neg, exp: 63 - lz, mant: m << lz }
    }

    pub fn from_u32(v: u32) -> F80 {
        F80::from_i64(v as i64)
    }

    /// FLD qword (double) — exact.
    pub fn from_f64(v: f64) -> F80 {
        assert!(v.is_finite(), "F80::from_f64({v})");
        if v == 0.0 {
            return F80::ZERO;
        }
        let bits = v.to_bits();
        let neg = bits >> 63 != 0;
        let e = ((bits >> 52) & 0x7ff) as i32;
        let frac = bits & ((1u64 << 52) - 1);
        let (m, exp) = if e == 0 { (frac, -1074 + 52) } else { (frac | (1u64 << 52), e - 1023) };
        let lz = m.leading_zeros() as i32;
        // value = m * 2^(exp-52)
        let mant = m << lz;
        F80 { neg, exp: exp + (11 - lz), mant }
    }

    /// FLD dword (single) — exact.
    pub fn from_f32(v: f32) -> F80 {
        F80::from_f64(v as f64)
    }

    /// Round an unbounded-precision magnitude `q * 2^(e - (bits(q)-1))` (plus a sticky flag for
    /// discarded nonzero bits below q) to a 64-bit significand, nearest-even.
    fn round_from(neg: bool, q: u128, top_exp: i32, sticky: bool) -> F80 {
        if q == 0 {
            return F80::ZERO;
        }
        let bits = 128 - q.leading_zeros() as i32;
        // `top_exp` is the exponent of the most significant set bit of q.
        if bits <= 64 {
            let mant = (q as u64) << (64 - bits);
            // No bits dropped; sticky only matters if bits remained below — callers ensure q carries
            // at least 2 extra bits whenever sticky can be set.
            debug_assert!(!sticky || bits > 64);
            return F80 { neg, exp: top_exp, mant };
        }
        let drop = bits - 64;
        let kept = (q >> drop) as u64;
        let rem = q & ((1u128 << drop) - 1);
        let half = 1u128 << (drop - 1);
        let round_up = rem > half || (rem == half && (sticky || kept & 1 == 1));
        let (mant, exp) = if round_up {
            match kept.checked_add(1) {
                Some(m) => (m, top_exp),
                None => (1u64 << 63, top_exp + 1),
            }
        } else {
            (kept, top_exp)
        };
        F80 { neg, exp, mant }
    }

    pub fn neg(self) -> F80 {
        if self.is_zero() { self } else { F80 { neg: !self.neg, ..self } }
    }

    pub fn abs(self) -> F80 {
        F80 { neg: false, ..self }
    }

    /// FADD.
    pub fn add(self, o: F80) -> F80 {
        if self.is_zero() {
            return if o.is_zero() { F80 { neg: self.neg && o.neg, ..F80::ZERO } } else { o };
        }
        if o.is_zero() {
            return self;
        }
        let (a, b) = if (self.exp, self.mant) >= (o.exp, o.mant) { (self, o) } else { (o, self) };
        let diff = (a.exp - b.exp) as u32;
        // Work with 62 guard bits below the 64-bit significands.
        let am = (a.mant as u128) << 62;
        let (bm, sticky) = if diff >= 126 {
            (0u128, true)
        } else {
            let full = (b.mant as u128) << 62;
            let shifted = full >> diff;
            (shifted, shifted << diff != full)
        };
        // Exponent of bit 125 (the top bit of am) is a.exp.
        if a.neg == b.neg {
            let s = am + bm;
            let top = 127 - s.leading_zeros() as i32;
            F80::round_from(a.neg, s, a.exp + (top - 125), sticky)
        } else {
            // a >= b in magnitude. A sticky borrow means the true difference is slightly smaller.
            let mut s = am - bm;
            if sticky {
                s -= 1; // true value lies strictly between s-1 and s; keep sticky to break ties
            }
            if s == 0 && !sticky {
                return F80::ZERO;
            }
            let top = 127 - s.leading_zeros() as i32;
            F80::round_from(a.neg, s, a.exp + (top - 125), sticky)
        }
    }

    /// FSUB.
    pub fn sub(self, o: F80) -> F80 {
        self.add(o.neg())
    }

    /// FMUL.
    pub fn mul(self, o: F80) -> F80 {
        if self.is_zero() || o.is_zero() {
            return F80 { neg: self.neg != o.neg, ..F80::ZERO };
        }
        let p = self.mant as u128 * o.mant as u128; // in [2^126, 2^128)
        let top = 127 - p.leading_zeros() as i32;
        F80::round_from(self.neg != o.neg, p, self.exp + o.exp + (top - 126), false)
    }

    /// FDIV.
    pub fn div(self, o: F80) -> F80 {
        assert!(!o.is_zero(), "x87 division by zero (EZeroDivide in the original)");
        if self.is_zero() {
            return F80 { neg: self.neg != o.neg, ..F80::ZERO };
        }
        let num = (self.mant as u128) << 64;
        let d = o.mant as u128;
        let q1 = num / d;
        let r1 = num % d;
        let q2 = (r1 << 2) / d;
        let r2 = (r1 << 2) % d;
        let q = (q1 << 2) | q2; // value = q * 2^(self.exp - o.exp - 66)
        let top = 127 - q.leading_zeros() as i32;
        F80::round_from(self.neg != o.neg, q, self.exp - o.exp + (top - 66), r2 != 0)
    }

    /// Delphi `Int()` (FRNDINT with RC = truncate): drop the fractional bits.
    pub fn int(self) -> F80 {
        if self.is_zero() || self.exp >= 63 {
            return self;
        }
        if self.exp < 0 {
            return F80 { neg: self.neg, ..F80::ZERO };
        }
        let frac_bits = 63 - self.exp;
        let mant = self.mant & !((1u64 << frac_bits) - 1);
        F80 { mant, ..self }
    }

    /// FISTP qword under RC = nearest-even (Delphi `_ROUND`).
    pub fn round_i64(self) -> i64 {
        if self.is_zero() || self.exp < -1 {
            return 0;
        }
        assert!(self.exp < 63, "x87 integer overflow in FISTP");
        let frac_bits = (63 - self.exp) as u32;
        let int = if frac_bits >= 64 { 0 } else { self.mant >> frac_bits };
        let rem = if frac_bits >= 64 { self.mant as u128 } else { (self.mant & ((1u64 << frac_bits) - 1)) as u128 };
        let half = 1u128 << (frac_bits - 1);
        let up = rem > half || (rem == half && int & 1 == 1);
        let v = (int + up as u64) as i64;
        if self.neg { -v } else { v }
    }

    /// FISTP / FIST with truncation (Delphi `_TRUNC`).
    pub fn trunc_i64(self) -> i64 {
        let v = self.int();
        if v.is_zero() {
            return 0;
        }
        let m = (v.mant >> (63 - v.exp)) as i64;
        if v.neg { -m } else { m }
    }

    fn to_ieee(self, mant_bits: u32, exp_bias: i32, exp_bits: u32) -> u64 {
        if self.is_zero() {
            return (self.neg as u64) << (mant_bits + exp_bits);
        }
        let drop = 63 - mant_bits; // keep mant_bits+1 bits (incl. hidden one)
        let kept = self.mant >> drop;
        let rem = self.mant & ((1u64 << drop) - 1);
        let half = 1u64 << (drop - 1);
        let up = rem > half || (rem == half && kept & 1 == 1);
        let (mut m, mut e) = (kept + up as u64, self.exp);
        if m >> (mant_bits + 1) != 0 {
            m >>= 1;
            e += 1;
        }
        let be = e + exp_bias;
        assert!(be > 0 && be < (1 << exp_bits) - 1, "value out of IEEE range for store: {self:?}");
        ((self.neg as u64) << (mant_bits + exp_bits)) | ((be as u64) << mant_bits) | (m & ((1u64 << mant_bits) - 1))
    }

    /// FSTP qword (round to Double, nearest-even).
    pub fn to_f64(self) -> f64 {
        f64::from_bits(self.to_ieee(52, 1023, 11))
    }

    /// FSTP dword (round to Single, nearest-even).
    pub fn to_f32(self) -> f32 {
        f32::from_bits(self.to_ieee(23, 127, 8) as u32)
    }

    /// FCOM/FCOMP ordering.
    pub fn cmp(self, o: F80) -> Ordering {
        if self.is_zero() && o.is_zero() {
            return Ordering::Equal;
        }
        let key = |v: F80| -> (i8, i32, u64) {
            if v.is_zero() { (0, 0, 0) } else { (if v.neg { -1 } else { 1 }, v.exp, v.mant) }
        };
        let (sa, ea, ma) = key(self);
        let (sb, eb, mb) = key(o);
        if sa != sb {
            return sa.cmp(&sb);
        }
        let mag = (ea, ma).cmp(&(eb, mb));
        if sa < 0 { mag.reverse() } else { mag }
    }

    /// Build from (negative, mantissa with bit 63 set, exponent of bit 63). Zero if mant == 0.
    pub fn from_parts(neg: bool, mant: u64, exp: i32) -> F80 {
        if mant == 0 { F80::ZERO } else { assert!(mant >> 63 == 1); F80 { neg, exp, mant } }
    }

    /// Exact decomposition for tests: (negative, mantissa, exponent of bit 63).
    pub fn parts(self) -> (bool, u64, i32) {
        (self.neg, self.mant, self.exp)
    }
}

impl From<i32> for F80 {
    fn from(v: i32) -> F80 {
        F80::from_i64(v as i64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(v: i64) -> F80 {
        F80::from_i64(v)
    }

    #[test]
    fn exact_small_ops() {
        assert_eq!(f(3).add(f(4)), f(7));
        assert_eq!(f(3).sub(f(4)), f(-1));
        assert_eq!(f(6).mul(f(7)), f(42));
        assert_eq!(f(84).div(f(2)), f(42));
        assert_eq!(f(7).div(f(2)).to_f64(), 3.5);
        assert_eq!(f(1).div(f(3)).to_f64(), 1.0 / 3.0);
        assert_eq!(F80::from_f64(0.1).to_f64(), 0.1);
        assert_eq!(F80::from_f32(0.5f32).to_f32(), 0.5);
    }

    #[test]
    fn int_and_round() {
        assert_eq!(F80::from_f64(2.75).int().to_f64(), 2.0);
        assert_eq!(F80::from_f64(-2.75).int().to_f64(), -2.0);
        assert_eq!(F80::from_f64(0.75).int().to_f64(), 0.0);
        assert_eq!(F80::from_f64(2.5).round_i64(), 2);
        assert_eq!(F80::from_f64(3.5).round_i64(), 4);
        assert_eq!(F80::from_f64(-2.5).round_i64(), -2);
        assert_eq!(F80::from_f64(2.4999).round_i64(), 2);
        assert_eq!(F80::from_f64(0.5).round_i64(), 0);
        assert_eq!(F80::from_f64(1.5).round_i64(), 2);
        assert_eq!(F80::from_f64(7.9).trunc_i64(), 7);
    }

    /// TForm5.Freshen win %: Int(39/(39+11)*10000). On x87 39/50 rounds slightly below 0.78, so the
    /// original shows 77.99%; plain f64 would give 78%. (Verified with exact rational arithmetic.)
    #[test]
    fn win_percentage_case() {
        let x87 = f(39).div(f(50)).mul(f(10000)).int().to_f64();
        assert_eq!(x87, 7799.0);
        let f64_path = ((39.0f64 / 50.0) * 10000.0).trunc();
        assert_eq!(f64_path, 7800.0);
    }
}
