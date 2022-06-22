/// Helper functions for dealing with internal Pikmin 2 game data.


use once_cell::sync::Lazy;
use std::{borrow::Cow, sync::Mutex};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder="$CARGO_MANIFEST_DIR/resources"]
#[prefix="resources/"]
struct Resources;

pub fn get_resource_file(path: &str) -> Option<String> {
    let file = Resources::get(path)?;
    String::from_utf8(file.data.as_ref().to_vec()).ok()
}

pub fn get_resource_file_bytes(path: &str) -> Option<Cow<'static, [u8]>> {
    let file = Resources::get(path)?;
    Some(file.data)
}

pub static TREASURES: Lazy<Mutex<Vec<String>>> = Lazy::new(|| {
    let treasure_file = get_resource_file("resources/treasures.txt").unwrap();
    let exploration_kit_file = get_resource_file("resources/treasures_exploration_kit.txt").unwrap();

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

pub fn get_special_texture_name(internal_name: &str) -> Option<&str> {
    match internal_name.to_ascii_lowercase().as_ref() {
        "gashiba" => Some("Gas_pipe_icon.png"),
        "daiodogreen" => Some("daiodogreen.png"),
        "ooinu_s" => Some("ooinu_s.png"),
        "kareooinu_s" => Some("kareooinu_s.png"),
        "kareooinu_l" => Some("kareooinu.png"),
        "elechiba" => Some("Electrical_wire_icon.png"),
        "hiba" => Some("Fire_geyser_icon.png"),
        "bomb" => Some("Bingo_Battle_Bomb_icon.png"),
        "egg" => Some("36px-Egg_icon.png"),
        _ => None
    }
}