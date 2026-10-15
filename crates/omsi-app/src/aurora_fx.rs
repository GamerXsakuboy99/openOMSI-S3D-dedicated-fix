//! [ROLLBACK aurora-1] The aurora of the night sky as the game feeds it to the renderer: the
//! natural one of the map's place and day (`omsi_sim::aurora::natural`, silently, as a
//! surprise on the rare night a geomagnetic storm reaches so far south - or north), and one
//! called up by hand from the weather window (a button, a strength and a duration), which
//! rises, holds and fades on its own (`omsi_sim::aurora::envelope`) wherever the map lies.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use omsi_sim::aurora::{self, Aurora};

/// An event started by hand: when, for how long (seconds of play, wall clock) and how strong.
struct Manual {
    start: Instant,
    duration: f32,
    power: f32,
    /// [ROLLBACK aurora-3] this event's own look (see `Aurora::seed`): drawn anew at each start
    seed: u32,
}

static EVENT: Mutex<Option<Manual>> = Mutex::new(None);
/// The weather window's two sliders (the bits of an f32: 1.0 and 20.0).
static POWER: AtomicU32 = AtomicU32::new(0x3F80_0000);
static MINUTES: AtomicU32 = AtomicU32::new(0x41A0_0000);

pub(crate) fn power() -> f32 {
    f32::from_bits(POWER.load(Ordering::Relaxed))
}

pub(crate) fn set_power(v: f32) {
    POWER.store(v.clamp(0.05, 1.6).to_bits(), Ordering::Relaxed);
}

pub(crate) fn minutes() -> f32 {
    f32::from_bits(MINUTES.load(Ordering::Relaxed))
}

pub(crate) fn set_minutes(v: f32) {
    MINUTES.store(v.clamp(1.0, 480.0).to_bits(), Ordering::Relaxed);
}

/// Whether a manual aurora is on now.
pub(crate) fn running() -> bool {
    let mut e = EVENT.lock().unwrap_or_else(|e| e.into_inner());
    if e.as_ref().is_some_and(|m| m.start.elapsed().as_secs_f32() >= m.duration) {
        *e = None;
    }
    e.is_some()
}

/// The weather window's button: start one with the sliders' strength and duration, or end the one
/// that is on (it fades out over the next 20 seconds or more, as an event ends).
pub(crate) fn toggle() {
    let mut e = EVENT.lock().unwrap_or_else(|e| e.into_inner());
    match e.as_mut() {
        Some(m) if m.start.elapsed().as_secs_f32() < m.duration => {
            // end it now: the fall (the last fifth of an event, 20 s at the least, see
            // `aurora::envelope`) is made to begin at this moment, so that it goes down from
            // where it stands
            let age = m.start.elapsed().as_secs_f32();
            let ending = if age > 80.0 { age / 0.8 } else { age + 20.0 };
            m.duration = m.duration.min(ending);
        }
        _ => {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0x1234_5678, |d| d.as_nanos() as u64);
            *e = Some(Manual { start: Instant::now(), duration: minutes() * 60.0, power: power(), seed: aurora::seed_from(nanos) });
        }
    }
}

/// What the sky shows this frame: the brighter of the natural aurora of the place and hour and
/// the manual one. `strength` is the setting (0 = no natural aurora; a manual one is shown
/// whatever it says).
pub(crate) fn frame(strength: f32, clock: &omsi_sim::SimClock) -> Aurora {
    let here = omsi_sim::daylight::place();
    let hour = ((clock.time / 3600.0).rem_euclid(24.0)) as f32;
    // (the night after midnight belongs to the evening before it: one storm, one night)
    let mut seed = (clock.year.rem_euclid(10_000) as u32) * 400 + clock.day_of_year.max(0) as u32;
    if hour < 12.0 {
        seed = seed.wrapping_sub(1);
    }
    let natural = aurora::natural(here.latitude, here.longitude, seed, hour, strength);
    let manual = {
        let mut e = EVENT.lock().unwrap_or_else(|e| e.into_inner());
        match e.as_ref() {
            Some(m) => {
                let age = m.start.elapsed().as_secs_f32();
                if age >= m.duration {
                    *e = None;
                    Aurora::NONE
                } else {
                    aurora::manual(here.latitude, m.power, aurora::envelope(age, m.duration), m.seed)
                }
            }
            None => Aurora::NONE,
        }
    };
    natural.max(manual)
}
