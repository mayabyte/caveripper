/// Thread safe, write-once, lazy initialized smart pointers for the parsed FloorInfo
/// objects for every sublevel in Pikmin 2. Caveinfo for each sublevel is exposed as a static
/// variable, e.g. `SH3`, that can be imported from the `crate::caveinfo` module.

use std::convert::TryFrom;
use std::collections::HashMap;
use once_cell::sync::Lazy;
use paste::paste;
use maplit::hashmap;
use crate::caveinfo::{
    CaveInfo, CaveInfoError, FloorInfo,
    cave_name_to_caveinfo_filename,
    parse::parse_caveinfo,
};
use crate::assets::get_file_JIS;

fn load_caveinfo(cave: &'static str) -> Result<CaveInfo, CaveInfoError> {
    let caveinfo_filename = format!("assets/gcn/caveinfo/{}", cave_name_to_caveinfo_filename(&cave));
    let caveinfo_txt = get_file_JIS(&caveinfo_filename)
        .ok_or_else(|| CaveInfoError::MissingFileError(caveinfo_filename.clone()))?;
    let floor_chunks = parse_caveinfo(&caveinfo_txt)
        .map_err(|_| CaveInfoError::ParseFileError(caveinfo_filename.clone()))?
        .1;

    let mut result = CaveInfo::try_from(floor_chunks)?;
    for mut sublevel in result.floors.iter_mut() {
        sublevel.cave_name = Some(cave.to_owned());
    }
    Ok(result)
}

macro_rules! preload_caveinfo {
    ($($rest_cave:ident, $($rest_floors:literal),+),+) => {
        pub static ALL_SUBLEVELS: [&Lazy<FloorInfo>; 104] = [
            $(
                $(
                    paste! {
                        &[<$rest_cave $rest_floors>]
                    }
                ),+
            ),+
        ];
        pub static ALL_SUBLEVELS_MAP: Lazy<HashMap<String, &Lazy<FloorInfo>>> = Lazy::new(|| hashmap! {
            $(
                $(
                    concat!(stringify!($rest_cave), stringify!($rest_floors)).to_ascii_lowercase() => paste! {&[<$rest_cave $rest_floors>]}
                ),+
            ),+
        });
        preload_caveinfo_individual!($($rest_cave, $($rest_floors),+),+);
    }
}

macro_rules! preload_caveinfo_individual {
    ($cave:ident, $($floor:literal),+) => {
        #[allow(non_upper_case_globals)]
        static $cave: Lazy<CaveInfo> = Lazy::new(|| load_caveinfo(stringify!($cave)).expect(concat!("Failed to load Caveinfo for ", stringify!($cave))));
        $(
            paste! {
                #[allow(non_upper_case_globals)]
                pub static [<$cave $floor>]: Lazy<FloorInfo> = Lazy::new(|| $cave.floors[$floor-1].clone());
            }
        )+
    };
    ($cave:ident, $($floor:literal),+, $($rest_cave:ident, $($rest_floors:literal),+),+) => {
        preload_caveinfo_individual!($cave, $($floor),+);
        preload_caveinfo_individual!($($rest_cave, $($rest_floors),+),+);
    };
}

preload_caveinfo!(
    EC, 1, 2,
    SCx, 1, 2, 3, 4, 5, 6, 7, 8, 9,
    FC, 1, 2, 3, 4, 5, 6, 7, 8,
    HoB, 1, 2, 3, 4, 5,
    WFG, 1, 2, 3, 4, 5,
    SH, 1, 2, 3, 4, 5, 6, 7,
    BK, 1, 2, 3, 4, 5, 6, 7,
    CoS, 1, 2, 3, 4, 5,
    GK, 1, 2, 3, 4, 5, 6,
    SR, 1, 2, 3, 4, 5, 6,
    SmC, 1, 2, 3, 4, 5,
    CoC, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10,
    DD, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14,
    HoH, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15
);

pub fn force_load_all() {
    for sublevel in ALL_SUBLEVELS {
        Lazy::force(sublevel);
    }
}
