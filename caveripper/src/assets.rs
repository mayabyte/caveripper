use std::fs::{read_to_string, read_dir, read};
use std::path::{Path, PathBuf};
use encoding_rs::SHIFT_JIS;
use image::RgbaImage;
use itertools::Itertools;
use log::info;
use once_cell::sync::Lazy;
use dashmap::{DashMap, mapref::one::Ref};

use crate::caveinfo::CaveInfo;
use crate::errors::{AssetError, SublevelError};
use crate::sublevel::Sublevel;

pub static ASSETS: Lazy<AssetManager> = Lazy::new(|| AssetManager::new("."));

pub struct AssetManager {
    base_path: PathBuf,
    txt_cache: DashMap<String, String>,
    caveinfo_cache: DashMap<Sublevel, CaveInfo>,
    img_cache: DashMap<String, RgbaImage>,
    custom_img_cache: DashMap<String, RgbaImage>,
    pub cave_cfg: Vec<CaveConfig>,

    /// All known treasure names. All lowercase so they can be easily compared.
    pub treasures: Vec<Treasure>,

    /// All known teki names. All lowercase so they can be easily compared.
    pub teki: Vec<String>,

    /// All known room names.
    pub rooms: Vec<String>,
}

impl AssetManager {
    pub fn new<T: AsRef<Path>>(base_path: T) -> Self {
        let mut mgr = Self {
            base_path: base_path.as_ref().into(),
            txt_cache: DashMap::new(),
            caveinfo_cache: DashMap::new(),
            img_cache: DashMap::new(),
            custom_img_cache: DashMap::new(),
            cave_cfg: Vec::new(),
            treasures: Vec::new(),
            teki: Vec::new(),
            rooms: Vec::new(),
        };

        let treasures = SHIFT_JIS.decode(
            read(mgr.base_path.join("assets/cfg/pelletlist_us.d/otakara_config.txt"))
            .expect("Couldn't find otakara_config.txt!")
            .as_slice()
        ).0.into_owned();
        let ek_treasures = SHIFT_JIS.decode(
            read(mgr.base_path.join("assets/cfg/pelletlist_us.d/item_config.txt"))
            .expect("Couldn't find item_config.txt!")
            .as_slice()
        ).0.into_owned();

        let mut treasures = parse_treasure_config(&treasures);
        treasures.append(&mut parse_treasure_config(&ek_treasures));
        treasures.sort_by(|t1, t2| t1.internal_name.cmp(&t2.internal_name));
        mgr.treasures = treasures;

        let teki: Vec<String> = read_dir(mgr.base_path.join("assets/enemytex/arc.d")).expect("Couldn't read enemytex directory!")
            .filter_map(Result::ok)
            .filter(|dir_entry| dir_entry.path().is_dir())
            .map(|dir_entry| dir_entry.file_name().into_string().unwrap().to_ascii_lowercase())
            .collect();
        mgr.teki = teki;

        let rooms: Vec<String> = read_dir(mgr.base_path.join("assets/arc")).expect("Couldn't read arc directory!")
            .filter_map(Result::ok)
            .filter(|dir_entry| dir_entry.path().is_dir())
            .map(|dir_entry| dir_entry.file_name().into_string().unwrap().to_ascii_lowercase())
            .collect();
        mgr.rooms = rooms;

        let cave_cfg: Vec<CaveConfig> = read_to_string(mgr.base_path.join("resources/caveinfo_config.txt")).unwrap()
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

    pub fn get_txt_file(&self, path: &str) -> Result<String, AssetError> {
        if !self.txt_cache.contains_key(path) {
            info!("Loading {}...", path);
            let data = read(self.base_path.join(path))
                .map_err(|e| AssetError::IoError(path.to_string(), e.kind()))?;
            if path.starts_with("assets") {
                self.txt_cache.insert(path.to_string(), SHIFT_JIS.decode(data.as_slice()).0.into_owned());
            }
            else if path.starts_with("resources") {
                self.txt_cache.insert(path.to_string(), String::from_utf8(data).map_err(|_| AssetError::DecodingError(path.to_string()))?);
            }
        }
        Ok(
            self.txt_cache.get(path)
            .ok_or_else(|| AssetError::CacheError(path.to_string()))?
            .clone()
        )
    }

    pub fn get_caveinfo(&self, sublevel: &Sublevel) -> Result<CaveInfo, AssetError> {
        if !self.caveinfo_cache.contains_key(sublevel) {
            self.load_caveinfo(&sublevel.cfg)?;
        }
        Ok(
            self.caveinfo_cache.get(sublevel)
            .ok_or_else(|| SublevelError::UnrecognizedSublevel(sublevel.short_name()))?
            .clone()
        )
    }

    pub fn get_img(&self, path: &str) -> Result<Ref<String, RgbaImage>, AssetError> {
        if !self.img_cache.contains_key(path) {
            info!("Loading image {}...", path);
            let data = read(self.base_path.join(path))
                .map_err(|e| AssetError::IoError(path.to_string(), e.kind()))?;
            let img = image::load_from_memory(data.as_slice())
                .map_err(|_| AssetError::DecodingError(path.to_string()))?
                .into_rgba8();
            self.img_cache.insert(path.to_string(), img);
        }
        self.img_cache.get(path).ok_or_else(|| AssetError::DecodingError(path.to_string()))
    }

    pub fn get_custom_img(&self, key: &str) -> Result<Ref<String, RgbaImage>, AssetError> {
        self.custom_img_cache.get(key).ok_or_else(|| AssetError::DecodingError(key.to_string()))
    }

    pub fn cache_img(&self, key: &str, img: RgbaImage) {
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
        let caveinfo_txt = self.get_txt_file(&caveinfo_filename)?;
        let caveinfos = CaveInfo::parse_from(&caveinfo_txt)?;
        for mut caveinfo in caveinfos.into_iter() {
            let sublevel = Sublevel::from_cfg(cave, (caveinfo.floor_num+1) as usize);
            caveinfo.sublevel = Some(sublevel.clone());
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
        "wakame_s" => Some("wakame_s.png"),
        "chiyogami" => Some("chiyogami.PNG"),
        "rock" => Some("Bingo_Battle_Rock_Storm_icon.png"),
        _ => None
    }
}

static ALL_VANILLA_CAVES: [&str; 14] = ["ec", "scx", "fc", "hob", "wfg", "bk", "sh", "cos", "gk", "sr", "smc", "coc", "hoh", "dd"];

#[derive(Clone, Debug)]
pub struct Treasure {
    pub internal_name: String,
    pub min_carry: u32,
    pub max_carry: u32,
    pub value: u32,
}

fn parse_treasure_config(config_txt: &str) -> Vec<Treasure> {
    config_txt.lines().skip(4)
        .batching(|lines| {
            // Skip the opening bracket
            lines.next()?;
            Some(lines.take_while(|line| line != &"}").collect_vec())
        })
        .map(|section| {
            let internal_name = treasure_config_line_value(section[0]).to_string();
            let min_carry = treasure_config_line_value(section[13]).parse().unwrap();
            let max_carry = treasure_config_line_value(section[14]).parse().unwrap();
            let value = treasure_config_line_value(section[18]).parse().unwrap();
            Treasure { internal_name, min_carry, max_carry, value }
        })
        .collect_vec()
}

fn treasure_config_line_value(line: &str) -> &str {
    line.trim().split_ascii_whitespace().last().unwrap()
}
