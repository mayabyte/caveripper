pub fn sqrt(val: f32) -> f32 {
    (val as f64 * fast_inverse_sqrt(val as f64)) as f32
}

/// https://en.wikipedia.org/wiki/Fast_inverse_square_root
/// Approximates the inverse sqrt to within 1/32 of the true value.
/// Used in Pikmin 2 via PowerPC's `frsqrte` instruction.
pub fn fast_inverse_sqrt(val: f64) -> f64 {
    unimplemented!()
}
