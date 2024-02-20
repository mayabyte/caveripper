use super::StructuralQuery;
use crate::{assets::AssetManager, query::Query};

fn test_query(query_str: &str, success_seeds: &[u32], failure_seeds: &[u32]) {
    let mgr = AssetManager::init().expect("Couldn't init asset manager");
    let query = StructuralQuery::try_parse(query_str, &mgr).unwrap_or_else(|e| panic!("Couldn't parse query string '{query_str}'\n{e}"));
    for seed in success_seeds {
        assert!(query.matches(*seed, &mgr));
    }
    for seed in failure_seeds {
        assert!(!query.matches(*seed, &mgr));
    }
}

#[test]
fn test_towerless() {
    test_query(
        "scx7 minihoudai < 2",
        &[
            0xB5E72294, 0x5A7BED1B, 0x81B6883D, 0xF07C8497, 0xCCA91228, 0x69038F7C, 0xDAD4B7A9, 0xF9230A5C, 0xDF0ABBDC, 0xC419B9C9,
        ],
        &[
            0x42AC4C0F, 0x3E6026DE, 0x26834113, 0xF26DA583, 0x036B3F40, 0xFD37369A, 0x5ED119C1, 0x14CAFF6E, 0x906DC0EE, 0xDDD13F62,
        ],
    );
}

#[test]
fn test_room_path() {
    test_query(
        "sh6 room_4x4f_4_conc + ship -> room_4x4g_4_conc + bluekochappy/bey_goma + fuefuki -> room_4x4b_4_conc -> alcove + hole",
        &[
            0x17531C52, 0x7A1B9265, 0x2178B525, 0x19775101, 0x7E6568A2, 0xB98790CE, 0xE366862F, 0x7F9E8E7F, 0x73A52E53, 0x7B5058DF,
        ],
        &[
            0x42AC4C0F, 0x3E6026DE, 0x26834113, 0xF26DA583, 0x036B3F40, 0xFD37369A, 0x5ED119C1, 0x14CAFF6E, 0x906DC0EE, 0xDDD13F62,
        ],
    );
}

#[test]
fn test_clackerless() {
    test_query(
        "gk3 castanets = 0",
        &[
            0x3D10D570, 0xF5F4D7A8, 0x950A49A3, 0x072BDE2E, 0x9D1F0152, 0xE3EF8C67, 0xA45CE0BA, 0xA8DF21A4, 0x16968C0D, 0xA5D15522,
        ],
        &[
            0x0407B6C5, 0xEA493EAC, 0xA92697B2, 0xFFBF35A8, 0x31A9BEFF, 0x732B700C, 0x505282B2, 0x240E934C, 0x23BBEA60, 0xD9D7CC12,
        ],
    );
}

#[test]
fn test_carry_path_dist() {
    test_query(
        "fc3 toy_ring_a_green carry dist < 300",
        &[
            0x9D4164CC, 0x842ABAB1, 0x54A1B893, 0x10EDCE57, 0x81A33CA9, 0xEFA3E148, 0x6630339E, 0xAFC3EE43, 0x41E8F241, 0xA37FF7D4,
        ],
        &[
            0xB1435939, 0x9FB6BBBF, 0xD2B591DF, 0x60D6A590, 0x9FA3A0BC, 0xB86699E2, 0x142BAC3D, 0xDFD03C02, 0x269E4A6F, 0x92618215,
        ],
    );
}

#[test]
fn test_not_gated() {
    test_query(
        "cos4 hole not gated",
        &[
            0x3217B696, 0xEBCBACA6, 0x676DBC71, 0x31FE7AD3, 0x439AC19F, 0x140784F1, 0x10C90541, 0xD5CE0227, 0xDE71515F, 0xBF9A701A,
        ],
        &[
            0xD975FE4B, 0xCB1BC5D5, 0x4AE3BF7D, 0xDD5F6117, 0xB5E1865C, 0x120BEC31, 0xEC30A1E8, 0x910B7612, 0xD766A798, 0x0654F60C,
        ],
    );
}

#[test]
fn test_parse_room_type_names() {
    let mgr = AssetManager::init().expect("Couldn't init asset manager");
    let query_strings = [
        "scx8 hallway > 50",
        "scx8 hall > 50",
        "cos1 cap > 10",
        "cos1 alcove > 10",
        "sr7 room < 2",
    ];
    for s in query_strings {
        StructuralQuery::try_parse(s, &mgr).unwrap_or_else(|_| panic!("Failed to parse query string \"{s}\""));
    }
}

#[test]
fn test_parse_count_queries() {
    let mgr = AssetManager::init().expect("Couldn't init asset manager");
    let query_strings = ["fc2 room_saka1_1_snow = 2", "scx7 room_ari1_3_metal < 2", "bd1 geyser = 0"];
    for s in query_strings {
        StructuralQuery::try_parse(s, &mgr).unwrap_or_else(|_| panic!("Failed to parse query string \"{s}\""));
    }
}

#[test]
fn test_room_path_whitespace() {
    let mgr = AssetManager::init().expect("Couldn't init asset manager");
    let query_strings = [
        "fc2 any+ship->any+toy_ring_c_blue",
        "fc2 any + ship -> any + toy_ring_c_blue",
        "fc2 any    +    ship   ->    any +        toy_ring_c_blue",
    ];
    for s in query_strings {
        StructuralQuery::try_parse(s, &mgr).unwrap_or_else(|_| panic!("Failed to parse query string \"{s}\""));
    }
}

#[test]
fn test_game_specifier_in_sublevel() {
    let mgr = AssetManager::init().expect("Couldn't init asset manager");
    let query_string = "216:tr12 randpom < 1";
    StructuralQuery::try_parse(query_string, &mgr).unwrap_or_else(|_| panic!("Failed to parse query string \"{query_string}\""));
}
