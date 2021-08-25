use once_cell::sync::Lazy;
use std::sync::Mutex;
use crate::assets::get_file;

pub static TREASURES: Lazy<Mutex<Vec<String>>> = Lazy::new(|| {
    let treasure_file = get_file("assets/gcn/gamedata/treasures.txt").unwrap();
    let exploration_kit_file = get_file("assets/gcn/gamedata/treasures_exploration_kit.txt").unwrap();

    let mut treasure_names: Vec<String> = treasure_file
        .lines()
        .chain(exploration_kit_file.lines())
        .filter(|line| line.len() > 0)
        .map(|line| line.split_once(',').unwrap().1.to_owned())
        .collect();
    treasure_names.sort();
    Mutex::new(treasure_names)
});

pub(super) fn cave_name_to_caveinfo_filename(cave_name: &str) -> &'static str {
    match cave_name.to_ascii_lowercase().as_str() {
        "ec" => "tutorial_1.txt",
        "scx" => "tutorial_2.txt",
        "fc" => "tutorial_3.txt",
        "hob" => "forest_1.txt",
        "wfg" => "forest_2.txt",
        "bk" => "forest_3.txt",
        "sh" => "forest_4.txt",
        "cos" => "yakushima_1.txt",
        "gk" => "yakushima_2.txt",
        "sr" => "yakushima_3.txt",
        "smc" | "sc" => "yakushima_4.txt",
        "coc" => "last_1.txt",
        "hoh" => "last_2.txt",
        "dd" => "last_3.txt",
        _ => panic!("Unrecognized cave name \"{}\"", cave_name),
    }
}

pub(super) fn is_item_name(name: &str) -> bool {
    TREASURES
        .lock()
        .unwrap()
        .binary_search(&name.trim_start_matches('_').to_owned())
        .is_ok()
}
