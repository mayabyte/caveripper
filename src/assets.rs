use std::fs::{read_to_string, read_dir, read};
use encoding_rs::SHIFT_JIS;
use image::DynamicImage;
use log::info;
use once_cell::sync::Lazy;
use dashmap::{DashMap, mapref::one::Ref};

use crate::caveinfo::CaveInfo;
use crate::errors::{AssetError, SublevelError, CaveInfoError};
use crate::sublevel::Sublevel;

pub static ASSETS: Lazy<AssetManager> = Lazy::new(|| AssetManager::new());

pub struct AssetManager {
    txt_cache: DashMap<String, String>,
    caveinfo_cache: DashMap<Sublevel, CaveInfo>,
    img_cache: DashMap<String, DynamicImage>,
    custom_img_cache: DashMap<String, DynamicImage>,

    /// All known treasure names. All lowercase so they can be easily compared.
    pub treasures: Vec<String>,

    /// All known teki name. All lowercase so they can be easily compared.
    pub teki: Vec<String>,
    pub cave_cfg: Vec<CaveConfig>,
}

impl AssetManager {
    pub fn new() -> Self {
        let mut mgr = Self {
            txt_cache: DashMap::new(),
            caveinfo_cache: DashMap::new(),
            img_cache: DashMap::new(),
            custom_img_cache: DashMap::new(),
            treasures: Vec::new(),
            teki: Vec::new(),
            cave_cfg: Vec::new(),
        };

        let treasures = read_to_string("resources/treasures.txt").unwrap();
        let ek_treasures = read_to_string("resources/treasures_exploration_kit.txt").unwrap();

        let mut treasure_names: Vec<String> = treasures
            .lines()
            .chain(ek_treasures.lines())
            .filter(|line| line.len() > 0)
            .map(|line| line.split_once(',').unwrap().1.to_owned())
            .map(|treasure| treasure.to_ascii_lowercase())
            .collect();
        treasure_names.sort();
        mgr.treasures = treasure_names;

        let teki: Vec<String> = read_dir("assets/enemytex/arc.d").expect("Couldn't read enemytex directory!")
            .filter_map(Result::ok)
            .filter(|dir_entry| dir_entry.path().is_dir())
            .map(|dir_entry| dir_entry.file_name().into_string().unwrap().to_ascii_lowercase())
            .collect();
        mgr.teki = teki;

        let cave_cfg: Vec<CaveConfig> = read_to_string("resources/caveinfo_config.txt").unwrap()
            .lines()
            .map(|line| {
                let mut data: Vec<String> = line.split(',').map(|e| e.trim().to_string()).collect();
                CaveConfig {
                    full_name: data.remove(0),
                    caveinfo_filename: data.remove(0),
                    shortened_names: data,
                    romhack: None,
                }
            })
            .collect();
        mgr.cave_cfg = cave_cfg;

        mgr
    }

    pub fn get_txt_file(&self, path: &str) -> Option<String> {
        if !self.txt_cache.contains_key(path) {
            info!("Loading {}...", path);
            let data = read(path).ok()?;
            if path.starts_with("assets") {
                self.txt_cache.insert(path.to_string(), SHIFT_JIS.decode(data.as_slice()).0.into_owned());
            }
            else if path.starts_with("resources") {
                self.txt_cache.insert(path.to_string(), String::from_utf8(data).ok()?);
            }
        }
        Some(self.txt_cache.get(path)?.clone())
    }

    pub fn get_caveinfo(&self, sublevel: &Sublevel) -> Result<CaveInfo, AssetError> {
        if !self.caveinfo_cache.contains_key(&sublevel) {
            self.load_caveinfo(&sublevel.cfg)?;
        }
        Ok(
            self.caveinfo_cache.get(&sublevel)
            .ok_or(SublevelError::UnrecognizedSublevel(sublevel.short_name()))?
            .clone()
        )
    }

    pub fn get_img(&self, path: &str) -> Option<Ref<String, DynamicImage>> {
        if !self.img_cache.contains_key(path) {
            info!("Loading image {}...", path);
            let data = read(path).expect(&format!("Couldn't read image file '{}'!", path));
            let img = image::load_from_memory(data.as_slice()).ok()?;
            self.img_cache.insert(path.to_string(), img);
        }
        self.img_cache.get(path)
    }

    pub fn get_custom_img(&self, key: &str) -> Option<Ref<String, DynamicImage>> {
        self.custom_img_cache.get(key)
    }

    pub fn cache_img(&self, key: &str, img: DynamicImage) {
        self.custom_img_cache.insert(key.to_string(), img);
    }

    /// Forces the asset manager to load all the Caveinfo files in Vanilla Pikmin 2.
    /// Most useful for testing and benchmarking purposes.
    pub fn preload_vanilla_caveinfo(&self) -> Result<(), AssetError> {
        for cave in ALL_VANILLA_CAVES {
            self.load_caveinfo(self.find_cave_cfg(cave).unwrap())?;
        }
        Ok(())
    }

    /// Clones the sublevel cache and returns it.
    /// Most useful for testing.
    pub fn all_sublevels(&self) -> DashMap<Sublevel, CaveInfo> {
        self.caveinfo_cache.clone()
    }

    pub(crate) fn find_cave_cfg(&self, name: &str) -> Option<&CaveConfig> {
        self.cave_cfg.iter()
            .find(|cfg| {
                cfg.shortened_names.iter().any(|n| name.eq_ignore_ascii_case(n))
                || cfg.full_name.eq_ignore_ascii_case(name)
            })
    }

    /// Loads and parses a caveinfo file, then stores the
    /// resultant FloorInfo structs in the cache.
    fn load_caveinfo(&self, cave: &CaveConfig) -> Result<(), AssetError> {
        info!("Loading CaveInfo for {}...", cave.full_name);
        let caveinfo_filename = format!("assets/caveinfo/{}", cave.caveinfo_filename);
        let caveinfo_txt = self.get_txt_file(&caveinfo_filename)
            .ok_or_else(|| CaveInfoError::MissingFileError(caveinfo_filename.clone()))?;
        let caveinfos = CaveInfo::parse_from(&caveinfo_txt)?;
        for mut caveinfo in caveinfos.into_iter() {
            let sublevel = Sublevel::from_cfg(cave, (caveinfo.sublevel+1) as usize);
            caveinfo.cave_name = Some(sublevel.short_name());
            if !self.caveinfo_cache.contains_key(&sublevel) {
                self.caveinfo_cache.insert(sublevel, caveinfo);
            }
        }

        Ok(())
    }
}

/// Metadata about a cave, including its full name, possible shortened names,
/// and caveinfo filename.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct CaveConfig {
    pub full_name: String,
    pub shortened_names: Vec<String>,
    pub caveinfo_filename: String,
    pub romhack: Option<String>,
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

static ALL_VANILLA_CAVES: [&'static str; 14] = ["ec", "scx", "fc", "hob", "wfg", "bk", "sh", "cos", "gk", "sr", "smc", "coc", "hoh", "dd"];
