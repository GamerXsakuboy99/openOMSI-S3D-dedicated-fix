//! Assets: small 3D objects (a can, a poster, a ticket holder...) that the player puts
//! anywhere on a bus in the customize screen. They come as folders of their own - the
//! description (`*.asset`), the model (`.o3d` or `.x`) and its textures, all side by side -
//! in the `Assets` folder next to the launcher's program. See `ASSETS.md` for the format.
//!
//! An asset is not part of any bus: the placed object is added to the bus like a sticker
//! (`decals`, `VehicleType::add_object`), as an extra mesh that shares the mesh of the bus it
//! stands on - so it moves with a door it stands on. What is kept is only the asset's id
//! (`<folder>/<file name of the .asset without its extension>`, lower case), the spot and the
//! size; a set naming an asset that is not installed skips it.

use glam::Vec3;
use omsi_geometry::MeshData;
use omsi_model::{MaterialDef, MeshDef};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Clone, Debug, PartialEq)]
pub struct Asset {
    /// `<folder>/<name of the .asset file without extension>`, lower case, `/` between.
    pub id: String,
    pub name: String,
    pub author: String,
    pub description: String,
    /// The folder of the `.asset` file.
    pub dir: PathBuf,
    /// Its `[mesh]` entries as a `model.cfg` has them, with their `[matl]` blocks.
    pub meshes: Vec<MeshDef>,
}

/// The `Assets` folder: next to the launcher's program (`OPENOMSI_ASSETS` names another).
pub fn root() -> PathBuf {
    if let Some(p) = std::env::var_os("OPENOMSI_ASSETS") {
        return PathBuf::from(p);
    }
    std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("Assets"))).unwrap_or_else(|| PathBuf::from("Assets"))
}

/// The lines after a `[tag]` up to the next one, joined (empty when there is no such tag).
fn tag_text(lines: &[String], tag: &str) -> String {
    let want = format!("[{tag}]");
    let Some(at) = lines.iter().position(|l| l.trim().eq_ignore_ascii_case(&want)) else { return String::new() };
    let mut out: Vec<&str> = Vec::new();
    for l in &lines[at + 1..] {
        let t = l.trim();
        if omsi_cfg::keyword_of(t).is_some() {
            break;
        }
        if !t.is_empty() {
            out.push(t);
        }
    }
    out.join(" ")
}

/// Read a `.asset` file: a `model.cfg` of its own. `[mesh]` names a model of the folder and
/// the `[matl]` blocks after it (`[matl_alpha]`, `[matl_noZwrite]`, `[matl_envmap]`...) are
/// the model.cfg's; `[name]`, `[author]` and `[description]` say what it is. `file` is its
/// path and `root` the Assets folder (the id is made from the two). Errors say what is wrong,
/// for the log.
pub fn parse(text: &str, file: &Path, root: &Path) -> Result<Asset, String> {
    let cfg = omsi_cfg::CfgFile::from_str(file, text.trim_start_matches('\u{feff}'));
    let model = omsi_model::Model::parse(&cfg);
    if model.meshes.is_empty() {
        return Err("no [mesh] in it".to_string());
    }
    let dir = file.parent().ok_or("no folder")?.to_path_buf();
    for m in &model.meshes {
        if find_file(&dir, &m.file).is_none() {
            return Err(format!("the model '{}' is not in the folder", m.file));
        }
    }
    let stem = file.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let rel = dir.strip_prefix(root).unwrap_or(&dir);
    let mut id = rel.to_string_lossy().replace('\\', "/").trim_matches('/').to_string();
    if !id.is_empty() {
        id.push('/');
    }
    id.push_str(&stem);
    let name = tag_text(&cfg.lines, "name");
    Ok(Asset {
        id: id.to_ascii_lowercase(),
        name: if name.is_empty() { stem } else { name },
        author: tag_text(&cfg.lines, "author"),
        description: tag_text(&cfg.lines, "description"),
        dir,
        meshes: model.meshes,
    })
}

/// A file of a folder by the name an author wrote: `\` or `/` between folders, any case, and
/// when there is no such file the same name with another picture extension.
fn find_file(dir: &Path, name: &str) -> Option<PathBuf> {
    let name = name.trim().replace('\\', "/");
    let name = name.trim_start_matches("./").trim_start_matches('/');
    if name.is_empty() || name.split('/').any(|p| p == "..") {
        return None;
    }
    let direct = dir.join(name);
    if direct.is_file() {
        return Some(direct);
    }
    // case-insensitive, by the whole relative path and then by the file name alone
    let mut files = Vec::new();
    walk(dir, 0, &mut files);
    let want = name.to_ascii_lowercase();
    let rel = |p: &PathBuf| p.strip_prefix(dir).unwrap_or(p).to_string_lossy().replace('\\', "/").to_ascii_lowercase();
    if let Some(p) = files.iter().find(|p| rel(*p) == want) {
        return Some(p.clone());
    }
    let base = want.rsplit('/').next().unwrap_or(&want).to_string();
    if let Some(p) = files.iter().find(|p| p.file_name().is_some_and(|n| n.to_string_lossy().to_ascii_lowercase() == base)) {
        return Some(p.clone());
    }
    // another extension (a model that names a .bmp ships as .png)
    let stem = base.rsplit_once('.').map(|(s, _)| s.to_string()).unwrap_or(base);
    const PICTURES: [&str; 6] = ["png", "tga", "dds", "bmp", "jpg", "jpeg"];
    files.iter().find(|p| {
        let n = p.file_name().map(|n| n.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        PICTURES.iter().any(|e| n == format!("{stem}.{e}"))
    }).cloned()
}

fn walk(dir: &Path, depth: u32, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if depth < 3 {
                walk(&p, depth + 1, out);
            }
        } else {
            out.push(p);
        }
    }
}

fn scan(dir: &Path, root: &Path, depth: u32, out: &mut Vec<Asset>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            if depth < 4 {
                scan(&p, root, depth + 1, out);
            }
        } else if p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("asset")) {
            match std::fs::read(&p).map_err(|e| e.to_string()).and_then(|b| parse(&String::from_utf8_lossy(&b), &p, root)) {
                Ok(a) => out.push(a),
                Err(e) => log::warn!("asset {}: {e}", p.display()),
            }
        }
    }
}

fn state() -> &'static Mutex<(Option<Vec<Asset>>, HashMap<String, Option<Arc<Prepared>>>)> {
    static S: OnceLock<Mutex<(Option<Vec<Asset>>, HashMap<String, Option<Arc<Prepared>>>)>> = OnceLock::new();
    S.get_or_init(|| Mutex::new((None, HashMap::new())))
}

/// Every installed asset, sorted by name. Read from the disk once; see
/// [`rescan`].
pub fn list() -> Vec<Asset> {
    let mut g = state().lock().unwrap_or_else(|e| e.into_inner());
    if g.0.is_none() {
        let root = root();
        let mut v = Vec::new();
        scan(&root, &root, 0, &mut v);
        v.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        v.dedup_by(|a, b| a.id == b.id);
        g.0 = Some(v);
    }
    g.0.clone().unwrap_or_default()
}

static REVISION: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Counts the [`rescan`]s: a picture made of an asset is out of date when this changed.
pub fn revision() -> u32 {
    REVISION.load(std::sync::atomic::Ordering::Relaxed)
}

/// Look at the disk again (after an install, or when the customize screen opens).
pub fn rescan() {
    REVISION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut g = state().lock().unwrap_or_else(|e| e.into_inner());
    g.0 = None;
    g.1.clear();
}

pub fn find(id: &str) -> Option<Asset> {
    let id = id.trim().to_ascii_lowercase();
    list().into_iter().find(|a| a.id == id)
}

/// An asset ready to be put on a bus.
pub struct Prepared {
    /// All its meshes in one, in the vehicle's frame of axes (x right, y forward, z up), as
    /// modelled; one material slot for each of `materials`.
    pub data: MeshData,
    /// The materials, their textures renamed to the copies in the decals' cache folder.
    pub materials: Vec<omsi_o3d::Material>,
    /// The `[matl]` blocks of the asset (what a model.cfg says of the materials), with the
    /// texture names renamed to the same copies and the indices counted over all meshes.
    pub overrides: Vec<MaterialDef>,
    /// The size of the model's box (m).
    pub extent: Vec3,
}

impl Prepared {
    /// Every cached texture file the asset uses.
    pub fn textures(&self) -> Vec<String> {
        let mut v: Vec<String> = self.materials.iter().map(|m| m.texture.clone()).collect();
        for o in &self.overrides {
            v.push(o.texture.clone());
            v.extend(o.envmap.iter().map(|e| e.0.clone()));
            v.extend(o.envmap_mask.iter().cloned());
            v.extend(o.bumpmap.iter().map(|e| e.0.clone()));
            v.extend(o.transmap.iter().cloned());
            v.extend(o.nightmap.iter().cloned());
        }
        v.retain(|t| t.starts_with(crate::decals::PREFIX));
        v
    }

    /// The `[matl_alpha]` of a material slot (0 solid, 1 cut out, 2 blended) and whether it
    /// writes no depth.
    pub fn alpha_of(&self, slot: usize) -> (i32, bool) {
        let mut out = (0, false);
        for o in &self.overrides {
            if omsi_sim::vehicle::override_slot(&self.materials, o) == Some(slot) {
                if o.alpha_set {
                    out.0 = o.alpha;
                }
                out.1 |= o.no_z_write;
            }
        }
        out
    }
}

/// Load an asset's models and copy its textures to the cache (once per run and asset).
pub fn prepare(id: &str) -> Option<Arc<Prepared>> {
    let key = id.trim().to_ascii_lowercase();
    let known = state().lock().unwrap_or_else(|e| e.into_inner()).1.get(&key).cloned();
    if let Some(p) = known {
        // (the cache folder is tidied now and then: a texture copy that is gone is made again)
        let gone = p.as_ref().is_some_and(|p| p.textures().iter().any(|t| !crate::decals::cache_file(t).is_file()));
        if !gone {
            return p;
        }
    }
    let made = make(&key).map(Arc::new);
    if made.is_none() {
        log::warn!("asset '{key}' is not installed or cannot be read: left out");
    }
    state().lock().unwrap_or_else(|e| e.into_inner()).1.insert(key, made.clone());
    made
}

fn make(id: &str) -> Option<Prepared> {
    let asset = find(id)?;
    let mut data = MeshData { one_sided: true, ..Default::default() };
    let mut materials: Vec<omsi_o3d::Material> = Vec::new();
    let mut overrides: Vec<MaterialDef> = Vec::new();
    for md in &asset.meshes {
        let Some(path) = find_file(&asset.dir, &md.file) else { continue };
        let Ok(mut mesh) = omsi_o3d::load_mesh(&path).map_err(|e| log::warn!("asset {id}: {}: {e}", path.display())) else { continue };
        mesh.drop_bad_triangles();
        if mesh.materials.is_empty() {
            mesh.materials.push(omsi_o3d::Material::default());
        }
        let d = omsi_geometry::mesh_from_o3d(&mesh);
        if d.indices.is_empty() {
            continue;
        }
        let (slot0, base_v, base_i) = (materials.len() as u32, data.positions.len() as u32, data.indices.len() as u32);
        data.positions.extend(d.positions.iter().copied());
        data.normals.extend(d.normals.iter().copied());
        data.uvs.extend(d.uvs.iter().copied());
        data.indices.extend(d.indices.iter().map(|i| i + base_v));
        data.ranges.extend(d.ranges.iter().map(|&(f, c, s)| (f + base_i, c, s + slot0)));
        let original: Vec<String> = mesh.materials.iter().map(|m| m.texture.clone()).collect();
        let mut renamed = mesh.materials.clone();
        for m in &mut renamed {
            m.texture = cache_texture(&asset, &m.texture);
        }
        for o in &md.materials {
            // (what a variable would drive does not exist for a still object)
            if o.change.is_some() || o.item {
                continue;
            }
            let mut nth = 0;
            let slot = original.iter().position(|t| {
                if t.eq_ignore_ascii_case(&o.texture) {
                    nth += 1;
                    nth - 1 == o.index.max(0)
                } else {
                    false
                }
            });
            let Some(k) = slot else { continue };
            let mut o = o.clone();
            o.texture = renamed[k].texture.clone();
            o.index = materials.iter().chain(renamed[..k].iter()).filter(|m| m.texture.eq_ignore_ascii_case(&o.texture)).count() as i32;
            o.envmap = o.envmap.map(|(t, f)| (cache_texture(&asset, &t), f));
            o.envmap_mask = o.envmap_mask.map(|t| cache_texture(&asset, &t));
            o.bumpmap = o.bumpmap.map(|(t, f)| (cache_texture(&asset, &t), f));
            o.transmap = o.transmap.filter(|t| !t.trim().starts_with("\\S:")).map(|t| cache_texture(&asset, &t));
            o.nightmap = o.nightmap.map(|t| cache_texture(&asset, &t));
            o.envmap_realtime = false;
            o.raindropmap = None;
            o.texcoord_trans_x = None;
            o.texcoord_trans_y = None;
            o.use_script_texture = None;
            o.use_text_texture = None;
            o.alphascale = None;
            o.freetex = None;
            o.lightmap = None;
            o.lightmaps.clear();
            overrides.push(o);
        }
        materials.extend(renamed);
    }
    if data.indices.is_empty() {
        return None;
    }
    let (lo, hi) = data.positions.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(l, h), p| (l.min(*p), h.max(*p)));
    Some(Prepared { data, materials, overrides, extent: (hi - lo).max(Vec3::ZERO) })
}

/// A texture of the asset's folder, copied into the decals' cache under a name of its own
/// (so that two assets that both ship a `tex.png` do not meet). The name it has there; a
/// texture that is not found keeps its name (the game then shows its "missing" picture).
fn cache_texture(asset: &Asset, name: &str) -> String {
    let n = name.trim();
    if n.is_empty() {
        return String::new();
    }
    let Some(src) = find_file(&asset.dir, n) else {
        log::warn!("asset {}: the texture '{n}' is not in its folder", asset.id);
        return n.to_string();
    };
    let file = src.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
    let safe: String = file.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' { c } else { '_' }).collect();
    let mut h = 0xcbf29ce484222325u64;
    let rel = src.strip_prefix(&asset.dir).unwrap_or(&src).to_string_lossy().to_ascii_lowercase();
    for b in asset.id.bytes().chain(rel.bytes()) {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    let out = format!("{}{:08x}_{}", crate::decals::PREFIX, h & 0xffff_ffff, safe);
    let dst = crate::decals::cache_file(&out);
    let same = match (std::fs::metadata(&src), std::fs::metadata(&dst)) {
        (Ok(a), Ok(b)) => a.len() == b.len(),
        _ => false,
    };
    if !same {
        if let Err(e) = std::fs::copy(&src, &dst) {
            log::warn!("asset {}: the texture '{n}' cannot be copied: {e}", asset.id);
            return n.to_string();
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// the picture of an asset (for the tiles of the object browser)

/// A small picture of the asset, drawn in software: seen from the front right and a little
/// from above, lit from the camera's side, textured. `None` when the asset cannot be read.
pub fn thumbnail(id: &str, w: u32, h: u32) -> Option<image::RgbaImage> {
    let p = prepare(id)?;
    let (sw, sh) = (w * 2, h * 2);
    let bg = [28u8, 30, 36, 255];
    let mut color = vec![bg; (sw * sh) as usize];
    let mut depth = vec![f32::MAX; (sw * sh) as usize];
    // the camera: looks from the front-left-above at the model (x right, y forward, z up)
    let (sy, cy) = 215.0f32.to_radians().sin_cos();
    let (sp, cp) = 22.0f32.to_radians().sin_cos();
    let fwd = Vec3::new(sy * cp, cy * cp, -sp);
    let right = Vec3::new(cy, -sy, 0.0);
    let up = right.cross(fwd);
    let d = &p.data;
    if d.positions.is_empty() {
        return None;
    }
    let view: Vec<Vec3> = d.positions.iter().map(|q| Vec3::new(q.dot(right), q.dot(up), q.dot(fwd))).collect();
    let (lo, hi) = view.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(l, h), q| (l.min(*q), h.max(*q)));
    let size = (hi - lo).max(Vec3::splat(1e-4));
    let scale = (sw as f32 * 0.86 / size.x).min(sh as f32 * 0.86 / size.y);
    let mid = (lo + hi) * 0.5;
    let screen: Vec<(f32, f32, f32)> = view
        .iter()
        .map(|q| (sw as f32 * 0.5 + (q.x - mid.x) * scale, sh as f32 * 0.5 - (q.y - mid.y) * scale, q.z))
        .collect();
    let light = (-fwd * 0.55 + up * 0.55 + right * -0.35).normalize();
    // textures, decoded once
    let tex: Vec<Option<omsi_texture::Image>> = p
        .materials
        .iter()
        .map(|m| if m.texture.is_empty() { None } else { omsi_texture::decode_file(&crate::decals::cache_file(&m.texture)).ok() })
        .collect();
    for &(first, count, slot) in &d.ranges {
        let slot = slot as usize;
        let mat = p.materials.get(slot);
        let cut = p.alpha_of(slot).0 >= 1;
        let diffuse = mat.map(|m| m.diffuse).unwrap_or([0.8, 0.8, 0.8, 1.0]);
        let img = tex.get(slot).and_then(|t| t.as_ref());
        let end = ((first + count) as usize).min(d.indices.len());
        let mut i = first as usize;
        while i + 3 <= end {
            let ix = [d.indices[i] as usize, d.indices[i + 1] as usize, d.indices[i + 2] as usize];
            i += 3;
            if ix.iter().any(|&k| k >= screen.len() || k >= d.normals.len()) {
                continue;
            }
            let (a, b, c) = (screen[ix[0]], screen[ix[1]], screen[ix[2]]);
            let area = (b.0 - a.0) * (c.1 - a.1) - (c.0 - a.0) * (b.1 - a.1);
            if area.abs() < 1e-6 {
                continue;
            }
            let minx = a.0.min(b.0).min(c.0).floor().max(0.0) as i32;
            let maxx = a.0.max(b.0).max(c.0).ceil().min(sw as f32 - 1.0) as i32;
            let miny = a.1.min(b.1).min(c.1).floor().max(0.0) as i32;
            let maxy = a.1.max(b.1).max(c.1).ceil().min(sh as f32 - 1.0) as i32;
            for y in miny..=maxy {
                for x in minx..=maxx {
                    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                    let w0 = ((b.0 - px) * (c.1 - py) - (c.0 - px) * (b.1 - py)) / area;
                    let w1 = ((c.0 - px) * (a.1 - py) - (a.0 - px) * (c.1 - py)) / area;
                    let w2 = 1.0 - w0 - w1;
                    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                        continue;
                    }
                    let z = w0 * a.2 + w1 * b.2 + w2 * c.2;
                    let at = y as usize * sw as usize + x as usize;
                    if z >= depth[at] {
                        continue;
                    }
                    let mut n = d.normals[ix[0]] * w0 + d.normals[ix[1]] * w1 + d.normals[ix[2]] * w2;
                    if n.length_squared() < 1e-9 {
                        n = Vec3::Z;
                    }
                    let mut n = n.normalize();
                    if n.dot(fwd) > 0.0 {
                        n = -n;
                    }
                    let lit = 0.40 + 0.60 * n.dot(light).max(0.0);
                    let mut rgb = [diffuse[0], diffuse[1], diffuse[2]];
                    if let (Some(img), true) = (img, d.uvs.len() == d.positions.len()) {
                        let uv = d.uvs[ix[0]] * w0 + d.uvs[ix[1]] * w1 + d.uvs[ix[2]] * w2;
                        let tx = ((uv.x.rem_euclid(1.0)) * img.width as f32) as u32 % img.width.max(1);
                        let ty = ((uv.y.rem_euclid(1.0)) * img.height as f32) as u32 % img.height.max(1);
                        let o = ((ty * img.width + tx) * 4) as usize;
                        if o + 3 < img.rgba.len() {
                            if cut && img.rgba[o + 3] < 128 {
                                continue;
                            }
                            rgb = [img.rgba[o] as f32 / 255.0, img.rgba[o + 1] as f32 / 255.0, img.rgba[o + 2] as f32 / 255.0];
                        }
                    }
                    depth[at] = z;
                    color[at] = [(rgb[0] * lit * 255.0).clamp(0.0, 255.0) as u8, (rgb[1] * lit * 255.0).clamp(0.0, 255.0) as u8, (rgb[2] * lit * 255.0).clamp(0.0, 255.0) as u8, 255];
                }
            }
        }
    }
    let mut big = image::RgbaImage::new(sw, sh);
    for (i, px) in big.pixels_mut().enumerate() {
        *px = image::Rgba(color[i]);
    }
    Some(image::imageops::resize(&big, w, h, image::imageops::FilterType::Triangle))
}

// ---------------------------------------------------------------------------------------------
// installing

/// Show the Assets folder in the system's file manager (made if it is not there).
pub fn open_folder() {
    let dir = root();
    let _ = std::fs::create_dir_all(&dir);
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer").arg(dir.to_string_lossy().replace('/', "\\")).spawn();
    }
    #[cfg(not(target_os = "windows"))]
    crate::version::open_url(&dir.to_string_lossy());
}

/// Whether a dropped folder or zip holds assets (a `.asset` file).
pub fn is_pack(path: &Path) -> bool {
    if path.is_dir() {
        let mut files = Vec::new();
        walk(path, 0, &mut files);
        return files.iter().any(|f| f.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("asset")));
    }
    if path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("zip")) {
        if let Some(mut z) = std::fs::File::open(path).ok().and_then(|f| zip::ZipArchive::new(f).ok()) {
            for i in 0..z.len() {
                if let Ok(f) = z.by_index(i) {
                    if f.name().to_ascii_lowercase().ends_with(".asset") {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<usize> {
    std::fs::create_dir_all(to)?;
    let mut n = 0;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let p = e.path();
        let t = to.join(e.file_name());
        if p.is_dir() {
            n += copy_dir(&p, &t)?;
        } else {
            std::fs::copy(&p, &t)?;
            n += 1;
        }
    }
    Ok(n)
}

/// Put a dropped folder or zip into the Assets folder. A folder (or a zip that has its
/// `.asset` files at its top) goes into a folder of its own name; a zip whose files are
/// already in folders is unpacked as it is. Returns a line for the status bar.
pub fn install(path: &Path) -> Result<String, String> {
    let root = root();
    std::fs::create_dir_all(&root).map_err(|e| format!("The Assets folder cannot be made: {e}"))?;
    let before = list().len();
    if path.is_dir() {
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "asset".to_string());
        copy_dir(path, &root.join(name)).map_err(|e| format!("The assets cannot be copied: {e}"))?;
    } else {
        let file = std::fs::File::open(path).map_err(|e| format!("The file cannot be opened: {e}"))?;
        let mut z = zip::ZipArchive::new(file).map_err(|e| format!("Not a zip file: {e}"))?;
        let top_level = (0..z.len()).any(|i| z.by_index(i).is_ok_and(|f| {
            let n = f.name().replace('\\', "/").to_ascii_lowercase();
            n.ends_with(".asset") && !n.trim_start_matches('/').contains('/')
        }));
        let base = if top_level {
            root.join(path.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "asset".to_string()))
        } else {
            root.clone()
        };
        for i in 0..z.len() {
            let mut f = z.by_index(i).map_err(|e| format!("The zip cannot be read: {e}"))?;
            let Some(rel) = f.enclosed_name() else { continue };
            let out = base.join(rel);
            if f.is_dir() {
                let _ = std::fs::create_dir_all(&out);
                continue;
            }
            if let Some(d) = out.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            let mut w = std::fs::File::create(&out).map_err(|e| format!("{}: {e}", out.display()))?;
            std::io::copy(&mut f, &mut w).map_err(|e| format!("{}: {e}", out.display()))?;
        }
    }
    rescan();
    let now = list().len();
    Ok(format!("Assets installed ({} new, {} in all)", now.saturating_sub(before), now))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("openomsi_assets_test_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn reads_an_asset_file() {
        let root = temp("parse");
        let dir = root.join("Cans");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Cola.O3D"), b"x").unwrap();
        let text = "\u{feff}[name]\nCola can\n[author]\nJan\n[description]\nA 0.33 l can\nfor the cabin\n[mesh]\ncola.o3d\n[matl]\nlabel.png\n0\n[matl_alpha]\n1\n";
        let a = parse(text, &dir.join("cola.asset"), &root).unwrap();
        assert_eq!(a.id, "cans/cola");
        assert_eq!(a.name, "Cola can");
        assert_eq!(a.author, "Jan");
        assert_eq!(a.description, "A 0.33 l can for the cabin");
        assert_eq!(a.meshes.len(), 1);
        assert_eq!(a.meshes[0].materials.len(), 1);
        assert_eq!(a.meshes[0].materials[0].alpha, 1);
        assert!(a.meshes[0].materials[0].alpha_set);
        // the name falls back to the file's
        let b = parse("[mesh]\ncola.o3d\n", &dir.join("plain.asset"), &root).unwrap();
        assert_eq!(b.name, "plain");
        // no mesh, or a model that is not there, or one outside the folder
        assert!(parse("[name]\nx\n", &dir.join("x.asset"), &root).is_err());
        assert!(parse("[mesh]\nnone.o3d\n", &dir.join("x.asset"), &root).is_err());
        assert!(parse("[mesh]\n../x.o3d\n", &dir.join("x.asset"), &root).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn textures_are_found_by_any_case_and_extension() {
        let dir = temp("tex");
        std::fs::create_dir_all(dir.join("maps")).unwrap();
        std::fs::write(dir.join("maps").join("Label.PNG"), b"x").unwrap();
        assert!(find_file(&dir, "maps\\label.png").is_some());
        assert!(find_file(&dir, "label.bmp").is_some());
        assert!(find_file(&dir, "other.png").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_folder_with_assets_is_a_pack() {
        let dir = temp("pack");
        assert!(!is_pack(&dir));
        std::fs::write(dir.join("a.asset"), b"[mesh]\na.o3d").unwrap();
        assert!(is_pack(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
