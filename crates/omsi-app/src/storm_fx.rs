//! [ROLLBACK storm-1] The thunderstorm as the game feeds it to the renderer and the mixer: one
//! called up by hand from the weather window's Sky page (a strength, a duration and a button),
//! which rises, strikes at random while it lasts and dies away (`omsi_sim::storm`). Each strike
//! is a flash now - the sky's bolt and the glow of the clouds, the light on the scene - and a
//! thunder later, after the time the sound takes to come the distance.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use omsi_sim::storm::{self, Rng, Strike};

/// A thunder on its way to the listener.
struct Pending {
    delay: f32,
    distance_km: f32,
    seed: u32,
}

/// A thunder to play now.
pub(crate) struct Thunder {
    pub distance_km: f32,
    pub seed: u32,
    /// 0 .. 1: how loud (the nearer, the louder).
    pub gain: f32,
}

struct Storm {
    /// Seconds since it was called up, and its length (seconds of play).
    age: f32,
    duration: f32,
    power: f32,
    rng: Rng,
    /// Seconds to the next strike.
    next_in: f32,
    running: bool,
    /// The age at which it was ended by hand: its strikes thin out over the next 20 seconds.
    ending: Option<f32>,
    /// What the renderer was last given (the flash and the bolt only ever come down smoothly).
    glow_s: f32,
    bolt_s: f32,
    strikes: Vec<Strike>,
    pending: Vec<Pending>,
}

static STORM: Mutex<Option<Storm>> = Mutex::new(None);
static READY: Mutex<Vec<Thunder>> = Mutex::new(Vec::new());
/// The weather window's two sliders (the bits of an f32: 0.6 and 15.0).
static POWER: AtomicU32 = AtomicU32::new(0x3F19_999A);
static MINUTES: AtomicU32 = AtomicU32::new(0x4170_0000);

pub(crate) fn power() -> f32 {
    f32::from_bits(POWER.load(Ordering::Relaxed))
}

pub(crate) fn set_power(v: f32) {
    POWER.store(v.clamp(0.05, 1.0).to_bits(), Ordering::Relaxed);
}

pub(crate) fn minutes() -> f32 {
    f32::from_bits(MINUTES.load(Ordering::Relaxed))
}

pub(crate) fn set_minutes(v: f32) {
    MINUTES.store(v.clamp(1.0, 240.0).to_bits(), Ordering::Relaxed);
}

/// Whether a storm is on now (its strikes still playing out after it has ended do not count).
pub(crate) fn running() -> bool {
    STORM.lock().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(|s| s.running)
}

/// The weather window's button: start a storm with the sliders' strength and duration, or end
/// the one that is on (its strikes thin out and stop over the next 20 seconds).
pub(crate) fn toggle() {
    let mut g = STORM.lock().unwrap_or_else(|e| e.into_inner());
    if g.as_ref().is_some_and(|s| s.running) {
        if let Some(s) = g.as_mut() {
            s.ending.get_or_insert(s.age);
        }
        return;
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0x1234_5678, |d| d.as_nanos() as u64);
    let mut rng = Rng::new(nanos);
    // (the first strike comes within a few seconds of the storm's beginning; the thunders
    // of the last storm's strikes, still on their way, are kept)
    let next_in = rng.range(3.0, 7.0);
    let pending = g.take().map(|s| s.pending).unwrap_or_default();
    *g = Some(Storm { age: 0.0, duration: minutes() * 60.0, power: power(), rng, next_in, running: true, ending: None, glow_s: 0.0, bolt_s: 0.0, strikes: Vec::new(), pending });
}

/// What the renderer shows this frame.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Frame {
    /// The flash's light on the scene and in the clouds, 0 .. 1.3.
    pub flash: f32,
    /// Where it comes from (unit vector, x east, y north, z up).
    pub dir: [f32; 3],
    /// The strongest strike's place in the sky (unit vector: the middle of its bolt, which a
    /// strike in the cloud has too), how visible its bolt is (0 = none), its shape and its
    /// distance (km).
    pub bolt: f32,
    pub bolt_dir: [f32; 3],
    pub bolt_seed: u32,
    pub dist_km: f32,
}

impl Default for Frame {
    fn default() -> Frame {
        // (no light; a direction that is a direction all the same, for the shaders' sake)
        Frame { flash: 0.0, dir: [0.0, 0.0, 1.0], bolt: 0.0, bolt_dir: [0.0, 0.95, 0.31], bolt_seed: 0, dist_km: 3.0 }
    }
}

/// One frame of the storm: strikes made and aged, thunders counted down. `dt` is the seconds
/// of play (0 when the game is paused).
pub(crate) fn frame(dt: f32) -> Frame {
    let mut g = STORM.lock().unwrap_or_else(|e| e.into_inner());
    let Some(s) = g.as_mut() else {
        return Frame::default();
    };
    if s.running {
        s.age += dt;
        let mut env = storm::envelope(s.age, s.duration);
        if let Some(t0) = s.ending {
            env *= (1.0 - (s.age - t0) / 20.0).clamp(0.0, 1.0);
        }
        if s.age >= s.duration || s.ending.is_some_and(|t0| s.age - t0 >= 20.0) {
            s.running = false;
        } else if env > 0.12 {
            s.next_in -= dt;
            if s.next_in <= 0.0 {
                let strike = Strike::new(&mut s.rng, s.power * env);
                s.pending.push(Pending { delay: strike.thunder_delay(), distance_km: strike.distance_km, seed: strike.seed ^ 0x55 });
                s.strikes.push(strike);
                s.next_in = storm::next_interval(&mut s.rng, s.power * env);
            }
        }
    }
    for k in &mut s.strikes {
        k.age += dt;
    }
    s.strikes.retain(|k| !k.over());
    // the thunders that have come: handed to the mixer (a few at most are kept waiting)
    let mut ready = READY.lock().unwrap_or_else(|e| e.into_inner());
    for p in &mut s.pending {
        p.delay -= dt;
        if p.delay <= 0.0 {
            ready.push(Thunder {
                distance_km: p.distance_km,
                seed: p.seed,
                gain: (1.15 / (1.0 + 0.2 * p.distance_km)).clamp(0.2, 1.0),
            });
        }
    }
    s.pending.retain(|p| p.delay > 0.0);
    if ready.len() > 8 {
        let extra = ready.len() - 8;
        ready.drain(..extra);
    }
    drop(ready);
    let mut out = Frame::default();
    let mut best = 0.0f32;
    for k in &s.strikes {
        let (glow, bolt) = k.flash();
        out.flash += glow;
        // (the strongest flash of the moment is the one whose place, bolt and light are shown)
        if glow > best {
            best = glow;
            out.dir = k.light_direction();
            out.bolt = bolt;
            out.bolt_dir = k.direction();
            out.bolt_seed = k.seed;
            out.dist_km = k.distance_km;
        }
    }
    // (overlapping strikes: a soft ceiling at 1.3, not a cut)
    if out.flash > 1.0 {
        out.flash = 1.0 + 0.3 * (1.0 - (-(out.flash - 1.0) / 0.3).exp());
    }
    // the flash and the bolt come on at once but only go down smoothly, over at least a tenth
    // of a second (a frame or two at the 15-30 frames a second a big city runs at: the
    // strike's own decay lasts as few)
    if dt > 0.0 {
        s.glow_s = out.flash.max(s.glow_s * (-dt / 0.1).exp());
        s.bolt_s = out.bolt.max(s.bolt_s * (-dt / 0.05).exp());
    } else {
        s.glow_s = s.glow_s.max(out.flash);
        s.bolt_s = s.bolt_s.max(out.bolt);
    }
    if s.strikes.is_empty() {
        s.glow_s = 0.0;
        s.bolt_s = 0.0;
    }
    out.flash = s.glow_s;
    out.bolt = s.bolt_s;
    if !s.running && s.strikes.is_empty() && s.pending.is_empty() {
        *g = None;
    }
    out
}

/// The thunders that have reached the listener since the last call.
pub(crate) fn take_thunder() -> Vec<Thunder> {
    std::mem::take(&mut *READY.lock().unwrap_or_else(|e| e.into_inner()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (one test: the storm is one static, shared by every test of the process)
    #[test]
    fn a_storm_flashes_then_thunders_and_ends() {
        assert!(!running());
        assert_eq!(frame(0.1).flash, 0.0);
        set_power(1.0);
        set_minutes(2.0);
        toggle();
        assert!(running());
        let (mut flashed, mut bolted, mut thunders) = (0, 0, 0);
        let mut first_flash = None;
        let mut first_thunder = None;
        let mut t = 0.0f32;
        // two minutes of a fierce storm at 20 frames a second, and the strikes' end
        while t < 160.0 {
            let f = frame(0.05);
            t += 0.05;
            if f.flash > 0.2 {
                flashed += 1;
                first_flash.get_or_insert(t);
                let n = f.dir[0] * f.dir[0] + f.dir[1] * f.dir[1] + f.dir[2] * f.dir[2];
                assert!((n - 1.0).abs() < 1e-3, "{n}");
            }
            if f.bolt > 0.2 {
                bolted += 1;
                assert!(f.bolt_seed < 1024 && f.dist_km >= 0.7);
            }
            for th in take_thunder() {
                thunders += 1;
                first_thunder.get_or_insert(t);
                assert!((0.2..=1.0).contains(&th.gain));
            }
        }
        assert!(flashed > 40 && bolted > 10 && thunders > 5, "{flashed} {bolted} {thunders}");
        // the thunder comes after the flash (a strike at 0.7 km is 2 s from its sound)
        assert!(first_thunder.unwrap() > first_flash.unwrap() + 1.5);
        // the storm is over, nothing is left of it
        assert!(!running());
        assert_eq!(frame(0.05).flash, 0.0);
        // ending it by hand: it dies away within 20 s
        toggle();
        assert!(running());
        for _ in 0..200 {
            frame(0.05);
        }
        toggle();
        let mut t = 0.0;
        while t < 40.0 {
            frame(0.05);
            t += 0.05;
        }
        assert!(!running());
    }
}
