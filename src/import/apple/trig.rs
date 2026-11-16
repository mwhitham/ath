//! Sine, cosine, and arcsine as 2.1.0 computed them for route distances.
//!
//! 2.1.0 measured routes in JavaScript. V8 implements `Math.sin`, `Math.cos`,
//! and `Math.asin` with fdlibm. The build that recorded the reference results
//! compiled fdlibm with clang, which fuses a multiply and an add into one
//! rounding wherever a C expression allows it on 64-bit ARM. The platform's
//! own libm differs from that in the last bit for roughly one input in a
//! hundred, and a split rounds its time to a tenth of a second, so the last bit
//! can change a stored value. These functions repeat each V8 step, including
//! each fused step, so every platform gets the same result.
//!
//! The fused steps follow clang's rule for `-ffp-contract=on`: in `a * b + c`
//! or `c + a * b` within one expression, the multiply joins the add; when both
//! sides are products, the left one joins. `f64::mul_add` is one rounding on
//! every platform.
//!
//! Arguments larger than 2^19 * pi/2 need fdlibm's long reduction. No real
//! coordinate reaches that size, so those arguments use the platform's own
//! function.

// The constants are fdlibm's published literals, kept as written so they can
// be checked against the source. Each names one exact double.
#![allow(clippy::excessive_precision)]

fn high_word(x: f64) -> i32 {
    (x.to_bits() >> 32) as i32
}

fn low_word(x: f64) -> u32 {
    x.to_bits() as u32
}

fn with_low_word(x: f64, low: u32) -> f64 {
    f64::from_bits((x.to_bits() & 0xFFFF_FFFF_0000_0000) | u64::from(low))
}

fn from_words(high: u32, low: u32) -> f64 {
    f64::from_bits((u64::from(high) << 32) | u64::from(low))
}

/// High words of n * pi/2 for n = 1 to 32, from fdlibm's `npio2_hw`.
const NPIO2_HW: [i32; 32] = [
    0x3FF921FB, 0x400921FB, 0x4012D97C, 0x401921FB, 0x401F6A7A, 0x4022D97C, 0x4025FDBB, 0x402921FB, 0x402C463A,
    0x402F6A7A, 0x4031475C, 0x4032D97C, 0x40346B9C, 0x4035FDBB, 0x40378FDB, 0x403921FB, 0x403AB41B, 0x403C463A,
    0x403DD85A, 0x403F6A7A, 0x40407E4C, 0x4041475C, 0x4042106C, 0x4042D97C, 0x4043A28C, 0x40446B9C, 0x404534AC,
    0x4045FDBB, 0x4046C6CB, 0x40478FDB, 0x404858EB, 0x404921FB,
];

fn kernel_sin(x: f64, y: f64, iy: bool) -> f64 {
    const HALF: f64 = 5.00000000000000000000e-01;
    const S1: f64 = -1.66666666666666324348e-01;
    const S2: f64 = 8.33333333332248946124e-03;
    const S3: f64 = -1.98412698298579493134e-04;
    const S4: f64 = 2.75573137070700676789e-06;
    const S5: f64 = -2.50507602534068634195e-08;
    const S6: f64 = 1.58969099521155010221e-10;
    let ix = high_word(x) & 0x7fffffff;
    if ix < 0x3e400000 && x as i32 == 0 {
        return x;
    }
    let z = x * x;
    let v = z * x;
    let r = z.mul_add(z.mul_add(z.mul_add(z.mul_add(S6, S5), S4), S3), S2);
    if !iy {
        v.mul_add(z.mul_add(r, S1), x)
    } else {
        // x - ((z*(half*y - v*r) - y) - v*S1)
        x - (-v).mul_add(S1, z.mul_add(HALF.mul_add(y, -(v * r)), -y))
    }
}

fn kernel_cos(x: f64, y: f64) -> f64 {
    const C1: f64 = 4.16666666666666019037e-02;
    const C2: f64 = -1.38888888888741095749e-03;
    const C3: f64 = 2.48015872894767294178e-05;
    const C4: f64 = -2.75573143513906633035e-07;
    const C5: f64 = 2.08757232129817482790e-09;
    const C6: f64 = -1.13596475577881948265e-11;
    let ix = high_word(x) & 0x7fffffff;
    if ix < 0x3e400000 && x as i32 == 0 {
        return 1.0;
    }
    let z = x * x;
    let r = z * z.mul_add(z.mul_add(z.mul_add(z.mul_add(z.mul_add(C6, C5), C4), C3), C2), C1);
    if ix < 0x3FD33333 {
        // one - (0.5*z - (z*r - x*y))
        1.0 - 0.5f64.mul_add(z, -z.mul_add(r, -(x * y)))
    } else {
        let qx = if ix > 0x3fe90000 { 0.28125 } else { from_words((ix - 0x00200000) as u32, 0) };
        let hz = 0.5f64.mul_add(z, -qx);
        let a = 1.0 - qx;
        a - (hz - z.mul_add(r, -(x * y)))
    }
}

/// fdlibm's `__ieee754_rem_pio2` for |x| up to 2^19 * pi/2: the quadrant and
/// the remainder as a head and tail.
fn rem_pio2(x: f64) -> Option<(i32, f64, f64)> {
    // 2/pi, 0x3FE45F30_6DC9C883.
    const INVPIO2: f64 = std::f64::consts::FRAC_2_PI;
    const PIO2_1: f64 = 1.57079632673412561417e+00;
    const PIO2_1T: f64 = 6.07710050650619224932e-11;
    const PIO2_2: f64 = 6.07710050630396597660e-11;
    const PIO2_2T: f64 = 2.02226624879595063154e-21;
    const PIO2_3: f64 = 2.02226624871116645580e-21;
    const PIO2_3T: f64 = 8.47842766036889956997e-32;
    let hx = high_word(x);
    let ix = hx & 0x7fffffff;
    if ix <= 0x3fe921fb {
        return Some((0, x, 0.0));
    }
    if ix < 0x4002d97c {
        let (n, head, tail) = if hx > 0 {
            let z = x - PIO2_1;
            if ix != 0x3ff921fb {
                let head = z - PIO2_1T;
                (1, head, (z - head) - PIO2_1T)
            } else {
                let z = z - PIO2_2;
                let head = z - PIO2_2T;
                (1, head, (z - head) - PIO2_2T)
            }
        } else {
            let z = x + PIO2_1;
            if ix != 0x3ff921fb {
                let head = z + PIO2_1T;
                (-1, head, (z - head) + PIO2_1T)
            } else {
                let z = z + PIO2_2;
                let head = z + PIO2_2T;
                (-1, head, (z - head) + PIO2_2T)
            }
        };
        return Some((n, head, tail));
    }
    if ix > 0x413921fb {
        return None;
    }
    let t = x.abs();
    let n = t.mul_add(INVPIO2, 0.5) as i32;
    let fn_ = f64::from(n);
    let mut r = (-fn_).mul_add(PIO2_1, t);
    let mut w = fn_ * PIO2_1T;
    let head = if n < 32 && ix != NPIO2_HW[(n - 1) as usize] {
        r - w
    } else {
        let j = ix >> 20;
        let mut head = r - w;
        let i = j - ((high_word(head) >> 20) & 0x7ff);
        if i > 16 {
            let t = r;
            w = fn_ * PIO2_2;
            r = t - w;
            w = fn_.mul_add(PIO2_2T, -((t - r) - w));
            head = r - w;
            let i = j - ((high_word(head) >> 20) & 0x7ff);
            if i > 49 {
                let t = r;
                w = fn_ * PIO2_3;
                r = t - w;
                w = fn_.mul_add(PIO2_3T, -((t - r) - w));
                head = r - w;
            }
        }
        head
    };
    let tail = (r - head) - w;
    if hx < 0 {
        Some((-n, -head, -tail))
    } else {
        Some((n, head, tail))
    }
}

/// `Math.sin`.
pub fn sin(x: f64) -> f64 {
    let ix = high_word(x) & 0x7fffffff;
    if ix <= 0x3fe921fb {
        return kernel_sin(x, 0.0, false);
    }
    if ix >= 0x7ff00000 {
        return f64::NAN;
    }
    let Some((n, head, tail)) = rem_pio2(x) else {
        return x.sin();
    };
    match n & 3 {
        0 => kernel_sin(head, tail, true),
        1 => kernel_cos(head, tail),
        2 => -kernel_sin(head, tail, true),
        _ => -kernel_cos(head, tail),
    }
}

/// `Math.cos`.
pub fn cos(x: f64) -> f64 {
    let ix = high_word(x) & 0x7fffffff;
    if ix <= 0x3fe921fb {
        return kernel_cos(x, 0.0);
    }
    if ix >= 0x7ff00000 {
        return f64::NAN;
    }
    let Some((n, head, tail)) = rem_pio2(x) else {
        return x.cos();
    };
    match n & 3 {
        0 => kernel_cos(head, tail),
        1 => -kernel_sin(head, tail, true),
        2 => -kernel_cos(head, tail),
        _ => kernel_sin(head, tail, true),
    }
}

/// `Math.asin`.
pub fn asin(x: f64) -> f64 {
    const HUGE: f64 = 1.000e+300;
    // pi/2 and pi/4 as doubles: 0x3FF921FB_54442D18 and 0x3FE921FB_54442D18.
    const PIO2_HI: f64 = std::f64::consts::FRAC_PI_2;
    const PIO2_LO: f64 = 6.12323399573676603587e-17;
    const PIO4_HI: f64 = std::f64::consts::FRAC_PI_4;
    const PS0: f64 = 1.66666666666666657415e-01;
    const PS1: f64 = -3.25565818622400915405e-01;
    const PS2: f64 = 2.01212532134862925881e-01;
    const PS3: f64 = -4.00555345006794114027e-02;
    const PS4: f64 = 7.91534994289814532176e-04;
    const PS5: f64 = 3.47933107596021167570e-05;
    const QS1: f64 = -2.40339491173441421878e+00;
    const QS2: f64 = 2.02094576023350569471e+00;
    const QS3: f64 = -6.88283971605453293030e-01;
    const QS4: f64 = 7.70381505559019352791e-02;
    let p_of = |t: f64| t * t.mul_add(t.mul_add(t.mul_add(t.mul_add(t.mul_add(PS5, PS4), PS3), PS2), PS1), PS0);
    let q_of = |t: f64| t.mul_add(t.mul_add(t.mul_add(t.mul_add(QS4, QS3), QS2), QS1), 1.0);
    let hx = high_word(x);
    let ix = hx & 0x7fffffff;
    if ix >= 0x3ff00000 {
        if ((ix - 0x3ff00000) as u32 | low_word(x)) == 0 {
            return x.mul_add(PIO2_HI, x * PIO2_LO);
        }
        return f64::NAN;
    }
    if ix < 0x3fe00000 {
        if ix < 0x3e400000 && HUGE + x > 1.0 {
            return x;
        }
        let t = x * x;
        let w = p_of(t) / q_of(t);
        return x.mul_add(w, x);
    }
    let w = 1.0 - x.abs();
    let t = w * 0.5;
    let p = p_of(t);
    let q = q_of(t);
    let s = t.sqrt();
    let result = if ix >= 0x3FEF3333 {
        let w = p / q;
        PIO2_HI - 2.0f64.mul_add(s.mul_add(w, s), -PIO2_LO)
    } else {
        let w = with_low_word(s, 0);
        let c = (-w).mul_add(w, t) / (s + w);
        let r = p / q;
        let p = (2.0 * s).mul_add(r, -(-2.0f64).mul_add(c, PIO2_LO));
        let q = (-2.0f64).mul_add(w, PIO4_HI);
        PIO4_HI - (p - q)
    };
    if hx > 0 {
        result
    } else {
        -result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_quadrant_table_holds_the_high_words_of_multiples_of_half_pi() {
        for (index, word) in NPIO2_HW.iter().enumerate() {
            let multiple = (index + 1) as f64 * std::f64::consts::FRAC_PI_2;
            assert_eq!(high_word(multiple), *word, "{}", index + 1);
        }
    }

    #[test]
    fn the_standard_constants_are_fdlibms_doubles() {
        assert_eq!(std::f64::consts::FRAC_2_PI.to_bits(), 0x3FE45F30_6DC9C883);
        assert_eq!(std::f64::consts::FRAC_PI_2.to_bits(), 0x3FF921FB_54442D18);
        assert_eq!(std::f64::consts::FRAC_PI_4.to_bits(), 0x3FE921FB_54442D18);
    }

    #[test]
    fn special_values_follow_fdlibm() {
        assert_eq!(sin(0.0), 0.0);
        assert!(sin(-0.0).is_sign_negative());
        assert_eq!(cos(0.0), 1.0);
        assert!(sin(f64::INFINITY).is_nan());
        assert!(asin(1.5).is_nan());
        assert_eq!(asin(1.0), std::f64::consts::FRAC_PI_2);
        assert_eq!(asin(0.0), 0.0);
    }

    #[test]
    fn values_stay_within_one_unit_of_the_true_result() {
        for index in 0..2000 {
            let x = (index as f64 - 1000.0) * 0.0123;
            assert!((sin(x) - x.sin()).abs() <= 2.0 * f64::EPSILON, "{x}");
            assert!((cos(x) - x.cos()).abs() <= 2.0 * f64::EPSILON, "{x}");
            let y = index as f64 / 2000.0;
            assert!((asin(y) - y.asin()).abs() <= 4.0 * f64::EPSILON, "{y}");
        }
    }
}
