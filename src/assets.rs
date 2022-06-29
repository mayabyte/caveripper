use encoding_rs::SHIFT_JIS;
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use image::DynamicImage;
use log::debug;
use once_cell::sync::Lazy;
use rust_embed::RustEmbed;
use dashmap::{DashMap, mapref::one::Ref};

use crate::caveinfo::{CaveInfo, CaveInfoError, FloorInfo, parse::parse_caveinfo};
use crate::errors::AssetError;
use crate::sublevel::Sublevel;

#[derive(RustEmbed)]
#[folder="$CARGO_MANIFEST_DIR/assets"]
#[prefix="assets/"]
struct StaticAssets;

#[derive(RustEmbed)]
#[folder="$CARGO_MANIFEST_DIR/resources"]
#[prefix="resources/"]
struct StaticResources;

pub static ASSETS: Lazy<AssetManager> = Lazy::new(|| AssetManager::new());

pub struct AssetManager {
    txt_cache: DashMap<String, String>,
    caveinfo_cache: DashMap<Sublevel, FloorInfo>,
    img_cache: DashMap<String, DynamicImage>,
    custom_img_cache: DashMap<String, DynamicImage>,
    pub treasures: Vec<String>,
    pub enemies: Vec<String>,
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
            enemies: Vec::new(),
            cave_cfg: Vec::new(),
        };

        let treasures = String::from_utf8(StaticResources::get("resources/treasures.txt").unwrap().data.as_ref().to_vec()).unwrap();
        let ek_treasures = String::from_utf8(StaticResources::get("resources/treasures_exploration_kit.txt").unwrap().data.as_ref().to_vec()).unwrap();
        let mut treasure_names: Vec<String> = treasures
            .lines()
            .chain(ek_treasures.lines())
            .filter(|line| line.len() > 0)
            .map(|line| line.split_once(',').unwrap().1.to_owned())
            .collect();
        treasure_names.sort();
        mgr.treasures = treasure_names;

        let enemies: Vec<String> = StaticAssets::iter()
            .filter_map(|p| p.strip_prefix("assets/enemytex/arc.d/").and_then(|p| p.strip_suffix("/texture.bti.png")).map(|p| p.to_string()))
            .filter(|path| !path.contains("/"))
            .collect();
        mgr.enemies = enemies;

        let cave_cfg: Vec<CaveConfig> = String::from_utf8(StaticResources::get("resources/caveinfo_config.txt").unwrap().data.as_ref().to_vec()).unwrap()
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
            debug!("Loading {}...", path);
            if path.starts_with("assets") {
                let file = StaticAssets::get(path)?;
                self.txt_cache.insert(path.to_string(), SHIFT_JIS.decode(&file.data).0.into_owned());
            }
            else if path.starts_with("resources") {
                let file = StaticResources::get(path)?;
                self.txt_cache.insert(path.to_string(), String::from_utf8(file.data.as_ref().to_vec()).ok()?);
            }
        }
        Some(self.txt_cache.get(path)?.clone())
    }

    pub fn get_caveinfo(&self, cave: &str) -> Result<FloorInfo, AssetError> {
        let sublevel: Sublevel = cave.try_into()?;
        if !self.caveinfo_cache.contains_key(&sublevel) {
            self.load_caveinfo(&sublevel.cfg)?;
        }
        Ok(self.caveinfo_cache.get(&sublevel).expect("Caveinfo cache possibly corrupted!").clone())
    }

    pub fn get_img(&self, path: &str) -> Option<Ref<String, DynamicImage>> {
        if !self.img_cache.contains_key(path) {
            debug!("Loading image {}...", path);
            if path.starts_with("assets") {
                let img = image::load_from_memory(StaticAssets::get(path)?.data.as_ref()).ok()?;
                self.img_cache.insert(path.to_string(), img);
            }
            else if path.starts_with("resources") {
                let img = image::load_from_memory(StaticResources::get(path)?.data.as_ref()).ok()?;
                self.img_cache.insert(path.to_string(), img);
            }
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
    pub fn all_sublevels(&self) -> DashMap<Sublevel, FloorInfo> {
        self.caveinfo_cache.clone()
    }

    pub(crate) fn find_cave_cfg(&self, name: &str) -> Option<&CaveConfig> {
        // Check short specifiers first since they're the most common use case.
        // This is done in its own loop to avoid doing the more expensive matching
        // if it's not needed.
        for cfg in self.cave_cfg.iter() {
            if let Some(_) = cfg.shortened_names.iter().find(|e| name.eq_ignore_ascii_case(e)) {
                return Some(cfg);
            }
        }

        // Attempt to fuzzy match against the full cave name as a backup
        let matcher = SkimMatcherV2::default().ignore_case();
        self.cave_cfg.iter()
            .max_by_key(|cfg| matcher.fuzzy_match(&cfg.full_name, name))
    }

    /// Loads and parses a caveinfo file, then stores the
    /// resultant FloorInfo structs in the cache.
    fn load_caveinfo(&self, cave: &CaveConfig) -> Result<(), AssetError> {
        debug!("Loading CaveInfo for {}...", cave.full_name);
        let caveinfo_filename = format!("assets/caveinfo/{}", cave.caveinfo_filename);
        let caveinfo_txt = self.get_txt_file(&caveinfo_filename)
            .ok_or_else(|| CaveInfoError::MissingFileError(caveinfo_filename.clone()))?;
        let floor_chunks = parse_caveinfo(&caveinfo_txt)
            .map_err(|_| CaveInfoError::ParseFileError(caveinfo_filename.clone()))?
            .1;
    
        let result = CaveInfo::try_from(floor_chunks)?;
        for mut floorinfo in result.floors.into_iter() {
            let sublevel = Sublevel::from_cfg(cave, (floorinfo.sublevel+1) as usize);
            floorinfo.cave_name = Some(sublevel.normalized_name());
            if !self.caveinfo_cache.contains_key(&sublevel) {
                self.caveinfo_cache.insert(sublevel, floorinfo);
            }
        }

        Ok(())
    }
}

/// Metadata about a cave, including its full name, possible shortened names,
/// and caveinfo filename.
#[derive(Clone, Hash, Eq, PartialEq)]
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