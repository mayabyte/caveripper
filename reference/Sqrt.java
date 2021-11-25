public class Sqrt {
    public static void main(String[] args) {
        for (float i = 0.0f; i < 5000; i += 0.3) {
            System.out.println(sqrt(i));
        }
    }

    public static float sqrt(float x) {
        return (float)(x * ApproximateReciprocalSquareRoot((double)x));
    }

    // https://github.com/dolphin-emu/dolphin/commit/cffa848b9960bcf3dd7a5f3dfd8cdbe417b6ec55#diff-903a032099cd9031620bb1c10e0f7409
    // https://en.wikipedia.org/wiki/Fast_inverse_square_root
    static int expected_base[] = {
        0x3ffa000, 0x3c29000, 0x38aa000, 0x3572000,
        0x3279000, 0x2fb7000, 0x2d26000, 0x2ac0000,
        0x2881000, 0x2665000, 0x2468000, 0x2287000,
        0x20c1000, 0x1f12000, 0x1d79000, 0x1bf4000,
        0x1a7e800, 0x17cb800, 0x1552800, 0x130c000,
        0x10f2000, 0x0eff000, 0x0d2e000, 0x0b7c000,
        0x09e5000, 0x0867000, 0x06ff000, 0x05ab800,
        0x046a000, 0x0339800, 0x0218800, 0x0105800,
    };
    static int expected_dec[] = {
        0x7a4, 0x700, 0x670, 0x5f2,
        0x584, 0x524, 0x4cc, 0x47e,
        0x43a, 0x3fa, 0x3c2, 0x38e,
        0x35e, 0x332, 0x30a, 0x2e6,
        0x568, 0x4f3, 0x48d, 0x435,
        0x3e7, 0x3a2, 0x365, 0x32e,
        0x2fc, 0x2d0, 0x2a8, 0x283,
        0x261, 0x243, 0x226, 0x20b,
    };
    static double ApproximateReciprocalSquareRoot(double val)
    {
        double valf = val;
        long vali = Double.doubleToRawLongBits(valf);

        long mantissa = vali & ((1L << 52) - 1);
        long sign = vali & (1L << 63);
        long exponent = vali & (0x7FFL << 52);

        // Special case 0
        if (mantissa == 0 && exponent == 0)
            return 0;
        // Special case NaN-ish numbers
        if (exponent == (0x7FFL << 52))
            {
                if (mantissa == 0)
                    {
                        if (sign != 0)
                            return Double.NaN;
                        return 0.0;
                    }
                return 0.0 + valf;
            }
        // Negative numbers return NaN
        if (sign != 0)
            return Double.NaN;

        if (exponent == 0)
            {
                // "Normalize" denormal values
                do
                    {
                        exponent -= 1L << 52;
                        mantissa <<= 1;
                    } while ((mantissa & (1L << 52)) == 0);
                mantissa &= (1L << 52) - 1;
                exponent += 1L << 52;
            }

        boolean odd_exponent = 0 == (exponent & (1L << 52));
        exponent = ((0x3FFL << 52) - ((exponent - (0x3FEL << 52)) / 2)) & (0x7FFL << 52);

        int i = (int)(mantissa >> 37);
        vali = sign | exponent;
        int index = i / 2048 + (odd_exponent ? 16 : 0);
        vali |= (long)(expected_base[index] - expected_dec[index] * (i % 2048)) << 26;
        return Double.longBitsToDouble(vali);
    }
}
