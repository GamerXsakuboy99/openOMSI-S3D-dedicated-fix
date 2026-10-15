//! The launcher's bus preview: the chosen bus drawn by the game's own renderer - its
//! model, paint and materials - on the plain (Vanilla+) path without the costly passes,
//! under the light of the chosen time and weather, into a picture the launcher shows
//! in a card. It is drawn again only when something changed (another bus, paint, light,
//! the preview turned by the mouse), never every frame.
//!
//! A bus is read on a worker (its type, its scripts run to their resting state, its
//! textures and meshes put on the GPU ahead) and then placed into a scene of its own with
//! a fresh `World` (whose caches belong to one scene); the old scene goes when the new one
//! is ready, so the picture never goes blank while switching.

use super::super::*;
use glam::DVec3;
use omsi_render::{Camera, Lighting, Renderer, Scene};
use std::sync::mpsc::{channel, Receiver};

/// What the showroom shows.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Look {
    pub root: PathBuf,
    pub map: String,
    pub bus: String,
    pub paint: String,
    pub weather: String,
    /// Minutes of the day, and the date.
    pub time: i32,
    pub date: String,
    /// The stickers to put on the bus: None = the ones saved for it (its own set when `owner`
    /// is a bus of the company, else its model's), Some = these (the livery editor's work).
    pub decals: Option<Vec<crate::decals::Decal>>,
    /// The company's bus this is (0: none).
    pub owner: u32,
    /// `decals::revision()` when the look was asked for: saving a set shows it again.
    pub rev: u32,
}

/// Where the preview's camera was, for turning a click into a ray.
#[derive(Clone, Copy)]
struct CamInfo {
    pos: DVec3,
    yaw: f32,
    pitch: f32,
    half_v: f32,
    aspect: f32,
}

impl CamInfo {
    /// (right, up, forward) of the camera.
    fn axes(&self) -> (glam::Vec3, glam::Vec3, glam::Vec3) {
        let (sy, cy) = self.yaw.to_radians().sin_cos();
        let (sp, cp) = self.pitch.to_radians().sin_cos();
        let fwd = glam::Vec3::new(sy * cp, cy * cp, -sp);
        let right = glam::Vec3::new(cy, -sy, 0.0);
        (right, right.cross(fwd), fwd)
    }
}

struct Ready {
    look: Look,
    world: Arc<scene::World>,
    vt: Arc<omsi_sim::VehicleType>,
    vehicle: omsi_sim::VehicleInstance,
    scheme: Option<usize>,
}

struct Shown {
    look: Look,
    scene: Scene,
    #[allow(dead_code)]
    world: Option<Arc<scene::World>>,
    vehicle: Option<omsi_sim::VehicleInstance>,
    render: Option<scene::VehicleRender>,
    trailers: Vec<scene::VehicleRender>,
    /// Centre and size of the bus (its bounding box).
    centre: glam::Vec3,
    length: f32,
    /// [ROLLBACK showroom-119] the widest and the tallest of the sections (m)
    width: f32,
    height: f32,
    weather: omsi_content::weather::Weather,
    lighting: Lighting,
    /// The decal editor's stickers, drawn over the bus without reading it again.
    live: Vec<LiveItem>,
    /// Their pictures on the GPU, by file name in the decal cache.
    live_tex: std::collections::HashMap<String, usize>,
    /// Instances of removed stickers, to be used again (one material slot each).
    live_free: Vec<usize>,
}

/// One sticker of the live layer.
struct LiveItem {
    decal: crate::decals::Decal,
    instances: Vec<usize>,
    /// The mesh of the bus each instance sits on (it moves with it).
    hosts: Vec<usize>,
    meshes: Vec<usize>,
    materials: Vec<usize>,
}

pub struct Showroom {
    shown: Option<Shown>,
    wanted: Option<Look>,
    /// The last look that could not be shown (not read again until something changes).
    failed: Option<Look>,
    loading: Option<(Look, Receiver<Result<Ready, String>>)>,
    pub error: Option<String>,
    /// Orbit: yaw and pitch (degrees) and distance factor, eased towards the targets.
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
    yaw_to: f32,
    pitch_to: f32,
    zoom_to: f32,
    pub auto_turn: bool,
    /// A photo studio instead of the live preview: the camera is as close as the whole bus
    /// fits the picture, and the sun stands behind the camera, whatever the time and map.
    pub studio: bool,
    /// Studio fit, found by measuring the picture: how far (in screen widths / heights) the
    /// camera's aim is moved, and the factor of the distance.
    fit_shift: (f32, f32),
    fit_scale: f32,
    /// Measuring pass: no shadows (only the bus differs from the empty studio).
    measure: bool,
    idle: f32,
    /// Where the bus should appear on screen: the share of the width its centre is at.
    pub focus_x: f32,
    focus_now: f32,
    pub busy: bool,
    /// The picture: its texture and size, and whether it must be drawn again.
    target: Option<(wgpu::Texture, wgpu::TextureView, u32, u32)>,
    dirty: bool,
    /// Bumped whenever `target` was made anew (the interface binds it again).
    pub generation: u64,
    /// The camera of the last picture drawn, and of the last one drawn for the preview.
    last_cam: Option<CamInfo>,
    cam_preview: Option<CamInfo>,
    /// How near the camera may come (the decal editor looks at a door handle's sticker).
    zoom_min: f32,
    min_dist: f32,
    /// The aim, moved off the bus's middle: where it is, where it goes, and the spot of the
    /// bus the cursor is over (the wheel zooms towards it).
    aim: glam::Vec3,
    aim_to: glam::Vec3,
    /// The camera's distance from its aim (m), from the last picture.
    cam_dist: f32,
    /// Seen from inside: the meshes only the inside shows are drawn, the outside ones not.
    inside: bool,
    /// The editor's field of view (degrees) - the wheel changes it - and the free-flight camera.
    fov: f32,
    free: Option<FreeCam>,
}

/// A camera that flies anywhere: where it is and where it looks (degrees; pitch down positive).
#[derive(Clone, Copy)]
struct FreeCam {
    pos: DVec3,
    yaw: f32,
    pitch: f32,
}

fn args_for(look: &Look) -> Args {
    let mut v = vec!["omsi".to_string(), "--root".into(), look.root.to_string_lossy().to_string()];
    if !look.map.is_empty() {
        v.extend(["--map".into(), look.map.clone()]);
    }
    if !look.bus.is_empty() {
        v.extend(["--bus".into(), look.bus.clone()]);
    }
    if !look.paint.is_empty() {
        v.extend(["--paint".into(), look.paint.clone()]);
    }
    // (the current weather is fetched by the game, not by the preview)
    if !look.weather.is_empty() && !look.weather.starts_with("metar:") {
        v.extend(["--weather".into(), look.weather.clone()]);
    }
    v.extend(["--time".into(), format!("{:02}:{:02}", look.time / 60, look.time % 60)]);
    if !look.date.is_empty() {
        v.extend(["--date".into(), look.date.clone()]);
    }
    Args::try_parse_from(v).unwrap_or_else(|_| Args::parse_from(["omsi"]))
}

impl Showroom {
    pub fn new() -> Showroom {
        Showroom {
            shown: None,
            wanted: None,
            failed: None,
            loading: None,
            error: None,
            yaw: 215.0,
            pitch: 8.0,
            zoom: 1.0,
            yaw_to: 215.0,
            pitch_to: 8.0,
            zoom_to: 1.0,
            auto_turn: false,
            studio: false,
            fit_shift: (0.0, 0.0),
            fit_scale: 1.0,
            measure: false,
            idle: 0.0,
            focus_x: 0.5,
            focus_now: 0.5,
            busy: false,
            target: None,
            dirty: true,
            generation: 0,
            last_cam: None,
            zoom_min: 0.55,
            min_dist: 8.0,
            aim: glam::Vec3::ZERO,
            aim_to: glam::Vec3::ZERO,
            cam_dist: 10.0,
            inside: false,
            cam_preview: None,
            fov: 30.0,
            free: None,
        }
    }

    /// Show this (a bus, its paint, the time and weather to light it with).
    pub fn want(&mut self, look: Look) {
        if self.wanted.as_ref() != Some(&look) {
            self.wanted = Some(look);
        }
    }

    /// The mouse dragged over the empty part of the window (degrees), or turned the wheel.
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        if let Some(f) = self.free.as_mut() {
            // free flight: the mouse turns the head
            f.yaw += dx * 0.25;
            f.pitch = (f.pitch + dy * 0.25).clamp(-89.0, 89.0);
            self.idle = 0.0;
            self.dirty = true;
            return;
        }
        self.yaw_to += dx * 0.35;
        self.pitch_to = (self.pitch_to + dy * 0.25).clamp(-4.0, 55.0);
        self.idle = 0.0;
    }
    pub fn zoom_by(&mut self, k: f32) {
        self.zoom_to = (self.zoom_to * k).clamp(self.zoom_min, 2.2);
        self.idle = 0.0;
    }

    /// The wheel: in the editor it changes the field of view (a longer lens zooms in, without
    /// the camera moving), elsewhere it moves the camera nearer or farther.
    pub fn wheel(&mut self, k: f32) {
        if self.min_dist < 1.0 {
            self.fov = (self.fov * k).clamp(8.0, 100.0);
            self.idle = 0.0;
            self.dirty = true;
        } else {
            self.zoom_by(k);
        }
    }
    pub fn set_fov(&mut self, f: f32) {
        self.fov = f.clamp(8.0, 100.0);
        self.idle = 0.0;
        self.dirty = true;
    }
    /// The field of view now (degrees).
    pub fn fov(&self) -> f32 {
        self.fov
    }

    /// Free flight on or off: on, the camera stays where it is and from then on goes anywhere,
    /// through the bus and the ground too.
    pub fn set_free(&mut self, on: bool) {
        if on == self.free.is_some() {
            return;
        }
        if on {
            if let Some(c) = self.last_cam {
                self.free = Some(FreeCam { pos: c.pos, yaw: c.yaw, pitch: c.pitch });
            }
        } else {
            self.free = None;
        }
        self.idle = 0.0;
        self.dirty = true;
    }
    pub fn is_free(&self) -> bool {
        self.free.is_some()
    }

    /// Move the camera (and what it aims at) sideways and up by a mouse drag of `dx`, `dy`
    /// pixels: the bus follows the cursor.
    pub fn pan(&mut self, dx: f32, dy: f32) {
        let Some(cam) = self.last_cam else { return };
        let (right, up, _) = cam.axes();
        if let Some(f) = self.free.as_mut() {
            let k = 0.004f64;
            let d = (right * -dx + up * dy) * k as f32;
            f.pos += DVec3::new(d.x as f64, d.y as f64, d.z as f64);
            self.idle = 0.0;
            self.dirty = true;
            return;
        }
        let k = self.cam_dist * 0.0011;
        let lim = self.shown.as_ref().map(|s| s.length.max(s.width) * 0.7).unwrap_or(10.0);
        let mut p = self.aim_to - right * dx * k + up * dy * k;
        p = p.clamp(glam::Vec3::splat(-lim), glam::Vec3::splat(lim));
        self.aim_to = p;
        self.aim = p;
        self.idle = 0.0;
        self.dirty = true;
    }

    /// The keys W, A, S, D, Q, E: fly the camera (and what it looks at) forward, to the sides
    /// and up, each -1..1, for `dt` seconds, at a pace that follows how far the camera is
    /// (in free flight: a steady pace, faster with `fast`).
    pub fn pan_keys(&mut self, side: f32, forward: f32, up: f32, fast: bool, dt: f32) {
        if side == 0.0 && forward == 0.0 && up == 0.0 {
            return;
        }
        let Some(cam) = self.last_cam else { return };
        let (right, _, fwd) = cam.axes();
        if let Some(f) = self.free.as_mut() {
            let speed = if fast { 12.0 } else { 3.0 } * dt;
            let d = (right * side + fwd * forward + glam::Vec3::Z * up) * speed;
            f.pos += DVec3::new(d.x as f64, d.y as f64, d.z as f64);
            self.idle = 0.0;
            self.dirty = true;
            return;
        }
        let speed = (self.cam_dist * 0.6).max(0.3) * dt;
        let lim = self.shown.as_ref().map(|s| s.length.max(s.width) * 0.7).unwrap_or(10.0);
        let p = (self.aim_to + (right * side + fwd * forward + glam::Vec3::Z * up) * speed).clamp(glam::Vec3::splat(-lim), glam::Vec3::splat(lim));
        self.aim_to = p;
        self.aim = p;
        self.idle = 0.0;
        self.dirty = true;
    }

    /// Look from inside the bus (or from outside again).
    pub fn set_inside(&mut self, on: bool) {
        if self.inside != on {
            self.inside = on;
            self.dirty = true;
        }
    }

    /// The decal editor's camera: close up, and the view back to normal when it is over.
    pub fn close_up(&mut self, on: bool) {
        let (zmin, dmin) = if on { (0.03, 0.5) } else { (0.55, 8.0) };
        if (self.zoom_min, self.min_dist) != (zmin, dmin) {
            self.zoom_min = zmin;
            self.min_dist = dmin;
            if !on {
                self.zoom_to = self.zoom_to.max(0.55);
                self.aim_to = glam::Vec3::ZERO;
                self.fov = 30.0;
                self.free = None;
                self.dirty = true;
            }
        }
    }

    /// Look at the bus from this direction (degrees; 180 = from the front, 0 = from the rear,
    /// 90 = from the left, 270 = from the right), level; the aim in the middle again.
    pub fn view_from(&mut self, yaw: f32, pitch: f32) {
        // (by the shortest way round)
        self.yaw_to = yaw + 360.0 * ((self.yaw_to - yaw) / 360.0).round();
        self.pitch_to = pitch;
        self.aim_to = glam::Vec3::ZERO;
        self.zoom_to = 1.0;
        self.idle = 0.0;
    }

    /// Per frame: start loading what is wanted, take over what finished loading, move the
    /// camera. Returns true when a new scene was put in place.
    pub fn update(&mut self, renderer: &Renderer, dt: f32) -> bool {
        let mut swapped = false;
        // what finished loading
        if let Some((look, rx)) = self.loading.as_ref() {
            match rx.try_recv() {
                Ok(Ok(ready)) => {
                    self.loading = None;
                    self.shown = Some(self.place(renderer, ready));
                    self.error = None;
                    self.dirty = true;
                    swapped = true;
                }
                Ok(Err(e)) => {
                    log::warn!("showroom {}: {e}", look.bus);
                    self.error = Some(e);
                    // (the bus before stayed in the picture as if it were the one chosen, and
                    // the failed one was read again every frame)
                    self.failed = Some(look.clone());
                    self.shown = None;
                    self.loading = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(_) => {
                    self.failed = Some(look.clone());
                    self.loading = None;
                }
            }
        }
        // what is wanted and not shown or loading
        if let Some(w) = self.wanted.clone() {
            let shown = self.shown.as_ref().map(|s| &s.look);
            let loading = self.loading.as_ref().map(|l| &l.0);
            if shown != Some(&w) && loading != Some(&w) && self.loading.is_none() && self.failed.as_ref() != Some(&w) {
                // only the light changed: no need to read the bus again
                let same_bus = shown.map(|s| s.bus == w.bus && s.paint == w.paint && s.map == w.map && s.root == w.root && s.decals == w.decals && s.owner == w.owner && s.rev == w.rev).unwrap_or(false);
                if same_bus {
                    if let Some(s) = self.shown.as_mut() {
                        s.look = w.clone();
                        let args = args_for(&w);
                        s.weather = load_weather(&args);
                        setup_sky(&args, renderer, &mut s.scene, omsi_content::Envir::load(&args.root.join("envir.cfg")).ok().as_ref(), Some(&s.weather));
                        s.lighting = lighting_for(&args, &s.weather);
                    }
                    self.dirty = true;
                } else {
                    self.start_loading(renderer, w);
                }
            }
        }
        self.busy = self.loading.is_some();
        // camera
        self.idle += dt;
        if self.auto_turn && self.idle > 4.0 {
            self.yaw_to += dt * 6.0;
        }
        let k = 1.0 - (-dt / 0.18).exp();
        // still turning: the picture must follow
        if (self.yaw_to - self.yaw).abs() > 0.05 || (self.pitch_to - self.pitch).abs() > 0.05 || (self.zoom_to - self.zoom).abs() > 0.001 {
            self.dirty = true;
        }
        self.yaw += (self.yaw_to - self.yaw) * k;
        self.pitch += (self.pitch_to - self.pitch) * k;
        self.zoom += (self.zoom_to - self.zoom) * k;
        if (self.aim_to - self.aim).length() > 0.002 {
            self.aim += (self.aim_to - self.aim) * k;
            self.dirty = true;
        }
        self.focus_now += (self.focus_x - self.focus_now) * (1.0 - (-dt / 0.35).exp());
        // the bus's own state, whenever the picture is drawn again
        let inside = self.inside;
        if let (true, Some(s)) = (self.dirty, self.shown.as_mut()) {
            if let (Some(v), Some(r)) = (s.vehicle.as_mut(), s.render.as_mut()) {
                player::sync_vehicle_transforms(renderer, &mut s.scene, v, r, &mut s.trailers, inside);
            }
            // (the stickers move with the part of the body they sit on: a door that opens)
            if let Some(v) = s.vehicle.as_ref() {
                for it in &s.live {
                    for (inst, host) in it.instances.iter().zip(&it.hosts) {
                        renderer.set_transform(&mut s.scene, *inst, v.position, v.mesh_local_transform(*host));
                    }
                }
            }
        }
        swapped
    }

    fn start_loading(&mut self, renderer: &Renderer, look: Look) {
        let args = args_for(&look);
        let root = look.root.clone();
        let map_cfg = omsi_cfg::resolve_path(&root, &look.map);
        let date = start_clock(&args).date_code();
        let t0 = std::time::Instant::now();
        let world = match scene::World::open(&root, &map_cfg, date) {
            Ok(w) => {
                log::info!("showroom: {} opened in {:.2} s", map_cfg.display(), t0.elapsed().as_secs_f64());
                Arc::new(w)
            }
            Err(e) => {
                self.error = Some(format!("{e:#}"));
                self.wanted = None;
                return;
            }
        };
        let prefetch = world.vehicle_prefetch(renderer);
        let (tx, rx) = channel();
        let l2 = look.clone();
        std::thread::spawn(move || {
            let r = (|| -> Result<Ready> {
                if l2.bus.is_empty() {
                    return Err(anyhow!("no bus"));
                }
                let path = player_bus_path(&root, &l2.bus)?;
                let mut vt0 = omsi_sim::VehicleType::load(&root, &path)?;
                // (the stickers: the editor's, or the ones saved for this bus or its model)
                match &l2.decals {
                    Some(list) => {
                        crate::decals::apply(&mut vt0, list);
                    }
                    None => crate::decals::apply_saved(&mut vt0, Some(l2.owner).filter(|o| *o != 0)),
                }
                // (a paint that mixes the repaints of the bus becomes a scheme of its own)
                omsi_sim::paint_mix::apply(&mut vt0, &l2.paint, None);
                let vt = Arc::new(vt0);
                let scheme = paint_scheme(&vt, Some(l2.paint.as_str()).filter(|p| !p.is_empty()));
                let host = omsi_sim::VehicleHost::new(start_clock(&args));
                let mut vehicle = omsi_sim::VehicleInstance::new(vt.clone(), host);
                if let Some(i) = scheme {
                    for (var, v) in vt.paint_schemes[i].set_vars.clone() {
                        vehicle.set_var(&var, v);
                    }
                }
                let mix = Some(l2.paint.as_str()).filter(|p| omsi_sim::paint_mix::is_mix(p)).zip(scheme);
                load_coupled_parts(&root, &mut vehicle, mix);
                for _ in 0..3 {
                    vehicle.update(1.0 / 30.0);
                }
                prefetch.prefetch(&vt, scheme);
                for t in &vehicle.trailers {
                    prefetch.prefetch(&t.ty, scheme.filter(|i| *i < t.ty.paint_schemes.len()));
                }
                Ok(Ready { look: l2, world, vt, vehicle, scheme })
            })();
            let _ = tx.send(r.map_err(|e| format!("{e:#}")));
        });
        self.loading = Some((look, rx));
    }

    fn place(&mut self, renderer: &Renderer, r: Ready) -> Shown {
        let t0 = std::time::Instant::now();
        let mut scene = renderer.new_scene();
        let args = args_for(&r.look);
        let weather = load_weather(&args);
        let envir = omsi_content::Envir::load(&args.root.join("envir.cfg")).ok();
        setup_sky(&args, renderer, &mut scene, envir.as_ref(), Some(&weather));
        add_floor(renderer, &mut scene);
        let world = r.world;
        let mut vehicle = r.vehicle;
        vehicle.position = DVec3::ZERO;
        vehicle.heading = 0.0;
        let render = world.add_vehicle(renderer, &mut scene, &r.vt, r.scheme);
        let trailers: Vec<scene::VehicleRender> = vehicle.trailers.iter().map(|t| world.add_vehicle_part(renderer, &mut scene, &t.ty, r.scheme.filter(|i| *i < t.ty.paint_schemes.len()), &render)).collect();
        vehicle.init_text_textures(&mut world.fonts.lock(), &|p| omsi_texture::decode_file(p).ok().map(|i| (i.width, i.height, i.rgba)));
        for t in vehicle.trailers.iter_mut() {
            t.init_text_textures(&mut world.fonts.lock(), &|p| omsi_texture::decode_file(p).ok().map(|i| (i.width, i.height, i.rgba)));
        }
        vehicle.update(1.0 / 30.0);
        // the bus's size from its bounding box (with the rear section behind it)
        let bb = r.vt.def.bounding_box.unwrap_or([2.5, 12.0, 3.0, 0.0, 0.0, 1.5]);
        let mut length = bb[1];
        let mut centre = glam::Vec3::new(bb[3], bb[4], bb[5]);
        // [ROLLBACK showroom-119] the whole group from the front of the first section to the back of the
        // last one (before: the length counted from the first one's origin as if that were its middle, which
        // put the pivot off the group's middle and cut the front off when the camera turned)
        let (mut width, mut height) = (bb[0], bb[2]);
        let front = bb[4] + bb[1] * 0.5;
        let mut rear = bb[4] - bb[1] * 0.5;
        for t in &vehicle.trailers {
            let tb = t.ty.def.bounding_box.unwrap_or([2.5, 8.0, 3.0, 0.0, 0.0, 1.5]);
            let behind = (t.position - vehicle.position).truncate().length() as f32;
            rear = rear.min(-behind + tb[4] - tb[1] * 0.5);
            width = width.max(tb[0]);
            height = height.max(tb[2]);
        }
        if !vehicle.trailers.is_empty() {
            centre.y = (front + rear) * 0.5;
            length = front - rear;
        }
        let lighting = lighting_for(&args, &weather);
        log::info!("showroom: {} ({} meshes, {:.1} m long) placed in {:.2} s", r.look.bus, render.instances.len(), length, t0.elapsed().as_secs_f64());
        Shown { look: r.look, scene, world: Some(world), vehicle: Some(vehicle), render: Some(render), trailers, centre, length, width, height, weather, lighting, live: Vec::new(), live_tex: Default::default(), live_free: Vec::new() }
    }

    /// The picture of the bus at `w` x `h` pixels, drawn again when something changed.
    pub fn preview(&mut self, renderer: &mut Renderer, w: u32, h: u32) -> Option<wgpu::TextureView> {
        self.shown.as_ref()?;
        let (w, h) = (w.max(16), h.max(16));
        if self.target.as_ref().map(|t| (t.2, t.3) != (w, h)).unwrap_or(true) {
            let tex = renderer.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("bus preview"),
                size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: renderer.format(),
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let view = tex.create_view(&Default::default());
            self.target = Some((tex, view, w, h));
            self.generation += 1;
            self.dirty = true;
        }
        if self.dirty {
            self.dirty = false;
            let view = self.target.as_ref().unwrap().1.clone();
            self.render(renderer, &view, w, h);
            self.cam_preview = self.last_cam;
        }
        self.target.as_ref().map(|t| t.1.clone())
    }

    fn render(&mut self, renderer: &mut Renderer, target: &wgpu::TextureView, w: u32, h: u32) {
        let (yaw, pitch, zoom, focus) = (self.yaw, self.pitch, self.zoom, self.focus_now);
        let aim = self.aim;
        let Some(s) = self.shown.as_mut() else { return };
        // (the fit of the bus is worked out for 30 degrees; the picture itself uses the editor's lens)
        let fov = 30.0f32;
        let lens = self.fov;
        let free_cam = self.free;
        let aspect = w as f32 / h.max(1) as f32;
        // far enough that the whole bus fits the free part of the window
        let half_v = (fov.to_radians() * 0.5).tan();
        // (the free part is the share of the window right of the panels: twice the room
        // from the focus to the right edge)
        let free = ((1.0 - focus) * 2.0).clamp(0.3, 1.0);
        // [ROLLBACK showroom-119] far enough that the sphere round the whole vehicle (about the point the camera
        // turns on) fits the free part of the picture from every side - before: the length alone, with a metre
        // to spare, so a three-quarter view put the nearer end past the edge. The distance does not change
        // as the camera turns.
        let (sx, sz) = (s.width * 0.5, s.height * 0.5 + s.centre.z * 0.25);
        let radius = ((s.length * 0.5).powi(2) + sx * sx + sz * sz).sqrt() + 0.5;
        let half_h = (half_v * aspect * free).atan();
        let half_limit = half_h.min(half_v.atan()).max(0.05);
        let fit = radius / half_limit.sin();
        let (sy, cy) = yaw.to_radians().sin_cos();
        let (sp, cp) = pitch.to_radians().sin_cos();
        let mut dist = (fit * 1.04 * zoom).max(self.min_dist);
        let mut target_pt = DVec3::new(s.centre.x as f64, s.centre.y as f64, (s.centre.z * 0.75) as f64);
        if self.studio {
            // the box of the whole vehicle, looked at through the picture: the distance at which
            // its eight corners just fit (a margin of 6 % round it)
            let fwd = glam::Vec3::new(sy * cp, cy * cp, -sp);
            let right = glam::Vec3::new(cy, -sy, 0.0);
            let up = right.cross(fwd);
            let (tan_v, tan_h) = (half_v * 0.94, half_v * aspect * 0.94);
            let mut need = 0.0f32;
            for ix in [-0.5f32, 0.5] {
                for iy in [-0.5f32, 0.5] {
                    for iz in [-0.5f32, 0.5] {
                        let o = glam::Vec3::new(ix * s.width, iy * s.length, iz * s.height);
                        let (x, y, z) = (o.dot(right), o.dot(up), o.dot(fwd));
                        need = need.max(x.abs() / tan_h - z).max(y.abs() / tan_v - z);
                    }
                }
            }
            dist = (need * zoom * self.fit_scale).max(3.0);
            target_pt = DVec3::new(s.centre.x as f64, s.centre.y as f64, s.centre.z as f64);
            // (the aim moved by what the measuring found: the real bus is not its bounding box)
            let kx = 2.0 * dist * half_v * aspect * self.fit_shift.0;
            let ky = 2.0 * dist * half_v * self.fit_shift.1;
            target_pt += DVec3::new((right.x * kx + up.x * ky) as f64, (right.y * kx + up.y * ky) as f64, (right.z * kx + up.z * ky) as f64);
        }
        target_pt += DVec3::new(aim.x as f64, aim.y as f64, aim.z as f64);
        // from the camera towards the bus: forward along the yaw, down by the pitch
        let dir = DVec3::new((sy * cp) as f64, (cy * cp) as f64, -sp as f64);
        let mut pos = target_pt - dir * dist as f64;
        self.cam_dist = dist;
        pos.z = pos.z.max(0.6);
        // the bus stands in the free part of the window: the camera looks past it to the
        // side by as much
        let side = (focus - 0.5) * 2.0 * half_v * aspect;
        let mut look_yaw = yaw - side.atan().to_degrees();
        let mut look_pitch = pitch;
        if let Some(f) = free_cam {
            pos = f.pos;
            look_yaw = f.yaw;
            look_pitch = f.pitch;
        }
        let half_v = (lens.to_radians() * 0.5).tan();
        let cam = Camera { position: pos, yaw: look_yaw, pitch: -look_pitch, roll: 0.0, fov_deg: lens, near: if free_cam.is_some() { 0.05 } else { 0.2 }, far: 6000.0 };
        self.last_cam = Some(CamInfo { pos, yaw: look_yaw, pitch: look_pitch, half_v, aspect });
        s.scene.overlays.clear();
        let _ = &s.weather;
        if self.studio {
            // the sun a little left of straight behind the camera and high: the bus's front and
            // side facing the lens are lit, and its shadow falls away from the lens
            let a = (look_yaw + 28.0).to_radians();
            let el = 46.0f32.to_radians();
            let mut l = s.lighting.clone();
            l.sun_dir = glam::Vec3::new(-a.sin() * el.cos(), -a.cos() * el.cos(), el.sin());
            l.sun_color = l.sun_color.max(glam::Vec3::splat(0.85));
            l.sun_intensity = l.sun_intensity.max(1.0);
            l.sun_azimuth = (-a.sin()).atan2(-a.cos()).rem_euclid(std::f32::consts::TAU);
            l.shadows = !self.measure;
            l.night = 0.0;
            renderer.render(&mut s.scene, target, w, h, &cam, &l);
        } else {
            renderer.render(&mut s.scene, target, w, h, &cam, &s.lighting);
        }
    }

    /// Show these stickers over the bus (the decal editor's): the ones that did not change
    /// stay as they are, the others are made again from the bus's meshes, which is a matter
    /// of milliseconds - the bus is not read again.
    pub fn set_live(&mut self, renderer: &Renderer, list: &[crate::decals::Decal]) {
        let Some(s) = self.shown.as_mut() else { return };
        let Some(v) = s.vehicle.as_ref() else { return };
        let mut changed = false;
        for (i, d) in list.iter().enumerate() {
            if s.live.get(i).is_some_and(|it| it.decal == *d) {
                continue;
            }
            if i < s.live.len() {
                let old = std::mem::replace(&mut s.live[i], LiveItem { decal: d.clone(), instances: Vec::new(), hosts: Vec::new(), meshes: Vec::new(), materials: Vec::new() });
                free_live(renderer, &mut s.scene, &mut s.live_free, old);
            }
            let item = build_live(renderer, &mut s.scene, &mut s.live_tex, &mut s.live_free, v, d);
            if i < s.live.len() {
                s.live[i] = item;
            } else {
                s.live.push(item);
            }
            changed = true;
        }
        while s.live.len() > list.len() {
            if let Some(old) = s.live.pop() {
                free_live(renderer, &mut s.scene, &mut s.live_free, old);
                changed = true;
            }
        }
        if s.live_tex.len() > 80 {
            // (the pictures of texts typed and dropped: let go of them when the layer is empty)
            if s.live.is_empty() {
                for (_, t) in s.live_tex.drain() {
                    renderer.free_texture(&mut s.scene, t);
                }
            }
        }
        if changed {
            self.dirty = true;
        }
    }

    /// The bus file and paint of what is in the scene (loaded and placed).
    pub fn shown_bus(&self) -> Option<(String, String)> {
        let s = self.shown.as_ref()?;
        Some((s.look.bus.clone(), s.look.paint.clone()))
    }

    /// The repaints of the bus shown, and its texture slots with the repaints that give each
    /// one a texture (the Paint tab of the livery editor offers these).
    pub fn paint_options(&self) -> Option<(Vec<String>, Vec<(String, Vec<String>)>)> {
        let v = self.shown.as_ref()?.vehicle.as_ref()?;
        Some((omsi_sim::paint_mix::repaints(&v.ty), omsi_sim::paint_mix::slots(&v.ty)))
    }

    /// The `[model]` line as written (what the decal files are keyed by).
    pub fn model_line(&self) -> Option<String> {
        let v = self.shown.as_ref()?.vehicle.as_ref()?;
        Some(v.ty.def.model.clone().unwrap_or_default())
    }

    /// The place on the shown bus under a point of the preview picture: `ndc` is the point in
    /// the picture, -1..1 across and up (0, 0 at its middle).
    pub fn pick_at(&self, ndc: glam::Vec2, glass: bool) -> Option<crate::decals::Hit> {
        let cam = self.cam_preview?;
        let v = self.shown.as_ref()?.vehicle.as_ref()?;
        let (right, up, fwd) = cam.axes();
        let dir = fwd + right * (ndc.x * cam.half_v * cam.aspect) + up * (ndc.y * cam.half_v);
        let origin = (cam.pos - v.position).as_vec3();
        let inside = self.inside;
        let shown = |i: usize| {
            let on = v.mesh_props.get(i).map(|p| p.visible).unwrap_or(true);
            let vp = v.ty.meshes.get(i).and_then(|m| v.ty.model.meshes.get(m.def_index)).map(|d| d.viewpoint).unwrap_or(0);
            on && (vp == 0 || (inside && vp & 2 != 0) || (!inside && vp & 1 != 0))
        };
        crate::decals::pick(&v.ty, &|i| v.mesh_local_transform(i), &shown, origin, dir, glass)
    }

    /// Where a point of the bus (its frame) is in the preview picture: -1..1 across and up.
    pub fn project(&self, p: glam::Vec3) -> Option<glam::Vec2> {
        let cam = self.cam_preview?;
        let v = self.shown.as_ref()?.vehicle.as_ref()?;
        let (right, up, fwd) = cam.axes();
        let rel = (v.position + p.as_dvec3() - cam.pos).as_vec3();
        let z = rel.dot(fwd);
        if z < 0.1 {
            return None;
        }
        Some(glam::Vec2::new(rel.dot(right) / (z * cam.half_v * cam.aspect), rel.dot(up) / (z * cam.half_v)))
    }

    /// How many pixels a metre is at this point of the bus, in a preview picture `view_h`
    /// pixels high (facing the camera).
    pub fn px_per_metre(&self, p: glam::Vec3, view_h: f32) -> Option<f32> {
        let cam = self.cam_preview?;
        let v = self.shown.as_ref()?.vehicle.as_ref()?;
        let (_, _, fwd) = cam.axes();
        let z = (v.position + p.as_dvec3() - cam.pos).as_vec3().dot(fwd);
        if z < 0.1 {
            return None;
        }
        Some(view_h / (2.0 * z * cam.half_v))
    }

    /// A bus is there to show.
    pub fn has_picture(&self) -> bool {
        self.shown.as_ref().map(|s| s.vehicle.is_some()).unwrap_or(false) && self.target.is_some()
    }

    /// Look from this angle (degrees) and at this distance factor at once, with no easing:
    /// the thumbnail cache wants the same picture of every bus.
    pub fn set_view(&mut self, yaw: f32, pitch: f32, zoom: f32) {
        self.yaw = yaw;
        self.yaw_to = yaw;
        self.pitch = pitch;
        self.pitch_to = pitch;
        self.zoom = zoom;
        self.zoom_to = zoom;
        self.dirty = true;
    }

    /// This look is the one in the scene, loaded and placed.
    pub fn is_shown(&self, look: &Look) -> bool {
        self.loading.is_none() && self.shown.as_ref().is_some_and(|s| &s.look == look)
    }

    /// This look could not be shown (the bus would not read).
    pub fn has_failed(&self, look: &Look) -> bool {
        self.failed.as_ref() == Some(look)
    }

    /// A still of the shown bus, `w` x `h` pixels, drawn into a texture of its own and read
    /// back (the thumbnail cache keeps it as a PNG). None when nothing is shown.
    pub fn capture(&mut self, renderer: &mut Renderer, w: u32, h: u32) -> Option<image::RgbaImage> {
        self.shown.as_ref()?;
        Some(self.grab(renderer, w, h))
    }

    /// The same, with the bus in the middle of the picture and filling it: the studio camera
    /// first looks at the bounding box from the file, then the picture is measured (the bus
    /// against the empty studio) and the aim and the distance are corrected a few times.
    pub fn capture_fitted(&mut self, renderer: &mut Renderer, w: u32, h: u32) -> Option<image::RgbaImage> {
        self.shown.as_ref()?;
        self.fit_shift = (0.0, 0.0);
        self.fit_scale = 1.0;
        if self.studio {
            let (mw, mh) = (320u32, 180u32);
            let ids: Vec<usize> = {
                let s = self.shown.as_ref()?;
                let mut v: Vec<usize> = s.render.as_ref().map(|r| r.instances.clone()).unwrap_or_default();
                for t in &s.trailers {
                    v.extend(t.instances.iter().copied());
                }
                v
            };
            for _ in 0..4 {
                self.measure = true;
                let bus = self.grab(renderer, mw, mh);
                // the studio without the bus
                let saved: Vec<(usize, Vec<f32>, bool, Vec<[f32; 2]>)> = {
                    let s = self.shown.as_mut()?;
                    let mut out = Vec::new();
                    for &i in &ids {
                        if let Some(inst) = s.scene.instances.get(i) {
                            out.push((i, inst.slot_alpha.clone(), inst.visible, inst.slot_uv.clone()));
                        }
                    }
                    for (i, a, _, uv) in &out {
                        renderer.set_params(&mut s.scene, *i, a, false, uv);
                    }
                    out
                };
                let empty = self.grab(renderer, mw, mh);
                if let Some(s) = self.shown.as_mut() {
                    for (i, a, vis, uv) in &saved {
                        renderer.set_params(&mut s.scene, *i, a, *vis, uv);
                    }
                }
                self.measure = false;
                // the box round what differs
                let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0u32, 0u32);
                for y in 0..mh {
                    for x in 0..mw {
                        let (a, b) = (bus.get_pixel(x, y).0, empty.get_pixel(x, y).0);
                        let d = (a[0] as i32 - b[0] as i32).abs() + (a[1] as i32 - b[1] as i32).abs() + (a[2] as i32 - b[2] as i32).abs();
                        if d > 14 {
                            x0 = x0.min(x);
                            y0 = y0.min(y);
                            x1 = x1.max(x);
                            y1 = y1.max(y);
                        }
                    }
                }
                if x1 <= x0 || y1 <= y0 {
                    break;
                }
                let cx = (x0 + x1 + 1) as f32 * 0.5 / mw as f32;
                let cy = (y0 + y1 + 1) as f32 * 0.5 / mh as f32;
                let bw = (x1 - x0 + 1) as f32 / mw as f32;
                let bh = (y1 - y0 + 1) as f32 / mh as f32;
                // (the bus fills nine tenths of the width or the height)
                let r = (bw / 0.9).max(bh / 0.9).clamp(0.4, 2.5);
                let off = ((cx - 0.5).abs(), (cy - 0.5).abs());
                if (r - 1.0).abs() < 0.03 && off.0 < 0.012 && off.1 < 0.02 {
                    break;
                }
                self.fit_shift.0 += cx - 0.5;
                self.fit_shift.1 -= cy - 0.5;
                self.fit_scale = (self.fit_scale * r).clamp(0.3, 3.0);
            }
            self.measure = false;
        }
        Some(self.grab(renderer, w, h))
    }

    /// Draw the shown scene into a texture of its own and read it back.
    fn grab(&mut self, renderer: &mut Renderer, w: u32, h: u32) -> image::RgbaImage {
        let (w, h) = (w.max(16), h.max(16));
        let tex = renderer.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("bus thumbnail"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: renderer.format(),
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        self.render(renderer, &view, w, h);
        let stride = (w * 4).div_ceil(256) * 256;
        let buf = renderer.device.create_buffer(&wgpu::BufferDescriptor { label: None, size: (stride * h) as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
        let mut enc = renderer.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            tex.as_image_copy(),
            wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(stride), rows_per_image: None } },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        renderer.queue.submit([enc.finish()]);
        buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        renderer.device.poll(wgpu::PollType::wait_indefinitely()).ok();
        let data = buf.slice(..).get_mapped_range();
        let bgra = matches!(renderer.format(), wgpu::TextureFormat::Bgra8UnormSrgb | wgpu::TextureFormat::Bgra8Unorm);
        let mut img = image::RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let i = (y * stride + x * 4) as usize;
                let (r, g, b) = if bgra { (data[i + 2], data[i + 1], data[i]) } else { (data[i], data[i + 1], data[i + 2]) };
                img.put_pixel(x, y, image::Rgba([r, g, b, 255]));
            }
        }
        img
    }

}

/// Let go of a sticker of the live layer (its instances wait to be used again).
fn free_live(renderer: &Renderer, scene: &mut Scene, free: &mut Vec<usize>, item: LiveItem) {
    for i in item.instances {
        renderer.remove_instance(scene, i);
        free.push(i);
    }
    for m in item.meshes {
        renderer.free_mesh(scene, m);
    }
    for m in item.materials {
        renderer.free_material(scene, m);
    }
}

/// The meshes of a sticker over the bus, in the scene.
fn build_live(renderer: &Renderer, scene: &mut Scene, tex_cache: &mut std::collections::HashMap<String, usize>, free: &mut Vec<usize>, v: &omsi_sim::VehicleInstance, d: &crate::decals::Decal) -> LiveItem {
    let mut item = LiveItem { decal: d.clone(), instances: Vec::new(), hosts: Vec::new(), meshes: Vec::new(), materials: Vec::new() };
    if let crate::decals::Kind::Object { asset } = &d.kind {
        return build_live_object(renderer, scene, tex_cache, free, v, d, asset, item);
    }
    let Some((name, aspect)) = crate::decals::texture(d) else { return item };
    let tex = match tex_cache.get(&name) {
        Some(t) => *t,
        None => {
            let Ok(img) = omsi_texture::decode_file(&crate::decals::cache_file(&name)) else { return item };
            let t = renderer.add_texture(scene, &img, true);
            tex_cache.insert(name.clone(), t);
            t
        }
    };
    let parts = crate::decals::overlay_meshes(&v.ty, d, aspect);
    if parts.is_empty() {
        return item;
    }
    let extra = omsi_render::MaterialExtra { no_z_write: true, z_bias: 16, ..Default::default() };
    let mat = renderer.add_material_extra(scene, Some(tex), omsi_render::AlphaMode::Blend, [1.0; 4], false, None, None, None, None, [0.0; 3], extra);
    for (i, data) in parts {
        let mesh = renderer.add_mesh(scene, &data);
        let mut inst = renderer.add_instance(scene, mesh, v.position, v.mesh_local_transform(i), vec![mat]);
        if let Some(old) = free.pop() {
            let now = renderer.recycle_instance(scene, inst, old);
            if now == old {
                inst = now;
            } else {
                free.push(old);
            }
        }
        item.meshes.push(mesh);
        item.instances.push(inst);
        item.hosts.push(i);
    }
    item.materials.push(mat);
    item
}

/// A placed object (an asset) over the bus: every material of the model, the mesh as it
/// stands on its host.
#[allow(clippy::too_many_arguments)]
fn build_live_object(renderer: &Renderer, scene: &mut Scene, tex_cache: &mut std::collections::HashMap<String, usize>, free: &mut Vec<usize>, v: &omsi_sim::VehicleInstance, d: &crate::decals::Decal, asset: &str, mut item: LiveItem) -> LiveItem {
    let Some(p) = crate::assets::prepare(asset) else { return item };
    let Some(host) = crate::decals::object_host(&v.ty, d) else { return item };
    let mut mats = Vec::new();
    for (slot, m) in p.materials.iter().enumerate() {
        let tex = if m.texture.is_empty() {
            None
        } else if let Some(t) = tex_cache.get(&m.texture) {
            Some(*t)
        } else if let Ok(img) = omsi_texture::decode_file(&crate::decals::cache_file(&m.texture)) {
            let t = renderer.add_texture(scene, &img, true);
            tex_cache.insert(m.texture.clone(), t);
            Some(t)
        } else {
            None
        };
        let color = if tex.is_some() { [1.0; 4] } else { [m.diffuse[0], m.diffuse[1], m.diffuse[2], 1.0] };
        // (the preview shows the picture and the alpha mode; the rest of the [matl] blocks -
        // reflections, bump, night maps - shows in the game)
        let (alpha, no_z_write) = p.alpha_of(slot);
        let id = match alpha {
            2 => {
                let extra = omsi_render::MaterialExtra { no_z_write: true, ..Default::default() };
                renderer.add_material_extra(scene, tex, omsi_render::AlphaMode::Blend, color, false, None, None, None, None, [0.0; 3], extra)
            }
            1 => renderer.add_material(scene, tex, omsi_render::AlphaMode::Test, color, false),
            _ if no_z_write => {
                let extra = omsi_render::MaterialExtra { no_z_write: true, ..Default::default() };
                renderer.add_material_extra(scene, tex, omsi_render::AlphaMode::Opaque, color, false, None, None, None, None, [0.0; 3], extra)
            }
            _ => renderer.add_material(scene, tex, omsi_render::AlphaMode::Opaque, color, false),
        };
        mats.push(id);
    }
    let data = crate::decals::place_object(&p, d);
    let mesh = renderer.add_mesh(scene, &data);
    let mut inst = renderer.add_instance(scene, mesh, v.position, v.mesh_local_transform(host), mats.clone());
    if let Some(old) = free.pop() {
        let now = renderer.recycle_instance(scene, inst, old);
        if now == old {
            inst = now;
        } else {
            free.push(old);
        }
    }
    item.meshes.push(mesh);
    item.instances.push(inst);
    item.hosts.push(host);
    item.materials = mats;
    item
}

/// The light of the look's time and weather, with the sun's shadow under the bus. Always
/// the plain renderer, whatever the game's graphics setting: a preview is to be quick and
/// clear, not the game's picture (no enhanced exposure and glow, no weather effects).
fn lighting_for(args: &Args, weather: &omsi_content::weather::Weather) -> Lighting {
    let clock = start_clock(args);
    let envir = omsi_content::Envir::load(&args.root.join("envir.cfg")).ok();
    let daylight = omsi_sim::Daylight::compute(&clock, envir.as_ref());
    let mut l = weather_lighting(&daylight, weather, crate::weather_setup::cloud_drift_at(weather, clock.time), 0.0, true);
    l.shadows = daylight.altitude_deg > 2.0;
    l.enhanced = false;
    l.classic = false;
    l.detail = false;
    l.wetness = 0.0;
    l.snow = 0.0;
    l.fog_density = 0.0;
    l
}

/// A round showroom floor under the bus: dark, matt, catching its shadow.
fn add_floor(renderer: &Renderer, scene: &mut Scene) {
    let n = 96;
    let r = 400.0f32;
    let mut positions = vec![glam::Vec3::ZERO];
    let mut normals = vec![glam::Vec3::Z];
    let mut uvs = vec![glam::Vec2::ZERO];
    for k in 0..n {
        let a = std::f32::consts::TAU * k as f32 / n as f32;
        positions.push(glam::Vec3::new(a.cos() * r, a.sin() * r, 0.0));
        normals.push(glam::Vec3::Z);
        uvs.push(glam::Vec2::new(a.cos(), a.sin()));
    }
    let mut indices = Vec::new();
    for k in 0..n {
        indices.extend([0u32, 1 + ((k + 1) % n) as u32, 1 + k as u32]);
    }
    let data = omsi_geometry::MeshData { positions, normals, uvs, ranges: vec![(0, indices.len() as u32, 0)], indices, one_sided: false };
    let mesh = renderer.add_mesh(scene, &data);
    let mat = renderer.add_material(scene, None, omsi_render::AlphaMode::Opaque, [0.12, 0.125, 0.135, 1.0], false);
    renderer.add_instance(scene, mesh, DVec3::new(0.0, 0.0, -0.005), glam::Mat4::IDENTITY, vec![mat]);
}
