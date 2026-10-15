//! Environment sounds: what is heard *around* the camera rather than out of a vehicle.
//!
//! OMSI keeps them in `Sounds\` next to the per-vehicle configurations: `rain_outside.wav`
//! is the rain as it sounds in the street (the bus plays its own `regen.wav` on the roof,
//! `[viewpoint] 2`, inside the cab), and `Sounds\Passengers\sound.cfg` holds the footstep
//! entry with its volume and its `[3d]` range - one metre, so a step is only heard from a
//! few metres away; which file a step plays is the vehicle's: the `[stepsoundpack]` of the
//! path link the passenger walks on (its `paths.cfg`). The wet-road hiss (`Sounds\WetLane_1.wav`, `WetLane_2.wav`) belongs to
//! the vehicles and comes out of their own sound configurations once `StreetCond` is fed.

use glam::DVec3;
use omsi_audio::{AudioEngine, Clip, VoiceId, VoiceParams};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A footfall to be heard: where it happened and whether it is on a bus floor.
#[derive(Debug, Clone)]
pub struct Footfall {
    pub position: DVec3,
    pub inside: bool,
    /// On the floor of the player's own bus.
    pub own_bus: bool,
    /// The files (in `Sounds\Passengers\`) of the `[stepsoundpack]` of the path link the
    /// step is on, one picked at random as Omsi.exe does (0x6274c9); none, no sound - a
    /// link without a pack, a bus whose paths.cfg has none, the street.
    pub pack: Option<Arc<[String]>>,
}

pub struct Ambience {
    /// `Sounds\rain_outside.wav`, the rain in the open.
    rain: Option<Arc<Clip>>,
    rain_voice: Option<VoiceId>,
    /// `Sounds\Passengers\`, the step samples read from it as the packs name them, and
    /// what the sound configuration there says about a step (volume, `[3d]` range).
    step_dir: PathBuf,
    steps: hashbrown::HashMap<String, Option<Arc<Clip>>>,
    step_volume: f32,
    step_range: f32,
    /// Steps still allowed this second: a crowd getting off would otherwise fire a dozen
    /// voices a frame.
    step_budget: f32,
    rng: u64,
    /// [ROLLBACK storm-1] Thunders being made on a helper thread (a few milliseconds of
    /// samples each): when each is ready it is played with the gain and the muffling it was
    /// asked for.
    thunders: Vec<(std::sync::mpsc::Receiver<Vec<i16>>, f32, bool)>,
    /// [ROLLBACK storm-2] Recorded thunders from `Sounds\Thunder\*.wav` (level-matched at
    /// load) with the distance class their file name says: 0 near, 1 mid, 2 far, 3 any.
    /// None found: the made-up thunder of `omsi_audio::thunder` is used.
    thunder_clips: Vec<(u8, Arc<Clip>)>,
    /// The last recordings played (indices into `thunder_clips`, the latest last), so that the
    /// same one is not heard twice running.
    thunder_recent: Vec<usize>,
    /// What was heard last, for `OMSI_DEBUG_SOUND`.
    pub last: String,
}

impl Ambience {
    /// Read the environment sounds of a content root. Missing files are not an error: a
    /// map pack without `Sounds\` simply has no rain of its own.
    pub fn load(engine: &AudioEngine, root: &Path) -> Ambience {
        let mut a = Ambience {
            rain: None,
            rain_voice: None,
            step_dir: PathBuf::new(),
            steps: hashbrown::HashMap::new(),
            step_volume: 1.0,
            step_range: 1.0,
            step_budget: 0.0,
            rng: 0x5EED_1234_ABCD,
            thunders: Vec::new(),
            thunder_clips: Vec::new(),
            thunder_recent: Vec::new(),
            last: String::new(),
        };
        if !engine.enabled {
            return a;
        }
        a.rain = engine.load_clip(&omsi_cfg::resolve_path(root, "Sounds\\rain_outside.wav"));
        let dir = omsi_cfg::resolve_path(root, "Sounds\\Passengers");
        // (the configuration's own file is only a stand-in: TSoundPack plays the pack's)
        if let Ok(cfg) = omsi_vehicle::SoundCfg::load(&dir.join("sound.cfg")) {
            if let Some(step) = cfg
                .sounds
                .iter()
                .find(|s| s.triggers.iter().any(|t| t.eq_ignore_ascii_case("step")))
            {
                a.step_volume = step.volume.max(0.0);
                a.step_range = if step.range > 0.0 { step.range } else { 1.0 };
            }
        }
        a.load_thunders(engine, root);
        log::info!(
            "environment sounds: rain {}, footsteps from {}",
            if a.rain.is_some() { "yes" } else { "missing" },
            dir.display()
        );
        a.step_dir = dir;
        a
    }

    /// [ROLLBACK storm-2] The recordings a thunder is played from. They are part of the
    /// executable (`assets/sounds/thunder/*.wav`, compiled in by `build.rs`), so that they are
    /// there whatever the player does with the folders; a `Sounds\Thunder\*.wav` in the content
    /// root, if the player has put one there on purpose, is used instead. The name says what
    /// a file is for - `near*`/`close*` (a crack right overhead), `mid*`/`medium*` (a roll a
    /// few kilometres away), `far*`/`distant*` (a rumble on the horizon); a file called anything
    /// else is used for every distance. Read once here (not when the first thunder is due: a
    /// long file would stall a frame), and brought to one level so that the strength of a
    /// thunder is the storm's business, not the file's.
    fn load_thunders(&mut self, engine: &AudioEngine, root: &Path) {
        let dir = omsi_cfg::resolve_path(root, "Sounds\\Thunder");
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| p.extension().map_or(false, |x| x.eq_ignore_ascii_case("wav")))
                    .collect()
            })
            .unwrap_or_default();
        files.sort();
        for path in files.into_iter().take(16) {
            let Some(clip) = engine.load_clip(&path) else {
                continue;
            };
            let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            self.add_thunder(&name, &clip);
        }
        let from_folder = self.thunder_clips.len();
        if from_folder == 0 {
            for (name, bytes) in THUNDER_EMBEDDED.iter().take(32) {
                match omsi_audio::wav::parse_wav(bytes) {
                    Ok(w) => self.add_thunder(name, &Clip { sample_rate: w.sample_rate, channels: w.channels, samples: w.samples }),
                    Err(e) => log::warn!("thunder {name}: {e}"),
                }
            }
        }
        log::info!(
            "thunder: {} recording(s) {}",
            self.thunder_clips.len(),
            if from_folder > 0 { format!("from {}", dir.display()) } else { "built into the executable".into() }
        );
    }

    /// One recording, with the distance class its name gives, brought to one level.
    fn add_thunder(&mut self, name: &str, clip: &Clip) {
        if clip.samples.is_empty() {
            return;
        }
        let peak = clip.samples.iter().map(|&v| (v as i32).abs()).max().unwrap_or(0);
        let k = if peak > 64 { 0.9 * 32767.0 / peak as f32 } else { 1.0 };
        let samples = clip.samples.iter().map(|&v| (v as f32 * k).round().clamp(-32768.0, 32767.0) as i16).collect();
        self.thunder_clips.push((
            thunder_class(name),
            Arc::new(Clip { sample_rate: clip.sample_rate, channels: clip.channels, samples }),
        ));
    }

    /// The recording for a strike `km` away: of the class that fits the distance, else one
    /// for any distance, else the nearest class there is.
    fn pick_thunder(&mut self, km: f32) -> Option<Arc<Clip>> {
        if self.thunder_clips.is_empty() {
            return None;
        }
        let want: i32 = if km < 4.0 {
            0
        } else if km < 12.0 {
            1
        } else {
            2
        };
        let exact: Vec<usize> = (0..self.thunder_clips.len())
            .filter(|&i| self.thunder_clips[i].0 as i32 == want)
            .collect();
        let pool: Vec<usize> = if !exact.is_empty() {
            exact
        } else {
            let any: Vec<usize> = (0..self.thunder_clips.len())
                .filter(|&i| self.thunder_clips[i].0 == 3)
                .collect();
            if !any.is_empty() {
                any
            } else {
                let best = self
                    .thunder_clips
                    .iter()
                    .map(|c| (c.0 as i32 - want).abs())
                    .min()
                    .unwrap_or(0);
                (0..self.thunder_clips.len())
                    .filter(|&i| (self.thunder_clips[i].0 as i32 - want).abs() == best)
                    .collect()
            }
        };
        // [ROLLBACK storm-3] not what was heard last (nor the one before, if there is a choice
        // left): two of the same in a row sound like a loop
        let mut fresh: Vec<usize> = pool.iter().copied().filter(|i| !self.thunder_recent.contains(i)).collect();
        if fresh.is_empty() {
            let last = self.thunder_recent.last().copied();
            fresh = pool.iter().copied().filter(|i| Some(*i) != last).collect();
        }
        if fresh.is_empty() {
            fresh = pool;
        }
        let k = ((self.rand() * fresh.len() as f32) as usize).min(fresh.len() - 1);
        let i = fresh[k];
        self.thunder_recent.push(i);
        if self.thunder_recent.len() > 2 {
            self.thunder_recent.remove(0);
        }
        Some(self.thunder_clips[i].1.clone())
    }

    fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 40) as f32 / (1u64 << 24) as f32
    }

    /// One frame. `precip` is the kind (1 rain, 2 snow) and the rate 0 … 1, `inside` says
    /// whether the camera sits in a vehicle (the rain is then muffled - the bus's own
    /// `regen.wav` takes over), `street_cond` the state of the road and `footfalls` the steps
    /// taken since the last frame.
    pub fn update(
        &mut self,
        engine: &AudioEngine,
        dt: f32,
        precip: (i32, f32),
        inside: bool,
        street_cond: f32,
        listener: DVec3,
        footfalls: &[Footfall],
    ) {
        if !engine.enabled {
            return;
        }
        self.rain(engine, precip, inside);
        self.play_thunders(engine);
        self.footsteps(engine, dt, street_cond, listener, inside, footfalls);
    }

    /// [ROLLBACK storm-1] A thunder that has reached the listener (`storm_fx`): made up for
    /// the strike's distance (`omsi_audio::thunder`, on a helper thread: nine seconds of it are
    /// some hundred thousand samples), heard everywhere alike, and from the cab through the
    /// bodywork - quieter and dull, a rumble rather than a crack.
    pub(crate) fn thunder(&mut self, engine: &AudioEngine, t: &crate::storm_fx::Thunder, inside: bool) {
        if !engine.enabled || self.thunders.len() >= 6 {
            return;
        }
        // a recording, if there is one: played at once (air swallows the highs of a far one)
        if let Some(clip) = self.pick_thunder(t.distance_km) {
            let far = if t.distance_km > 8.0 {
                (9000.0 - 400.0 * (t.distance_km - 8.0)).max(2500.0)
            } else {
                0.0
            };
            let lowpass_hz = if inside { 700.0 } else { far };
            let pitch = 0.93 + 0.14 * self.rand();
            engine.play(
                clip,
                VoiceParams {
                    gain: t.gain * if inside { 0.5 } else { 1.0 },
                    pitch,
                    looping: false,
                    position: None,
                    doppler: false,
                    range: 1.0,
                    lowpass_hz,
                    important: true,
                },
            );
            self.last = format!("thunder {:.1} km (recording)", t.distance_km);
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let (d, seed) = (t.distance_km, t.seed);
        std::thread::spawn(move || {
            let _ = tx.send(omsi_audio::thunder::synth(d, seed, THUNDER_RATE));
        });
        self.thunders.push((rx, t.gain, inside));
        self.last = format!("thunder {:.1} km", t.distance_km);
    }

    /// The thunders made since the last frame are played.
    fn play_thunders(&mut self, engine: &AudioEngine) {
        self.thunders.retain(|(rx, gain, inside)| match rx.try_recv() {
            Ok(samples) => {
                let clip = Arc::new(Clip { sample_rate: THUNDER_RATE, channels: 1, samples });
                engine.play(
                    clip,
                    VoiceParams {
                        gain: gain * if *inside { 0.5 } else { 1.0 },
                        pitch: 1.0,
                        looping: false,
                        position: None,
                        doppler: false,
                        range: 1.0,
                        lowpass_hz: if *inside { 700.0 } else { 0.0 },
                        important: true,
                    },
                );
                false
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => true,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => false,
        });
    }

    /// The rain in the street: it only rains audibly, snow is silent. Heard at a quarter
    /// through the bodywork, and muffled on top of that, so that a shower is still there -
    /// duller, not just quieter - when you sit down in the cab.
    fn rain(&mut self, engine: &AudioEngine, precip: (i32, f32), inside: bool) {
        let Some(clip) = self.rain.clone() else {
            return;
        };
        let (kind, rate) = precip;
        let gain = if kind == 1 {
            (0.15 + 0.85 * rate.clamp(0.0, 1.0)) * if inside { 0.25 } else { 0.9 }
        } else {
            0.0
        };
        let params = VoiceParams {
            gain,
            pitch: 1.0,
            looping: true,
            position: None,
            doppler: true,
            range: 1.0,
            lowpass_hz: if inside { 400.0 } else { 0.0 },
            important: false,
        };
        match (self.rain_voice, gain > 0.001) {
            (Some(id), true) => {
                if engine.is_playing(id) {
                    engine.set_params(id, params);
                } else {
                    self.rain_voice = Some(engine.play(clip, params));
                }
            }
            (Some(id), false) => {
                engine.stop(id);
                self.rain_voice = None;
            }
            (None, true) => self.rain_voice = Some(engine.play(clip, params)),
            (None, false) => {}
        }
        self.last = format!("rain {gain:.2}");
    }

    /// Footsteps. A step is a 3D one-shot at the foot, with the `[3d]` range of the
    /// configuration (1 m), so it fades within a few metres - the pavement in front of the
    /// bus is alive, the crowd at the far end of the street is not. Snow swallows a step
    /// (quieter and duller), a wet pavement sharpens it a little.
    fn footsteps(
        &mut self,
        engine: &AudioEngine,
        dt: f32,
        street_cond: f32,
        listener: DVec3,
        listener_inside: bool,
        footfalls: &[Footfall],
    ) {
        self.step_budget = (self.step_budget + dt * STEPS_PER_SECOND).min(STEPS_PER_SECOND);
        let snow = ((street_cond - 1.0) * 2.0).clamp(0.0, 1.0);
        let wet = street_cond.clamp(0.0, 1.0) * (1.0 - snow);
        let mut played = 0;
        for f in footfalls {
            if self.step_budget < 1.0 {
                break;
            }
            // (the samples are steps on a bus's floor, `Sounds\Passengers` - the passengers'
            // sound in OMSI; people in the street walked with them as if still aboard, #236)
            let Some(pack) = f.pack.as_ref().filter(|p| f.inside && !p.is_empty()) else {
                continue;
            };
            let d = (f.position - listener).length();
            if d > self.step_range as f64 * 12.0 {
                continue;
            }
            let k = ((self.rand() * pack.len() as f32) as usize).min(pack.len() - 1);
            let file = pack[k].trim();
            let dir = &self.step_dir;
            let Some(clip) = self
                .steps
                .entry(file.to_ascii_lowercase())
                .or_insert_with(|| engine.load_clip(&omsi_cfg::resolve_path(dir, file)))
                .clone()
            else {
                continue;
            };
            self.step_budget -= 1.0;
            // indoors the floor is a hard panel however deep the snow outside is
            let (gain, pitch) = if f.inside {
                (1.0, 1.0)
            } else {
                (1.0 - 0.55 * snow + 0.1 * wet, 1.0 - 0.12 * snow)
            };
            // a step on the other side of the bus's bodywork from the listener - the pavement
            // heard from the driver's seat, the saloon heard from the street - comes through
            // it: quieter and dull (it used to sound as if the people walked in the bus)
            let (through, lowpass) = if f.own_bus != listener_inside { (0.3, 600.0) } else { (1.0, 0.0) };
            let params = VoiceParams {
                gain: self.step_volume * gain * through * (0.8 + 0.4 * self.rand()),
                pitch: pitch * (0.94 + 0.12 * self.rand()),
                looping: false,
                position: Some(f.position.as_vec3()),
                doppler: true,
                range: self.step_range,
                lowpass_hz: lowpass,
                important: false,
            };
            engine.play(clip, params);
            played += 1;
        }
        if played > 0 {
            self.last = format!("{}, {played} steps (snow {snow:.2})", self.last);
        }
    }
}

/// [ROLLBACK storm-2] The thunders compiled into the executable: (file name without the
/// extension, the WAV file), made by `build.rs` from `assets/sounds/thunder/*.wav`.
include!(concat!(env!("OUT_DIR"), "/thunder_embedded.rs"));

/// Which distance a recording is for, by its name: 0 near, 1 mid, 2 far, 3 any.
fn thunder_class(name: &str) -> u8 {
    let n = name.to_ascii_lowercase();
    if n.starts_with("near") || n.starts_with("close") {
        0
    } else if n.starts_with("mid") {
        1
    } else if n.starts_with("far") || n.starts_with("distant") {
        2
    } else {
        3
    }
}

/// The sample rate the thunder is made at.
const THUNDER_RATE: u32 = 22_050;

/// At most this many footsteps a second are heard, however big the crowd.
const STEPS_PER_SECOND: f32 = 12.0;
