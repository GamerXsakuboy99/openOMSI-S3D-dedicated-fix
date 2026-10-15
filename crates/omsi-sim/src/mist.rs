//! [ROLLBACK twilight-1] The morning mist: on many a still morning, above all in autumn,
//! the air near the ground holds a thin mist from before the first light until the sun has
//! warmed it away - the street soft and pale in the low sun, the far end of it lost.
//!
//! Which mornings have one is drawn from the calendar day (the same every time it is
//! asked), about three in five in October and one in three in high summer; how thick it
//! is - a visibility of five kilometres down to two - and how long it lasts (from the
//! first light, thickest at sunrise, gone by the time the sun is twenty degrees up) follow.

use std::f32::consts::PI;

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// 0 .. 1 from the calendar day.
fn roll(seed: u32, k: u32) -> f32 {
    let mut x = seed.wrapping_mul(0x9E37_79B9) ^ k.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^= x >> 16;
    (x & 0xFF_FFFF) as f32 / 16_777_216.0
}

/// The extinction (1/m) the morning's mist adds to the air at this moment: `sun_alt_deg` the
/// sun's height, `morning` whether it is east of the meridian (rising), `day_of_year` 1..366
/// and `day_seed` the calendar day's number. Zero in the afternoon and on a clear morning.
pub fn dawn_mist(sun_alt_deg: f32, morning: bool, day_of_year: f32, day_seed: u32) -> f32 {
    if !morning {
        return 0.0;
    }
    let shape = smoothstep(-9.0, -3.0, sun_alt_deg) * (1.0 - smoothstep(4.0, 22.0, sun_alt_deg));
    if shape <= 0.0 {
        return 0.0;
    }
    // (1 about the end of October, 0 in the high summer: most of it in the cool, damp months)
    let season = 0.5 + 0.5 * ((day_of_year - 295.0) * 2.0 * PI / 365.0).cos();
    let threshold = 0.72 - 0.32 * season;
    let amount = ((roll(day_seed, 77) - threshold) / (1.0 - threshold)).clamp(0.0, 1.0);
    if amount <= 0.0 {
        return 0.0;
    }
    let range = 5000.0 - 3000.0 * amount;
    2.3 / range * shape
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_is_a_morning_thing_and_most_in_autumn() {
        // never in the afternoon, never at noon, never at night
        for seed in 0..50u32 {
            assert_eq!(dawn_mist(5.0, false, 290.0, seed), 0.0);
            assert_eq!(dawn_mist(40.0, true, 290.0, seed), 0.0);
            assert_eq!(dawn_mist(-30.0, true, 290.0, seed), 0.0);
        }
        let days = |doy: f32| (0..2000u32).filter(|&s| dawn_mist(1.0, true, doy, s) > 0.0).count() as f32 / 2000.0;
        let (autumn, summer) = (days(295.0), days(172.0));
        assert!(autumn > 0.5 && autumn < 0.75, "{autumn}");
        assert!(summer > 0.2 && summer < 0.45 && summer < autumn - 0.15, "{summer}");
    }

    #[test]
    fn it_is_thickest_at_sunrise_and_burns_off() {
        // pick a misty day
        let seed = (0..500u32).find(|&s| dawn_mist(1.0, true, 295.0, s) > 2.3 / 4000.0).expect("a misty day");
        let at = |alt: f32| dawn_mist(alt, true, 295.0, seed);
        assert!(at(-10.0) < 1e-9);
        assert!(at(-5.0) > 0.0 && at(-5.0) < at(0.0));
        assert!(at(0.0) >= at(10.0) && at(10.0) > at(18.0));
        assert!(at(25.0) < 1e-9);
        // never thicker than a visibility of two kilometres
        assert!((0..500u32).all(|s| dawn_mist(1.0, true, 295.0, s) <= 2.3 / 2000.0 + 1e-9));
        // smooth in the sun's height: no step between neighbouring tenths of a degree
        let mut prev = at(-9.0);
        let mut a = -9.0f32;
        while a < 22.0 {
            a += 0.1;
            let v = at(a);
            assert!((v - prev).abs() < 4e-5, "{a}: {prev} {v}");
            prev = v;
        }
    }
}
