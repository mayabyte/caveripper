use std::path::Path;

use caveripper::{
    assets::{AssetManager, CaveConfig, Treasure},
    caveinfo::CaveInfo,
    errors::CaveripperError,
    sublevel::Sublevel,
};
use error_stack::Result;
use image::RgbaImage;

pub struct NetworkAssetManager {}

impl AssetManager for NetworkAssetManager {
    fn load_txt<P: AsRef<Path>>(&self, path: P) -> Result<String, CaveripperError> {
        todo!()
    }

    fn load_caveinfo<'a>(&'a self, sublevel: &Sublevel) -> Result<&'a CaveInfo, CaveripperError> {
        todo!()
    }

    fn load_image(&self, kind: caveripper::assets::ImageKind, game: &str, name: &str) -> Result<&RgbaImage, CaveripperError> {
        todo!()
    }

    fn load_raw<P: AsRef<Path>>(&self, path: P) -> Result<Vec<u8>, CaveripperError> {
        todo!()
    }

    fn all_teki(&self, game: Option<&str>) -> Result<Vec<String>, CaveripperError> {
        todo!()
    }

    fn all_units(&self, game: Option<&str>) -> Result<Vec<String>, CaveripperError> {
        todo!()
    }

    fn all_treasures(&self, game: Option<&str>) -> Result<Vec<Treasure>, CaveripperError> {
        todo!()
    }

    fn get_treasure_info(&self, game: &str, name: &str) -> Result<&Treasure, CaveripperError> {
        todo!()
    }

    fn get_cave_cfg(&self, name: &str, game: Option<&str>, force_challenge_mode: bool) -> Result<&CaveConfig, CaveripperError> {
        todo!()
    }
}
