use once_cell::sync::Lazy;
use super::ALL_SUBLEVELS;

/// Simple check to ensure all caves can be parsed without panicking.
#[test]
fn test_caveinfo_parsing() {
    for sublevel_caveinfo in ALL_SUBLEVELS {
        Lazy::force(sublevel_caveinfo);
    }
}
