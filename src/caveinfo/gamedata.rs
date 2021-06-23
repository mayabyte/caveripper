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
