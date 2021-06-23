use once_cell::sync::Lazy;
use std::sync::Mutex;

pub static TREASURES: Lazy<Mutex<Vec<&'static str>>> = Lazy::new(|| {
    let treasure_file = include_str!("../../gamedata/treasures.txt");
    let exploration_kit_file = include_str!("../../gamedata/treasures_exploration_kit.txt");

    let mut treasure_names: Vec<&'static str> = treasure_file
        .lines()
        .chain(exploration_kit_file.lines())
        .filter(|line| line.len() > 0)
        .map(|line| line.split_once(',').unwrap().1)
        .collect();
    treasure_names.sort();
    Mutex::new(treasure_names)
});

pub const ALL_SUBLEVELS_POD: [&'static str; 52] = [
    "EC1", "EC2", "HoB1", "HoB2", "HoB3", "HoB4", "HoB5", "WFG1", "WFG2", "WFG3", "WFG4", "WFG5",
    "SH1", "SH2", "SH3", "SH4", "SH5", "SH6", "SH7", "BK1", "BK2", "BK3", "BK4", "BK5", "BK6",
    "BK7", "SCx1", "SCx2", "SCx3", "SCx4", "SCx5", "SCx6", "SCx7", "SCx8", "FC1", "FC2", "FC3",
    "FC4", "FC5", "FC6", "FC7", "CoS1", "CoS2", "CoS3", "CoS4", "CoS5", "GK1", "GK2", "GK3", "GK4",
    "GK5", "GK6",
];

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
        "smc" => "yakushima_4.txt",
        // TODO: add Wistful Wilds caves
        _ => panic!("Unrecognized cave name \"{}\"", cave_name),
    }
}

pub(super) fn is_item_name(name: &str) -> bool {
    TREASURES
        .lock()
        .unwrap()
        .binary_search(&name.trim_start_matches('_'))
        .is_ok()
}
