use std::collections::HashSet;
use std::fs::{read_to_string, read_dir, read};
use std::path::{Path, PathBuf};
use encoding_rs::SHIFT_JIS;
use image::RgbaImage;
use itertools::Itertools;
use log::{info, warn};
use once_cell::sync::OnceCell;

use crate::caveinfo::CaveInfo;
use crate::errors::{AssetError, SublevelError};
use crate::pinmap::PinMap;
use crate::sublevel::{Sublevel, DIRECT_MODE_TAG};

static ASSETS: OnceCell<AssetManager> = OnceCell::new();

pub struct AssetManager {
    asset_path: PathBuf, /// Folder that assets are kept in
    resources_loc: PathBuf, /// Relative path to where the resources folder is located.

    txt_cache: PinMap<String, String>,
    caveinfo_cache: PinMap<Sublevel, CaveInfo>,
    img_cache: PinMap<String, RgbaImage>,
    pub cave_cfg: Vec<CaveConfig>,

    /// All known treasure names. All lowercase so they can be easily compared.
    pub treasures: Vec<Treasure>,

    /// All known teki names. All lowercase so they can be easily compared.
    pub teki: Vec<String>,

    /// All known room names.
    pub rooms: Vec<String>,
}

impl AssetManager {
    pub fn init(asset_path: impl AsRef<Path>, resources_loc: impl AsRef<Path>) {
        let mut manager = Self {
            asset_path: asset_path.as_ref().into(),
            resources_loc: resources_loc.as_ref().into(),
            txt_cache: PinMap::new(),
            caveinfo_cache: PinMap::new(),
            img_cache: PinMap::new(),
            cave_cfg: Vec::new(),
            treasures: Vec::new(),
            teki: Vec::new(),
            rooms: Vec::new(),
        };

        let cave_cfg: Vec<CaveConfig> = read_to_string(resources_loc.as_ref().join("resources/caveinfo_config.txt")).unwrap()
            .lines()
            .map(|line| {
                let mut data: Vec<String> = line.split(',').map(|e| e.trim().to_string()).collect();
                CaveConfig {
                    game: data.remove(0),
                    full_name: data.remove(0),
                    is_challenge_mode: data.remove(0).parse().expect("Invalid cave_config file!"),
                    caveinfo_filename: data.remove(0),
                    shortened_names: data,
                }
            })
            .collect();
        manager.cave_cfg = cave_cfg;

        let all_games: HashSet<&str> = manager.cave_cfg.iter().map(|cfg| cfg.game.as_str()).collect();
        for game in all_games.into_iter() {
            if !manager.asset_path.join(game).is_dir() {
                info!("No files found for game {}; skipping.", game);
                continue;
            }

            let treasures = SHIFT_JIS.decode(
                read(manager.asset_path.join(game).join("user/Abe/Pellet/us/pelletlist_us/otakara_config.txt"))
                .expect("Couldn't find otakara_config.txt!")
                .as_slice()
            ).0.into_owned();
            let ek_treasures = SHIFT_JIS.decode(
                read(manager.asset_path.join(game).join("user/Abe/Pellet/us/pelletlist_us/item_config.txt"))
                .expect("Couldn't find item_config.txt!")
                .as_slice()
            ).0.into_owned();
    
            let mut treasures = parse_treasure_config(&treasures);
            treasures.append(&mut parse_treasure_config(&ek_treasures));
            treasures.sort_by(|t1, t2| t1.internal_name.cmp(&t2.internal_name));
            manager.treasures = treasures;
    
            let teki: Vec<String> = read_dir(manager.asset_path.join(game).join("user/Yamashita/enemytex/arc")).expect("Couldn't read enemytex directory!")
                .filter_map(Result::ok)
                .filter(|dir_entry| dir_entry.path().is_dir())
                .map(|dir_entry| dir_entry.file_name().into_string().unwrap().to_ascii_lowercase())
                .collect();
            manager.teki = teki;
    
            let rooms: Vec<String> = read_dir(manager.asset_path.join(game).join("user/Mukki/mapunits/arc")).expect("Couldn't read arc directory!")
                .filter_map(Result::ok)
                .filter(|dir_entry| dir_entry.path().is_dir())
                .map(|dir_entry| dir_entry.file_name().into_string().unwrap().to_ascii_lowercase())
                .collect();
            manager.rooms = rooms;
        }

        ASSETS.get_or_init(|| manager);
    }

    pub fn get_txt_file<P: AsRef<Path>>(path: P) -> Result<&'static str, AssetError> {
        ASSETS.get().ok_or(AssetError::Uninitialized)?._get_txt_file(path)
    }

    pub fn get_caveinfo(sublevel: &Sublevel) -> Result<&'static CaveInfo, AssetError> {
        ASSETS.get().ok_or(AssetError::Uninitialized)?._get_caveinfo(sublevel)
    }

    pub fn get_img<P: AsRef<Path>>(path: P) -> Result<&'static RgbaImage, AssetError> {
        ASSETS.get().ok_or(AssetError::Uninitialized)?._get_img(path)
    }

    pub fn get_or_store_img(key: String, generator: impl FnOnce() -> Result<RgbaImage, AssetError>) -> Result<&'static RgbaImage, AssetError> {
        let manager = ASSETS.get().ok_or(AssetError::Uninitialized)?;
        if manager.img_cache.get(&key).is_none() {
            manager._store_img(key.clone(), generator()?);
        }
        manager._get_img(&key)
    }

    pub fn teki_list() -> Result<&'static [String], AssetError> {
        Ok(ASSETS.get().ok_or(AssetError::Uninitialized)?.teki.as_slice())
    }

    pub fn treasure_list() -> Result<&'static [Treasure], AssetError> {
        Ok(ASSETS.get().ok_or(AssetError::Uninitialized)?.treasures.as_slice())
    }

    pub fn room_list() -> Result<&'static [String], AssetError> {
        Ok(ASSETS.get().ok_or(AssetError::Uninitialized)?.rooms.as_slice())
    }

    /// Forces the asset manager to load all the Caveinfo files in Vanilla Pikmin 2.
    /// Most useful for testing and benchmarking purposes.
    pub fn preload_vanilla_caveinfo() -> Result<(), AssetError> {
        let assets = ASSETS.get().ok_or(AssetError::Uninitialized)?;
        for cave in ALL_VANILLA_CAVES {
            assets.load_caveinfo(AssetManager::find_cave_cfg(cave, Some("pikmin2"), false)?)?;
        }
        Ok(())
    }

    /// Clones the sublevel cache and returns it.
    /// Most useful for testing.
    pub fn all_sublevels() -> Result<PinMap<Sublevel, CaveInfo>, AssetError> {
        Ok(ASSETS.get().ok_or(AssetError::Uninitialized)?.caveinfo_cache.clone())
    }

    pub(crate) fn find_cave_cfg(name: &str, game: Option<&str>, force_challenge_mode: bool) -> Result<&'static CaveConfig, AssetError> {
        ASSETS.get().ok_or(AssetError::Uninitialized)?.cave_cfg.iter()
            .filter(|cfg| {
                game.map(|game_name| cfg.game.eq_ignore_ascii_case(game_name)).unwrap_or(true) && (!force_challenge_mode || cfg.is_challenge_mode)
            })
            .find(|cfg| {
                cfg.shortened_names.iter().any(|n| name.eq_ignore_ascii_case(n))
                || cfg.full_name.eq_ignore_ascii_case(name.as_ref())
            })
            .ok_or_else(|| Box::new(SublevelError::UnrecognizedSublevel(name.to_string())).into())
    }

    fn _get_txt_file<P: AsRef<Path>>(&self, path: P) -> Result<&str, AssetError> {
        let p_str: String = path.as_ref().to_string_lossy().into();
        if let Some(value) = self.txt_cache.get(&p_str) {
            Ok(value)
        }
        else {
            info!("Loading {}...", &p_str);
            if path.as_ref().starts_with("resources") {
                let data = read(self.resources_loc.join(path)).map_err(|e| AssetError::IoError(p_str.clone(), e.kind()))?;
                let _ = self.txt_cache.insert(
                    p_str.clone(), 
                    String::from_utf8(data)
                        .map_err(|_| AssetError::DecodingError(p_str.clone()))?
                );
            }
            else {
                let data = read(self.asset_path.join(path)).map_err(|e| AssetError::IoError(p_str.clone(), e.kind()))?;
                let _ = self.txt_cache.insert(p_str.clone(), SHIFT_JIS.decode(data.as_slice()).0.into_owned());
            }
            Ok(self.txt_cache.get(&p_str).unwrap())
        }
    }

    fn _get_caveinfo<'a>(&'a self, sublevel: &Sublevel) -> Result<&'a CaveInfo, AssetError> {
        if let Some(value) = self.caveinfo_cache.get(sublevel) && !sublevel.cfg.game.eq_ignore_ascii_case(DIRECT_MODE_TAG) {
            Ok(value)
        }
        else {
            self.load_caveinfo(&sublevel.cfg)?;
            Ok(self.caveinfo_cache.get(sublevel).unwrap())
        }
    }

    fn _get_img<P: AsRef<Path>>(&self, path: P) -> Result<&RgbaImage, AssetError> {
        let p_str: String = path.as_ref().to_string_lossy().into();
        let path: PathBuf = if path.as_ref().starts_with("resources") { 
            self.resources_loc.join(path.as_ref()) 
        } else { 
            self.asset_path.join(path) 
        };

        if let Some(value) = self.img_cache.get(&p_str) {
            Ok(value)
        }
        else {
            info!("Loading image {}...", &p_str);
            let data = read(&path).map_err(|e| AssetError::IoError(p_str.clone(), e.kind()))?;
            let img = image::load_from_memory(data.as_slice()).map_err(|_| AssetError::DecodingError(p_str.clone()))?
                .into_rgba8();
            let _ = self.img_cache.insert(p_str.clone(), img);
            Ok(self.img_cache.get(&p_str).unwrap())
        }
    }

    fn _store_img(&self, key: String, img: RgbaImage) {
        let _ = self.img_cache.insert(key, img);
    }

    /// Loads, parses, and stores a CaveInfo file
    fn load_caveinfo(&self, cave: &CaveConfig) -> Result<(), AssetError> {
        info!("Loading CaveInfo for {}...", cave.full_name);
        let caveinfo_txt = self._get_txt_file(&cave.get_caveinfo_path())?;
        let caveinfos = CaveInfo::parse_from(caveinfo_txt, cave)
            .map_err(|e| AssetError::CaveInfoError(cave.get_caveinfo_path().to_string_lossy().to_string(), Box::new(e)))?;
        for mut caveinfo in caveinfos.into_iter() {
            let sublevel = Sublevel::from_cfg(cave, (caveinfo.floor_num+1) as usize);
            caveinfo.sublevel = sublevel.clone();
            
            if self.caveinfo_cache.insert(sublevel, caveinfo).is_ok() {
                warn!("Tried to replace CaveInfo {} in cache. Caveinfo NOT updated.", cave.caveinfo_filename);
                //info!("Replaced CaveInfo {} in cache", cave.caveinfo_filename);
            }
        }

        Ok(())
    }
}

/// Metadata about a cave. Defined in resources/cave_config.txt
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct CaveConfig {
    pub game: String,  // Indicates either the vanilla game or a romhack
    pub full_name: String,
    pub is_challenge_mode: bool,
    pub shortened_names: Vec<String>,
    pub caveinfo_filename: String,
}

impl CaveConfig {
    pub(crate) fn get_caveinfo_path(&self) -> PathBuf {
        if self.game.eq_ignore_ascii_case("caveinfo") {
            PathBuf::from(&self.caveinfo_filename)
        }
        else {
            PathBuf::from(&self.game).join("user/Mukki/mapunits/caveinfo").join(&self.caveinfo_filename)
        }
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
        "wakame_s" => Some("wakame_s.png"),
        "chiyogami" => Some("chiyogami.PNG"),
        "rock" => Some("Bingo_Battle_Rock_Storm_icon.png"),
        _ => None
    }
}

static ALL_VANILLA_CAVES: [&str; 44] = [
    "ec", "scx", "fc", "hob", "wfg", "bk", "sh", "cos", "gk", "sr", "smc", "coc", "hoh", 
    "dd", "exc", "nt", "ltb", "cg", "gh", "hh", "ba", "rc", "tg", "twg", "cc", "cm", 
    "cr", "dn", "ca", "sp", "tct", "ht", "csn", "gb", "rg", "sl", "hg", "ad", "str", 
    "bg", "cop", "bd", "snr", "er"
];

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
