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
/// Pictures of a run over every bus waiting in the queue (taken, read ahead, queued) at a time.
const FEED: usize = 4;

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

/// [ROLLBACK thumbsall-1] The run that takes a picture of every bus in every livery (Settings >
/// "Bus pictures"): the pairs still without one are fed to the queue a few at a time, so the
/// pictures a page asks for meanwhile are not stuck behind thousands of others.
pub struct Bulk {
    /// (key, bus file, paint) of the pictures to take, in the order of the bus list.
    items: Vec<(String, String, String)>,
    next: usize,
    labels: HashMap<String, (String, String)>,
    skipped: usize,
    pub taken: usize,
    pub failed: usize,
    started: Instant,
    /// When the last picture was done (the run is over).
    pub finished: Option<Instant>,
    /// The picture taken last, and the one the dialog shows (only that one is kept as a texture:
    /// thousands of 480x270 pictures would fill the graphics memory).
    pub last: Option<String>,
}

impl Bulk {
    pub fn total(&self) -> usize {
        self.items.len()
    }
    pub fn done(&self) -> usize {
        (self.skipped + self.taken + self.failed).min(self.items.len())
    }
    /// Seconds still to go, once there is a measure of the pace.
    pub fn eta(&self) -> Option<f32> {
        let n = self.taken + self.failed;
        if n < 2 || self.finished.is_some() {
            return None;
        }
        let per = self.started.elapsed().as_secs_f32() / n as f32;
        Some(per * (self.items.len() - self.done()) as f32)
    }
    pub fn elapsed(&self) -> f32 {
        self.finished.unwrap_or_else(Instant::now).duration_since(self.started).as_secs_f32()
    }
    /// What the picture `key` shows: (bus file, paint).
    pub fn label(&self, key: &str) -> Option<&(String, String)> {
        self.labels.get(key)
    }
    fn note(&mut self, key: &str, ok: bool) {
        if !self.labels.contains_key(key) {
            return;
        }
        if ok {
            self.taken += 1;
        } else {
            self.failed += 1;
        }
        if self.done() >= self.items.len() {
            self.finished = Some(Instant::now());
        }
    }
}

/// What a count of the pictures found (the dialog's figures).
#[derive(Clone, Default)]
pub struct Scan {
    pub buses: usize,
    /// Every bus in every livery.
    pub pictures: usize,
    pub missing: usize,
    pub files: usize,
    pub bytes: u64,
}

impl Scan {
    /// How big a picture is, on average: the ones on disk, else a guess for a 480x270 PNG.
    pub fn average(&self) -> u64 {
        if self.files >= 5 { self.bytes / self.files as u64 } else { 190_000 }
    }
}

pub struct Thumbs {
    pub bulk: Option<Bulk>,
    pub scan: Option<Scan>,
    /// Seconds a picture took in the last run of this session.
    pub pace: Option<f32>,
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
        Thumbs { bulk: None, scan: None, pace: None, room: studio(), queue: VecDeque::new(), queued: HashSet::new(), active: None, ahead: VecDeque::new(), failed: HashSet::new(), loads: 0, taken: 0 }
    }
}

impl Thumbs {
    /// Pictures still to take.
    pub fn waiting(&self) -> usize {
        self.queue.len() + self.ahead.len() + usize::from(self.active.is_some())
    }

    /// [ROLLBACK thumbsall-2] Give up the pictures that were asked for by a page (the livery picker, a
    /// list) and are not taken yet - the ones taken or being read for the run over every bus
    /// stay (`keep_bulk`), as does the picture on the stage. Whoever still shows a picture asks
    /// for it again with the next frame.
    pub fn cancel_queued(&mut self, keep_bulk: bool) {
        let mine = |bulk: &Option<Bulk>, k: &str| keep_bulk && bulk.as_ref().is_some_and(|b| b.labels.contains_key(k));
        let drop: Vec<String> = self.queue.iter().filter(|(k, _, _)| !mine(&self.bulk, k)).map(|(k, _, _)| k.clone()).collect();
        for k in &drop {
            self.queued.remove(k);
        }
        let bulk = &self.bulk;
        self.queue.retain(|(k, _, _)| mine(bulk, k));
        let gone: Vec<String> = self.ahead.iter().filter(|a| !mine(bulk, &a.key)).map(|a| a.key.clone()).collect();
        self.ahead.retain(|a| mine(bulk, &a.key));
        for k in gone {
            self.queued.remove(&k);
        }
        if let Some(a) = self.active.as_ref() {
            if !mine(&self.bulk, &a.key) {
                self.queued.remove(&a.key);
                self.active = None;
            }
        }
    }

    /// Forget what failed (after the content changed) and the queue.
    pub fn reset(&mut self) {
        self.bulk = None;
        self.scan = None;
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
        // [ROLLBACK thumbsall-1] the run over every bus feeds the queue a few pictures at a time
        if let Some(b) = th.bulk.as_mut() {
            while !map.is_empty() && b.finished.is_none() && b.next < b.items.len() && th.queue.len() + th.ahead.len() + usize::from(th.active.is_some()) < FEED {
                let (k, bus, paint) = b.items[b.next].clone();
                b.next += 1;
                if file_of(&k).is_file() || th.failed.contains(&k) {
                    b.skipped += 1;
                    continue;
                }
                if th.queued.insert(k.clone()) {
                    th.queue.push_back((k, bus, paint));
                }
            }
            if b.finished.is_none() && b.done() >= b.items.len() {
                b.finished = Some(Instant::now());
            }
        }
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
            if let Some(b) = th.bulk.as_mut() {
                b.note(&k, false);
            }
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
            if let Some(b) = th.bulk.as_mut() {
                b.note(&key, false);
            }
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
                if let Some(b) = th.bulk.as_mut() {
                    if b.labels.contains_key(&key) {
                        // (only the last picture of the run is kept as a texture, for the dialog)
                        if let Some(prev) = b.last.replace(key.clone()) {
                            self.icons.remove(&prev);
                            self.icons_pending.retain(|(k, _)| *k != prev);
                        }
                        b.note(&key, true);
                    }
                }
                self.icons_pending.push((key, img));
            }
            None => {
                if let Some(b) = th.bulk.as_mut() {
                    b.note(&key, false);
                }
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

// --- [ROLLBACK thumbsall-1] a picture of every bus in every livery, at the player's word ----------

fn size_text(bytes: u64) -> String {
    let mb = bytes as f64 / 1_048_576.0;
    if mb >= 1024.0 {
        format!("{:.1} GB", mb / 1024.0)
    } else {
        format!("{:.0} MB", mb.max(0.0))
    }
}

fn time_text(secs: f32) -> String {
    let s = secs.max(0.0).round() as u64;
    if s < 90 {
        format!("{s} s")
    } else if s < 5400 {
        format!("{} min", (s + 30) / 60)
    } else {
        format!("{} h {:02} min", s / 3600, (s % 3600) / 60)
    }
}

impl Launcher {
    /// Count the pictures: how many the buses come to, how many are on disk and how big.
    pub fn thumbs_scan(&mut self) {
        let mut seen = HashSet::new();
        let mut sc = Scan::default();
        for v in &self.state.vehicles {
            sc.buses += 1;
            for paint in std::iter::once("").chain(v.paints.iter().map(|p| p.as_str())) {
                let k = key(&v.file, paint);
                if !seen.insert(k.clone()) {
                    continue;
                }
                sc.pictures += 1;
                if !file_of(&k).is_file() {
                    sc.missing += 1;
                }
            }
        }
        if let Ok(list) = std::fs::read_dir(dir()) {
            for e in list.flatten() {
                if e.path().extension().is_some_and(|x| x == "png") {
                    sc.files += 1;
                    sc.bytes += e.metadata().map(|m| m.len()).unwrap_or(0);
                }
            }
        }
        self.thumbs.scan = Some(sc);
    }

    /// Start taking the pictures that are still missing.
    pub fn thumbs_bulk_start(&mut self) {
        let mut items = Vec::new();
        let mut labels = HashMap::new();
        let mut seen = HashSet::new();
        for v in &self.state.vehicles {
            for paint in std::iter::once("").chain(v.paints.iter().map(|p| p.as_str())) {
                let k = key(&v.file, paint);
                if !seen.insert(k.clone()) || file_of(&k).is_file() {
                    continue;
                }
                labels.insert(k.clone(), (v.file.clone(), paint.to_string()));
                items.push((k, v.file.clone(), paint.to_string()));
            }
        }
        // (a bus that did not load before gets another try)
        self.thumbs.failed.clear();
        self.thumbs.bulk = Some(Bulk { items, next: 0, labels, skipped: 0, taken: 0, failed: 0, started: Instant::now(), finished: None, last: None });
    }

    /// Stop the run (or put its report away): the pictures taken stay.
    pub fn thumbs_bulk_cancel(&mut self) {
        if let Some(b) = self.thumbs.bulk.take() {
            let th = &mut self.thumbs;
            // (what is queued, read ahead or being taken for the run stops at once)
            let drop: Vec<String> = th.queue.iter().filter(|(k, _, _)| b.labels.contains_key(k)).map(|(k, _, _)| k.clone()).collect();
            th.queue.retain(|(k, _, _)| !b.labels.contains_key(k));
            for k in drop {
                th.queued.remove(&k);
            }
            let gone: Vec<String> = th.ahead.iter().filter(|a| b.labels.contains_key(&a.key)).map(|a| a.key.clone()).collect();
            th.ahead.retain(|a| !b.labels.contains_key(&a.key));
            for k in gone {
                th.queued.remove(&k);
            }
            if th.active.as_ref().is_some_and(|a| b.labels.contains_key(&a.key)) {
                if let Some(a) = th.active.take() {
                    th.queued.remove(&a.key);
                }
            }
            let n = b.taken + b.failed;
            if n >= 3 {
                th.pace = Some(b.elapsed() / n as f32);
            }
            if let Some(k) = b.last {
                self.icons.remove(&k);
                self.icons_pending.retain(|(p, _)| *p != k);
            }
        }
        self.thumbs.scan = None;
    }
}

/// The dialog of Settings > "Bus pictures": what the pictures come to (how many, how much room,
/// how long) and, once started, the run: the bar, the time left and each picture as it is taken.
pub(super) fn all_dialog(l: &mut Launcher) {
    use super::ui::ButtonKind;
    use omsi_ui::paint::Align;
    use omsi_ui::Weight;
    if l.thumbs.scan.is_none() && l.thumbs.bulk.is_none() {
        l.thumbs_scan();
    }
    let size = l.ui.size;
    let full = Rect::new(0.0, 0.0, size.x, size.y);
    l.ui.solid(full);
    l.ui.p().rect(full, omsi_ui::Color::rgba(0, 0, 0, 0.62));
    let w = (size.x - 48.0).min(640.0);
    let h = (size.y - 48.0).min(520.0);
    let r = Rect::new((size.x - w) * 0.5, (size.y - h) * 0.5, w, h);
    l.ui.panel(r);
    let inner = Rect::new(r.x + 24.0, r.y + 20.0, r.w - 48.0, r.h - 40.0);
    l.ui.icon("photo_camera", Vec2::new(inner.x + 14.0, inner.y + 14.0), 26.0, ACCENT);
    l.ui.text_in("Pictures of the buses", Rect::new(inner.x + 38.0, inner.y, inner.w - 38.0, 28.0), 18.0, omsi_ui::Weight::Bold, TEXT, Align::Left);
    let mut y = inner.y + 40.0;
    y += l.ui.paragraph(
        "The lists (the dealership, the fleet, the livery picker) show a picture of each bus. A page takes the ones it shows by itself, one by one, and they are kept; here every bus in every livery gets its picture at once.",
        Vec2::new(inner.x, y),
        inner.w,
        12.5,
        Weight::Regular,
        TEXT_DIM,
    ) + 10.0;

    let can_work = l.thumbs_can_work();
    let run = l.thumbs.bulk.as_ref().map(|b| (b.total(), b.done(), b.taken, b.failed, b.eta(), b.elapsed(), b.finished.is_some(), b.last.clone()));
    let foot_y = inner.bottom() - 38.0;

    match run {
        None => {
            let sc = l.thumbs.scan.clone().unwrap_or_default();
            let per = l.thumbs.pace.unwrap_or(1.3);
            let rows = [
                ("Buses", format!("{} · {} pictures with the liveries", sc.buses, sc.pictures)),
                ("Taken already", format!("{} · {} on the disk", sc.pictures - sc.missing, size_text(sc.bytes))),
                ("Still to take", format!("{} · about {} more", sc.missing, size_text(sc.missing as u64 * sc.average()))),
                ("Time it takes", if sc.missing == 0 { "-".to_string() } else { format!("about {} (it depends on the computer and on the buses)", time_text(sc.missing as f32 * per)) }),
            ];
            for (label, value) in rows {
                l.ui.text_in(label, Rect::new(inner.x, y, 130.0, 22.0), 13.0, Weight::Medium, TEXT_SOFT, Align::Left);
                l.ui.text_in(&value, Rect::new(inner.x + 140.0, y, inner.w - 140.0, 22.0), 13.0, Weight::Regular, TEXT, Align::Left);
                y += 26.0;
            }
            y += 6.0;
            if !can_work {
                y += l.ui.paragraph("The pictures are taken in front of a map: choose one on the Drive page first.", Vec2::new(inner.x, y), inner.w, 12.5, Weight::Medium, WARN);
            } else if sc.missing == 0 {
                y += l.ui.paragraph("Every bus has its picture.", Vec2::new(inner.x, y), inner.w, 12.5, Weight::Medium, OK);
            } else {
                y += l.ui.paragraph("It runs in the background: the pictures are taken while this window is open or not, and the launcher stays usable. Starting the game pauses it.", Vec2::new(inner.x, y), inner.w, 12.0, Weight::Regular, TEXT_DIM);
            }
            if l.ui.button("pics-close", Rect::new(inner.right() - 110.0, foot_y, 110.0, 38.0), "Close", None, ButtonKind::Normal) {
                l.pages.thumbs_dialog = false;
            }
            let go = Rect::new(inner.right() - 110.0 - 10.0 - 210.0, foot_y, 210.0, 38.0);
            if can_work && sc.missing > 0 {
                if l.ui.button("pics-start", go, "Take all pictures", Some("photo_camera"), ButtonKind::Primary) {
                    l.thumbs_bulk_start();
                }
            } else {
                l.ui.button_off(go, "Take all pictures", Some("photo_camera"), ButtonKind::Primary);
            }
            if l.ui.button("pics-rescan", Rect::new(inner.x, foot_y, 130.0, 38.0), "Count again", Some("refresh"), ButtonKind::Normal) {
                l.thumbs.scan = None;
            }
        }
        Some((total, done, taken, failed, eta, elapsed, finished, last)) => {
            let frac = if total == 0 { 1.0 } else { done as f32 / total as f32 };
            l.ui.progress(Rect::new(inner.x, y + 4.0, inner.w, 10.0), frac, !finished);
            y += 24.0;
            let line = if finished {
                format!("Done: {taken} pictures in {}", time_text(elapsed))
            } else {
                let left = match eta {
                    Some(e) => format!("about {} left", time_text(e)),
                    None => "working out the time...".to_string(),
                };
                format!("{done} of {total} · {left}")
            };
            l.ui.text_in(&line, Rect::new(inner.x, y, inner.w, 22.0), 14.0, Weight::Bold, TEXT, Align::Left);
            y += 24.0;
            let mut sub = format!("{taken} taken · {} elapsed", time_text(elapsed));
            if failed > 0 {
                sub.push_str(&format!(" · {failed} did not load (no picture)"));
            }
            l.ui.text_in(&sub, Rect::new(inner.x, y, inner.w, 18.0), 12.0, Weight::Regular, if failed > 0 { WARN } else { TEXT_DIM }, Align::Left);
            y += 26.0;
            if !can_work && !finished {
                l.ui.text_in("Waiting: the pictures need a map chosen on the Drive page.", Rect::new(inner.x, y, inner.w, 18.0), 12.0, Weight::Medium, WARN, Align::Left);
            }
            // the picture taken last, with its name
            let avail = (foot_y - y - 12.0).max(40.0);
            let ph = avail.min(inner.w * 0.5 * 9.0 / 16.0).min(190.0);
            let pic = Rect::new(inner.x, y + 4.0, ph * 16.0 / 9.0, ph);
            if let Some(k) = last {
                let lab = l.thumbs.bulk.as_ref().and_then(|b| b.label(&k)).cloned();
                if let Some((bus, paint)) = lab {
                    thumb_draw(&mut l.ui, &l.icons, pic, &bus, &paint, 6.0);
                    let name = l.state.vehicles.iter().find(|v| v.file == bus).map(|v| core::display_bus_name(&v.name)).unwrap_or_else(|| bus.clone());
                    let paint_name = if paint.is_empty() { "Default paint".to_string() } else { paint.clone() };
                    let tx = pic.right() + 16.0;
                    l.ui.text_in("Last picture", Rect::new(tx, pic.y, inner.right() - tx, 18.0), 11.5, Weight::Regular, TEXT_DIM, Align::Left);
                    l.ui.text_in(&name, Rect::new(tx, pic.y + 20.0, inner.right() - tx, 24.0), 15.0, Weight::Bold, TEXT, Align::Left);
                    l.ui.text_in(&paint_name, Rect::new(tx, pic.y + 46.0, inner.right() - tx, 20.0), 13.0, Weight::Regular, TEXT_SOFT, Align::Left);
                }
            } else {
                l.ui.p().rounded(pic, 6.0, FIELD);
                l.ui.icon("photo_camera", pic.center(), 30.0, TEXT_FAINT);
            }
            if finished {
                if l.ui.button("pics-done", Rect::new(inner.right() - 110.0, foot_y, 110.0, 38.0), "Close", None, ButtonKind::Primary) {
                    l.thumbs_bulk_cancel();
                    l.pages.thumbs_dialog = false;
                }
            } else {
                if l.ui.button("pics-cancel", Rect::new(inner.right() - 130.0, foot_y, 130.0, 38.0), "Stop", Some("close"), ButtonKind::Danger) {
                    l.thumbs_bulk_cancel();
                }
                if l.ui.button("pics-hide", Rect::new(inner.right() - 130.0 - 10.0 - 150.0, foot_y, 150.0, 38.0), "Hide window", None, ButtonKind::Normal) {
                    l.pages.thumbs_dialog = false;
                }
            }
        }
    }
}
