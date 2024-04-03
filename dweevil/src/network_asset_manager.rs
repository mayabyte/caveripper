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
    fn treasure_info(&self, game: &str, name: &str) -> Result<&Treasure, CaveripperError> {
        todo!()
    }

    fn combined_treasure_list(&self) -> Result<Vec<Treasure>, CaveripperError> {
        todo!()
    }

    fn teki_list(&self, game: &str) -> Result<&Vec<String>, CaveripperError> {
        todo!()
    }

    fn combined_teki_list(&self) -> Result<Vec<String>, CaveripperError> {
        todo!()
    }

    fn room_list(&self, game: &str) -> Result<&Vec<String>, CaveripperError> {
        todo!()
    }

    fn combined_room_list(&self) -> Result<Vec<String>, CaveripperError> {
        todo!()
    }

    fn get_bytes<P: AsRef<Path>>(&self, path: P) -> Result<Vec<u8>, CaveripperError> {
        todo!()
    }

    fn find_cave_cfg(&self, name: &str, game: Option<&str>, force_challenge_mode: bool) -> Result<&CaveConfig, CaveripperError> {
        todo!()
    }

    fn get_txt_file<P: AsRef<Path>>(&self, path: P) -> Result<String, CaveripperError> {
        todo!()
    }

    fn get_caveinfo<'a>(&'a self, sublevel: &Sublevel) -> Result<&'a CaveInfo, CaveripperError> {
        todo!()
    }

    fn get_img<P: AsRef<Path>>(&self, path: P) -> Result<&RgbaImage, CaveripperError> {
        todo!()
    }
}
