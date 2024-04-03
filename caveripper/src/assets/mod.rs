#[cfg(not(feature = "wasm"))]
pub mod fs_asset_manager;
mod pinmap;

use std::{collections::HashMap, path::Path};

use error_stack::Result;
use image::RgbaImage;
use itertools::Itertools;
use serde::Serialize;

use crate::{caveinfo::CaveInfo, errors::CaveripperError, sublevel::Sublevel};

pub trait AssetManager {
    fn treasure_info(&self, game: &str, name: &str) -> Result<&Treasure, CaveripperError>;
    fn combined_treasure_list(&self) -> Result<Vec<Treasure>, CaveripperError>;
    fn teki_list(&self, game: &str) -> Result<&Vec<String>, CaveripperError>;
    fn combined_teki_list(&self) -> Result<Vec<String>, CaveripperError>;
    fn room_list(&self, game: &str) -> Result<&Vec<String>, CaveripperError>;
    fn combined_room_list(&self) -> Result<Vec<String>, CaveripperError>;
    fn get_bytes<P: AsRef<Path>>(&self, path: P) -> Result<Vec<u8>, CaveripperError>;
    fn find_cave_cfg(&self, name: &str, game: Option<&str>, force_challenge_mode: bool) -> Result<&CaveConfig, CaveripperError>;
    fn get_txt_file<P: AsRef<Path>>(&self, path: P) -> Result<String, CaveripperError>;
    fn get_caveinfo<'a>(&'a self, sublevel: &Sublevel) -> Result<&'a CaveInfo, CaveripperError>;
    fn get_img<P: AsRef<Path>>(&self, path: P) -> Result<&RgbaImage, CaveripperError>;
}

/// Metadata about a cave. Defined in resources/cave_config.txt
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize)]
pub struct CaveConfig {
    pub game: String, // Indicates either the vanilla game or a romhack
    pub full_name: String,
    pub is_challenge_mode: bool,
    pub shortened_names: Vec<String>,
    pub caveinfo_filename: String,
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct Treasure {
    pub internal_name: String,
    pub game: String,
    pub min_carry: u32,
    pub max_carry: u32,
    pub value: u32,
}

fn parse_treasure_config(config_txt: &str, game: &str) -> Vec<Treasure> {
    config_txt
        .chars()
        .peekable()
        .batching(|chars| {
            if chars.peek().is_none() {
                None
            } else {
                let val = chars
                    .skip_while(|c| c != &'{')
                    .skip(1)
                    .take_while(|c| c != &'}')
                    .skip(1)
                    .collect::<String>();
                Some(val)
            }
        })
        .filter(|section| !section.trim().is_empty())
        .map(|section| {
            let section: HashMap<&str, &str> = section
                .lines()
                .filter(|line| !line.is_empty())
                .map(|line| {
                    let line = line.split_whitespace().collect_vec();
                    (*line.first().unwrap(), *line.last().unwrap())
                })
                .collect();
            let internal_name = section["name"].to_string();
            let min_carry = section["min"].parse().unwrap();
            let max_carry = section["max"].parse().unwrap();
            let value = section["money"].parse().unwrap();
            Treasure {
                internal_name,
                game: game.to_string(),
                min_carry,
                max_carry,
                value,
            }
        })
        .collect_vec()
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
        "wakame_s" => Some("wakame_s.png"),
        "chiyogami" => Some("chiyogami.PNG"),
        "rock" => Some("Roulette_Wheel_boulder.png"),
        "panhouse" => Some("ooinu_s.png"),
        _ => None,
    }
}
