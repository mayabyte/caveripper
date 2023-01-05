use rand::random;

use crate::assets::AssetManager;
use super::Layout;

#[test]
fn test_bloysterless() {
    AssetManager::init_global("../assets", "..").unwrap();
    let caveinfo = AssetManager::get_caveinfo(&"SR7".try_into().unwrap()).unwrap();
    let layout = Layout::generate(0x31D70855, caveinfo);

    let bloyster = layout.get_spawn_objects().find(|so| so.name().eq_ignore_ascii_case("UmiMushi"));
    assert!(bloyster.is_none());
}
