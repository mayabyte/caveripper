use super::*;

#[test]
fn check_parsing_succeeds() {
    for sublevel in ALL_SUBLEVELS_POD {
        println!("{}", sublevel);
        get_sublevel_info(sublevel).unwrap();
    }
}
