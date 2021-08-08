use crate::layout::boxes_overlap;

#[test]
fn test_collision() {
    assert!(!boxes_overlap(0, 0, 5, 7, 5, 5, 5, 5))
}
