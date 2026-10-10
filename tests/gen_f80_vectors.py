"""Reference vectors for F80: exact rational results rounded to a 64-bit significand (nearest-even).
Run: python3 -I tests/gen_f80_vectors.py > tests/data/f80_vectors.txt"""
from fractions import Fraction
import random, struct

def round64(x: Fraction):
    """Round to 64-bit significand, return (neg, mant, exp) with value = mant * 2^(exp-63)."""
    if x == 0: return (0, 0, 0)
    neg = x < 0; x = abs(x)
    e = x.numerator.bit_length() - x.denominator.bit_length()
    while Fraction(2) ** e > x: e -= 1
    while Fraction(2) ** (e + 1) <= x: e += 1
    scaled = x / Fraction(2) ** (e - 63)          # in [2^63, 2^64)
    m = scaled.numerator // scaled.denominator
    rem = scaled - m
    if rem > Fraction(1, 2) or (rem == Fraction(1, 2) and m & 1): m += 1
    if m == 1 << 64: m >>= 1; e += 1
    return (int(neg), m, e)

def val(t):
    neg, m, e = t
    v = Fraction(m) * Fraction(2) ** (e - 63)
    return -v if neg else v

rnd = random.Random(1234)
def operand():
    k = rnd.random()
    if k < 0.4: return Fraction(rnd.randint(-100000, 100000))
    if k < 0.7: return Fraction(struct.unpack('<f', struct.pack('<f', rnd.choice([0.5, 0.25, 2.0, 1.0, 0.1, 0.7, 1.5, 0.3, 3.0, 0.9])))[0])
    a = round64(Fraction(rnd.randint(1, 10**6), rnd.randint(1, 10**6)) * rnd.choice([1, -1]))
    return val(a)

for _ in range(20000):
    a, b = operand(), operand()
    op = rnd.choice('+-*/')
    if op == '/' and b == 0: continue
    r = {'+': a + b, '-': a - b, '*': a * b, '/': a / b if b else 0}[op]
    ta, tb, tr = round64(a), round64(b), round64(r)
    print(op, *ta, *tb, *tr)
