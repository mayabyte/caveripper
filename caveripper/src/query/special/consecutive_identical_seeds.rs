use itertools::Itertools;

use crate::{
    assets::AssetManager,
    caveinfo::TekiInfo,
    layout::{Layout, SpawnObject},
    query::Query,
    sublevel::Sublevel,
};

pub struct ConsecutiveIdenticalSeedsQuery {
    pub sublevel: Sublevel,
    pub num_consecutive: u32,
}

impl Query for ConsecutiveIdenticalSeedsQuery {
    fn matches(&self, seed: u32, mgr: &impl AssetManager) -> bool {
        let caveinfo = mgr.get_caveinfo(&self.sublevel).unwrap();
        let layouts: Vec<Layout> = (seed..seed + self.num_consecutive)
            .into_iter()
            .map(|actual_seed| Layout::generate(actual_seed, caveinfo))
            .collect();

        let mut units_equal = false;
        let num_units_equal = layouts.iter().map(|layout| layout.map_units.len()).all_equal();
        if num_units_equal {
            units_equal = layouts
                .iter()
                .map(|layout| {
                    layout
                        .map_units
                        .iter()
                        .map(|unit| (&unit.unit.unit_folder_name, unit.unit.rotation, unit.x, unit.z))
                        .collect_vec()
                })
                .all_equal();
        }

        let treasures_equal = layouts
            .iter()
            .map(|layout| {
                layout
                    .get_spawn_objects()
                    .filter_map(|(so, pos)| match so {
                        SpawnObject::Item(info) => Some((pos, &info.internal_name)),
                        SpawnObject::Teki(
                            TekiInfo {
                                carrying: Some(carried), ..
                            },
                            _,
                        ) => Some((pos, carried)),
                        _ => None,
                    })
                    .collect_vec()
            })
            .all_equal();

        return units_equal && treasures_equal;
    }
}
