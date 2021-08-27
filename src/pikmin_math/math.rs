pub fn sqrt(val: f32) -> f32 {
    (val as f64 * fast_inverse_sqrt(val as f64)) as f32
}

/// https://en.wikipedia.org/wiki/Fast_inverse_square_root
/// Approximates the inverse sqrt to within 1/32 of the true value.
/// Used in Pikmin 2 via PowerPC's `frsqrte` instruction.
/// Implemented in Dolphin Emulator here:
/// https://github.com/dolphin-emu/dolphin/commit/cffa848b9960bcf3dd7a5f3dfd8cdbe417b6ec55#diff-903a032099cd9031620bb1c10e0f7409
const EXPECTED_BASE: [u32; 32] = [
    0x7ff800, 0x783800, 0x70ea00, 0x6a0800,
    0x638800, 0x5d6200, 0x579000, 0x520800,
    0x4cc800, 0x47ca00, 0x430800, 0x3e8000,
    0x3a2c00, 0x360800, 0x321400, 0x2e4a00,
    0x2aa800, 0x272c00, 0x23d600, 0x209e00,
    0x1d8800, 0x1a9000, 0x17ae00, 0x14f800,
    0x124400, 0x0fbe00, 0x0d3800, 0x0ade00,
    0x088400, 0x065000, 0x041c00, 0x020c00,
];
const EXPECTED_DEC: [u32; 32] = [
    0x3e1, 0x3a7, 0x371, 0x340,
    0x313, 0x2ea, 0x2c4, 0x2a0,
    0x27f, 0x261, 0x245, 0x22a,
    0x212, 0x1fb, 0x1e5, 0x1d1,
    0x1be, 0x1ac, 0x19b, 0x18b,
    0x17c, 0x16e, 0x15b, 0x15b,
    0x143, 0x143, 0x12d, 0x12d,
    0x11a, 0x11a, 0x108, 0x106,
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
            else {
                return 0.0;
            }
        }
        else {
            return 0.0 + valf;
        }
    }

    // Negative numbers return NaN
    if sign != 0 {
        return f64::NAN;
    }

    if exponent == 0 {
        loop {
            exponent -= 1u64 << 52;
            mantissa <<= 1;
            if (mantissa & (1u64 << 52)) == 0 {
                break;
            }
        }

        mantissa &= (1u64 << 52) - 1;
        exponent += 1u64 << 52;
    }

    let odd_exponent: bool = 0 == (exponent & (1u64 << 52));
    exponent = ((0x3FFu64 << 52) - ((exponent - (0x3FEu64 << 52)) / 2)) & (0x7FFu64 << 52);

    let i: u32 = (mantissa >> 37) as u32;
    vali = sign | exponent;
    let index = (i / 2048 + (if odd_exponent {16} else {0})) as usize;
    vali |= ((EXPECTED_BASE[index].wrapping_sub(EXPECTED_DEC[index] * (i % 2048))) << 26) as u64;
    return f64::from_bits(vali);
}
