//! [ROLLBACK storm-1] The thunderstorm: when and where lightning strikes, and what the sky and the
//! scene make of each strike over time.
//!
//! A storm is a stream of strikes. Each one is drawn at random (its direction round the
//! observer, its distance, whether it reaches the ground or stays in the cloud, the shape
//! of its bolt) and then plays itself out: two to four return strokes within a quarter of a
//! second, each a flash that dies away in a few hundredths of a second, and a glow in the
//! cloud that stays a little longer. The thunder comes after the flash by the time sound takes
//! to travel the distance (343 m/s). The sky shader (`sky_enhanced.wgsl` `lightning_layer`)
//! draws the bolt and lights the clouds from the numbers made here, the scene is lit by the
//! same flash, and `omsi_audio::thunder` makes the sound.

/// A small random-number generator (xorshift): a storm's strikes do not repeat from one
/// storm to the next, the tests need them repeatable.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    /// 0 .. 1.
    pub fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.next()
    }
}

/// How high the cloud base is over the ground (m): the top of the bolt that is seen.
pub const CLOUD_BASE_M: f32 = 1500.0;
/// The speed of sound (m/s).
pub const SOUND_SPEED: f32 = 343.0;

/// One strike of lightning.
#[derive(Clone, Debug)]
pub struct Strike {
    /// Direction from the observer, radians clockwise from north.
    pub azimuth: f32,
    /// Ground distance (km).
    pub distance_km: f32,
    /// A bolt to the ground (seen as a line) or a flash within the cloud (only the glow).
    pub ground: bool,
    /// The shape of the bolt, 0 .. 1023.
    pub seed: u32,
    /// When each return stroke begins (seconds after the strike), `n` of them.
    pub strokes: [f32; 4],
    pub n: usize,
    /// How bright the flash is for the observer, 0 .. 1.3 (distance and kind).
    pub peak: f32,
    /// Seconds since it struck.
    pub age: f32,
}

impl Strike {
    /// A new strike of a storm of strength `intensity` (0 .. 1): the stronger, the nearer.
    pub fn new(rng: &mut Rng, intensity: f32) -> Strike {
        let k = intensity.clamp(0.0, 1.0);
        let azimuth = rng.range(0.0, std::f32::consts::TAU);
        // [ROLLBACK storm-3] mostly far ones (half beyond 12 km: a rumble on the horizon),
        // the close ones rare (about one in eleven within 4 km): 0.8 km at the nearest, 20.8 km
        // at the farthest of a mild storm, 15 of a fierce one
        let u = rng.next();
        let distance_km = 0.8 + (20.0 - 5.0 * k) * u.powf(0.7);
        // about one in three stays in the cloud (more of them far away)
        let ground = rng.next() > 0.28 + 0.12 * (distance_km / 20.0);
        let seed = ((rng.next() * 1024.0) as u32).min(1023);
        let n = (if ground { 2 + (rng.next() * 3.0) as usize } else { 3 + (rng.next() * 2.0) as usize }).min(4);
        let mut strokes = [0.0f32; 4];
        for i in 1..n {
            strokes[i] = strokes[i - 1] + rng.range(0.035, 0.15);
        }
        // (a bolt at 1 km lights the street, one at 9 km a far horizon; the cloud's own flash
        // is wider and fainter)
        let mut peak = 1.35 / (1.0 + distance_km / 2.2);
        if !ground {
            peak *= 0.7;
        }
        Strike { azimuth, distance_km, ground, seed, strokes, n, peak: peak.clamp(0.0, 1.3), age: 0.0 }
    }

    /// The strike's light now: (the glow of the cloud and the scene, 0 .. 1.3; how visible the
    /// bolt itself is, 0 .. 1). Smooth in time: the strokes' glows add (a bright core that dies
    /// in a few hundredths of a second and a cloud's afterglow that lasts a good half second)
    /// and the sum is bent softly towards its ceiling, so that nothing in it is a step or a
    /// kink, whatever the frame rate.
    pub fn flash(&self) -> (f32, f32) {
        let mut g = 0.0f32;
        let mut bolt = 0.0f32;
        for i in 0..self.n {
            let x = self.age - self.strokes[i];
            if x < 0.0 {
                continue;
            }
            // each later stroke is a little weaker; the first is the brightest
            let w = 1.0 - 0.18 * i as f32;
            let rise = smoothstep(0.0, 0.006, x);
            g += w * rise * (0.62 * (-x / 0.07).exp() + 0.38 * (-x / 0.4).exp());
            if self.ground {
                bolt = bolt.max(w * rise * (-x / 0.04).exp());
            }
        }
        (g / (1.0 + 0.25 * g) * self.peak, bolt.clamp(0.0, 1.0))
    }

    /// Seconds from the flash to the thunder reaching the observer.
    pub fn thunder_delay(&self) -> f32 {
        self.distance_km * 1000.0 / SOUND_SPEED
    }

    /// Nothing left of it to see.
    pub fn over(&self) -> bool {
        self.age > self.strokes[self.n.saturating_sub(1)] + 1.8
    }

    /// Where the bolt's top stands in the sky: the elevation (radians) of the cloud base over
    /// the strike point.
    pub fn top_elevation(&self) -> f32 {
        (CLOUD_BASE_M / (self.distance_km * 1000.0)).atan()
    }

    /// The unit vector (x east, y north, z up) towards the middle of the bolt.
    pub fn direction(&self) -> [f32; 3] {
        let el = self.top_elevation() * 0.5;
        [el.cos() * self.azimuth.sin(), el.cos() * self.azimuth.cos(), el.sin()]
    }

    /// The unit vector towards the cloud over the strike: where the flash's light comes from.
    pub fn light_direction(&self) -> [f32; 3] {
        let el = (self.top_elevation() * 1.15).min(1.2);
        [el.cos() * self.azimuth.sin(), el.cos() * self.azimuth.cos(), el.sin()]
    }
}

/// The seconds to the next strike of a storm of strength `intensity` (0 .. 1): a few seconds in
/// a fierce one, twenty in a mild one, now and then two in quick succession.
pub fn next_interval(rng: &mut Rng, intensity: f32) -> f32 {
    let k = intensity.clamp(0.0, 1.0);
    if rng.next() < 0.22 {
        return rng.range(0.4, 2.0);
    }
    let mean = 20.0 - 15.5 * k;
    let u = rng.next().min(0.999);
    (-(1.0 - u).ln() * mean).clamp(1.5, 60.0)
}

/// The rise and fall of a storm called up by hand: `age` seconds after its start, `duration`
/// seconds in all - up in 20 s, steady, down over the last fifth (20 s at the least).
pub fn envelope(age: f32, duration: f32) -> f32 {
    if age < 0.0 || age >= duration {
        return 0.0;
    }
    let up = smoothstep(0.0, 20.0, age);
    let fall = (duration * 0.2).max(20.0).min(duration);
    let down = smoothstep(0.0, fall, duration - age);
    up.min(down)
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_strike_flashes_dies_away_and_ends() {
        let mut rng = Rng::new(7);
        let mut s = Strike::new(&mut rng, 0.8);
        s.ground = true;
        s.peak = 1.0;
        s.age = -0.01;
        assert_eq!(s.flash(), (0.0, 0.0));
        s.age = 0.01;
        let (g, b) = s.flash();
        assert!(g > 0.5 && b > 0.5, "{g} {b}");
        // the bolt is gone long before the glow
        s.age = s.strokes[s.n - 1] + 0.4;
        let (g2, b2) = s.flash();
        assert!(b2 < 0.01 && g2 > 0.0 && g2 < g);
        assert!(!s.over());
        s.age = s.strokes[s.n - 1] + 1.9;
        assert!(s.over());
        assert!(s.flash().0 < 0.01);
    }

    #[test]
    fn the_flash_has_no_steps_whatever_the_frame_rate() {
        let mut rng = Rng::new(21);
        let mut s = Strike::new(&mut rng, 1.0);
        s.ground = true;
        s.peak = 1.0;
        // sampled every millisecond, the glow never changes by more than a small part of its
        // height from one sample to the next (the stroke's own rise is 6 ms), and is never
        // negative; and it falls monotonically after the last stroke
        let mut prev = 0.0f32;
        let mut t = 0.0f32;
        let end = s.strokes[s.n - 1] + 2.0;
        let mut max_step = 0.0f32;
        while t < end {
            s.age = t;
            let g = s.flash().0;
            assert!(g >= 0.0 && g <= 1.3);
            max_step = max_step.max((g - prev).abs());
            prev = g;
            t += 0.001;
        }
        assert!(max_step < 0.25, "{max_step}");
        let mut prev = f32::MAX;
        let mut t = s.strokes[s.n - 1] + 0.02;
        while t < end {
            s.age = t;
            let g = s.flash().0;
            assert!(g <= prev + 1e-6, "{t} {g} {prev}");
            prev = g;
            t += 0.002;
        }
    }

    #[test]
    fn strokes_come_in_order_within_half_a_second_or_so() {
        let mut rng = Rng::new(3);
        for _ in 0..200 {
            let k = rng.next();
            let s = Strike::new(&mut rng, k);
            assert!((2..=4).contains(&s.n));
            for i in 1..s.n {
                assert!(s.strokes[i] > s.strokes[i - 1]);
            }
            assert!(s.strokes[s.n - 1] < 0.65, "{:?}", s.strokes);
            assert!(s.seed < 1024);
            assert!((0.8..20.9).contains(&s.distance_km));
            assert!((0.0..=1.3).contains(&s.peak));
            let d = s.direction();
            assert!((d[0] * d[0] + d[1] * d[1] + d[2] * d[2] - 1.0).abs() < 1e-4);
            assert!(d[2] > 0.0);
        }
    }

    #[test]
    fn near_strikes_are_brighter_and_the_thunder_follows_by_the_distance() {
        let mut rng = Rng::new(1);
        let mut near = Strike::new(&mut rng, 0.5);
        let mut far = near.clone();
        near.distance_km = 1.0;
        far.distance_km = 8.0;
        near.ground = true;
        far.ground = true;
        // (the same recipe as `new`)
        near.peak = 1.35 / (1.0 + 1.0 / 2.2);
        far.peak = 1.35 / (1.0 + 8.0 / 2.2);
        near.age = 0.01;
        far.age = 0.01;
        assert!(near.flash().0 > far.flash().0 * 2.0);
        assert!((near.thunder_delay() - 2.915).abs() < 0.01);
        assert!((far.thunder_delay() - 23.3).abs() < 0.1);
        // the top of the bolt stands higher in the sky the nearer it is
        assert!(near.top_elevation() > far.top_elevation() * 4.0);
    }

    #[test]
    fn a_stronger_storm_strikes_oftener_and_nearer() {
        let mut rng = Rng::new(11);
        let mean = |k: f32, rng: &mut Rng| (0..2000).map(|_| next_interval(rng, k)).sum::<f32>() / 2000.0;
        let mild = mean(0.1, &mut rng);
        let fierce = mean(1.0, &mut rng);
        assert!(mild > fierce * 2.0, "{mild} {fierce}");
        assert!(fierce > 2.0 && mild < 30.0);
        let dist = |k: f32, rng: &mut Rng| (0..2000).map(|_| Strike::new(rng, k).distance_km).sum::<f32>() / 2000.0;
        assert!(dist(1.0, &mut rng) < dist(0.1, &mut rng));
        // [ROLLBACK storm-3] the close ones are rare, the far ones half of all
        let n = 4000;
        let d: Vec<f32> = (0..n).map(|_| Strike::new(&mut rng, 0.5).distance_km).collect();
        let near = d.iter().filter(|&&x| x < 4.0).count() as f32 / n as f32;
        let far = d.iter().filter(|&&x| x > 12.0).count() as f32 / n as f32;
        assert!(near < 0.15 && near > 0.03, "{near}");
        assert!(far > 0.35 && far < 0.6, "{far}");
        assert!(d.iter().all(|&x| (0.8..=20.8).contains(&x)));
        // about a third stay in the cloud
        let cloud = (0..3000).filter(|_| !Strike::new(&mut rng, 0.5).ground).count() as f32 / 3000.0;
        assert!((0.2..0.45).contains(&cloud), "{cloud}");
    }

    #[test]
    fn the_storm_rises_holds_and_falls() {
        let d = 600.0;
        assert_eq!(envelope(-1.0, d), 0.0);
        assert_eq!(envelope(0.0, d), 0.0);
        assert!(envelope(10.0, d) > 0.0 && envelope(10.0, d) < 1.0);
        assert_eq!(envelope(300.0, d), 1.0);
        assert!(envelope(d - 10.0, d) < 1.0);
        assert_eq!(envelope(d, d), 0.0);
    }
}
