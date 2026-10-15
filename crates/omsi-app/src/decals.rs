//! Decals (stickers) on a bus: a picture, or a text typed in, put on the body where the player
//! clicked it in the livery editor's preview.
//!
//! A decal is not painted into a texture: the part of the body's surface under the sticker is
//! copied (`project`: the triangles inside a box standing on the clicked spot, cut at the
//! box's sides), lifted off the body by 3 mm and given the sticker's picture alone as an
//! alpha-blended material (`VehicleType::add_overlay`). So it works with every repaint (the
//! body's texture is not touched, nor is any file of a mod), it keeps its real size in
//! metres on every bus of the model, and a sticker on a door opens with the door.
//!
//! The pictures are written to the launcher's own folder (`<data>/decals/cache`, named by a
//! hash of what they show), which the vehicles look in for textures last
//! (`omsi_sim::vehicle::set_extra_texture_dir`).
//!
//! What is kept (`<data>/decals`): `bus_<id>.json` the stickers of one bus of the company's
//! fleet; `model_<name>.json` the stickers of every bus of a model (the `[model]` line of the
//! `.bus` file: all the variants and repaints of a vehicle share it, and so do their
//! coordinates); `presets/<name>.json` named sets to put on another bus of the model;
//! `images/` the pictures the player imported. A bus's own file wins over its model's.

use glam::{Mat4, Vec2, Vec3};
use omsi_geometry::MeshData;
use omsi_sim::VehicleType;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// The name every decal texture starts with (a mesh whose material has it is a decal).
pub const PREFIX: &str = "openomsi_decal_";
/// How far a sticker's surface lies above the body (m).
const LIFT: f32 = 0.003;
/// How far in front of and behind the clicked spot's plane the body is copied (m).
const DEPTH: f32 = 0.12;
/// How much a triangle has to face the way the sticker does (cosine).
const FACING: f32 = 0.35;
/// The widest a decal's picture is made (pixels).
const MAX_PX: u32 = 2048;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Kind {
    /// A picture of the player's (a file of the `images` folder).
    Image { file: String },
    /// Lettering: lines separated by `\n`, in Roboto or a font of the system.
    Text {
        text: String,
        #[serde(default)]
        bold: bool,
        #[serde(default)]
        italic: bool,
        #[serde(default = "white")]
        color: [u8; 3],
        /// A font file of the system ("" = Roboto, the launcher's own).
        #[serde(default)]
        font: String,
    },
    /// A 3D object of an installed asset (`assets`): `width` is its longest side in metres,
    /// its +z stands off the surface and `turn` turns it about that.
    Object { asset: String },
}

fn white() -> [u8; 3] {
    [255, 255, 255]
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Decal {
    pub kind: Kind,
    /// The spot on the body, in the vehicle's frame (x right, y forward, z up; m).
    pub pos: [f32; 3],
    /// Which way the surface there faces (outwards).
    pub normal: [f32; 3],
    /// Width of the sticker (m); its height follows its picture.
    pub width: f32,
    /// Turned this far (degrees, counter-clockwise as the viewer sees it).
    #[serde(default)]
    pub turn: f32,
    #[serde(default)]
    pub mirror: bool,
    /// On a window (a pane of glass takes it) rather than on the solid body.
    #[serde(default)]
    pub glass: bool,
    /// An object's turn on top of the position the surface gives it (a quaternion: x, y, z,
    /// w; seen in the surface's own axes, see [`Frame`]).
    #[serde(default = "no_turn")]
    pub rot: [f32; 4],
    /// An object's mesh of the bus it stands on (so it can be moved off the surface and
    /// still move with its door). None: the surface under it.
    #[serde(default)]
    pub host: Option<u32>,
}

fn no_turn() -> [f32; 4] {
    [0.0, 0.0, 0.0, 1.0]
}

impl Decal {
    pub fn text(text: &str, font: &str, pos: Vec3, normal: Vec3) -> Decal {
        let kind = Kind::Text { text: text.to_string(), bold: true, italic: false, color: white(), font: font.to_string() };
        let mut d = Decal { kind, pos: pos.to_array(), normal: normal.to_array(), width: 0.5, turn: 0.0, mirror: false, glass: false, rot: no_turn(), host: None };
        d.width = d.default_width();
        d
    }

    pub fn image(file: &str, pos: Vec3, normal: Vec3) -> Decal {
        let kind = Kind::Image { file: file.to_string() };
        let mut d = Decal { kind, pos: pos.to_array(), normal: normal.to_array(), width: 0.5, turn: 0.0, mirror: false, glass: false, rot: no_turn(), host: None };
        d.width = d.default_width();
        d
    }

    /// An asset put at a spot, as it was modelled (`width` is its scale, 1 = the model's own
    /// size).
    pub fn object(asset: &str, pos: Vec3, normal: Vec3) -> Decal {
        let kind = Kind::Object { asset: asset.to_string() };
        let mut d = Decal { kind, pos: pos.to_array(), normal: normal.to_array(), width: 1.0, turn: 0.0, mirror: false, glass: false, rot: no_turn(), host: None };
        d.width = d.default_width();
        d
    }

    pub fn is_object(&self) -> bool {
        matches!(self.kind, Kind::Object { .. })
    }

    /// An object's turn: as modelled (the model's axes are the bus's axes) unless turned.
    pub fn object_matrix(&self) -> glam::Mat3 {
        let q = glam::Quat::from_array(self.rot);
        let q = if q.length_squared() > 1e-6 { q.normalize() } else { glam::Quat::IDENTITY };
        glam::Mat3::from_quat(q)
    }

    /// Turn an object by `angle` radians about a world axis (right-handed).
    pub fn turn_about(&mut self, axis: Vec3, angle: f32) {
        let Some(axis) = axis.try_normalize() else { return };
        let q = glam::Quat::from_array(self.rot);
        let q = if q.length_squared() > 1e-6 { q.normalize() } else { glam::Quat::IDENTITY };
        self.rot = (glam::Quat::from_axis_angle(axis, angle) * q).normalize().to_array();
    }

    /// Stand an object on the surface it is on: its +z away from it, its +y the picture's top.
    pub fn stand_on_surface(&mut self) {
        let f = Frame::new(self, 1.0);
        self.rot = glam::Quat::from_mat3(&glam::Mat3::from_cols(f.right, f.up, f.n)).normalize().to_array();
    }

    /// A text sticker's width for lettering about 18 cm tall; a picture's 50 cm.
    pub fn default_width(&self) -> f32 {
        match &self.kind {
            Kind::Text { .. } => match texture(self) {
                Some((_, aspect)) => (0.18 * aspect).clamp(0.2, 3.0),
                None => 0.5,
            },
            Kind::Image { .. } => 0.5,
            Kind::Object { .. } => 1.0,
        }
    }

    /// A short name for a list.
    pub fn label(&self) -> String {
        match &self.kind {
            Kind::Text { text, .. } => format!("Text: {}", text.replace('\n', " / ")),
            Kind::Image { file } => format!("Image: {file}"),
            Kind::Object { asset } => format!("Object: {}", crate::assets::find(asset).map(|a| a.name).unwrap_or_else(|| asset.clone())),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// where things are kept

pub fn dir() -> PathBuf {
    let d = omsi_launcher_lib::data_dir().join("decals");
    let _ = std::fs::create_dir_all(&d);
    d
}

pub fn images_dir() -> PathBuf {
    let d = dir().join("images");
    let _ = std::fs::create_dir_all(&d);
    d
}

fn cache_dir() -> PathBuf {
    let d = dir().join("cache");
    let _ = std::fs::create_dir_all(&d);
    d
}

fn presets_dir() -> PathBuf {
    let d = dir().join("presets");
    let _ = std::fs::create_dir_all(&d);
    d
}

/// Bumped whenever a set was saved: a showroom showing the bus reads its stickers again.
static REVISION: AtomicU32 = AtomicU32::new(0);

pub fn revision() -> u32 {
    REVISION.load(Ordering::Relaxed)
}

/// What a model is called here: its `[model]` line, lower case with slashes forward.
pub fn model_key(model: &str) -> String {
    model.trim().replace('\\', "/").to_ascii_lowercase()
}

/// A file-name-safe form of a name.
fn safe(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    let s: String = s.chars().take(80).collect();
    if s.is_empty() { "_".to_string() } else { s }
}

#[derive(Serialize, Deserialize, Default)]
struct File {
    #[serde(default)]
    model: String,
    #[serde(default)]
    decals: Vec<Decal>,
}

fn read(path: &Path) -> Option<File> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

fn write(path: &Path, f: &File) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(f).map_err(std::io::Error::other)?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)?;
    REVISION.fetch_add(1, Ordering::Relaxed);
    Ok(())
}

fn bus_path(id: u32) -> PathBuf {
    dir().join(format!("bus_{id}.json"))
}

fn model_path(model: &str) -> PathBuf {
    dir().join(format!("model_{}.json", safe(model)))
}

/// The stickers a bus wears: its own set when it has one (even an empty one: "none"), else
/// its model's.
pub fn load_for(bus_id: Option<u32>, model: &str) -> Vec<Decal> {
    if let Some(f) = bus_id.and_then(|id| read(&bus_path(id))) {
        return f.decals;
    }
    read(&model_path(&model_key(model))).map(|f| f.decals).unwrap_or_default()
}

/// Whether the bus has a set of its own.
pub fn has_own(bus_id: u32) -> bool {
    bus_path(bus_id).exists()
}

/// The set of one bus only.
pub fn save_bus(bus_id: u32, model: &str, decals: &[Decal]) -> std::io::Result<()> {
    write(&bus_path(bus_id), &File { model: model_key(model), decals: decals.to_vec() })
}

/// The set of every bus of the model (and the bus's own set, if it has one, is forgotten
/// by the caller with [`forget_bus`]).
pub fn save_model(model: &str, decals: &[Decal]) -> std::io::Result<()> {
    write(&model_path(&model_key(model)), &File { model: model_key(model), decals: decals.to_vec() })
}

/// The bus wears its model's set again.
pub fn forget_bus(bus_id: u32) {
    let _ = std::fs::remove_file(bus_path(bus_id));
    REVISION.fetch_add(1, Ordering::Relaxed);
}

/// A named set, to be put on another bus of the model.
pub fn save_preset(name: &str, model: &str, decals: &[Decal]) -> std::io::Result<()> {
    write(&presets_dir().join(format!("{}.json", safe(name))), &File { model: model_key(model), decals: decals.to_vec() })
}

/// The names of the saved presets of a model, with their sets.
pub fn presets_for(model: &str) -> Vec<(String, Vec<Decal>)> {
    let key = model_key(model);
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(presets_dir()) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("json") {
                continue;
            }
            if let Some(f) = read(&p).filter(|f| f.model == key) {
                let name = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                out.push((name, f.decals));
            }
        }
    }
    out.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    out
}

pub fn delete_preset(name: &str) {
    let _ = std::fs::remove_file(presets_dir().join(format!("{}.json", safe(name))));
}

/// The imported pictures (file names), sorted.
pub fn image_files() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(images_dir())
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| e.file_name().to_str().map(str::to_string))
                .filter(|n| is_image(n))
                .collect()
        })
        .unwrap_or_default();
    v.sort_by_key(|n| n.to_lowercase());
    v
}

fn is_image(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    ["png", "jpg", "jpeg", "bmp", "tga"].iter().any(|e| n.ends_with(&format!(".{e}")))
}

/// Copy a picture of the player's into the `images` folder, cut down to `max_side` pixels
/// on its longer side when it is bigger (0: as it is). A different file of the same name is
/// not overwritten: it gets a number. Returns the name it is kept under.
pub fn import_image(from: &Path, max_side: u32) -> Result<String, String> {
    let name = from.file_name().and_then(|n| n.to_str()).ok_or("No file name")?.to_string();
    if !is_image(&name) {
        return Err("Not a picture (png, jpg, bmp, tga)".to_string());
    }
    let img = image::open(from).map_err(|e| format!("The picture cannot be read: {e}"))?;
    let (w, h) = (img.width(), img.height());
    let resize = max_side > 0 && w.max(h) > max_side;
    let dir = images_dir();
    let stem = name.rsplit_once('.').map(|(s, _)| s.to_string()).unwrap_or_else(|| name.clone());
    let ext = if resize { "png".to_string() } else { name.rsplit_once('.').map(|(_, e)| e.to_string()).unwrap_or_else(|| "png".to_string()) };
    let base = if resize { format!("{stem}_{max_side}") } else { stem };
    let mut target = format!("{base}.{ext}");
    let mut n = 1;
    while dir.join(&target).exists() {
        if !resize && std::fs::read(dir.join(&target)).ok() == std::fs::read(from).ok() {
            return Ok(target);
        }
        n += 1;
        target = format!("{base}_{n}.{ext}");
    }
    if resize {
        let k = max_side as f32 / w.max(h) as f32;
        let small = image::imageops::resize(&img.to_rgba8(), ((w as f32 * k).round() as u32).max(1), ((h as f32 * k).round() as u32).max(1), image::imageops::FilterType::Lanczos3);
        small.save(dir.join(&target)).map_err(|e| format!("The picture cannot be saved: {e}"))?;
    } else {
        std::fs::copy(from, dir.join(&target)).map_err(|e| format!("The picture cannot be copied: {e}"))?;
    }
    Ok(target)
}

// ---------------------------------------------------------------------------------------------
// the pictures

fn fnv(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// The picture of a decal, drawn.
fn draw(d: &Decal) -> Option<image::RgbaImage> {
    match &d.kind {
        Kind::Image { file } => {
            let img = image::open(images_dir().join(file)).ok()?.to_rgba8();
            let (w, h) = img.dimensions();
            if w == 0 || h == 0 {
                return None;
            }
            let m = w.max(h);
            if m > 1024 {
                let k = 1024.0 / m as f32;
                return Some(image::imageops::resize(&img, ((w as f32 * k) as u32).max(1), ((h as f32 * k) as u32).max(1), image::imageops::FilterType::Triangle));
            }
            Some(img)
        }
        Kind::Text { text, bold, italic, color, font } => text_picture(text, font, *bold, *italic, *color),
        Kind::Object { .. } => None,
    }
}

/// Lettering as a picture: the lines centred, trimmed to the ink, slanted for italic.
pub fn text_picture(text: &str, font: &str, bold: bool, italic: bool, color: [u8; 3]) -> Option<image::RgbaImage> {
    let lines: Vec<&str> = text.split('\n').map(|l| l.trim_end()).collect();
    if lines.iter().all(|l| l.trim().is_empty()) {
        return None;
    }
    let fonts = omsi_ui::Fonts::new();
    let weight = if bold { omsi_ui::Weight::Bold } else { omsi_ui::Weight::Medium };
    let own = !font.is_empty();
    let mut bitmaps: Vec<omsi_ui::text::Bitmap> = Vec::new();
    for l in &lines {
        let t = if l.is_empty() { " " } else { l };
        // (a font that cannot be read is Roboto)
        let b = if own { fonts.render_file(Path::new(font), t, 128.0) } else { None };
        bitmaps.push(b.unwrap_or_else(|| fonts.render(t, 128.0, weight)));
    }
    let w = bitmaps.iter().map(|b| b.w).max()?.max(1);
    let h: u32 = bitmaps.iter().map(|b| b.h).sum::<u32>().max(1);
    // the ink of all lines on one canvas of alpha
    let mut alpha = vec![0u8; (w * h) as usize];
    let mut y0 = 0u32;
    for b in &bitmaps {
        let x0 = (w - b.w) / 2;
        for y in 0..b.h {
            for x in 0..b.w {
                alpha[((y0 + y) * w + x0 + x) as usize] = b.alpha[(y * b.w + x) as usize];
            }
        }
        y0 += b.h;
    }
    // (a font of the system has one weight: bold thickens its strokes)
    if own && bold {
        for _ in 0..2 {
            let src = alpha.clone();
            for y in 0..h as usize {
                for x in 0..w as usize {
                    let i = y * w as usize + x;
                    let mut m = src[i];
                    if x > 0 { m = m.max(src[i - 1]); }
                    if x + 1 < w as usize { m = m.max(src[i + 1]); }
                    if y > 0 { m = m.max(src[i - w as usize]); }
                    if y + 1 < h as usize { m = m.max(src[i + w as usize]); }
                    alpha[i] = m;
                }
            }
        }
    }
    // slanted: the top moves right (about 0.2 of the height)
    let slant = if italic { 0.2 } else { 0.0 };
    let extra = (slant * h as f32).ceil() as u32;
    let ow = w + extra;
    let mut out = vec![0u8; (ow * h) as usize];
    for y in 0..h {
        let shift = ((h - 1 - y) as f32 * slant).round() as u32;
        for x in 0..w {
            out[(y * ow + x + shift) as usize] = alpha[(y * w + x) as usize];
        }
    }
    // trimmed to the ink, with 3 pixels of room
    let (mut x0, mut y0, mut x1, mut y1) = (ow, h, 0u32, 0u32);
    for y in 0..h {
        for x in 0..ow {
            if out[(y * ow + x) as usize] > 8 {
                x0 = x0.min(x);
                x1 = x1.max(x);
                y0 = y0.min(y);
                y1 = y1.max(y);
            }
        }
    }
    if x1 < x0 {
        return None;
    }
    let (x0, y0) = (x0.saturating_sub(3), y0.saturating_sub(3));
    let (x1, y1) = ((x1 + 3).min(ow - 1), (y1 + 3).min(h - 1));
    let (cw, ch) = (x1 - x0 + 1, y1 - y0 + 1);
    let mut img = image::RgbaImage::new(cw, ch);
    for y in 0..ch {
        for x in 0..cw {
            let a = out[((y0 + y) * ow + x0 + x) as usize];
            img.put_pixel(x, y, image::Rgba([color[0], color[1], color[2], a]));
        }
    }
    if cw > MAX_PX {
        let k = MAX_PX as f32 / cw as f32;
        return Some(image::imageops::resize(&img, MAX_PX, ((ch as f32 * k) as u32).max(1), image::imageops::FilterType::Triangle));
    }
    Some(img)
}

/// The cache file name of a decal's picture.
fn texture_name(d: &Decal) -> String {
    let mut key = serde_json::to_string(&d.kind).unwrap_or_default();
    if let Kind::Image { file } = &d.kind {
        if let Ok(m) = std::fs::metadata(images_dir().join(file)) {
            let t = m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
            key.push_str(&format!("|{}|{}", m.len(), t));
        }
    }
    format!("{PREFIX}{:016x}.png", fnv(key.as_bytes()))
}

/// The picture's file name in the cache (made if it is not there) and its width over height.
pub fn texture(d: &Decal) -> Option<(String, f32)> {
    let name = texture_name(d);
    let path = cache_dir().join(&name);
    if let Ok((w, h)) = image::image_dimensions(&path) {
        if w > 0 && h > 0 {
            return Some((name, w as f32 / h as f32));
        }
    }
    let img = draw(d)?;
    let (w, h) = img.dimensions();
    img.save(&path).ok()?;
    Some((name, w as f32 / h as f32))
}

// ---------------------------------------------------------------------------------------------
// putting a decal on the body

/// The box a sticker stands in: its spot, which way it faces, and how its picture lies.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub pos: Vec3,
    pub n: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    pub half: Vec2,
    pub mirror: bool,
    /// For a window: the pane may face either way (the sticker is turned to the viewer).
    pub glass: bool,
}

impl Frame {
    /// `aspect` is the picture's width over its height.
    pub fn new(d: &Decal, aspect: f32) -> Frame {
        let n = Vec3::from(d.normal).try_normalize().unwrap_or(Vec3::X);
        // the viewer looks against the normal; up is the vehicle's up unless the surface
        // faces up or down (then the vehicle's front is the picture's top)
        let up_ref = if n.z.abs() > 0.95 { Vec3::Y } else { Vec3::Z };
        let fwd = -n;
        let right = fwd.cross(up_ref).normalize();
        let up = right.cross(fwd).normalize();
        let (s, c) = d.turn.to_radians().sin_cos();
        let (right, up) = (right * c + up * s, up * c - right * s);
        let width = d.width.max(0.01);
        Frame { pos: Vec3::from(d.pos), n, right, up, half: Vec2::new(width * 0.5, width / aspect.max(0.01) * 0.5), mirror: d.mirror, glass: d.glass }
    }

    fn uv(&self, p: Vec3) -> Vec2 {
        let r = p - self.pos;
        let u = 0.5 + r.dot(self.right) / (self.half.x * 2.0);
        let v = 0.5 - r.dot(self.up) / (self.half.y * 2.0);
        Vec2::new(if self.mirror { 1.0 - u } else { u }, v)
    }
}

#[derive(Clone, Copy)]
struct V {
    p: Vec3,
    n: Vec3,
}

/// The polygon cut to the side of the plane where `limit - sign * dot(p - centre, axis) >= 0`.
fn clip(poly: &[V], centre: Vec3, axis: Vec3, limit: f32, sign: f32) -> Vec<V> {
    let f = |v: &V| limit - sign * (v.p - centre).dot(axis);
    let mut out = Vec::with_capacity(poly.len() + 2);
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        let (fa, fb) = (f(&a), f(&b));
        if fa >= 0.0 {
            out.push(a);
        }
        if (fa >= 0.0) != (fb >= 0.0) {
            let t = fa / (fa - fb);
            out.push(V { p: a.p + (b.p - a.p) * t, n: a.n + (b.n - a.n) * t });
        }
    }
    out
}

/// The part of `mesh` under the sticker, as a mesh of its own: the triangles that face the
/// sticker's way, lie within `DEPTH` of its plane and are in a slot `slot_ok` takes, cut at
/// the box's four sides, lifted by `LIFT` along their normals. One material slot (0).
pub fn project(mesh: &MeshData, frame: &Frame, slot_ok: &dyn Fn(u32) -> bool) -> MeshData {
    let mut out = MeshData { one_sided: mesh.one_sided, ..Default::default() };
    for &(first, count, slot) in &mesh.ranges {
        if !slot_ok(slot) {
            continue;
        }
        let end = ((first + count) as usize).min(mesh.indices.len());
        for tri in mesh.indices[(first as usize).min(end)..end].chunks_exact(3) {
            let ix = [tri[0] as usize, tri[1] as usize, tri[2] as usize];
            if ix.iter().any(|&i| i >= mesh.positions.len()) {
                continue;
            }
            let p = [mesh.positions[ix[0]], mesh.positions[ix[1]], mesh.positions[ix[2]]];
            // the normals as authored; the triangle's own when the mesh has none
            let geo = (p[1] - p[0]).cross(p[2] - p[0]);
            let vn: Vec<Vec3> = ix.iter().map(|&i| mesh.normals.get(i).copied().filter(|n| n.length_squared() > 1e-6).map(|n| n.normalize()).unwrap_or_else(|| geo.try_normalize().unwrap_or(Vec3::Z))).collect();
            let avg = (vn[0] + vn[1] + vn[2]).try_normalize().unwrap_or(Vec3::Z);
            let facing = avg.dot(frame.n);
            // (a pane faces out or in as its modeller made it: the sticker goes on whichever
            // side the viewer is, turned to face him)
            let flip = frame.glass && facing < 0.0;
            if (if frame.glass { facing.abs() } else { facing }) < FACING {
                continue;
            }
            let d: Vec<f32> = p.iter().map(|q| (*q - frame.pos).dot(frame.n)).collect();
            if d.iter().all(|x| *x > DEPTH) || d.iter().all(|x| *x < -DEPTH) {
                continue;
            }
            let mut poly: Vec<V> = (0..3).map(|k| V { p: p[k], n: vn[k] }).collect();
            for (axis, limit, sign) in [(frame.right, frame.half.x, 1.0), (frame.right, frame.half.x, -1.0), (frame.up, frame.half.y, 1.0), (frame.up, frame.half.y, -1.0)] {
                poly = clip(&poly, frame.pos, axis, limit, sign);
                if poly.len() < 3 {
                    break;
                }
            }
            if poly.len() < 3 {
                continue;
            }
            // a polygon of a tilted triangle may still reach out of the depth: dropped
            if poly.iter().any(|v| (v.p - frame.pos).dot(frame.n).abs() > DEPTH * 1.5) {
                continue;
            }
            let base = out.positions.len() as u32;
            for v in &poly {
                let mut n = v.n.try_normalize().unwrap_or(avg);
                if flip {
                    n = -n;
                }
                let q = v.p + n * LIFT;
                out.positions.push(q);
                out.normals.push(n);
                out.uvs.push(frame.uv(v.p));
            }
            // (the winding of the body's triangle is kept: the fan follows the clipped
            // polygon, which clipping leaves in the triangle's order)
            for k in 1..poly.len() as u32 - 1 {
                if flip {
                    out.indices.extend([base, base + k + 1, base + k]);
                } else {
                    out.indices.extend([base, base + k, base + k + 1]);
                }
            }
            // a pane is seen from both sides: a second copy under it, facing the other way
            // (the picture reads mirrored from behind, as a sticker on real glass does)
            if frame.glass {
                let back = out.positions.len() as u32;
                for v in &poly {
                    let mut n = v.n.try_normalize().unwrap_or(avg);
                    if flip {
                        n = -n;
                    }
                    out.positions.push(v.p - n * LIFT);
                    out.normals.push(-n);
                    out.uvs.push(frame.uv(v.p));
                }
                for k in 1..poly.len() as u32 - 1 {
                    if flip {
                        out.indices.extend([back, back + k, back + k + 1]);
                    } else {
                        out.indices.extend([back, back + k + 1, back + k]);
                    }
                }
            }
        }
    }
    if !out.indices.is_empty() {
        out.ranges.push((0, out.indices.len() as u32, 0));
    }
    out
}

/// Whether mesh `i` of the vehicle takes stickers: not a tyre, a switch, a shadow, a
/// skinned bellows, nor a decal itself.
pub fn host_ok(vt: &VehicleType, i: usize) -> bool {
    let Some(vm) = vt.meshes.get(i) else { return false };
    if vm.overlay || vm.materials.first().is_some_and(|m| m.texture.starts_with(PREFIX)) || !vm.skin.is_empty() {
        return false;
    }
    let Some(def) = vt.model.meshes.get(vm.def_index) else { return false };
    // (a switch or button is clickable and small; a door that is clicked open is big, and
    // takes stickers. A lamp's mesh does too: its lights are made per mesh of the model,
    // which the decal shares.)
    let (lo, hi) = vt.mesh_boxes.get(i).copied().unwrap_or((Vec3::ZERO, Vec3::ZERO));
    if def.is_shadow || (def.mouse_event.is_some() && (hi - lo).max_element() < 0.4) {
        return false;
    }
    !def.animations.iter().any(|a| a.variable.to_ascii_lowercase().starts_with("wheel"))
}

/// Whether the triangles of material slot `slot` of mesh `i` take stickers: the solid body
/// (`glass` false) or the panes (`glass` true). A text or script texture (a display) takes none.
pub fn slot_takes(vt: &VehicleType, i: usize, slot: u32, glass: bool) -> bool {
    let Some(vm) = vt.meshes.get(i) else { return false };
    let Some(m) = vm.materials.get(slot as usize) else { return false };
    let mut see_through = m.diffuse[3] < 0.99;
    for o in vm.overrides.iter().filter(|o| omsi_sim::vehicle::override_slot(&vm.materials, o) == Some(slot as usize)) {
        if o.use_text_texture.is_some() || o.use_script_texture.is_some() {
            return false;
        }
        // (only a blended slot is a pane: a door's cut-out `[matl_alpha] 1` or transmapped
        // texture is solid paint, which is why doors took nothing)
        see_through |= o.alpha >= 2;
    }
    see_through == glass
}

/// Put the decals on a vehicle that has just been loaded (before an instance is made of it):
/// the cache folder becomes a texture folder, and every decal adds a mesh over each mesh of
/// the body it covers. Returns how many decals found any surface.
pub fn apply(vt: &mut VehicleType, decals: &[Decal]) -> usize {
    if decals.is_empty() {
        return 0;
    }
    omsi_sim::vehicle::set_extra_texture_dir(Some(cache_dir()));
    let mut used = 0;
    let mut keep: Vec<String> = Vec::new();
    for d in decals {
        if let Kind::Object { asset } = &d.kind {
            let Some(p) = crate::assets::prepare(asset) else { continue };
            let Some(host) = object_host(vt, d) else { continue };
            keep.extend(p.textures());
            vt.add_object(host, place_object(&p, d), p.materials.clone(), p.overrides.clone());
            used += 1;
            continue;
        }
        let Some((name, aspect)) = texture(d) else { continue };
        keep.push(name.clone());
        let parts = overlay_meshes(vt, d, aspect);
        if !parts.is_empty() {
            used += 1;
        }
        for (i, data) in parts {
            vt.add_overlay(i, data, &name);
        }
    }
    tidy_cache(&keep);
    used
}

/// The mesh of the bus an object stands on: the one it was put on (`host`) when that is a
/// mesh that takes objects, else the surface under it (looked for from 3 cm above), else the
/// mesh whose box is the nearest.
pub fn object_host(vt: &VehicleType, d: &Decal) -> Option<usize> {
    if let Some(h) = d.host.map(|h| h as usize).filter(|h| *h < vt.meshes.len() && host_ok(vt, *h)) {
        return Some(h);
    }
    let n = Vec3::from(d.normal).try_normalize()?;
    let origin = Vec3::from(d.pos) + n * 0.03;
    for glass in [false, true] {
        if let Some(h) = pick(vt, &|_| Mat4::IDENTITY, &|_| true, origin, -n, glass) {
            if h.t < 0.25 {
                return Some(h.mesh);
            }
        }
    }
    let p = Vec3::from(d.pos);
    let mut best: Option<(f32, usize)> = None;
    for i in 0..vt.meshes.len() {
        if !host_ok(vt, i) || vt.meshes[i].data.indices.is_empty() {
            continue;
        }
        let (lo, hi) = vt.mesh_boxes.get(i).copied().unwrap_or((Vec3::ZERO, Vec3::ZERO));
        let dist = (lo - p).max(p - hi).max(Vec3::ZERO).length();
        if dist < 0.5 && best.is_none_or(|(b, _)| dist < b) {
            best = Some((dist, i));
        }
    }
    best.map(|b| b.1)
}

/// An object's model put at its spot: its origin there, turned as the object's own turn says
/// (none: as modelled, the model's axes being the bus's), `width` times its modelled size.
pub fn place_object(p: &crate::assets::Prepared, d: &Decal) -> MeshData {
    let r = d.object_matrix();
    let s = d.width.max(0.001);
    let at = Vec3::from(d.pos);
    MeshData {
        positions: p.data.positions.iter().map(|v| at + r * (*v * s)).collect(),
        normals: p.data.normals.iter().map(|v| (r * *v).normalize_or_zero()).collect(),
        uvs: p.data.uvs.clone(),
        ranges: p.data.ranges.clone(),
        indices: p.data.indices.clone(),
        one_sided: p.data.one_sided,
    }
}

/// The meshes of the body a decal covers, each as (the mesh's number, the covered part).
pub fn overlay_meshes(vt: &VehicleType, d: &Decal, aspect: f32) -> Vec<(usize, MeshData)> {
    let frame = Frame::new(d, aspect);
    let mut parts: Vec<(usize, MeshData)> = Vec::new();
    for i in 0..vt.meshes.len() {
        if !host_ok(vt, i) || vt.meshes[i].data.indices.is_empty() {
            continue;
        }
        // (a quick look at the mesh's box first)
        let (lo, hi) = vt.mesh_boxes.get(i).copied().unwrap_or((Vec3::ZERO, Vec3::ZERO));
        let reach = frame.half.length() + DEPTH;
        if frame.pos.cmplt(lo - Vec3::splat(reach)).any() || frame.pos.cmpgt(hi + Vec3::splat(reach)).any() {
            continue;
        }
        let data = project(&vt.meshes[i].data, &frame, &|slot| slot_takes(vt, i, slot, d.glass));
        if !data.indices.is_empty() {
            parts.push((i, data));
        }
    }
    parts
}

/// Where a decal's picture is kept (see [`texture`]).
pub fn cache_file(name: &str) -> PathBuf {
    cache_dir().join(name)
}

/// The fonts installed on the system: (name, file), sorted by name. The first is Roboto, the
/// launcher's own, with no file.
pub fn system_fonts() -> &'static [(String, String)] {
    static FONTS: std::sync::OnceLock<Vec<(String, String)>> = std::sync::OnceLock::new();
    FONTS.get_or_init(|| {
        let mut dirs: Vec<PathBuf> = Vec::new();
        if let Some(w) = std::env::var_os("WINDIR").or_else(|| std::env::var_os("SystemRoot")) {
            dirs.push(PathBuf::from(w).join("Fonts"));
        }
        if let Some(l) = std::env::var_os("LOCALAPPDATA") {
            dirs.push(PathBuf::from(l).join("Microsoft").join("Windows").join("Fonts"));
        }
        for d in ["/Library/Fonts", "/System/Library/Fonts", "/usr/share/fonts", "/usr/local/share/fonts"] {
            dirs.push(PathBuf::from(d));
        }
        if let Some(h) = std::env::var_os("HOME") {
            let h = PathBuf::from(h);
            dirs.extend([h.join("Library/Fonts"), h.join(".fonts"), h.join(".local/share/fonts")]);
        }
        let mut files: Vec<PathBuf> = Vec::new();
        for d in dirs {
            collect_fonts(&d, 0, &mut files);
        }
        let mut out: Vec<(String, String)> = files
            .into_iter()
            .filter_map(|p| {
                let name = font_name(&p).unwrap_or_else(|| p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default());
                Some((name, p.to_string_lossy().to_string()))
            })
            .collect();
        out.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()).then(a.1.cmp(&b.1)));
        out.dedup_by(|a, b| a.0 == b.0);
        out.insert(0, ("Roboto (default)".to_string(), String::new()));
        out
    })
}

fn collect_fonts(dir: &Path, depth: u32, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if depth < 4 {
                collect_fonts(&p, depth + 1, out);
            }
        } else if p.extension().and_then(|x| x.to_str()).is_some_and(|x| x.eq_ignore_ascii_case("ttf") || x.eq_ignore_ascii_case("otf")) {
            out.push(p);
        }
    }
}

/// The full name of a font (name id 4 of its `name` table), read without loading the file.
fn font_name(path: &Path) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    let mut head = [0u8; 12];
    f.read_exact(&mut head).ok()?;
    if !matches!(&head[0..4], b"\0\x01\0\0" | b"OTTO" | b"true") {
        return None;
    }
    let n = u16::from_be_bytes([head[4], head[5]]) as usize;
    let mut dir = vec![0u8; n * 16];
    f.read_exact(&mut dir).ok()?;
    let rec = dir.chunks_exact(16).find(|r| &r[0..4] == b"name")?;
    let off = u32::from_be_bytes([rec[8], rec[9], rec[10], rec[11]]) as u64;
    let len = (u32::from_be_bytes([rec[12], rec[13], rec[14], rec[15]]) as usize).min(1 << 20);
    f.seek(SeekFrom::Start(off)).ok()?;
    let mut t = vec![0u8; len];
    f.read_exact(&mut t).ok()?;
    let count = u16::from_be_bytes([*t.get(2)?, *t.get(3)?]) as usize;
    let base = u16::from_be_bytes([*t.get(4)?, *t.get(5)?]) as usize;
    let mut best: Option<String> = None;
    for k in 0..count {
        let r = t.get(6 + k * 12..6 + k * 12 + 12)?;
        let (platform, id) = (u16::from_be_bytes([r[0], r[1]]), u16::from_be_bytes([r[6], r[7]]));
        if id != 4 {
            continue;
        }
        let (l, o) = (u16::from_be_bytes([r[8], r[9]]) as usize, u16::from_be_bytes([r[10], r[11]]) as usize);
        let Some(bytes) = t.get(base + o..base + o + l) else { continue };
        let text = if platform == 3 || platform == 0 {
            let u: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
            String::from_utf16_lossy(&u)
        } else {
            String::from_utf8_lossy(bytes).to_string()
        };
        if !text.trim().is_empty() && (platform == 3 || best.is_none()) {
            best = Some(text.trim().to_string());
        }
    }
    best
}

/// Dress a bus of the game the way the player saved it (its own set, else its model's).
pub fn apply_saved(vt: &mut VehicleType, bus_id: Option<u32>) {
    let model = vt.def.model.clone().unwrap_or_default();
    let set = load_for(bus_id, &model);
    let n = apply(vt, &set);
    if !set.is_empty() {
        log::info!("decals: {} of {} found a surface on the {}", n, set.len(), vt.def.type_name);
    }
}

/// The cache keeps the pictures of the latest sets only: past 300 files, the ones this set
/// does not use are removed.
fn tidy_cache(keep: &[String]) {
    let Ok(rd) = std::fs::read_dir(cache_dir()) else { return };
    let files: Vec<_> = rd.flatten().collect();
    if files.len() <= 300 {
        return;
    }
    for e in files {
        let name = e.file_name().to_string_lossy().to_string();
        if !keep.contains(&name) {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

// ---------------------------------------------------------------------------------------------
// choosing the spot

/// The body under a ray.
#[derive(Clone, Copy, Debug)]
pub struct Hit {
    /// The vehicle's frame.
    pub pos: Vec3,
    /// Facing the viewer.
    pub normal: Vec3,
    pub mesh: usize,
    pub t: f32,
}

/// The nearest surface a sticker can go on along a ray (`origin`, `dir` in the vehicle's
/// frame). `xf` is each mesh's current transform, `visible` whether it is drawn.
pub fn pick(vt: &VehicleType, xf: &dyn Fn(usize) -> Mat4, visible: &dyn Fn(usize) -> bool, origin: Vec3, dir: Vec3, glass: bool) -> Option<Hit> {
    let dir = dir.try_normalize()?;
    let mut best: Option<Hit> = None;
    for (i, vm) in vt.meshes.iter().enumerate() {
        if !visible(i) || !host_ok(vt, i) || vm.data.indices.is_empty() {
            continue;
        }
        let m = xf(i);
        let inv = m.inverse();
        let o = inv.transform_point3(origin);
        let d = inv.transform_vector3(dir);
        let scale = d.length();
        if scale < 1e-9 {
            continue;
        }
        let dn = d / scale;
        // (a mesh the ray passes by is not looked into)
        if let Some(&(c, r)) = vt.mesh_bounds.get(i) {
            if r > 0.0 && !omsi_geometry::ray_near_sphere(o, dn, c, r) {
                continue;
            }
        }
        let mesh = &vm.data;
        for &(first, count, slot) in &mesh.ranges {
            if !slot_takes(vt, i, slot, glass) {
                continue;
            }
            let end = ((first + count) as usize).min(mesh.indices.len());
            for tri in mesh.indices[(first as usize).min(end)..end].chunks_exact(3) {
                let ix = [tri[0] as usize, tri[1] as usize, tri[2] as usize];
                if ix.iter().any(|&k| k >= mesh.positions.len()) {
                    continue;
                }
                let (a, b, c) = (mesh.positions[ix[0]], mesh.positions[ix[1]], mesh.positions[ix[2]]);
                let Some((t, _, _)) = omsi_geometry::ray_triangle_bary(o, dn, a, b, c) else { continue };
                let t = t / scale;
                if t < 0.02 || best.is_some_and(|h| h.t <= t) {
                    continue;
                }
                let vn = |k: usize| mesh.normals.get(ix[k]).copied().unwrap_or(Vec3::ZERO);
                let mut n = (vn(0) + vn(1) + vn(2)).try_normalize().or_else(|| (b - a).cross(c - a).try_normalize()).unwrap_or(Vec3::Z);
                n = m.transform_vector3(n).try_normalize().unwrap_or(n);
                // facing the viewer (a back face seen through a gap is turned round)
                if n.dot(dir) > 0.0 {
                    n = -n;
                }
                best = Some(Hit { pos: origin + dir * t, normal: n, mesh: i, t });
            }
        }
    }
    best
}

// ---------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn wall() -> MeshData {
        // a 2 m x 2 m wall in the plane x = 0 (the bus's left side), facing -x, vertex normals -x
        let positions = vec![Vec3::new(0.0, -1.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 1.0, 2.0), Vec3::new(0.0, -1.0, 2.0)];
        let normals = vec![-Vec3::X; 4];
        let uvs = vec![Vec2::ZERO; 4];
        let indices = vec![0, 1, 2, 0, 2, 3];
        MeshData { positions, normals, uvs, ranges: vec![(0, 6, 0)], indices, one_sided: true }
    }

    fn decal(pos: Vec3, w: f32) -> Decal {
        Decal { kind: Kind::Image { file: "x.png".into() }, pos: pos.to_array(), normal: [-1.0, 0.0, 0.0], width: w, turn: 0.0, mirror: false, glass: false, rot: no_turn(), host: None }
    }

    fn area(m: &MeshData) -> f32 {
        m.indices.chunks_exact(3).map(|t| (m.positions[t[1] as usize] - m.positions[t[0] as usize]).cross(m.positions[t[2] as usize] - m.positions[t[0] as usize]).length() * 0.5).sum()
    }

    #[test]
    fn frame_reads_left_to_right_for_the_viewer() {
        // on the left side, looking at it from outside (looking along +x), the bus's front
        // (+y) is on the viewer's left: the picture's right is -y, its up is +z
        let f = Frame::new(&decal(Vec3::new(0.0, 0.0, 1.0), 1.0), 2.0);
        assert!((f.right - (-Vec3::Y)).length() < 1e-5, "{:?}", f.right);
        assert!((f.up - Vec3::Z).length() < 1e-5, "{:?}", f.up);
        // on the right side (normal +x): the picture's right is +y
        let mut d = decal(Vec3::ZERO, 1.0);
        d.normal = [1.0, 0.0, 0.0];
        let f = Frame::new(&d, 1.0);
        assert!((f.right - Vec3::Y).length() < 1e-5);
    }

    #[test]
    fn sticker_covers_exactly_its_box() {
        let d = decal(Vec3::new(0.0, 0.0, 1.0), 1.0);
        let f = Frame::new(&d, 2.0); // 1 m wide, 0.5 m tall
        let m = project(&wall(), &f, &|_| true);
        assert!((area(&m) - 0.5).abs() < 1e-4, "area {}", area(&m));
        // lifted off the wall by 3 mm, towards the viewer (-x)
        assert!(m.positions.iter().all(|p| (p.x + LIFT).abs() < 1e-5));
        // the picture is mapped over the whole box
        let (umin, umax) = m.uvs.iter().fold((9.0f32, -9.0f32), |a, u| (a.0.min(u.x), a.1.max(u.x)));
        let (vmin, vmax) = m.uvs.iter().fold((9.0f32, -9.0f32), |a, u| (a.0.min(u.y), a.1.max(u.y)));
        assert!(umin.abs() < 1e-4 && (umax - 1.0).abs() < 1e-4 && vmin.abs() < 1e-4 && (vmax - 1.0).abs() < 1e-4);
        assert_eq!(m.ranges, vec![(0, m.indices.len() as u32, 0)]);
    }

    #[test]
    fn picture_is_not_upside_down() {
        let f = Frame::new(&decal(Vec3::new(0.0, 0.0, 1.0), 1.0), 1.0);
        // a point above the centre has a smaller v (the picture's top row)
        assert!(f.uv(Vec3::new(0.0, 0.0, 1.2)).y < 0.5);
        // a point towards the bus's rear (-y), the viewer's right, has a larger u
        assert!(f.uv(Vec3::new(0.0, -0.2, 1.0)).x > 0.5);
        let mut d = decal(Vec3::new(0.0, 0.0, 1.0), 1.0);
        d.mirror = true;
        assert!(Frame::new(&d, 1.0).uv(Vec3::new(0.0, -0.2, 1.0)).x < 0.5);
    }

    #[test]
    fn sticker_half_off_the_wall_is_cut_at_its_edge() {
        // the box reaches 0.5 m past the wall's top edge (z = 2): only the wall's part counts
        let f = Frame::new(&decal(Vec3::new(0.0, 0.0, 2.0), 1.0), 1.0);
        let m = project(&wall(), &f, &|_| true);
        assert!((area(&m) - 0.5).abs() < 1e-4, "area {}", area(&m));
    }

    #[test]
    fn far_or_back_facing_walls_are_left_alone() {
        let mut d = decal(Vec3::new(0.0, 0.0, 1.0), 1.0);
        d.normal = [1.0, 0.0, 0.0]; // asked from the other side of the wall
        assert!(project(&wall(), &Frame::new(&d, 1.0), &|_| true).indices.is_empty());
        let mut far = decal(Vec3::new(-0.5, 0.0, 1.0), 1.0); // half a metre in front of the wall
        far.normal = [-1.0, 0.0, 0.0];
        assert!(project(&wall(), &Frame::new(&far, 1.0), &|_| true).indices.is_empty());
        // a slot that does not take stickers (glass)
        let f = Frame::new(&decal(Vec3::new(0.0, 0.0, 1.0), 1.0), 1.0);
        assert!(project(&wall(), &f, &|_| false).indices.is_empty());
    }

    #[test]
    fn pane_facing_away_is_turned_to_the_viewer() {
        let mut w = wall();
        w.normals = vec![Vec3::X; 4]; // the pane faces +x, the viewer is on the -x side
        let mut d = decal(Vec3::new(0.0, 0.0, 1.0), 1.0);
        assert!(project(&w, &Frame::new(&d, 2.0), &|_| true).indices.is_empty());
        d.glass = true;
        let m = project(&w, &Frame::new(&d, 2.0), &|_| true);
        assert!(!m.indices.is_empty());
        // a copy on the viewer's side and one behind the pane (seen from the inside)
        assert!(m.positions.iter().all(|p| (p.x.abs() - LIFT).abs() < 1e-5));
        assert!(m.positions.iter().any(|p| p.x < 0.0) && m.positions.iter().any(|p| p.x > 0.0));
        assert!((area(&m) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn serde_round_trip_and_defaults() {
        let d = Decal::image("logo.png", Vec3::new(1.0, 2.0, 3.0), Vec3::X);
        let s = serde_json::to_string(&d).unwrap();
        assert_eq!(serde_json::from_str::<Decal>(&s).unwrap(), d);
        let old = r#"{"kind":{"kind":"text","text":"ABC"},"pos":[0,0,1],"normal":[1,0,0],"width":0.4}"#;
        let t: Decal = serde_json::from_str(old).unwrap();
        assert_eq!(t.kind, Kind::Text { text: "ABC".into(), bold: false, italic: false, color: [255, 255, 255], font: String::new() });
        assert!(!t.mirror && t.turn == 0.0);
    }

    #[test]
    fn an_object_is_put_down_as_modelled_and_can_stand_on_the_surface() {
        // a 1 m model; scale 0.5 on the left wall (facing -x) at (0, 0, 1)
        let data = MeshData {
            positions: vec![Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)],
            normals: vec![Vec3::Z; 4],
            uvs: vec![Vec2::ZERO; 4],
            ranges: vec![(0, 3, 0)],
            indices: vec![0, 1, 2],
            one_sided: true,
        };
        let p = crate::assets::Prepared { data, materials: Vec::new(), overrides: Vec::new(), extent: Vec3::ONE };
        let mut d = Decal::object("a/b", Vec3::new(0.0, 0.0, 1.0), -Vec3::X);
        d.width = 0.5;
        // as modelled: the model's axes are the bus's
        let m = place_object(&p, &d);
        assert!((m.positions[0] - Vec3::new(0.0, 0.0, 1.0)).length() < 1e-5);
        assert!((m.positions[1] - Vec3::new(0.5, 0.0, 1.0)).length() < 1e-5, "{:?}", m.positions[1]);
        assert!((m.positions[2] - Vec3::new(0.0, 0.5, 1.0)).length() < 1e-5, "{:?}", m.positions[2]);
        assert!((m.positions[3] - Vec3::new(0.0, 0.0, 1.5)).length() < 1e-5, "{:?}", m.positions[3]);
        assert!((m.normals[0] - Vec3::Z).length() < 1e-5);
        // standing on the wall: z away from it (to -x), y up, x the picture's right (-y)
        d.stand_on_surface();
        let m = place_object(&p, &d);
        assert!((m.positions[3] - Vec3::new(-0.5, 0.0, 1.0)).length() < 1e-5, "{:?}", m.positions[3]);
        assert!((m.positions[2] - Vec3::new(0.0, 0.0, 1.5)).length() < 1e-5, "{:?}", m.positions[2]);
        assert!((m.positions[1] - Vec3::new(0.0, -0.5, 1.0)).length() < 1e-5, "{:?}", m.positions[1]);
        assert!((m.normals[0] - (-Vec3::X)).length() < 1e-5);
    }

    #[test]
    fn an_object_turns_about_a_world_axis() {
        let mut d = Decal::object("a/b", Vec3::ZERO, -Vec3::X);
        // a quarter turn about the bus's up axis: the object's x goes to y, its z stays up
        d.turn_about(Vec3::Z, std::f32::consts::FRAC_PI_2);
        let m = d.object_matrix();
        assert!((m * Vec3::X - Vec3::Y).length() < 1e-5, "{:?}", m * Vec3::X);
        assert!((m * Vec3::Z - Vec3::Z).length() < 1e-5);
        // and back
        d.turn_about(Vec3::Z, -std::f32::consts::FRAC_PI_2);
        assert!((d.object_matrix() * Vec3::X - Vec3::X).length() < 1e-4);
    }

    #[test]
    fn an_object_is_kept_by_its_asset() {
        let d = Decal::object("cans/cola", Vec3::new(1.0, 2.0, 3.0), Vec3::X);
        let s = serde_json::to_string(&d).unwrap();
        assert!(s.contains("\"object\""), "{s}");
        assert_eq!(serde_json::from_str::<Decal>(&s).unwrap(), d);
    }

    #[test]
    fn keys_and_names() {
        assert_eq!(model_key(" Vehicles\\MAN\\Model\\A.cfg "), "vehicles/man/model/a.cfg");
        assert_eq!(safe("a b/c"), "a_b_c");
        assert_eq!(safe(""), "_");
    }
}
