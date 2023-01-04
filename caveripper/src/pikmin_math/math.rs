pub fn sqrt(val: f32) -> f32 {
    (val as f64 * fast_inverse_sqrt(val as f64)) as f32
}

/// https://en.wikipedia.org/wiki/Fast_inverse_square_root
/// Approximates the inverse sqrt to within 1/32 of the true value.
/// Used in Pikmin 2 via PowerPC's `frsqrte` instruction.
/// Implemented in Dolphin Emulator here:
/// https://github.com/dolphin-emu/dolphin/commit/cffa848b9960bcf3dd7a5f3dfd8cdbe417b6ec55#diff-903a032099cd9031620bb1c10e0f7409
const EXPECTED_BASE: [i32; 32] = [
    0x3ffa000, 0x3c29000, 0x38aa000, 0x3572000,
    0x3279000, 0x2fb7000, 0x2d26000, 0x2ac0000,
    0x2881000, 0x2665000, 0x2468000, 0x2287000,
    0x20c1000, 0x1f12000, 0x1d79000, 0x1bf4000,
    0x1a7e800, 0x17cb800, 0x1552800, 0x130c000,
    0x10f2000, 0x0eff000, 0x0d2e000, 0x0b7c000,
    0x09e5000, 0x0867000, 0x06ff000, 0x05ab800,
    0x046a000, 0x0339800, 0x0218800, 0x0105800,
];
const EXPECTED_DEC: [i32; 32] = [
    0x7a4, 0x700, 0x670, 0x5f2,
    0x584, 0x524, 0x4cc, 0x47e,
    0x43a, 0x3fa, 0x3c2, 0x38e,
    0x35e, 0x332, 0x30a, 0x2e6,
    0x568, 0x4f3, 0x48d, 0x435,
    0x3e7, 0x3a2, 0x365, 0x32e,
    0x2fc, 0x2d0, 0x2a8, 0x283,
    0x261, 0x243, 0x226, 0x20b,
];
pub fn fast_inverse_sqrt(val: f64) -> f64 {
    let valf: f64 = val;
    let mut vali: u64 = val.to_bits();

    let mut mantissa: u64 = vali & ((1u64 << 52) - 1);
    let sign: u64 = vali & (1u64 << 63);
    let mut exponent: u64 = vali & (0x7FF << 52);

    if mantissa == 0 && exponent == 0 {
        return 0.0;
    }

    // Special case NaN
    if exponent == (0x7FF << 52) {
        if mantissa == 0 {
            if sign != 0 {
                return f64::NAN;
            }
            return 0.0;
        }
        return 0.0 + valf;
    }

    // Negative numbers return NaN
    if sign != 0 {
        return f64::NAN;
    }

    if exponent == 0 {
        loop {
            exponent -= 1u64 << 52;
            mantissa <<= 1;
            if (mantissa & (1u64 << 52)) != 0 {
                break;
            }
        }

        mantissa &= (1u64 << 52) - 1;
        exponent += 1u64 << 52;
    }

    let odd_exponent: bool = 0 == (exponent & (1u64 << 52));
    exponent = ((0x3FFu64 << 52).wrapping_sub((exponent.wrapping_sub(0x3FEu64 << 52)) / 2)) & (0x7FFu64 << 52);

    let i: i32 = (mantissa >> 37) as i32;
    vali = sign | exponent;

    let index = (i / 2048 + (if odd_exponent {16} else {0})) as usize;
    vali |= ((EXPECTED_BASE[index] - EXPECTED_DEC[index] * (i % 2048)) as u64) << 26;

    f64::from_bits(vali)
}
