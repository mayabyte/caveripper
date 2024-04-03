#[cfg(not(feature = "wasm"))]
pub mod fs_asset_manager;
mod pinmap;

use std::{
    collections::HashMap,
    fmt::Display,
    path::{Path, PathBuf},
};

use error_stack::Result;
use image::RgbaImage;
use itertools::Itertools;
use serde::Serialize;

use crate::{caveinfo::CaveInfo, errors::CaveripperError, sublevel::Sublevel};

pub trait AssetManager {
    fn load_txt<P: AsRef<Path>>(&self, path: P) -> Result<String, CaveripperError>;
    fn load_caveinfo<'a>(&'a self, sublevel: &Sublevel) -> Result<&'a CaveInfo, CaveripperError>;
    fn load_image(&self, kind: ImageKind, game: &str, name: &str) -> Result<&RgbaImage, CaveripperError>;
    fn load_raw<P: AsRef<Path>>(&self, path: P) -> Result<Vec<u8>, CaveripperError>;

    fn all_teki(&self, game: Option<&str>) -> Result<Vec<String>, CaveripperError>;
    fn all_units(&self, game: Option<&str>) -> Result<Vec<String>, CaveripperError>;
    fn all_treasures(&self, game: Option<&str>) -> Result<Vec<Treasure>, CaveripperError>;

    fn get_treasure_info(&self, game: &str, name: &str) -> Result<&Treasure, CaveripperError>;
    fn get_cave_cfg(&self, name: &str, game: Option<&str>, force_challenge_mode: bool) -> Result<&CaveConfig, CaveripperError>;
}

#[derive(PartialEq)]
pub enum ImageKind {
    Teki,
    Treasure,
    CaveUnit,
    Special,
}

impl Display for ImageKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ImageKind::Teki => "teki",
                ImageKind::Treasure => "treasures",
                ImageKind::CaveUnit => "mapunits",
                ImageKind::Special => "enemytex_special",
            }
        )
    }
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

impl CaveConfig {
    pub fn is_colossal_caverns(&self) -> bool {
        self.full_name.eq_ignore_ascii_case("Colossal Caverns")
    }

    pub(crate) fn get_caveinfo_path(&self) -> PathBuf {
        if self.game.eq_ignore_ascii_case("caveinfo") {
            PathBuf::from(&self.caveinfo_filename)
        } else {
            PathBuf::from_iter(["assets", &self.game, "caveinfo", &self.caveinfo_filename])
        }
    }
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
        "gashiba" => Some("Gas_pipe_icon"),
        "daiodogreen" => Some("daiodogreen"),
        "ooinu_s" => Some("ooinu_s"),
        "kareooinu_s" => Some("kareooinu_s"),
        "kareooinu_l" => Some("kareooinu"),
        "elechiba" => Some("Electrical_wire_icon"),
        "hiba" => Some("Fire_geyser_icon"),
        "bomb" => Some("Bingo_Battle_Bomb_icon"),
        "egg" => Some("36px-Egg_icon"),
        "wakame_s" => Some("wakame_s"),
        "chiyogami" => Some("chiyogami"),
        "rock" => Some("Roulette_Wheel_boulder"),
        "panhouse" => Some("ooinu_s"),
        _ => None,
    }
}
