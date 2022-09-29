use crate::assets::AssetManager;

use super::Query;

#[test]
fn test_towerless() {
    AssetManager::init_global("../assets", "..").expect("Couldn't init asset manager");
    let query_string = "scx7 minihoudai < 2";
    let success_seeds = [0xB5E72294, 0x5A7BED1B, 0x81B6883D, 0xF07C8497, 0xCCA91228, 0x69038F7C, 0xDAD4B7A9, 0xF9230A5C, 0xDF0ABBDC, 0xC419B9C9];
    let failure_seeds = [0x42AC4C0F, 0x3E6026DE, 0x26834113, 0xF26DA583, 0x036B3F40, 0xFD37369A, 0x5ED119C1, 0x14CAFF6E, 0x906DC0EE, 0xDDD13F62];

    let query: Query = query_string.try_into().expect("couldn't parse query string");
    for seed in success_seeds {
        assert!(query.matches(seed));
    }
    for seed in failure_seeds {
        assert!(!query.matches(seed));
    }
}

#[test]
fn test_room_path() {
    AssetManager::init_global("../assets", "..").expect("Couldn't init asset manager");
    let query_string = "sh6 room_4x4f_4_conc + ship -> room_4x4g_4_conc + bluekochappy/bey_goma + fuefuki -> room_4x4b_4_conc -> alcove + hole";
    let success_seeds = [0x17531C52, 0x7A1B9265, 0x2178B525, 0x19775101, 0x7E6568A2, 0xB98790CE, 0xE366862F, 0x7F9E8E7F, 0x73A52E53, 0x7B5058DF];
    let failure_seeds = [0x42AC4C0F, 0x3E6026DE, 0x26834113, 0xF26DA583, 0x036B3F40, 0xFD37369A, 0x5ED119C1, 0x14CAFF6E, 0x906DC0EE, 0xDDD13F62];

    let query: Query = query_string.try_into().expect("couldn't parse query string");
    for seed in success_seeds {
        assert!(query.matches(seed));
    }
    for seed in failure_seeds {
        assert!(!query.matches(seed));
    }
}
