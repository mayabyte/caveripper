mod network_asset_manager;
mod utils;

use caveripper::sublevel::Sublevel;
use network_asset_manager::NetworkAssetManager;
use utils::set_panic_hook;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn cavegen(sublevel: &str) {
    set_panic_hook();

    let mgr = NetworkAssetManager {};
    let sublevel = Sublevel::try_from_str(sublevel, &mgr);
}
