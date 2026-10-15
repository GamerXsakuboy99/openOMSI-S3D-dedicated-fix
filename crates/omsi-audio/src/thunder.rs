//! [ROLLBACK storm-1] Thunder, made up: there is no thunder in the game's sound files, and a
//! recording would repeat itself every strike. A crack for a near strike (a burst of noise
//! that dies in a few hundredths of a second), and the rumble: low-passed noise, loud at its
//! start and rolling on for seconds in irregular swells (the lightning's long crooked channel
//! reaches the ear from its different parts at different times), duller and longer the
//! farther away it is - the air takes the high notes off the far thunder.
//!
//! Pure: samples out (mono, 16 bit), no engine.

/// A small random-number generator (xorshift).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.next()
    }
}

/// One-pole low-pass filter.
struct Lp {
    a: f32,
    y: f32,
}

impl Lp {
    fn new(fc: f32, rate: f32) -> Lp {
        Lp { a: 1.0 - (-std::f32::consts::TAU * fc / rate).exp(), y: 0.0 }
    }
    fn run(&mut self, x: f32) -> f32 {
        self.y += self.a * (x - self.y);
        self.y
    }
}

/// How long the thunder of a strike `distance_km` away lasts (seconds).
pub fn duration(distance_km: f32) -> f32 {
    (2.8 + 0.5 * distance_km).min(9.5)
}

/// The thunder of a strike `distance_km` away: mono samples at `rate` Hz, peak about 0.9 of
/// full scale (the player sets the loudness with the voice's gain). `seed` makes each one
/// its own.
pub fn synth(distance_km: f32, seed: u32, rate: u32) -> Vec<i16> {
    let sr = rate as f32;
    let dist = distance_km.clamp(0.3, 20.0);
    let dur = duration(dist);
    let n = (dur * sr) as usize;
    let mut rng = Rng(u64::from(seed).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    // the swells: where in time, how long and how loud; most of them early, a few late
    let swells = 6 + (rng.next() * 4.0) as usize;
    let mut parts = Vec::with_capacity(swells + 1);
    parts.push((0.0f32, 0.12 + 0.08 * rng.next(), 0.9 + 0.8 * rng.next(), 1.0f32));
    for _ in 0..swells {
        let at = dur * 0.85 * rng.next().powf(1.6);
        parts.push((at, rng.range(0.08, 0.35), rng.range(0.6, 2.0), rng.range(0.3, 1.0)));
    }
    // how dull: the farther, the lower the rumble's top
    let far = (dist / 12.0).clamp(0.0, 1.0);
    let fc = 950.0 - 780.0 * far;
    let mut lp1 = Lp::new(fc, sr);
    let mut lp2 = Lp::new(fc, sr);
    let mut sub = Lp::new(55.0, sr);
    let mut crack_lp = Lp::new(2500.0, sr);
    let crack_w = (1.0 - dist / 4.5).clamp(0.0, 1.0);
    let mut out = vec![0.0f32; n];
    for (i, o) in out.iter_mut().enumerate() {
        let t = i as f32 / sr;
        let w = rng.next() * 2.0 - 1.0;
        // the rumble: noise through two low-passes, plus the deep part below 55 Hz
        let rumble = lp2.run(lp1.run(w)) * 5.0 + sub.run(w) * 9.0;
        // the rolling envelope: a sum of swells (a quick rise, a long fall)
        let mut env = 0.0f32;
        for &(at, rise, fall, amp) in &parts {
            let x = t - at;
            if x >= 0.0 {
                env += amp * (1.0 - (-x / rise).exp()) * (-x / (fall * (1.0 + 0.6 * far))).exp();
            }
        }
        // (and all of it fading out over the whole length, to nothing at the end)
        let tail = (1.0 - t / dur).clamp(0.0, 1.0);
        let mut s = rumble * env * tail.powf(1.1);
        // the crack of a near strike: bright noise, gone in 40 ms, with a thump after it
        if crack_w > 0.0 && t < 0.25 {
            let hp = w - crack_lp.run(w);
            s += crack_w * (hp * 1.6 * (-t / 0.012).exp() + rumble * 1.2 * (-t / 0.09).exp());
        }
        *o = s;
    }
    // a soft start (no click) and the peak at 0.9
    let fade = ((0.004 * sr) as usize).max(1);
    for i in 0..fade.min(n) {
        out[i] *= i as f32 / fade as f32;
    }
    let peak = out.iter().fold(0.0f32, |m, v| m.max(v.abs())).max(1e-6);
    out.iter().map(|v| (v / peak * 0.9 * 32767.0).clamp(-32767.0, 32767.0) as i16).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn energy(s: &[i16]) -> f64 {
        s.iter().map(|&v| (f64::from(v) / 32768.0).powi(2)).sum::<f64>() / s.len() as f64
    }

    /// How bright the sound is: the energy of its first difference against its own.
    fn brightness(s: &[i16]) -> f64 {
        let d: Vec<i16> = s.windows(2).map(|w| (i32::from(w[1]) - i32::from(w[0])).clamp(-32768, 32767) as i16).collect();
        energy(&d) / energy(s).max(1e-12)
    }

    #[test]
    fn it_has_the_length_and_the_level_asked_for() {
        let s = synth(3.0, 1, 22_050);
        assert!((s.len() as f32 / 22_050.0 - duration(3.0)).abs() < 0.01);
        let peak = s.iter().map(|v| v.unsigned_abs()).max().unwrap();
        assert!((28_000..=29_800).contains(&peak), "{peak}");
        // longer for a far one, never longer than 9.5 s
        assert!(synth(9.0, 1, 22_050).len() > s.len());
        assert!(synth(20.0, 1, 22_050).len() as f32 / 22_050.0 <= 9.51);
    }

    #[test]
    fn far_thunder_is_duller_than_near_thunder() {
        let near = synth(0.8, 5, 22_050);
        let far = synth(10.0, 5, 22_050);
        assert!(brightness(&near) > brightness(&far) * 1.5, "{} {}", brightness(&near), brightness(&far));
    }

    #[test]
    fn it_rolls_on_and_dies_away() {
        let s = synth(5.0, 9, 22_050);
        let third = s.len() / 3;
        let (a, c) = (energy(&s[..third]), energy(&s[2 * third..]));
        assert!(a > c * 2.0, "{a} {c}");
        // the end is quiet and the start has no click
        assert!(s[s.len() - 1].unsigned_abs() < 200);
        assert!(s[0].unsigned_abs() < 100);
        // different seeds are different thunder
        assert_ne!(synth(5.0, 9, 22_050), synth(5.0, 10, 22_050));
        // it is not silence and not a constant
        assert!(energy(&s) > 1e-3);
    }
}
