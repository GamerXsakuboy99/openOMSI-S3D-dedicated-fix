//! Bus thumbnails: the picture of a bus in a given paint, drawn once by the showroom's
//! renderer and kept as a PNG, so lists of buses (the dealership, the fleet, the catalogue)
//! show real pictures without loading every bus onto the GPU each time.
//!
//! A page asks for a picture with `Launcher::thumb_request` and draws it with `thumb_draw`:
//! if the PNG is on disk it is read into the interface (a few per frame); if not, the bus
//! goes in a queue. `thumbs_frame` takes the pictures one at a time in a showroom of its
//! own (so the live preview on the page is left alone): the bus is shown, lets it settle for
//! a few frames, the picture is read back and written. The next `AHEAD` buses of the queue
//! are already being read (and placed and settled) in showrooms of their own meanwhile, so
//! that the reading, which is most of the time, overlaps the picture-taking instead of
//! following it. [ROLLBACK thumbs-2] A bus that does not load is marked as failed for this
//! run and gets a plain placeholder.
//!
//! The files are `<data folder>/cache/bus_thumbs/<key>.png`; the key is a hash of the bus
//! file, the paint and `VERSION`, so changing how the pictures are taken (the angle, the
//! size, the light) is one number here. The catalogue page has a button that empties the
//! folder.

use super::showroom::{Look, Showroom};
use super::theme::*;
use super::ui::Ui;
use super::Launcher;
use glam::Vec2;
use omsi_launcher_lib as core;
use omsi_ui::Rect;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Bump to take every picture again.
const VERSION: u64 = 3;
/// The size of a picture (16:9), pixels.
pub const W: u32 = 480;
pub const H: u32 = 270;
/// The angle the pictures are taken from: a front three-quarter view, a little from above.
const YAW: f32 = 215.0;
const PITCH: f32 = 9.0;
const ZOOM: f32 = 1.0;
/// Frames the bus is shown before the picture is taken (the textures arrive meanwhile).
const SETTLE_FRAMES: u32 = 4;
/// A bus that takes longer than this to load is given up.
const GIVE_UP: Duration = Duration::from_secs(45);
/// PNGs read per frame.
const LOADS_PER_FRAME: u32 = 6;
/// Buses read ahead while a picture is taken ([ROLLBACK thumbs-2]: 0 is one at a time, as before).
const AHEAD: usize = 2;

struct Active {
    key: String,
    look: Look,
    settled: u32,
    started: Instant,
}

/// A bus being read in a showroom of its own, for after the one being taken.
struct Ahead {
    key: String,
    look: Look,
    room: Showroom,
    settled: u32,
    started: Instant,
}

pub struct Thumbs {
    room: Showroom,
    queue: VecDeque<(String, String, String)>,
    queued: HashSet<String>,
    active: Option<Active>,
    ahead: VecDeque<Ahead>,
    failed: HashSet<String>,
    loads: u32,
    /// Pictures taken this run, and still waiting (for a line of progress).
    pub taken: u32,
}

fn studio() -> Showroom {
    let mut r = Showroom::new();
    r.studio = true;
    r
}

impl Default for Thumbs {
    fn default() -> Thumbs {
        Thumbs { room: studio(), queue: VecDeque::new(), queued: HashSet::new(), active: None, ahead: VecDeque::new(), failed: HashSet::new(), loads: 0, taken: 0 }
    }
}

impl Thumbs {
    /// Pictures still to take.
    pub fn waiting(&self) -> usize {
        self.queue.len() + self.ahead.len() + usize::from(self.active.is_some())
    }

    /// Forget what failed (after the content changed) and the queue.
    pub fn reset(&mut self) {
        self.queue.clear();
        self.queued.clear();
        self.failed.clear();
        self.active = None;
        self.ahead.clear();
    }
}

/// The folder of the pictures.
pub fn dir() -> PathBuf {
    core::data_dir().join("cache").join("bus_thumbs")
}

/// The key of a bus in a paint (`""` its own paint): what the interface's texture map and
/// the file name go by.
pub fn key(bus: &str, paint: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325 ^ VERSION.wrapping_mul(0x9E3779B97F4A7C15);
    let text = format!("{}\u{1}{}", bus.replace('\\', "/").to_lowercase(), paint.trim().to_lowercase());
    for b in text.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("bus:{h:016x}")
}

fn file_of(key: &str) -> PathBuf {
    dir().join(format!("{}.png", key.trim_start_matches("bus:")))
}

/// Empty the cache folder; returns how many pictures went.
pub fn clear_cache() -> usize {
    let mut n = 0;
    if let Ok(list) = std::fs::read_dir(dir()) {
        for e in list.flatten() {
            if e.path().extension().is_some_and(|x| x == "png") && std::fs::remove_file(e.path()).is_ok() {
                n += 1;
            }
        }
    }
    n
}

/// A bus's picture in `r`, or a placeholder while it is not there yet.
pub fn thumb_draw(ui: &mut Ui, icons: &HashMap<String, usize>, r: Rect, bus: &str, paint: &str, radius: f32) {
    match icons.get(&key(bus, paint)) {
        Some(&tex) => ui.image(r, tex, radius),
        None => {
            ui.p().rounded(r, radius, FIELD);
            ui.icon("directions_bus", r.center(), (r.h * 0.4).clamp(14.0, 40.0), TEXT_FAINT);
        }
    }
}

impl Launcher {
    /// Ask for the picture of `bus` in `paint`: read from the cache, or queued to be taken.
    pub fn thumb_request(&mut self, bus: &str, paint: &str) {
        if bus.is_empty() {
            return;
        }
        let k = key(bus, paint);
        if self.icons.contains_key(&k) || self.icons_pending.iter().any(|(p, _)| *p == k) || self.thumbs.failed.contains(&k) {
            return;
        }
        let path = file_of(&k);
        if path.is_file() {
            if self.thumbs.loads < LOADS_PER_FRAME {
                self.thumbs.loads += 1;
                match image::open(&path) {
                    Ok(img) => self.icons_pending.push((k, img.to_rgba8())),
                    Err(_) => {
                        // (a damaged file: taken again)
                        let _ = std::fs::remove_file(&path);
                    }
                }
            }
            return;
        }
        if self.thumbs.queued.insert(k.clone()) {
            self.thumbs.queue.push_back((k, bus.to_string(), paint.to_string()));
        }
    }

    /// Per frame: take the next queued picture, if there is one. Costs nothing while the
    /// queue is empty.
    pub(super) fn thumbs_frame(&mut self, dt: f32) {
        self.thumbs.loads = 0;
        if self.state.in_game() {
            return;
        }
        let Some(renderer) = self.renderer.as_mut() else { return };
        let th = &mut self.thumbs;
        let map = self.state.choice.map.clone();
        // the next picture: the bus that was read ahead, else the next of the queue
        if th.active.is_none() {
            if let Some(a) = th.ahead.pop_front() {
                th.room = a.room;
                th.active = Some(Active { key: a.key, look: a.look, settled: a.settled, started: a.started });
            } else if !th.queue.is_empty() && !map.is_empty() {
                let Some((k, bus, paint)) = th.queue.pop_front() else { return };
                let look = Look {
                    root: PathBuf::from(&self.state.config.root),
                    map: map.clone(),
                    bus,
                    paint,
                    weather: "Weather/#CAVOK.owt".to_string(),
                    time: 12 * 60,
                    date: self.state.choice.date.clone(),
                    // (no stickers on the cached pictures: they would go stale)
                    decals: Some(Vec::new()),
                    owner: 0,
                    rev: 0,
                };
                th.room.set_view(YAW, PITCH, ZOOM);
                th.room.want(look.clone());
                th.active = Some(Active { key: k, look, settled: 0, started: Instant::now() });
            }
        }
        // the next buses of the queue are read meanwhile, each in a showroom of its own
        while !map.is_empty() && th.ahead.len() < AHEAD && th.active.is_some() {
            let Some((k, bus, paint)) = th.queue.pop_front() else { break };
            let look = Look {
                root: PathBuf::from(&self.state.config.root),
                map: map.clone(),
                bus,
                paint,
                weather: "Weather/#CAVOK.owt".to_string(),
                time: 12 * 60,
                date: self.state.choice.date.clone(),
                decals: Some(Vec::new()),
                owner: 0,
                rev: 0,
            };
            let mut room = studio();
            room.set_view(YAW, PITCH, ZOOM);
            room.want(look.clone());
            th.ahead.push_back(Ahead { key: k, look, room, settled: 0, started: Instant::now() });
        }
        let mut gone: Vec<String> = Vec::new();
        for a in th.ahead.iter_mut() {
            a.room.update(renderer, dt);
            if a.room.has_failed(&a.look) || a.started.elapsed() > GIVE_UP {
                gone.push(a.key.clone());
            } else if a.room.is_shown(&a.look) {
                a.settled += 1;
            }
        }
        for k in gone {
            log::warn!("thumbnail {k}: the bus did not load, no picture");
            th.ahead.retain(|a| a.key != k);
            th.queued.remove(&k);
            th.failed.insert(k);
        }
        if th.active.is_none() {
            return;
        }
        th.room.update(renderer, dt);
        let Some(act) = th.active.as_mut() else { return };
        let failed = th.room.has_failed(&act.look) || act.started.elapsed() > GIVE_UP;
        let mut take = false;
        if !failed && th.room.is_shown(&act.look) {
            act.settled += 1;
            take = act.settled >= SETTLE_FRAMES;
        }
        if !failed && !take {
            return;
        }
        let key = act.key.clone();
        th.active = None;
        th.queued.remove(&key);
        if failed {
            log::warn!("thumbnail {key}: the bus did not load, no picture");
            th.failed.insert(key);
            return;
        }
        match th.room.capture_fitted(renderer, W, H) {
            Some(img) => {
                let path = file_of(&key);
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Err(e) = img.save(&path) {
                    log::warn!("thumbnail {}: not written: {e}", path.display());
                }
                th.taken += 1;
                self.icons_pending.push((key, img));
            }
            None => {
                th.failed.insert(key);
            }
        }
    }

    /// Whether the page `p` is drawn with the thumbnails' help; the pictures need a map to
    /// stand in (the showroom opens one), so with none chosen the placeholders stay.
    pub fn thumbs_can_work(&self) -> bool {
        !self.state.choice.map.is_empty() && self.renderer.is_some()
    }

    /// A line for the page: how many pictures are still being taken.
    pub fn thumbs_status(&self) -> Option<String> {
        let n = self.thumbs.waiting();
        (n > 0).then(|| format!("Taking pictures of the buses: {n} to go"))
    }

    /// Draw a bus's picture the way every list does: `r` should be 16:9.
    pub fn draw_thumb(&mut self, r: Rect, bus: &str, paint: &str, radius: f32) {
        self.thumb_request(bus, paint);
        thumb_draw(&mut self.ui, &self.icons, r, bus, paint, radius);
    }

    /// Where a spinner for the queue goes, drawn by pages that wait for pictures.
    pub fn thumbs_spinner(&mut self, c: Vec2) {
        if self.thumbs.waiting() > 0 {
            let a = self.ui.time * 5.0;
            self.ui.p().arc(c, 6.0, 8.0, a, a + 4.2, TEXT_SOFT);
        }
    }
}
