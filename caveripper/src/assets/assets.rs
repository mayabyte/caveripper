mod pinmap;

use std::collections::{HashSet, HashMap};
use std::fs::{read_to_string, read_dir, read};
use std::path::{Path, PathBuf};
use encoding_rs::SHIFT_JIS;
use image::RgbaImage;
use itertools::Itertools;
use log::info;
use serde::Serialize;
use error_stack::{Result, IntoReport, ResultExt};

use crate::caveinfo::CaveInfo;
use crate::errors::CaveripperError;
use pinmap::PinMap;
use crate::sublevel::{Sublevel, DIRECT_MODE_TAG};


/// Version number written into extracted folders to allow programmatic
/// compatibility checking between extracts and Caveripper binary versions.
/// This number should be incremented any time the extraction process
/// changes or improves.
pub const ASSET_VERSION: u32 = 1;

pub struct AssetManager {
    /// Folder that assets are kept in. This is in ~/.config/caveripper by default.
    asset_dir: PathBuf,

    caveinfo_cache: PinMap<Sublevel, CaveInfo>,
    img_cache: PinMap<String, RgbaImage>,
    pub cave_cfg: Vec<CaveConfig>,

    /// All known treasure names. All lowercase so they can be easily compared.
    treasures: PinMap<String, Vec<Treasure>>,

    /// All known teki names. All lowercase so they can be easily compared.
    teki: PinMap<String, Vec<String>>,

    /// All known room names.
    rooms: PinMap<String, Vec<String>>,
}

impl AssetManager {
    pub fn init() -> Result<AssetManager, CaveripperError> {
        let mut asset_dir = dirs::home_dir()
            .ok_or(CaveripperError::AssetLoadingError)
            .into_report().attach_printable("Couldn't access home directory!")?;
        asset_dir.push(".config/caveripper");

        #[cfg(debug_assertions)]
        {
            use std::fs::create_dir_all;
            use fs_extra::dir::{copy, CopyOptions};
            println!("(!) Copying resource and assets folders to {}", asset_dir.to_str().unwrap());
            create_dir_all(&asset_dir)
                .expect("Couldn't create assets dir in HOME!");
            let _ = copy(
                PathBuf::from_iter([env!("CARGO_MANIFEST_DIR"), "..", "resources"]),
                &asset_dir,
                &CopyOptions { overwrite: true, ..Default::default() }
            );
            let _ = copy(
                PathBuf::from_iter([env!("CARGO_MANIFEST_DIR"), "..", "assets"]),
                &asset_dir,
                &CopyOptions { overwrite: true, ..Default::default() }
            );
        }

        let cave_cfg: Vec<CaveConfig> = read_to_string(asset_dir.join("resources/caveinfo_config.txt"))
            .into_report().change_context(CaveripperError::AssetLoadingError)?
            .lines()
            .map(|line| {
                let mut data: Vec<String> = line.split(',').map(|e| e.trim().to_string()).collect();
                CaveConfig {
                    game: data.remove(0),
                    full_name: data.remove(0),
                    is_challenge_mode: data.remove(0).parse().expect("is_challenge_mode parse error"),
                    caveinfo_filename: data.remove(0),
                    shortened_names: data,
                }
            })
            .collect::<Vec<_>>();

        Ok(Self {
            asset_dir,
            caveinfo_cache: PinMap::new(),
            img_cache: PinMap::new(),
            cave_cfg,
            treasures: PinMap::new(),
            teki: PinMap::new(),
            rooms: PinMap::new(),
        })
    }

    fn all_games(&self) -> HashSet<&str> {
        self.cave_cfg.iter().map(|cfg| cfg.game.as_str()).collect()
    }

    pub fn treasure_list(&self, game: &str) -> Result<&Vec<Treasure>, CaveripperError> {
        if let Some(treasure_list) = self.treasures.get(game) {
            Ok(treasure_list)
        }
        else {
            let treasure_path = self.asset_dir.join("assets").join(game).join("otakara_config.txt");
            let ek_treasure_path = self.asset_dir.join("assets").join(game).join("item_config.txt");

            let treasures = SHIFT_JIS.decode(
                read(&treasure_path)
                .into_report().change_context(CaveripperError::AssetLoadingError).attach(treasure_path)?
                .as_slice()
            ).0.into_owned();
            let ek_treasures = SHIFT_JIS.decode(
                read(&ek_treasure_path)
                .into_report().change_context(CaveripperError::AssetLoadingError).attach(ek_treasure_path)?
                .as_slice()
            ).0.into_owned();

            let mut treasures = parse_treasure_config(&treasures);
            treasures.append(&mut parse_treasure_config(&ek_treasures));
            treasures.sort_by(|t1, t2| t1.internal_name.cmp(&t2.internal_name));

            let _ = self.treasures.insert(game.to_string(), treasures);
            Ok(self.treasures.get(game).unwrap())
        }
    }

    // Combines the Teki List from all known games.
    pub fn combined_treasure_list(&self) -> Result<Vec<Treasure>, CaveripperError> {
        self.all_games().into_iter()
            .try_fold(Vec::new(), |mut acc, game| {
                acc.extend(self.treasure_list(game)?.clone());
                Ok(acc)
            })
    }

    pub fn teki_list(&self, game: &str) -> Result<&Vec<String>, CaveripperError> {
        if let Some(teki_list) = self.teki.get(game) {
            Ok(teki_list)
        }
        else {
            // Eggs are not listed in enemytex, so they have to be added manually
            let mut all_teki = vec!["egg".to_string()];

            let teki_path = self.asset_dir.join("assets").join(game).join("teki");
            let teki = read_dir(&teki_path)
                .into_report().change_context(CaveripperError::AssetLoadingError).attach(teki_path)?
                .filter_map(|r| r.ok())
                .filter(|entry| entry.path().is_file())
                .map(|file_entry| file_entry.file_name().into_string().unwrap().strip_suffix(".png").unwrap().to_ascii_lowercase());
            all_teki.extend(teki);

            let _ = self.teki.insert(game.to_string(), all_teki);
            Ok(self.teki.get(game).unwrap())
        }
    }

    // Combines the Teki List from all known games.
    pub fn combined_teki_list(&self) -> Result<Vec<String>, CaveripperError> {
        self.all_games().into_iter()
            .try_fold(Vec::new(), |mut acc, game| {
                acc.extend(self.teki_list(game)?.clone());
                Ok(acc)
            })
    }

    pub fn room_list(&self, game: &str) -> Result<&Vec<String>, CaveripperError> {
        if let Some(room_list) = self.rooms.get(game) {
            Ok(room_list)
        }
        else {
            let mut all_rooms = Vec::new();

            let room_path = self.asset_dir.join("assets").join(game).join("mapunits");
            let rooms = read_dir(&room_path)
                .into_report().change_context(CaveripperError::AssetLoadingError).attach(room_path)?
                .filter_map(|r| r.ok())
                .filter(|dir_entry| dir_entry.path().is_dir())
                .map(|dir_entry| dir_entry.file_name().into_string().unwrap().to_ascii_lowercase());
            all_rooms.extend(rooms);

            let _ = self.rooms.insert(game.to_string(), all_rooms);
            Ok(self.rooms.get(game).unwrap())
        }
    }

    // Combines the Room List from all known games.
    pub fn combined_room_list(&self) -> Result<Vec<String>, CaveripperError> {
        self.all_games().into_iter()
            .try_fold(Vec::new(), |mut acc, game| {
                acc.extend(self.room_list(game)?.clone());
                Ok(acc)
            })
    }

    /// Get a file as raw bytes. Does not cache the file.
    pub fn get_bytes<P: AsRef<Path>>(&self, path: P) -> Result<Vec<u8>, CaveripperError> {
        let path = path.as_ref();
        read(self.asset_dir.join(path))
            .into_report()
            .change_context(CaveripperError::AssetLoadingError)
            .attach_lazy(|| path.to_owned())
    }

    pub fn get_or_store_img(&self, key: String, generator: impl FnOnce() -> Result<RgbaImage, CaveripperError>) -> Result<&RgbaImage, CaveripperError> {
        if self.img_cache.get(&key).is_none() {
            self.store_img(key.clone(), generator()?);
        }
        self.get_img(&key)
    }

    pub(crate) fn find_cave_cfg(&self, name: &str, game: Option<&str>, force_challenge_mode: bool) -> Result<&CaveConfig, CaveripperError> {
        self.cave_cfg.iter()
            .filter(|cfg| {
                game.map(|game_name| cfg.game.eq_ignore_ascii_case(game_name)).unwrap_or(true) && (!force_challenge_mode || cfg.is_challenge_mode)
            })
            .find(|cfg| {
                cfg.shortened_names.iter().any(|n| name.eq_ignore_ascii_case(n))
                || cfg.full_name.eq_ignore_ascii_case(name.as_ref())
            })
            .ok_or(CaveripperError::UnrecognizedSublevel)
            .into_report().attach_printable_lazy(|| name.to_string())
    }

    pub fn get_txt_file<P: AsRef<Path>>(&self, path: P) -> Result<String, CaveripperError> {
        let path = path.as_ref();
        let p_str = path.to_string_lossy().into_owned();
        info!("Loading {p_str}...");
        let data = read(self.asset_dir.join(path))
            .into_report().change_context(CaveripperError::AssetLoadingError)
            .attach_printable_lazy(|| p_str.clone())?;
        let text = if path.starts_with("assets") && let (text, _, false) = SHIFT_JIS.decode(&data) {
            text.into_owned()
        }
        else {
            String::from_utf8(data).into_report()
                .change_context(CaveripperError::AssetLoadingError)
                .attach_printable_lazy(|| format!("Couldn't decode file {p_str}"))?
        };
        Ok(text)
    }

    pub fn get_caveinfo<'a>(&'a self, sublevel: &Sublevel) -> Result<&'a CaveInfo, CaveripperError> {
        if let Some(value) = self.caveinfo_cache.get(sublevel) && !sublevel.cfg.game.eq_ignore_ascii_case(DIRECT_MODE_TAG) {
            Ok(value)
        }
        else {
            self.load_caveinfo(&sublevel.cfg)?;
            self.caveinfo_cache.get(sublevel).ok_or(CaveripperError::UnrecognizedSublevel)
                .into_report().attach_printable_lazy(|| sublevel.clone())
        }
    }

    pub fn get_img<P: AsRef<Path>>(&self, path: P) -> Result<&RgbaImage, CaveripperError> {
        let p_str: String = path.as_ref().to_string_lossy().into();
        let path: PathBuf = self.asset_dir.join(path.as_ref());

        if let Some(value) = self.img_cache.get(&p_str) {
            Ok(value)
        }
        else {
            info!("Loading image {}...", &p_str);
            let data = read(path).into_report().change_context(CaveripperError::AssetLoadingError).attach_printable_lazy(|| p_str.clone())?;
            let img = image::load_from_memory(data.as_slice()).into_report().change_context(CaveripperError::AssetLoadingError)?
                .into_rgba8();
            let _ = self.img_cache.insert(p_str.clone(), img);
            Ok(self.img_cache.get(&p_str).unwrap())
        }
    }

    pub fn store_img(&self, key: String, img: RgbaImage) {
        let _ = self.img_cache.insert(key, img);
    }

    /// Loads, parses, and stores a CaveInfo file
    fn load_caveinfo(&self, cave: &CaveConfig) -> Result<(), CaveripperError> {
        info!("Loading CaveInfo for {}...", cave.full_name);
        let caveinfos = CaveInfo::parse_from(cave, self)?;
        for mut caveinfo in caveinfos.into_iter() {
            let sublevel = Sublevel::from_cfg(cave, (caveinfo.floor_num+1) as usize);
            caveinfo.cave_cfg = cave.clone();

            if self.caveinfo_cache.insert(sublevel, caveinfo).is_err() {
                //warn!("Tried to replace CaveInfo {} in cache. Caveinfo NOT updated.", cave.caveinfo_filename);
                //info!("Replaced CaveInfo {} in cache", cave.caveinfo_filename);
            }
        }

        Ok(())
    }

    #[allow(dead_code)] // used in tests
    pub fn caveinfos_from_cave(&self, compound_name: &str) -> Result<Vec<&CaveInfo>, CaveripperError> {
        let (game_name, cave_name) = compound_name.split_once(':').unwrap_or(("pikmin2", compound_name));
        let cfg = self.find_cave_cfg(cave_name, Some(game_name), false)?;

        let mut floor = 1;
        let mut caveinfos = Vec::new();
        while let Ok(caveinfo) = self.get_caveinfo(&Sublevel::from_cfg(cfg, floor)) {
            caveinfos.push(caveinfo);
            floor += 1;
        }
        Ok(caveinfos)
    }
}

/// Metadata about a cave. Defined in resources/cave_config.txt
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize)]
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
            PathBuf::from_iter(["assets", &self.game, "caveinfo", &self.caveinfo_filename])
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
        "rock" => Some("Roulette_Wheel_boulder.png"),
        "panhouse" => Some("ooinu_s.png"),
        _ => None
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Treasure {
    pub internal_name: String,
    pub min_carry: u32,
    pub max_carry: u32,
    pub value: u32,
}

fn parse_treasure_config(config_txt: &str) -> Vec<Treasure> {
    config_txt.chars()
        .peekable()
        .batching(|chars| {
            let val = chars.skip_while(|c| c != &'{')
                .skip(1)
                .take_while(|c| c != &'}')
                .skip(1)
                .collect::<String>();
            if chars.peek().is_none() {
                None
            }
            else {
                Some(val)
            }
        })
        .filter(|section| !section.trim().is_empty())
        .map(|section| {
            let section: HashMap<&str, &str> = section.lines()
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
            Treasure { internal_name, min_carry, max_carry, value }
        })
        .collect_vec()
}
