use pokegame::delphi::F80;

#[test]
fn matches_exact_rational_reference() {
    let data = include_str!("data/f80_vectors.txt");
    let mut n = 0;
    for line in data.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        let p = |i: usize| F80::from_parts(t[i] == "1", t[i + 1].parse().unwrap(), t[i + 2].parse().unwrap());
        let (a, b, want) = (p(1), p(4), p(7));
        let got = match t[0] {
            "+" => a.add(b),
            "-" => a.sub(b),
            "*" => a.mul(b),
            "/" => a.div(b),
            _ => unreachable!(),
        };
        assert_eq!(got.parts(), want.parts(), "{line}");
        n += 1;
    }
    assert!(n > 19000);
}
