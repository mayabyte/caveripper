use std::path::Path;

use caveripper::{
    assets::{parse_treasure_config, pinmap::PinMap, AssetManager, CaveConfig, ImageKind, Treasure},
    caveinfo::CaveInfo,
    errors::CaveripperError,
    sublevel::Sublevel,
};
use encoding_rs::SHIFT_JIS;
use error_stack::{bail, report, Result, ResultExt};
use image::RgbaImage;

use super::data::*;

pub struct WebAssetManager {
    cave_cfg: Vec<CaveConfig>,
    caveinfo_cache: PinMap<Sublevel, CaveInfo>,
    image_cache: PinMap<String, RgbaImage>,
    treasure_info: Vec<Treasure>,
}

impl WebAssetManager {
    pub fn new() -> Self {
        // On web, only support base game caves, no romhacks
        let cave_cfg = CaveConfig::parse_from_file(RESOURCES.get_file("caveinfo_config.txt").unwrap().contents_utf8().unwrap())
            .into_iter()
            .filter(|cfg| cfg.game.eq_ignore_ascii_case("pikmin2"))
            .collect();

        let mut treasure_info = parse_treasure_config(
            &SHIFT_JIS.decode(PIKMIN2.get_file("otakara_config.txt").unwrap().contents()).0,
            "pikmin2",
        );
        treasure_info.extend(parse_treasure_config(
            &SHIFT_JIS.decode(PIKMIN2.get_file("item_config.txt").unwrap().contents()).0,
            "pikmin2",
        ));

        Self {
            cave_cfg,
            caveinfo_cache: PinMap::new(),
            image_cache: PinMap::new(),
            treasure_info,
        }
    }
}

impl AssetManager for WebAssetManager {
    fn load_txt<P: AsRef<Path>>(&self, path: P) -> Result<String, CaveripperError> {
        let path = path.as_ref().strip_prefix("assets/pikmin2").unwrap();
        let file_contents = PIKMIN2.get_file(path).ok_or(CaveripperError::AssetLoadingError)?.contents();
        Ok(SHIFT_JIS.decode(file_contents).0.into_owned())
    }

    fn load_caveinfo<'a>(&'a self, sublevel: &Sublevel) -> Result<&'a CaveInfo, CaveripperError> {
        if let Some(value) = self.caveinfo_cache.get(sublevel) {
            Ok(value)
        } else {
            let caveinfos = CaveInfo::parse_from(&sublevel.cfg, self)?;
            for mut caveinfo in caveinfos.into_iter() {
                let sublevel = Sublevel::from_cfg(&sublevel.cfg, (caveinfo.floor_num + 1) as usize);
                caveinfo.cave_cfg = sublevel.cfg.clone();

                let _ = self.caveinfo_cache.insert(sublevel, caveinfo);
            }

            self.caveinfo_cache
                .get(sublevel)
                .ok_or(CaveripperError::UnrecognizedSublevel)
                .attach_printable_lazy(|| sublevel.clone())
        }
    }

    fn load_image(&self, kind: ImageKind, _game: &str, name: &str) -> Result<&RgbaImage, CaveripperError> {
        let path = match kind {
            ImageKind::CaveUnit => format!("mapunits/{name}/arc/texture.png"),
            ImageKind::Teki => format!("teki/{name}.png"),
            ImageKind::Treasure => format!("treasures/{name}.png"),
            ImageKind::Special => format!("enemytex_special/{name}.png"),
        };
        if let Some(img) = self.image_cache.get(&path) {
            return Ok(img);
        }

        let raw = if path.starts_with("enemytex_special") {
            RESOURCES.get_file(&path)
        } else {
            PIKMIN2.get_file(&path)
        }
        .ok_or(CaveripperError::AssetLoadingError)?
        .contents();

        let img = image::load_from_memory_with_format(&raw, image::ImageFormat::Png)
            .change_context(CaveripperError::AssetLoadingError)
            .attach_printable(name.to_owned())?
            .to_rgba8();
        let _ = self.image_cache.insert(path.clone(), img);

        Ok(self.image_cache.get(&path).unwrap())
    }

    fn load_raw<P: AsRef<Path>>(&self, path: P) -> Result<Vec<u8>, CaveripperError> {
        let path = path.as_ref();
        if path.starts_with("resources") {
            Ok(RESOURCES
                .get_file(path.strip_prefix("resources").unwrap())
                .ok_or(CaveripperError::AssetLoadingError)?
                .contents()
                .to_owned())
        } else {
            todo!("nothing uses this path yet")
        }
    }

    fn all_teki(&self, _game: Option<&str>) -> Result<Vec<String>, CaveripperError> {
        Ok(PIKMIN2
            .get_dir("teki")
            .expect("No teki dir in Pikmin 2 game file extract")
            .files()
            .map(|file| {
                file.path()
                    .to_str()
                    .unwrap()
                    .split('/')
                    .last()
                    .unwrap()
                    .strip_suffix(".png")
                    .unwrap()
                    .to_owned()
            })
            .chain([String::from("egg"), String::from("bomb"), String::from("hiba")].into_iter())
            .collect())
    }

    fn all_units(&self, _game: Option<&str>) -> Result<Vec<String>, CaveripperError> {
        Ok(PIKMIN2
            .get_dir("mapunits")
            .expect("No mapunits dir in Pikmin 2 game file extract")
            .entries()
            .into_iter()
            .filter_map(|entry| entry.as_dir())
            .map(|dir| dir.path().to_str().unwrap().split('/').last().unwrap().to_owned())
            .collect())
    }

    fn all_treasures(&self, _game: Option<&str>) -> Result<Vec<Treasure>, CaveripperError> {
        Ok(PIKMIN2
            .get_dir("treasures")
            .expect("No treasures dir in Pikmin 2 game file extract")
            .files()
            .map(|file| {
                file.path()
                    .to_str()
                    .unwrap()
                    .split('/')
                    .last()
                    .unwrap()
                    .strip_suffix(".png")
                    .unwrap()
                    .to_owned()
            })
            .filter_map(|treasure_name| {
                self.treasure_info
                    .iter()
                    .find(|treasure| treasure.internal_name.eq_ignore_ascii_case(&treasure_name))
                    .cloned()
            })
            .collect())
    }

    fn get_treasure_info(&self, _game: &str, name: &str) -> Result<&Treasure, CaveripperError> {
        self.treasure_info
            .iter()
            .find(|treasure| treasure.internal_name.eq_ignore_ascii_case(name))
            .ok_or(report!(CaveripperError::AssetLoadingError))
            .attach_printable(name.to_owned())
    }

    fn get_cave_cfg(&self, name: &str, game: Option<&str>, _force_challenge_mode: bool) -> Result<&CaveConfig, CaveripperError> {
        if game.is_some_and(|v| !v.eq_ignore_ascii_case("pikmin2")) {
            bail!(CaveripperError::UnrecognizedGame);
        }
        self.cave_cfg
            .iter()
            .find(|cfg| {
                cfg.shortened_names.iter().any(|n| name.eq_ignore_ascii_case(n)) || cfg.full_name.eq_ignore_ascii_case(name.as_ref())
            })
            .ok_or(CaveripperError::UnrecognizedSublevel)
            .attach_printable_lazy(|| name.to_string())
    }
}
