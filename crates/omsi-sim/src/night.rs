//! The night's arithmetic: where the stars stand over a place at a time, how much light the
//! moon gives a night, and how bright a night is against the stock one. Pure functions (no
//! game types), so that they can be tested on their own; the renderer's star shader
//! (`omsi-render/src/stars.wgsl`) does the same sum on the GPU, and the tests here pin the
//! formulas it is written from.
//!
//! Directions are "local" (x east, y north, z up) or "equatorial" (x towards the vernal
//! equinox, z towards the celestial north pole). With the local sidereal time `lst` (the
//! right ascension that stands on the meridian) and the latitude `lat`, an object of right
//! ascension `ra` and declination `dec` has the hour angle `h = lst - ra` and stands at
//!
//! ```text
//! east  = -cos(dec) sin(h)
//! north =  sin(dec) cos(lat) - cos(dec) cos(h) sin(lat)
//! up    =  sin(dec) sin(lat) + cos(dec) cos(h) cos(lat)
//! ```

use std::f64::consts::PI;

/// Greenwich mean sidereal time in degrees, `d` days after J2000 (2000-01-01 12:00 UTC).
pub fn gmst_deg(d: f64) -> f64 {
    (280.460_618_37 + 360.985_647_366_29 * d).rem_euclid(360.0)
}

/// The local sidereal time (radians) at east longitude `longitude_deg`, `d` days after J2000.
pub fn local_sidereal_rad(d: f64, longitude_deg: f64) -> f64 {
    (gmst_deg(d) + longitude_deg).rem_euclid(360.0).to_radians()
}

/// The local direction of a star of right ascension `ra` and declination `dec` (radians).
pub fn equatorial_to_local(ra: f64, dec: f64, lst: f64, lat: f64) -> [f64; 3] {
    let h = lst - ra;
    [-dec.cos() * h.sin(), dec.sin() * lat.cos() - dec.cos() * h.cos() * lat.sin(), dec.sin() * lat.sin() + dec.cos() * h.cos() * lat.cos()]
}

/// The equatorial unit vector of a local direction (the inverse of `equatorial_to_local`;
/// this is the sum the star shader does for every sky pixel).
pub fn local_to_equatorial(d: [f64; 3], lst: f64, lat: f64) -> [f64; 3] {
    let (sl, cl) = (lat.sin(), lat.cos());
    let (st, ct) = (lst.sin(), lst.cos());
    // cos(dec) cos(h) and cos(dec) sin(h)
    let a = -d[1] * sl + d[2] * cl;
    let b = -d[0];
    [ct * a + st * b, st * a - ct * b, d[1] * cl + d[2] * sl]
}

/// The unit vector of right ascension `ra_h` (hours) and declination `dec_deg`.
pub fn equatorial_vector(ra_h: f64, dec_deg: f64) -> [f64; 3] {
    let (ra, dec) = (ra_h * PI / 12.0, dec_deg.to_radians());
    [dec.cos() * ra.cos(), dec.cos() * ra.sin(), dec.sin()]
}

fn smooth(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How far into the night the sky is by the sun's altitude: 0 by day and in the twilight's
/// first half, 1 from 12 degrees under the horizon on.
pub fn night_amount(sun_alt_deg: f64) -> f32 {
    smooth(-2.0, -12.0, sun_alt_deg) as f32
}

/// How strongly the moon lights the night (0..1): its lit fraction (light falls off faster
/// than the fraction does: a half moon gives about a third of the full moon's) and its
/// height (nothing under the horizon, full strength from 15 degrees).
pub fn moonlight(moon_alt_deg: f64, illum: f64) -> f32 {
    (illum.clamp(0.0, 1.0).powf(1.8) * smooth(-3.0, 15.0, moon_alt_deg)) as f32
}

/// The stock night's ambient against what the moon makes of it: a full moon high up more
/// than doubles the light of a night, a night without a moon (new, or set) is a quarter
/// darker than the stock. `strength` (the setting; 1 = as designed, 0 = the stock night).
pub fn night_light_gain(sun_alt_deg: f64, moon_alt_deg: f64, illum: f64, strength: f32) -> f32 {
    let n = night_amount(sun_alt_deg);
    let m = moonlight(moon_alt_deg, illum);
    let gain = 1.0 + n * (1.35 * m - 0.25 * (1.0 - m));
    1.0 + (gain - 1.0) * strength.max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deg(x: f64) -> f64 {
        x.to_radians()
    }

    fn alt(v: [f64; 3]) -> f64 {
        v[2].asin().to_degrees()
    }

    #[test]
    fn the_sidereal_time_starts_where_the_almanac_has_it() {
        // J2000.0: GMST 280.46 degrees (18h 41m)
        assert!((gmst_deg(0.0) - 280.46).abs() < 0.01);
        // a sidereal day is 3m56s shorter than a solar one: after a day the time is 0.9856 on
        assert!((gmst_deg(1.0) - gmst_deg(0.0) - 0.985_647).abs() < 1e-3);
        // east longitudes add
        assert!((local_sidereal_rad(0.0, 15.0).to_degrees() - 295.46).abs() < 0.01);
        assert!((local_sidereal_rad(0.0, 90.0).to_degrees() - 10.46).abs() < 0.01, "wraps at 360");
    }

    #[test]
    fn a_star_on_the_meridian_stands_south_at_its_culmination_height() {
        let lat = deg(52.5);
        // Sirius: RA 6h45.1, Dec -16.72; on the meridian (lst = ra) its height is 90 - lat + dec
        let (ra, dec) = (6.752 * PI / 12.0, deg(-16.72));
        let v = equatorial_to_local(ra, dec, ra, lat);
        assert!((alt(v) - (90.0 - 52.5 - 16.72)).abs() < 1e-6, "{}", alt(v));
        assert!(v[1] < 0.0 && v[0].abs() < 1e-12, "south: {v:?}");
        // six hours before it culminates it is rising in the east (lst = ra - 6 h)
        let v = equatorial_to_local(ra, dec, ra - PI / 2.0, lat);
        assert!(v[0] > 0.0, "east: {v:?}");
        // six hours after, setting in the west
        let v = equatorial_to_local(ra, dec, ra + PI / 2.0, lat);
        assert!(v[0] < 0.0, "west: {v:?}");
    }

    #[test]
    fn the_pole_star_stands_a_latitude_high_whatever_the_hour() {
        let lat = deg(52.5);
        let p = equatorial_vector(2.53, 89.26);
        let (ra, dec) = (2.53 * PI / 12.0, deg(89.26));
        for k in 0..24 {
            let lst = k as f64 * PI / 12.0;
            let v = equatorial_to_local(ra, dec, lst, lat);
            // within the 0.74 degrees it is off the pole
            assert!((alt(v) - 52.5).abs() < 0.75, "lst {k} h: {}", alt(v));
            assert!(v[1] > 0.0, "north");
        }
        assert!((p[2] - deg(89.26).sin()).abs() < 1e-12);
    }

    #[test]
    fn the_shaders_inverse_gives_the_star_back() {
        for (lst, lat) in [(0.3, 0.9), (4.0, -0.6), (2.2, 0.0), (5.9, 1.4)] {
            for (ra_h, dec) in [(6.75, -16.7), (14.26, 19.2), (18.6, 38.8), (0.0, 89.0), (23.9, -70.0)] {
                let (ra, d) = (ra_h * PI / 12.0, deg(dec));
                let local = equatorial_to_local(ra, d, lst, lat);
                let back = local_to_equatorial(local, lst, lat);
                let want = equatorial_vector(ra_h, dec);
                for k in 0..3 {
                    assert!((back[k] - want[k]).abs() < 1e-9, "{k}: {back:?} {want:?}");
                }
            }
        }
    }

    #[test]
    fn moonlight_follows_the_phase_and_the_height() {
        assert_eq!(moonlight(40.0, 0.0), 0.0);
        assert!((moonlight(40.0, 1.0) - 1.0).abs() < 1e-6);
        // a half moon gives about a third of the full moon's light
        let half = moonlight(40.0, 0.5);
        assert!((0.25..0.35).contains(&half), "{half}");
        // under the horizon nothing, low down a part
        assert_eq!(moonlight(-10.0, 1.0), 0.0);
        let low = moonlight(5.0, 1.0);
        assert!(low > 0.2 && low < 0.8, "{low}");
        assert!(moonlight(60.0, 1.0) >= moonlight(20.0, 1.0));
    }

    #[test]
    fn the_night_is_brighter_with_the_moon_and_darker_without() {
        // day: the stock picture whatever the moon does
        assert_eq!(night_light_gain(30.0, 40.0, 1.0, 1.0), 1.0);
        assert_eq!(night_light_gain(30.0, -40.0, 0.0, 1.0), 1.0);
        // deep night: full moon high up more than doubles it, a moonless night is a quarter darker
        let full = night_light_gain(-30.0, 50.0, 1.0, 1.0);
        let none = night_light_gain(-30.0, -20.0, 0.0, 1.0);
        let new_moon_up = night_light_gain(-30.0, 50.0, 0.0, 1.0);
        assert!((full - 2.35).abs() < 0.01, "{full}");
        assert!((none - 0.75).abs() < 0.01, "{none}");
        assert!((new_moon_up - 0.75).abs() < 0.01, "{new_moon_up}");
        let half = night_light_gain(-30.0, 50.0, 0.5, 1.0);
        assert!(none < half && half < full, "{none} {half} {full}");
        // the twilight eases in
        let dusk = night_light_gain(-6.0, 50.0, 1.0, 1.0);
        assert!(dusk > 1.0 && dusk < full, "{dusk}");
        // the setting: 0 is the stock night, 2 the doubled effect
        assert_eq!(night_light_gain(-30.0, 50.0, 1.0, 0.0), 1.0);
        assert!((night_light_gain(-30.0, 50.0, 1.0, 2.0) - 3.7).abs() < 0.02);
    }

    #[test]
    fn the_night_comes_on_with_the_sun_going_down() {
        assert_eq!(night_amount(10.0), 0.0);
        assert_eq!(night_amount(-2.0), 0.0);
        assert_eq!(night_amount(-12.0), 1.0);
        assert_eq!(night_amount(-40.0), 1.0);
        let mid = night_amount(-7.0);
        assert!((mid - 0.5).abs() < 1e-6, "{mid}");
    }
}
