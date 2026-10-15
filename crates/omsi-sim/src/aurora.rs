//! [ROLLBACK aurora-1] The aurora: when the night sky of this place has one, and how far off.
//!
//! The sky shader (`stars.wgsl` `aurora_sky`) draws curtains over the polar horizon from two
//! numbers: an amplitude (negative for the southern sky: the aurora australis) and the
//! distance of the arc's middle from the observer in kilometres. They come from here:
//!
//! * a natural one, from the place's geomagnetic latitude and a planetary Kp index drawn from
//!   the calendar day (most days quiet, now and then a storm that reaches a mid-latitude
//!   sky, always the same on the same day) and the hour (the evening's substorm);
//! * a manual one (the weather window's "Aurora" button), at a given strength, wherever the
//!   map lies.
//!
//! The oval's equatorward edge sits at about 66.5 - 2 Kp degrees of geomagnetic latitude and
//! the arcs a few degrees poleward of it (after the usual rule of thumb; the numbers are good
//! enough for a sky, not for a forecast).

/// What the sky shader needs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aurora {
    /// How bright (0 = none, 1 = a good display, up to 1.6); negative: the southern sky.
    pub amp: f32,
    /// Ground distance of the arc's middle towards the pole (km); negative: past the observer.
    pub dist_km: f32,
    /// [ROLLBACK aurora-3] Which display this is, 0 .. 1023: the shader draws its arcs' direction,
    /// number and thickness, its pace and the rhythm of its break-ups from it, so that no two
    /// events look alike (the same event always does).
    pub seed: u32,
}

impl Aurora {
    pub const NONE: Aurora = Aurora { amp: 0.0, dist_km: 1500.0, seed: 0 };

    /// The brighter of two.
    pub fn max(self, other: Aurora) -> Aurora {
        if other.amp.abs() > self.amp.abs() {
            other
        } else {
            self
        }
    }
}

/// The geomagnetic latitude (degrees) of a place, by the tilted-dipole pole (80.7 N, 72.7 W).
pub fn geomagnetic_latitude(lat_deg: f64, lon_deg: f64) -> f64 {
    let (pla, plo) = (80.7f64.to_radians(), (-72.7f64).to_radians());
    let (la, lo) = (lat_deg.to_radians(), lon_deg.to_radians());
    let s = la.sin() * pla.sin() + la.cos() * pla.cos() * (lo - plo).cos();
    s.clamp(-1.0, 1.0).asin().to_degrees()
}

/// A number 0..1 from an integer (a hash).
fn unit(mut x: u32) -> f32 {
    x = x.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^= x >> 16;
    (x & 0xFF_FFFF) as f32 / 16_777_216.0
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The planetary Kp index at `local_hour` of the day `day_seed` (0 .. 9): the day's peak is
/// drawn with an exponential tail (about one day in four hundred reaches 5, one in ten thousand 7:
/// at the latitude of Szczecin a handful of nights in a year show anything, a really big
/// display comes once in some years), and the activity gathers in a few hours of the evening
/// and night.
pub fn kp_at(day_seed: u32, local_hour: f32) -> f32 {
    let u = unit(day_seed ^ 0xA5A5_0001);
    let peak = (1.2 - 0.55 * (1.0 - u).max(1e-6).ln()).min(9.0);
    // the hour of the substorm: 20 .. 02 local time; it lasts a couple of hours
    let at = 20.0 + 6.0 * unit(day_seed ^ 0x5A5A_0002);
    let mut d = (local_hour - at).abs();
    if d > 12.0 {
        d = 24.0 - d;
    }
    let g = (-(d * d) / (2.0 * 2.2 * 2.2)).exp();
    1.2 + (peak - 1.2) * g
}

/// The natural aurora over a place at an hour of a day. `strength` is the setting (0 = off,
/// 1 = as the sky would have it, 2 = a storm's worth more often).
pub fn natural(lat_deg: f64, lon_deg: f64, day_seed: u32, local_hour: f32, strength: f32) -> Aurora {
    if strength <= 0.0 {
        return Aurora::NONE;
    }
    let gm = geomagnetic_latitude(lat_deg, lon_deg);
    let kp = (kp_at(day_seed, local_hour) + (strength - 1.0) * 1.2).clamp(0.0, 9.0);
    let boundary = 66.5 - 2.0 * f64::from(kp);
    let dist = (111.0 * (boundary + 2.5 - gm.abs())) as f32;
    // the edge of what a horizon shows: a glow low in the north fades out between 950 and 1250 km
    let edge = 1.0 - smoothstep(950.0, 1250.0, dist);
    // (a Kp of 6.5 is a full 100 %, the green; the red and the violet of the stronger displays
    // are above it, see `au_height_colour`)
    // (and under the oval itself it is never dark: a quiet night there still shows a good arc)
    let deep = 0.55 * smoothstep(560.0, 150.0, dist);
    let amp = (((kp - 2.0) * 0.2 + 0.1).max(deep) * edge).clamp(0.0, 1.6);
    // (below this the glow is lost in the haze of the horizon: not worth a night's mention)
    if amp < 0.25 {
        return Aurora::NONE;
    }
    // this storm's own look: the same for the whole night
    let seed = ((unit(day_seed ^ 0x77AA_0003) * 1024.0) as u32).min(1023);
    Aurora { amp: amp * lat_deg.signum() as f32, dist_km: dist.clamp(-300.0, 1500.0), seed }
}

/// An aurora called up by hand: `power` 0..1.6 (1 = a good display, the green; more: the red
/// and the violet join and the arcs come close, over 130 % to nearly overhead) and `envelope`
/// 0..1 (the event's rise and fall), anywhere on the map. `seed` is the event's own (see
/// `Aurora::seed`; the app draws a new one at each start).
pub fn manual(lat_deg: f64, power: f32, envelope: f32, seed: u32) -> Aurora {
    let amp = (power * envelope).clamp(0.0, 1.6);
    if amp < 0.01 {
        return Aurora::NONE;
    }
    let p = power.clamp(0.0, 2.0);
    let dist = (if p <= 1.0 { 900.0 - 420.0 * p } else { 480.0 - 700.0 * (p - 1.0) }).clamp(40.0, 900.0);
    Aurora { amp: amp * if lat_deg < 0.0 { -1.0 } else { 1.0 }, dist_km: dist, seed: seed.min(1023) }
}

/// A fresh event seed from any number (a clock, a counter).
pub fn seed_from(n: u64) -> u32 {
    (unit((n ^ (n >> 32)) as u32 ^ 0x3C6E_F372) * 1024.0) as u32 & 1023
}

/// The rise and fall of a manual event: `age` seconds after its start, `duration` seconds in
/// all - up in 25 s, steady, down over the last 20 % (at least 20 s).
pub fn envelope(age: f32, duration: f32) -> f32 {
    if age < 0.0 || age >= duration {
        return 0.0;
    }
    let up = smoothstep(0.0, 25.0, age);
    let fall = (duration * 0.2).max(20.0).min(duration);
    let down = smoothstep(0.0, fall, duration - age);
    up.min(down)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geomagnetic_latitudes_are_about_right() {
        // Szczecin and Tromsø: a few degrees under their geographic latitudes in Europe
        let szczecin = geomagnetic_latitude(53.4, 14.5);
        assert!((50.0..56.0).contains(&szczecin), "{szczecin}");
        let tromso = geomagnetic_latitude(69.7, 18.9);
        assert!((64.0..70.0).contains(&tromso), "{tromso}");
        // Sydney is in the south
        assert!(geomagnetic_latitude(-33.9, 151.2) < -30.0);
    }

    /// The strongest the sky gets on a night (evening to the small hours), over a place.
    fn night_best(lat: f64, lon: f64, seed: u32, strength: f32) -> Aurora {
        (0..16).map(|h| natural(lat, lon, seed, (18 + h) as f32 % 24.0, strength)).fold(Aurora::NONE, Aurora::max)
    }

    #[test]
    fn a_quiet_night_has_none_at_szczecin_and_one_in_tromso() {
        let nights = 20_000u32;
        let mut seen = 0;
        let mut big = 0;
        for day in 0..nights {
            let a = night_best(53.4, 14.5, 2025 * 400 + day, 1.0);
            if a.amp != 0.0 {
                seen += 1;
                assert!(a.dist_km > -300.0 && a.dist_km <= 1500.0);
                assert!(a.seed < 1024);
            }
            if a.amp > 1.0 {
                big += 1;
            }
        }
        // a handful of nights in a year (365 nights: 2..11), a really big one once in some years
        let share = seen as f32 / nights as f32;
        assert!((0.005..0.03).contains(&share), "share {share}");
        assert!(big as f32 / (nights as f32) < 0.004, "big {big}");
        // under the oval (Tromsø): almost every night
        let tromso_seen = (0..400u32).filter(|d| night_best(69.7, 18.9, 2025 * 400 + d, 1.0).amp > 0.0).count();
        assert!(tromso_seen > 380, "{tromso_seen}");
        // and the setting at 2 brings them much oftener
        let often = (0..2000u32).filter(|d| night_best(53.4, 14.5, 2025 * 400 + d, 2.0).amp > 0.0).count() as f32 / 2000.0;
        assert!(often > share * 3.0 && often < 0.4, "often {often} vs {share}");
    }

    #[test]
    fn the_hour_matters_and_the_strength_scales() {
        // a day with a big storm: the evening shows it, the forenoon does not
        let day = (0..400_000u32).find(|d| kp_at(2025 * 400 + d, 23.0) > 7.5).expect("a big storm day");
        let seed = 2025 * 400 + day;
        let hour_of_peak = (0..48).map(|h| h as f32 * 0.5).max_by(|a, b| kp_at(seed, *a).total_cmp(&kp_at(seed, *b))).unwrap();
        assert!(kp_at(seed, hour_of_peak) > 7.5);
        assert!(kp_at(seed, (hour_of_peak + 12.0) % 24.0) < 3.0);
        let strong = natural(53.4, 14.5, seed, hour_of_peak, 1.0);
        let weak = natural(53.4, 14.5, seed, hour_of_peak, 0.5);
        assert!(strong.amp > weak.amp);
        assert!(strong.dist_km < weak.dist_km);
        assert_eq!(strong.seed, natural(53.4, 14.5, seed, hour_of_peak - 1.0, 1.0).seed);
        assert_eq!(natural(53.4, 14.5, seed, hour_of_peak, 0.0), Aurora::NONE);
        // the southern hemisphere flips the sign
        let south = natural(-53.4, 14.5, seed, hour_of_peak, 1.0);
        assert!(south.amp < 0.0 || south.amp == 0.0);
    }

    #[test]
    fn the_events_differ_in_their_seeds() {
        let mut seen = std::collections::HashSet::new();
        for n in 0..400u64 {
            seen.insert(seed_from(n * 7919 + 13));
        }
        assert!(seen.len() > 200, "{}", seen.len());
        assert!(seen.iter().all(|&s| s < 1024));
    }

    #[test]
    fn a_manual_event_rises_holds_and_falls() {
        let d = 600.0;
        assert_eq!(envelope(-1.0, d), 0.0);
        assert_eq!(envelope(0.0, d), 0.0);
        assert!(envelope(12.0, d) > 0.0 && envelope(12.0, d) < 1.0);
        assert_eq!(envelope(300.0, d), 1.0);
        assert!(envelope(d - 10.0, d) < 1.0);
        assert_eq!(envelope(d + 1.0, d), 0.0);
        // a short event never goes above its own length
        for t in 0..60 {
            let e = envelope(t as f32, 60.0);
            assert!((0.0..=1.0).contains(&e));
        }
        let a = manual(53.4, 1.0, 1.0, 5);
        let b = manual(53.4, 1.5, 1.0, 5);
        assert!(b.amp > a.amp && b.dist_km < a.dist_km);
        assert_eq!(a.amp, 1.0);
        assert!(manual(53.4, 1.6, 1.0, 5).dist_km >= 40.0);
        assert_eq!(a.seed, 5);
        assert_eq!(manual(53.4, 1.0, 0.0, 5), Aurora::NONE);
        assert!(manual(-30.0, 1.0, 1.0, 5).amp < 0.0);
    }
}
