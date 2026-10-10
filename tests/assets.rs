//! Requires the user's own extracted game files; set POKEGAME_DIR (defaults to the decompile workspace).
use pokegame::assets::Assets;
use std::path::PathBuf;

fn game_dir() -> Option<PathBuf> {
    let p = std::env::var_os("POKEGAME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("../downloads/pokegame/extracted"));
    p.join("PokeGame.exe").exists().then_some(p)
}

#[test]
fn loads_everything() {
    let Some(dir) = game_dir() else {
        eprintln!("skipping: game files not found");
        return;
    };
    let a = Assets::load(&dir).expect("assets load");
    assert_eq!(a.forms.len(), 11);
    let big = &a.image_lists["Form4.PKMNImages"];
    assert_eq!((big.images.len(), big.width, big.height), (386, 64, 64));
    assert_eq!(a.image_lists["Form4.SmallPKMNImg"].images.len(), 386);
    assert!(a.pictures.contains_key("Form4.Image1"));
    assert!(a.pictures.contains_key("AboutBox.ProgramIcon"));
    let d = &a.data;
    assert_eq!(d.pkmn_name[1], "Bulbasaur");
    assert_eq!(d.pkmn_name[386], "Deoxys");
    assert_eq!(d.badge_name[1], "BoulderBadge");
    assert_eq!(d.move_name[1], "Tackle");
    assert_eq!((d.move_power[1], d.move_type[1]), (70, 1));
    assert_eq!(d.type_name[17], "Steel");
    assert_eq!(d.trainer_name[64], "Julia");
    assert_eq!(d.prize_text[6][3], "a random POKéMON at a random level");
    assert_eq!((d.evolve_level[1], d.evolve_to[1]), (16, 2));
    assert_eq!(d.type_eff[1][0], 1.0);
    let f4 = a.form("TFORM4");
    assert_eq!(f4.str("Caption"), Some("POKéMON Simulator V4.5"));
    let lv = f4.find("ListView1").unwrap();
    let items = match lv.prop("Items.Data") {
        Some(pokegame::assets::dfm::Value::Binary(b)) => pokegame::assets::dfm::parse_list_items(b).unwrap(),
        _ => panic!("no items"),
    };
    let caps: Vec<_> = items.iter().map(|i| (i.caption.as_str(), i.image_index)).collect();
    assert_eq!(caps, [("Bulbasaur", 0), ("Charmander", 3), ("Squirtle", 6)]);
}
