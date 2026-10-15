//! [ROLLBACK glassfx-1, glassfx-2] Condensation and frost on the player's glass, and the breath of a
//! cold exhaust: what the weather and the cabin's air make of them, and a preview of the
//! effects whatever the weather.
//!
//! The picture itself is drawn in the windows' rain film (the `Rain_Window_*` layer, `ice_layer`
//! in shader.wgsl, fed by `Lighting::glass_fog` / `glass_frost`: under the rain drops, so
//! only on the panes that are windows, and only on a bus that has such a layer); the
//! exhaust's puffs are dressed in `lights::particle_sprites_in` by [`cold_vapour`]. Here is
//! only the physics, in numbers:
//!
//! * **fog** (steam on the inside): the glass is colder than the cabin's air, by how much
//!   depends on how fast the air outside carries the heat off it (a standing bus keeps its
//!   windows warmer than a moving one). Where the air touching the glass holds more water
//!   than it can at the glass's temperature, it condenses - a full saloon on a cold or
//!   rainy day fogs its windows, a heated empty one does not;
//! * **frost**: on the outside when the glass (cooled by the clear sky a little below the
//!   air) is under freezing in humid air, on the inside when the condensation is on a pane
//!   under freezing;
//! * **cold exhaust**: the water vapour of the exhaust condenses in cold air, so the
//!   puffs show white and thick.
//!
//! The levels move slowly (condensation takes a minute or two to settle and clear, frost
//! several minutes). [ROLLBACK glassfx-3] The glass has a temperature of its own that follows
//! the air with a delay (a few minutes), and a bus starts every session as if it had stood
//! out all night: the glass at the outside temperature, so a cold day starts with its frost
//! and the cabin's heat melts it, as it would. A defroster switch of the bus, where the
//! bus has one (see [`is_defrost_var`]), blows the glass warm faster.
//!
//! [ROLLBACK glassfx-4] The fog and the frost are switched OFF for now (the exhaust's vapour stays):
//! `frame` gives no levels for them, so the renderer draws nothing and spends nothing. Set
//! `OMSI_GLASS_ICE=1` to have them back, previews included.
//!
//! Ctrl+Shift+F in the game cycles the preview (see [`PREVIEWS`]); `OMSI_FX_PREVIEW=n`
//! starts the game in one of them.

use std::sync::atomic::{AtomicU32, AtomicU8, Ordering::Relaxed};
use std::sync::Mutex;

/// What the effects are at the moment: 0..1 each.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Levels {
    /// Condensation on the glass.
    pub fog: f32,
    /// Frost on the glass.
    pub frost: f32,
    /// How visible the vapour of a cold exhaust is (the sprites read it from [`cold_vapour`]).
    #[allow(dead_code)]
    pub cold: f32,
}

/// The air in and round the player's vehicle.
#[derive(Clone, Copy, Debug)]
pub struct Air {
    /// Outside temperature (°C) and relative humidity (0..1).
    pub outside: f32,
    pub outside_rh: f32,
    /// The cabin's.
    pub cabin: f32,
    pub cabin_rh: f32,
    /// Speed (km/h).
    pub kmh: f32,
    /// [ROLLBACK glassfx-3] How hard the bus's defroster / demister blows on the glass (0..1; 0 where
    /// the bus has none).
    pub defrost: f32,
    /// Which vehicle this is (`Player::uid`): the glass starts afresh with another.
    pub uid: u64,
}

/// Whether a variable of a bus (lower-case name) looks like its defroster / demister switch:
/// the name says so, and does not say it is a temperature, a time, a lamp or a sound.
pub fn is_defrost_var(name: &str) -> bool {
    const WORDS: [&str; 5] = ["defrost", "demist", "defog", "entfrost", "entnebel"];
    const NOT: [&str; 8] = ["temp", "time", "timer", "snd", "sound", "lamp", "light", "led"];
    WORDS.iter().any(|w| name.contains(w)) && !NOT.iter().any(|w| name.contains(w))
}

/// How long the glass takes to follow the air (s).
const GLASS_TAU: f32 = 300.0;

/// The preview modes of Ctrl+Shift+F: name, fog, frost, cold exhaust. A negative value
/// leaves that effect to the weather.
pub const PREVIEWS: [(&str, f32, f32, f32); 7] = [
    ("weather decides", -1.0, -1.0, -1.0),
    ("fog 100 %", 1.0, 0.0, -1.0),
    ("fog 50 %", 0.5, 0.0, -1.0),
    ("frost 100 %", 0.0, 1.0, -1.0),
    ("frost 50 %", 0.0, 0.5, -1.0),
    ("cold exhaust only", 0.0, 0.0, 1.0),
    ("everything: fog 60 %, frost 60 %, cold exhaust", 0.6, 0.6, 1.0),
];

static PREVIEW: AtomicU8 = AtomicU8::new(u8::MAX);
/// 0 not asked yet, 1 off, 2 on.
static ICE: AtomicU8 = AtomicU8::new(0);

/// Whether the fog and the frost on the glass are on (`OMSI_GLASS_ICE=1`); off by default.
pub fn ice_enabled() -> bool {
    let mut m = ICE.load(Relaxed);
    if m == 0 {
        let on = omsi_cfg::env::var("OMSI_GLASS_ICE").ok().is_some_and(|v| matches!(v.trim(), "1" | "true" | "on" | "yes"));
        m = if on { 2 } else { 1 };
        ICE.store(m, Relaxed);
    }
    m == 2
}
static COLD: AtomicU32 = AtomicU32::new(0);

struct State {
    fog: f32,
    frost: f32,
    /// The glass's temperature (°C).
    glass: f32,
    uid: u64,
    started: bool,
}

static STATE: Mutex<State> = Mutex::new(State { fog: 0.0, frost: 0.0, glass: 0.0, uid: 0, started: false });

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Saturation vapour pressure (hPa) at `t` °C: over water above freezing, over ice below it
/// (Magnus).
fn saturation(t: f32) -> f32 {
    if t >= 0.0 {
        6.112 * (17.62 * t / (243.12 + t)).exp()
    } else {
        6.112 * (22.46 * t / (272.62 + t)).exp()
    }
}

/// The temperature of the glass (°C): between the air outside and the cabin's, nearer the
/// cabin's the less the air outside carries away (`kmh`: a moving bus loses more).
pub fn glass_temperature(outside: f32, cabin: f32, kmh: f32) -> f32 {
    let h_out = 10.0 + 0.8 * kmh.abs().min(120.0);
    let k = 8.0 / (8.0 + h_out);
    outside + (cabin - outside) * k
}

/// The temperature the glass would settle on (°C): [`glass_temperature`], and with the
/// defroster blowing it up towards the warm air it sends (a little above the cabin's).
pub fn glass_equilibrium(a: &Air) -> f32 {
    let base = glass_temperature(a.outside, a.cabin, a.kmh);
    let d = a.defrost.clamp(0.0, 1.0);
    if d <= 0.0 {
        return base;
    }
    let warm = a.cabin.max(a.outside) + 6.0;
    base + d * 0.8 * (warm - base).max(0.0)
}

/// The levels the air would settle on with the glass at `glass` °C: (fog, frost).
pub fn targets_at(a: &Air, glass: f32) -> (f32, f32) {
    // the cabin's air against the pane: more than 100 % there and it condenses
    let rh_in = a.cabin_rh.clamp(0.0, 1.5) * saturation(a.cabin) / saturation(glass).max(0.05);
    let fog = smooth(0.85, 1.25, rh_in);
    // outside, the clear sky cools the pane under the air's temperature
    let outer = glass - 3.0;
    let rh_out = a.outside_rh.clamp(0.0, 1.2) * saturation(a.outside) / saturation(outer).max(0.05);
    let freezing_out = smooth(0.5, -0.5, outer);
    let still = 1.0 - 0.4 * smooth(15.0, 60.0, a.kmh.abs());
    let frost_out = smooth(0.60, 0.95, rh_out) * freezing_out * still;
    // inside, condensation on a pane under freezing turns to ice
    let frost_in = fog * smooth(0.5, -1.0, glass);
    (fog, frost_out.max(frost_in).clamp(0.0, 1.0))
}

/// The levels the air would settle on, the glass having caught up with it: (fog, frost).
pub fn targets(a: &Air) -> (f32, f32) {
    targets_at(a, glass_equilibrium(a))
}

/// How visible a cold exhaust's vapour is at this outside temperature: nothing from 12 °C
/// up, all of it from -2 °C down.
pub fn cold_of(outside: f32) -> f32 {
    smooth(12.0, -2.0, outside)
}

fn approach(cur: f32, target: f32, dt: f32, up_s: f32, down_s: f32) -> f32 {
    let tau = if target > cur { up_s } else { down_s };
    cur + (target - cur) * (1.0 - (-dt.max(0.0) / tau).exp())
}

fn preview_mode() -> usize {
    let mut m = PREVIEW.load(Relaxed);
    if m == u8::MAX {
        // (the first time asked: the start-up choice, if there is one)
        let from_env = omsi_cfg::env::var("OMSI_FX_PREVIEW").ok().and_then(|v| v.trim().parse::<u8>().ok()).unwrap_or(0);
        m = from_env.min(PREVIEWS.len() as u8 - 1);
        PREVIEW.store(m, Relaxed);
    }
    m as usize
}

/// Ctrl+Shift+F: the next preview; the line to tell the player.
pub fn cycle_preview() -> String {
    let next = (preview_mode() + 1) % PREVIEWS.len();
    PREVIEW.store(next as u8, Relaxed);
    let (name, fog, frost, _) = PREVIEWS[next];
    if !ice_enabled() && (fog > 0.0 || frost > 0.0) {
        return format!("Glass effects: {name} (fog and frost are off: start with OMSI_GLASS_ICE=1)");
    }
    format!("Glass effects: {name}")
}

/// One frame: the effects' levels. `air` is None with no vehicle of one's own (the glass
/// keeps what it had; a preview still shows).
pub fn frame(dt: f32, air: Option<Air>) -> Levels {
    let (_, pf, pr, pc) = PREVIEWS[preview_mode()];
    let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(a) = air {
        if !st.started || st.uid != a.uid {
            // (a new session, or another bus: it has stood out all night - the glass at the
            // outside temperature, with its frost, and no condensation yet)
            st.started = true;
            st.uid = a.uid;
            st.glass = a.outside;
            st.fog = 0.0;
            st.frost = targets_at(&a, st.glass).1;
        }
        let tau = GLASS_TAU / (1.0 + 2.0 * a.defrost.clamp(0.0, 1.0));
        st.glass = approach(st.glass, glass_equilibrium(&a), dt, tau, tau);
        let (tf, tr) = targets_at(&a, st.glass);
        st.fog = approach(st.fog, tf, dt, 60.0, 120.0);
        st.frost = approach(st.frost, tr, dt, 420.0, 150.0);
        COLD.store(cold_of(a.outside).to_bits(), Relaxed);
    }
    let cold = if pc >= 0.0 { pc } else { f32::from_bits(COLD.load(Relaxed)) };
    if pc >= 0.0 {
        COLD.store(pc.to_bits(), Relaxed);
    }
    // (off: no levels at all, so that nothing is drawn or copied for them)
    let ice = ice_enabled();
    Levels {
        fog: if !ice { 0.0 } else if pf >= 0.0 { pf } else { st.fog.clamp(0.0, 1.0) },
        frost: if !ice { 0.0 } else if pr >= 0.0 { pr } else { st.frost.clamp(0.0, 1.0) },
        cold,
    }
}

/// How visible the vapour of a cold exhaust is right now (0..1): the last [`frame`]'s.
pub fn cold_vapour() -> f32 {
    f32::from_bits(COLD.load(Relaxed))
}

/// Whether a particle system is an exhaust: driven by the `auspuff_*` variables of the stock
/// vehicles.
pub fn is_exhaust(def: &omsi_model::ParticleSystemDef) -> bool {
    let exhaust = |v: &omsi_model::PsValue| matches!(v, omsi_model::PsValue::Var(n) if n.to_ascii_lowercase().starts_with("auspuff"));
    exhaust(&def.freq.0) || exhaust(&def.alpha_initial.0) || exhaust(&def.life.0)
}

/// An exhaust puff in cold air: whiter, thicker and a little bigger. `fade` is how much of
/// its life is left (1 at birth, 0 at the end). Returns its colour, alpha and size factor.
pub fn dress_puff(color: [f32; 3], alpha: f32, fade: f32, cold: f32) -> ([f32; 3], f32, f32) {
    if cold <= 0.01 {
        return (color, alpha, 1.0);
    }
    let white = [0.9, 0.92, 0.95];
    let k = 0.85 * cold;
    let colour = [color[0] + (white[0] - color[0]) * k, color[1] + (white[1] - color[1]) * k, color[2] + (white[2] - color[2]) * k];
    let thick = (alpha * (1.0 + 1.6 * cold)).max(0.3 * cold * fade.clamp(0.0, 1.0)).min(0.9);
    (colour, thick.max(alpha), 1.0 + 0.35 * cold)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn air(outside: f32, outside_rh: f32, cabin: f32, cabin_rh: f32, kmh: f32) -> Air {
        Air { outside, outside_rh, cabin, cabin_rh, kmh, defrost: 0.0, uid: 1 }
    }

    #[test]
    fn a_warm_dry_day_has_clear_glass() {
        let (fog, frost) = targets(&air(22.0, 0.5, 23.0, 0.5, 30.0));
        assert!(fog < 0.01 && frost < 0.01, "{fog} {frost}");
    }

    #[test]
    fn a_full_saloon_on_a_cold_day_fogs() {
        let (fog, frost) = targets(&air(4.0, 0.8, 20.0, 0.6, 25.0));
        assert!(fog > 0.8, "{fog}");
        assert!(frost < 0.05, "no frost above freezing: {frost}");
        // the same bus with dry cabin air is clear
        let (dry, _) = targets(&air(4.0, 0.8, 20.0, 0.2, 25.0));
        assert!(dry < 0.2, "{dry}");
    }

    #[test]
    fn a_standing_bus_keeps_its_glass_warmer() {
        assert!(glass_temperature(0.0, 20.0, 0.0) > glass_temperature(0.0, 20.0, 50.0) + 4.0);
    }

    #[test]
    fn a_frosty_humid_night_frosts_the_outside() {
        // parked: cabin as cold as the air
        let (fog, frost) = targets(&air(-4.0, 0.9, -4.0, 0.9, 0.0));
        assert!(frost > 0.5, "{frost}");
        assert!(fog < 0.9);
        // a dry frost leaves the glass clear
        let (_, dry) = targets(&air(-4.0, 0.3, -4.0, 0.3, 0.0));
        assert!(dry < 0.05, "{dry}");
    }

    #[test]
    fn condensation_under_freezing_is_ice() {
        let (fog, frost) = targets(&air(-10.0, 0.5, 18.0, 0.5, 40.0));
        assert!(fog > 0.9, "{fog}");
        assert!(frost > 0.7, "{frost}");
    }

    #[test]
    fn a_cold_start_has_its_frost_though_the_cabin_script_starts_warm() {
        // the bus's script says 20 degrees in the cabin, the day is -5 and humid: the glass
        // that has stood out all night is at -5, whatever the cabin says
        let a = air(-5.0, 0.8, 20.0, 0.5, 0.0);
        let (_, parked) = targets_at(&a, a.outside);
        assert!(parked > 0.8, "{parked}");
        // once the glass has caught up with the warm cabin it is gone
        let (_, warm) = targets(&a);
        assert!(warm < 0.05, "{warm}");
    }

    #[test]
    fn a_defroster_switch_is_told_from_its_neighbours() {
        assert!(is_defrost_var("cp_defrost_sw"));
        assert!(is_defrost_var("heizung_entfrosten"));
        assert!(!is_defrost_var("defrost_temp"));
        assert!(!is_defrost_var("defrost_lamp"));
        assert!(!is_defrost_var("wiper_sw"));
    }

    #[test]
    fn the_defroster_warms_the_glass_and_clears_it_sooner() {
        let mut a = air(-5.0, 0.8, 5.0, 0.5, 0.0);
        let plain = glass_equilibrium(&a);
        a.defrost = 1.0;
        let blown = glass_equilibrium(&a);
        assert!(blown > plain + 4.0, "{plain} {blown}");
        // a bus without one is as before
        a.defrost = 0.0;
        assert_eq!(glass_equilibrium(&a), glass_temperature(-5.0, 5.0, 0.0));
    }

    #[test]
    fn a_damp_cold_morning_frosts_without_a_perfect_dew() {
        // 82 % humid air at -2: frost on a standing bus's glass
        let a = air(-2.0, 0.82, -2.0, 0.82, 0.0);
        let (_, f) = targets(&a);
        assert!(f > 0.6, "{f}");
    }

    #[test]
    fn the_levels_move_slowly_and_stay_between_zero_and_one() {
        let one = approach(0.0, 1.0, 1.0, 60.0, 120.0);
        assert!(one > 0.0 && one < 0.05, "{one}");
        let late = approach(0.0, 1.0, 1000.0, 60.0, 120.0);
        assert!(late > 0.999 && late <= 1.0);
        let down = approach(1.0, 0.0, 1.0, 60.0, 120.0);
        assert!(down < 1.0 && down > 0.98);
        assert_eq!(approach(0.3, 0.3, 5.0, 60.0, 120.0), 0.3);
    }

    #[test]
    fn the_cold_exhaust_shows_below_twelve_degrees() {
        assert_eq!(cold_of(20.0), 0.0);
        assert_eq!(cold_of(12.0), 0.0);
        assert!(cold_of(5.0) > 0.2 && cold_of(5.0) < 0.8);
        assert_eq!(cold_of(-5.0), 1.0);
    }

    #[test]
    fn a_cold_puff_is_whiter_and_thicker_and_a_warm_one_is_untouched() {
        let (c, a, s) = dress_puff([0.3, 0.3, 0.3], 0.2, 1.0, 0.0);
        assert_eq!((c, a, s), ([0.3, 0.3, 0.3], 0.2, 1.0));
        let (c, a, s) = dress_puff([0.3, 0.3, 0.3], 0.2, 1.0, 1.0);
        assert!(c[0] > 0.7, "{c:?}");
        assert!(a > 0.4 && a <= 0.9, "{a}");
        assert!(s > 1.3);
        // near the end of its life it has faded: not held up above its own alpha by the floor
        let (_, late, _) = dress_puff([0.3; 3], 0.0, 0.0, 1.0);
        assert_eq!(late, 0.0);
    }
}
