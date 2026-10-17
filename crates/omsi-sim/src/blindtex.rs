//! Stand-in pictures for roller blinds and plug-in signs.
//!
//! A bus with a roller blind shows the destination the depot file (`.hof`) names with a
//! picture: a `[matl_freetex]` string that comes out as `..\..\Anzeigen\Rollband_SD79\<name>.bmp`.
//! The depot file lists the picture of every destination, but only the maps that came with
//! their pictures have them; on most maps the file is not there and the blind stays blank.
//! This module draws such a picture from the texts of the destination (the lines the depot
//! file holds for it) in the look of the blind the picture was asked for by, in a higher
//! resolution than the originals (the blind is read from close by):
//!
//! * `Rollband_SD79`: black on white, the destination above and a smaller second line (the via
//!   or the street) under it, inside the field the band's mesh shows (38..218 across and 4..58
//!   down of the original's 256 x 64).
//! * `Rollband_O307*`: a paper-coloured band: the line number, a rule, the line number again,
//!   and the destination with its second line at the right (the originals are 860 x 172).
//! * `Rollband_Phantom`: white text on near black, the destination and a smaller second line,
//!   the whole picture is usable (256 x 64 originally).
//! * `Rollband_Mirage`: yellow-green text on near black, the destination and a second line,
//!   with a visible margin (1024 x 256).
//! * `PR110`: four cells of a stencilled sign on beige paper (the line number, the destination on
//!   two lines and a double arrow), a big pair and a small pair.
//! * `Dykty_Juzek` (a plug-in sign's): grey plates, in a row each, with the destination stencilled: four big, four
//!   small plates and two blank rear ones.
//! * `Rollband_N40*`: three segments (774 x 64): the destination on two lines, the destination on one
//!   line, and the line number; light green on grey-black as the Mirage's.
//! * `Steckschild*` (and so `Dykty_Juzek`'s plates, which are plug-in signs): the plug-in sign's
//!   picture, 256 x 64 with the text in the top 35 rows, black on light grey, the line number at
//!   the left of it.
//!
//! Nothing here touches the GPU or the vehicle: the pictures are made from texts and the
//! caller (`scene::sync_materials`) uploads them and keeps them like any other picture.

use ab_glyph::{point, Font, FontRef, PxScale, ScaleFont, VariableFont};
use omsi_texture::Image;

/// The look of a stand-in picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// `Anzeigen\Rollband_SD79`: white, the destination and a smaller line.
    Sd79,
    /// `Anzeigen\Rollband_O307_V2`: paper, the line number twice and the destination.
    O307,
    /// `Anzeigen\Rollband_Phantom`: white on near black, text over the whole picture.
    Phantom,
    /// `Anzeigen\Rollband_Mirage`: yellow-green on near black, with margins.
    Mirage,
    /// `Anzeigen\PR110`: stencilled signs on beige paper, four cells.
    Pr110,
    /// `Anzeigen\Dykty_Juzek`: stencilled plates, grey, ten cells.
    Dykty,
    /// `Anzeigen\Rollband_N40XX`: three segments, light green on grey-black.
    N40,
    /// `Steckschild`: a plug-in sign's picture: light grey, black text in its top rows.
    Plate,
}

impl Style {
    /// Whether the picture carries the line number (so a picture is made again when the line
    /// changes, for the same destination).
    pub fn wants_line(self) -> bool {
        matches!(self, Style::O307 | Style::Pr110 | Style::N40 | Style::Plate)
    }
}

/// The style of the blind a picture path asks for, from the folder it lies in; None for a path
/// that is no blind's (or one of a blind this module does not know).
pub fn style_of(path: &str) -> Option<Style> {
    let p = path.trim().replace('\\', "/").to_lowercase();
    let folders: Vec<&str> = {
        let mut parts: Vec<&str> = p.split('/').filter(|s| !s.is_empty()).collect();
        parts.pop();
        parts
    };
    folders.iter().rev().find_map(|f| {
        if f.starts_with("rollband_sd79") || f.starts_with("rollband_sd83") {
            Some(Style::Sd79)
        } else if f.starts_with("rollband_o307") {
            Some(Style::O307)
        } else if f.starts_with("rollband_phantom") {
            Some(Style::Phantom)
        } else if f.starts_with("rollband_mirage") {
            Some(Style::Mirage)
        } else if f.starts_with("pr110") {
            Some(Style::Pr110)
        } else if f.starts_with("dykty") {
            Some(Style::Dykty)
        } else if f.starts_with("rollband_n40") {
            Some(Style::N40)
        } else if f.starts_with("steckschild") {
            Some(Style::Plate)
        } else {
            None
        }
    })
}

/// The texts of a picture.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lines {
    /// The line number ("" none; "5E").
    pub line: String,
    /// The destination.
    pub big: String,
    /// The second line under it ("" none).
    pub small: String,
}

fn is_picture_name(s: &str) -> bool {
    let t = s.trim().to_ascii_lowercase();
    t.contains('\\') || t.contains('/') || [".bmp", ".tga", ".dds", ".png", ".jpg", ".jpeg"].iter().any(|e| t.ends_with(e))
}

/// A line of text without its suffix: a depot file marks a variant of a destination with a
/// `*` and what follows ("Weeze*B"), which a picture does not show. The text before the `*`
/// (without the space left at its end) stays.
pub fn cut_suffix(s: &str) -> String {
    let t = s.trim();
    match t.find('*') {
        Some(i) => t[..i].trim_end().to_string(),
        None => t.to_string(),
    }
}

/// The two lines a destination's strings make: the first of the strings 1 and 0 that holds
/// text is the destination (string 0 is the whole text, strings 1 and 2 are the upper and the
/// lower line of a two-line display), and string 2 the second line under it when string 1 was
/// the destination. Picture names and the like are no text. The texts are kept as the depot
/// file has them (no change of case). `strings` are as the depot file lists them
/// (`Terminus::strings`).
pub fn pick_lines(strings: &[String]) -> (String, String) {
    let text = |i: usize| strings.get(i).map(|s| s.trim()).filter(|s| !s.is_empty() && !is_picture_name(s)).map(str::to_string);
    match (text(1), text(2)) {
        (Some(a), Some(b)) => (a, b),
        (Some(a), None) => (a, String::new()),
        (None, Some(b)) => (b, String::new()),
        (None, None) => (text(0).unwrap_or_default(), String::new()),
    }
}

fn norm_path(s: &str) -> String {
    let mut p = s.trim().replace('\\', "/").to_lowercase();
    for ext in [".bmp", ".tga", ".dds", ".png", ".jpg", ".jpeg"] {
        if let Some(q) = p.strip_suffix(ext) {
            p = q.to_string();
            break;
        }
    }
    p.trim_start_matches(['.', '/']).to_string()
}

/// Which destination (an index into `termini`, each one's strings) a picture path names: the one
/// that lists this path (or this file, in the same folder - or in none) among its strings. None
/// when no destination lists it.
pub fn match_terminus(termini: &[Vec<String>], path: &str) -> Option<usize> {
    let want = norm_path(path);
    if want.is_empty() {
        return None;
    }
    let (want_dir, want_file) = split_last(&want);
    termini.iter().position(|strings| {
        strings.iter().filter(|s| is_picture_name(s)).any(|s| {
            let have = norm_path(s);
            if have.is_empty() {
                return false;
            }
            if have == want || want.ends_with(&format!("/{have}")) || have.ends_with(&format!("/{want}")) {
                return true;
            }
            let (have_dir, have_file) = split_last(&have);
            have_file == want_file && (have_dir.is_empty() || want_dir.is_empty() || have_dir.rsplit('/').next() == want_dir.rsplit('/').next())
        })
    })
}

fn split_last(p: &str) -> (&str, &str) {
    p.rsplit_once('/').unwrap_or(("", p))
}

/// What a picture path says on its own when nothing else is known: its file's name with the
/// underscores as spaces ("Zehlendorf_Sachtlebenstr" is "Zehlendorf Sachtlebenstr").
pub fn text_of_name(path: &str) -> String {
    let p = path.trim().replace('\\', "/");
    let file = p.rsplit('/').next().unwrap_or("");
    let stem = file.rsplit_once('.').map(|(a, _)| a).unwrap_or(file);
    stem.replace('_', " ").trim().to_string()
}

// --- the look settings (Settings > Rollbands) ----------------------------------------------

/// [ROLLBACK rollbands-1] How the player has the pictures of one folder drawn (Settings >
/// Rollbands): colours, the size of the lettering and its shape. The default draws the picture
/// as it was before the settings existed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tweak {
    /// The colour of the picture's ground (None: the style's own); paper keeps its grain and shading.
    pub bg: Option<[u8; 3]>,
    /// The colour of the lettering (None: the style's own).
    pub ink: Option<[u8; 3]>,
    /// The size of the capitals, 1 = as drawn.
    pub size: f32,
    /// The second line's size against the first's, 1 = as drawn.
    pub second: f32,
    /// How wide a line may be, 1 = as drawn.
    pub width: f32,
    /// How wide the letters are drawn, 1 = as the face has them.
    pub letters: f32,
    /// Added to the weight of the face (Roboto's 100..900).
    pub weight: f32,
    /// Space between two letters, in em, added to the style's own.
    pub track: f32,
    /// The lettering moved down by this share of the picture's height (up when negative).
    pub shift: f32,
    /// Every letter a capital.
    pub caps: bool,
    /// The lettering's face: a font file on this machine (None: the built-in Roboto). See `local_fonts`.
    pub font: Option<&'static str>,
}

impl Default for Tweak {
    fn default() -> Tweak {
        Tweak { bg: None, ink: None, size: 1.0, second: 1.0, width: 1.0, letters: 1.0, weight: 0.0, track: 0.0, shift: 0.0, caps: false, font: None }
    }
}

impl Tweak {
    /// Whether nothing is changed.
    pub fn is_default(&self) -> bool {
        *self == Tweak::default()
    }
}

/// The folders' styles, in the order the settings list them.
pub const STYLES: [Style; 8] = [Style::Sd79, Style::O307, Style::Phantom, Style::Mirage, Style::N40, Style::Pr110, Style::Dykty, Style::Plate];

impl Style {
    /// The name a style has in the settings file.
    pub fn key(self) -> &'static str {
        match self {
            Style::Sd79 => "sd79",
            Style::O307 => "o307",
            Style::Phantom => "phantom",
            Style::Mirage => "mirage",
            Style::Pr110 => "pr110",
            Style::Dykty => "dykty",
            Style::N40 => "n40",
            Style::Plate => "steckschild",
        }
    }

    /// The folder of `Anzeigen` the style stands for, as the settings show it.
    pub fn folder(self) -> &'static str {
        match self {
            Style::Sd79 => "Rollband_SD79 (and SD83)",
            Style::O307 => "Rollband_O307*",
            Style::Phantom => "Rollband_Phantom",
            Style::Mirage => "Rollband_Mirage",
            Style::Pr110 => "PR110",
            Style::Dykty => "Dykty_Juzek",
            Style::N40 => "Rollband_N40*",
            Style::Plate => "Steckschild*",
        }
    }

    /// The colours the style is drawn in without a change: the ground and the lettering.
    pub fn colours(self) -> ([u8; 3], [u8; 3]) {
        match self {
            Style::Sd79 => ([255, 255, 255], [0, 0, 0]),
            Style::O307 => ([217, 209, 200], [24, 17, 12]),
            Style::Phantom => ([29, 29, 29], [255, 255, 255]),
            Style::Mirage => ([20, 20, 20], [193, 213, 27]),
            Style::Pr110 => ([208, 201, 169], [16, 14, 10]),
            Style::Dykty => ([184, 184, 184], [10, 10, 10]),
            Style::N40 => ([26, 27, 26], [193, 213, 27]),
            Style::Plate => ([228, 229, 229], [12, 12, 12]),
        }
    }
}

static TWEAKS: std::sync::RwLock<[Tweak; 8]> = std::sync::RwLock::new([Tweak { bg: None, ink: None, size: 1.0, second: 1.0, width: 1.0, letters: 1.0, weight: 0.0, track: 0.0, shift: 0.0, caps: false, font: None }; 8]);
static TWEAKS_VERSION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static TWEAKS_LOADED: std::sync::Once = std::sync::Once::new();

thread_local! {
    /// The look the picture being drawn is made in (see `render_with`).
    static CUR: std::cell::Cell<Tweak> = std::cell::Cell::new(Tweak::default());
}

fn cur() -> Tweak {
    CUR.with(|c| c.get())
}

fn style_index(style: Style) -> usize {
    STYLES.iter().position(|s| *s == style).unwrap_or(0)
}

/// The look the player has set for `style`.
pub fn tweak(style: Style) -> Tweak {
    TWEAKS.read().map(|t| t[style_index(style)]).unwrap_or_default()
}

/// Set the look of `style`. Pictures drawn from now on carry it (`tweaks_version` changes).
pub fn set_tweak(style: Style, t: Tweak) {
    if let Ok(mut all) = TWEAKS.write() {
        if all[style_index(style)] != t {
            all[style_index(style)] = t;
            TWEAKS_VERSION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

/// A number that changes whenever a look was set: part of what a drawn picture is kept under.
pub fn tweaks_version() -> u64 {
    TWEAKS_VERSION.load(std::sync::atomic::Ordering::Relaxed)
}

fn hex(c: [u8; 3]) -> String {
    format!("{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

/// A colour written as `RRGGBB` or `#RRGGBB`.
pub fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let t = s.trim().trim_start_matches('#');
    if t.len() != 6 || !t.is_ascii() {
        return None;
    }
    let v = u32::from_str_radix(t, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

/// A colour as the settings show it: `#RRGGBB`.
pub fn hex_of(c: [u8; 3]) -> String {
    format!("#{}", hex(c))
}

/// The looks of all styles as a settings file: a `[folder]` block of `key=value` lines each
/// (a block of a style left as it is is not written).
pub fn tweaks_to_text(all: &[Tweak; 8]) -> String {
    let mut out = String::from("# openOMSI: how the roller blinds' and plug-in signs' pictures are drawn (Settings > Rollbands)\n");
    for (i, st) in STYLES.iter().enumerate() {
        let t = &all[i];
        if t.is_default() {
            continue;
        }
        out.push_str(&format!("\n[{}]\n", st.key()));
        if let Some(c) = t.bg {
            out.push_str(&format!("bg={}\n", hex(c)));
        }
        if let Some(c) = t.ink {
            out.push_str(&format!("ink={}\n", hex(c)));
        }
        if let Some(f) = t.font {
            out.push_str(&format!("font={f}\n"));
        }
        out.push_str(&format!("size={:.3}\nsecond={:.3}\nwidth={:.3}\nletters={:.3}\nweight={:.1}\ntrack={:.4}\nshift={:.4}\ncaps={}\n", t.size, t.second, t.width, t.letters, t.weight, t.track, t.shift, u8::from(t.caps)));
    }
    out
}

/// The looks a settings file holds (what it does not name stays as drawn).
pub fn tweaks_from_text(text: &str) -> [Tweak; 8] {
    let mut all = [Tweak::default(); 8];
    let mut at: Option<usize> = None;
    for line in text.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') || l.starts_with(';') {
            continue;
        }
        if let Some(name) = l.strip_prefix('[').and_then(|x| x.strip_suffix(']')) {
            at = STYLES.iter().position(|s| s.key().eq_ignore_ascii_case(name.trim()));
            continue;
        }
        let (Some(i), Some((k, v))) = (at, l.split_once('=')) else { continue };
        let (k, v) = (k.trim().to_ascii_lowercase(), v.trim());
        let t = &mut all[i];
        let num = |lo: f32, hi: f32, d: f32| v.parse::<f32>().ok().filter(|x| x.is_finite()).map(|x| x.clamp(lo, hi)).unwrap_or(d);
        match k.as_str() {
            "bg" => t.bg = parse_hex(v),
            "ink" => t.ink = parse_hex(v),
            "size" => t.size = num(0.4, 2.0, 1.0),
            "second" => t.second = num(0.4, 2.0, 1.0),
            "width" => t.width = num(0.4, 1.5, 1.0),
            "letters" => t.letters = num(0.4, 1.8, 1.0),
            "weight" => t.weight = num(-600.0, 600.0, 0.0),
            "track" => t.track = num(-0.1, 0.5, 0.0),
            "shift" => t.shift = num(-0.5, 0.5, 0.0),
            "caps" => t.caps = matches!(v, "1" | "true" | "yes" | "on"),
            "font" => t.font = (!v.is_empty()).then(|| intern_font(v)),
            _ => {}
        }
    }
    all
}

/// Read the looks from `path` (a missing file leaves every style as drawn) and set them.
pub fn load_file(path: &std::path::Path) {
    let all = std::fs::read_to_string(path).map(|t| tweaks_from_text(&t)).unwrap_or([Tweak::default(); 8]);
    for (i, st) in STYLES.iter().enumerate() {
        set_tweak(*st, all[i]);
    }
}

/// Read the looks from `path` once in the process's life (the game's: the settings are read
/// when it starts; the launcher sets them itself while they are edited).
pub fn ensure_loaded(path: impl FnOnce() -> std::path::PathBuf) {
    TWEAKS_LOADED.call_once(|| load_file(&path()));
}

/// Keep the looks in `path`.
pub fn save_file(path: &std::path::Path) -> std::io::Result<()> {
    let all = TWEAKS.read().map(|t| *t).unwrap_or([Tweak::default(); 8]);
    std::fs::write(path, tweaks_to_text(&all))
}

// --- local fonts ----------------------------------------------------------------------------

static FONT_NAMES: std::sync::Mutex<Vec<&'static str>> = std::sync::Mutex::new(Vec::new());
static FONT_BYTES: std::sync::Mutex<Vec<(&'static str, Option<&'static [u8]>)>> = std::sync::Mutex::new(Vec::new());

/// A font file's path kept for the life of the process (a `Tweak` holds it as a `&'static str`).
pub fn intern_font(path: &str) -> &'static str {
    let mut all = FONT_NAMES.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(p) = all.iter().find(|p| **p == path) {
        return p;
    }
    let p: &'static str = Box::leak(path.to_string().into_boxed_str());
    all.push(p);
    p
}

/// The bytes of a font file, read once (None for a file that is not there or no font ab_glyph reads).
fn font_bytes(path: &'static str) -> Option<&'static [u8]> {
    let mut all = FONT_BYTES.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, b)) = all.iter().find(|(p, _)| *p == path) {
        return *b;
    }
    let bytes = std::fs::metadata(path)
        .ok()
        .filter(|m| m.len() < 40 << 20)
        .and_then(|_| std::fs::read(path).ok())
        .filter(|b| FontRef::try_from_slice(b).is_ok())
        .map(|b| &*Box::leak(b.into_boxed_slice()));
    all.push((path, bytes));
    bytes
}

/// The folders fonts are looked for in: the system's, the user's, and `~/.openomsi/fonts` (a font
/// put there is found on every system).
fn font_dirs() -> Vec<std::path::PathBuf> {
    use std::path::PathBuf;
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
    let mut dirs = Vec::new();
    if let Some(w) = env("WINDIR").or_else(|| env("SystemRoot")) {
        dirs.push(w.join("Fonts"));
    }
    if let Some(l) = env("LOCALAPPDATA") {
        dirs.push(l.join("Microsoft").join("Windows").join("Fonts"));
    }
    if let Some(h) = env("HOME").or_else(|| env("USERPROFILE")) {
        dirs.push(h.join(".openomsi").join("fonts"));
        dirs.push(h.join("Library").join("Fonts"));
        dirs.push(h.join(".local").join("share").join("fonts"));
        dirs.push(h.join(".fonts"));
    }
    for d in ["/System/Library/Fonts", "/Library/Fonts", "/usr/share/fonts", "/usr/local/share/fonts"] {
        dirs.push(PathBuf::from(d));
    }
    dirs
}

fn scan_fonts(dir: &std::path::Path, depth: u32, out: &mut Vec<(String, String)>) {
    if depth > 4 || out.len() > 4000 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            scan_fonts(&p, depth + 1, out);
        } else if p.extension().and_then(|x| x.to_str()).is_some_and(|x| x.eq_ignore_ascii_case("ttf") || x.eq_ignore_ascii_case("otf")) {
            let stem = p.file_stem().and_then(|x| x.to_str()).unwrap_or("").replace(['_', '-'], " ");
            if let Some(path) = p.to_str() {
                out.push((stem, path.to_string()));
            }
        }
    }
}

/// The fonts this machine has as (name, file): `.ttf` and `.otf` files of the system's font
/// folders, the user's and `~/.openomsi/fonts`, by name. Looked for once.
pub fn local_fonts() -> &'static [(String, String)] {
    static FONTS: std::sync::OnceLock<Vec<(String, String)>> = std::sync::OnceLock::new();
    FONTS.get_or_init(|| {
        let mut all = Vec::new();
        for d in font_dirs() {
            scan_fonts(&d, 0, &mut all);
        }
        all.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()).then(a.1.cmp(&b.1)));
        all.dedup_by(|a, b| a.1 == b.1);
        // (a name two files share: the folder it is in after it)
        let dup: Vec<bool> = (0..all.len()).map(|i| all.iter().filter(|x| x.0.eq_ignore_ascii_case(&all[i].0)).count() > 1).collect();
        for (i, f) in all.iter_mut().enumerate() {
            if dup[i] {
                let dir = std::path::Path::new(&f.1).parent().and_then(|p| p.file_name()).and_then(|x| x.to_str()).unwrap_or("");
                f.0 = format!("{} ({dir})", f.0);
            }
        }
        all
    })
}

/// The colour of the ground: the player's, else the style's own `d`.
fn bg_or(d: [u8; 3]) -> [u8; 3] {
    cur().bg.unwrap_or(d)
}

/// The colour of the lettering: the player's, else the style's own `d`.
fn ink_or(d: [f32; 3]) -> [f32; 3] {
    cur().ink.map(|c| [c[0] as f32, c[1] as f32, c[2] as f32]).unwrap_or(d)
}

// --- drawing --------------------------------------------------------------------------------

struct Canvas {
    w: usize,
    h: usize,
    rgba: Vec<u8>,
}

impl Canvas {
    fn new(w: usize, h: usize, c: [u8; 3]) -> Canvas {
        let mut rgba = Vec::with_capacity(w * h * 4);
        for _ in 0..w * h {
            rgba.extend_from_slice(&[c[0], c[1], c[2], 255]);
        }
        Canvas { w, h, rgba }
    }

    fn blend(&mut self, x: i32, y: i32, c: [f32; 3], a: f32) {
        // (the overlapping contours of a variable font add up to more than whole)
        let a = a.min(1.0);
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h || a <= 0.0 {
            return;
        }
        let i = (y as usize * self.w + x as usize) * 4;
        for k in 0..3 {
            let v = self.rgba[i + k] as f32 * (1.0 - a) + c[k] * a;
            self.rgba[i + k] = v.round().clamp(0.0, 255.0) as u8;
        }
    }

    fn rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: [u8; 3]) {
        for y in y0.max(0)..y1.min(self.h as i32) {
            for x in x0.max(0)..x1.min(self.w as i32) {
                let i = (y as usize * self.w + x as usize) * 4;
                self.rgba[i..i + 3].copy_from_slice(&c);
            }
        }
    }

    fn into_image(self) -> Image {
        Image { width: self.w as u32, height: self.h as u32, rgba: self.rgba, has_alpha: false }
    }
}

/// Roboto at a weight and width (the variable font's axes): its condensed, bold cut is the
/// nearest thing to the DIN lettering of a blind that the game carries.
fn face(wght: f32, wdth: f32) -> Option<FontRef<'static>> {
    // ([ROLLBACK rollbands-2] the player's own font, where it can be read; else Roboto as it was)
    let custom = cur().font.and_then(font_bytes).and_then(|b| FontRef::try_from_slice(b).ok());
    let mut f = match custom {
        Some(f) => f,
        None => FontRef::try_from_slice(crate::htmlengine::ROBOTO).ok()?,
    };
    f.set_variation(b"wght", wght);
    f.set_variation(b"wdth", wdth);
    Some(f)
}

/// The height of a capital as a share of the pixel scale the font is asked for at.
fn cap_ratio(f: &FontRef<'static>) -> f32 {
    let g = f.glyph_id('H').with_scale_and_position(PxScale::from(100.0), point(0.0, 0.0));
    f.outline_glyph(g).map(|o| o.px_bounds().height() / 100.0).filter(|r| *r > 0.1).unwrap_or(0.6)
}

/// The width of a line: the letters drawn at `sx` of the face's width, with `track` pixels of
/// extra space between them.
fn text_width(f: &FontRef<'static>, px: f32, sx: f32, track: f32, s: &str) -> f32 {
    let sf = f.as_scaled(PxScale { x: px * sx, y: px });
    let mut w = 0.0;
    let mut prev = None;
    let mut n = 0;
    for c in s.chars() {
        let g = f.glyph_id(c);
        if let Some(p) = prev {
            w += sf.kern(p, g);
        }
        w += sf.h_advance(g) + track;
        prev = Some(g);
        n += 1;
    }
    if n > 0 {
        w -= track;
    }
    w
}

/// One line of text laid out: the face, its size, and how wide it comes out.
struct Run<'a> {
    font: FontRef<'static>,
    px: f32,
    /// How wide the letters are drawn, as a share of the face's own width (1 = as it is).
    sx: f32,
    /// Extra space between two letters, in pixels.
    track: f32,
    /// Cut the round letters through, as a stencil does.
    stencil: bool,
    width: f32,
    text: &'a str,
}

/// How a line is made to fit a width.
#[derive(Clone, Copy)]
struct Opts {
    /// The letters' width to begin with (a narrow lettering from a wide face).
    sx0: f32,
    /// The narrowest they are squeezed to before the line is made smaller instead.
    min_sx: f32,
    /// Space between two letters as a share of the size.
    track_em: f32,
    stencil: bool,
}

const PLAIN: Opts = Opts { sx0: 1.0, min_sx: 1.0, track_em: 0.0, stencil: false };

/// A line with capitals `cap` pixels high, no wider than `max_w`: the width `wdth` first, the
/// condensed cut when that is too wide, then (when `o` allows) narrower letters, then a smaller
/// size when even that is not enough.
fn fit_ex<'a>(text: &'a str, wght: f32, wdth: f32, cap: f32, max_w: f32, o: Opts) -> Option<Run<'a>> {
    fit_look(text, wght, wdth, cap * cur().size, max_w, o)
}

/// As `fit_ex`, for a `cap` that already carries the player's size (a line set again at the
/// size the smaller of two came out in).
// [ROLLBACK rollbands-1] (before the settings: `fit_ex` was this function)
fn fit_ex_cap<'a>(text: &'a str, wght: f32, wdth: f32, cap: f32, max_w: f32, o: Opts) -> Option<Run<'a>> {
    fit_look(text, wght, wdth, cap, max_w, o)
}

/// The fitting, with the player's weight, width and spacing laid on the style's.
fn fit_look<'a>(text: &'a str, wght: f32, wdth: f32, cap: f32, max_w: f32, o: Opts) -> Option<Run<'a>> {
    let t = cur();
    let wght = (wght + t.weight).clamp(100.0, 900.0);
    let max_w = max_w * t.width;
    let o = Opts { sx0: o.sx0 * t.letters, min_sx: (o.min_sx * t.letters).min(o.sx0 * t.letters), track_em: (o.track_em + t.track).max(-0.1), ..o };
    let mut best: Option<Run<'a>> = None;
    for w in [wdth, 75.0f32] {
        let font = face(wght, w.max(75.0))?;
        let px = cap / cap_ratio(&font);
        let track = o.track_em * px;
        let width = text_width(&font, px, o.sx0, track, text);
        let run = Run { font, px, sx: o.sx0, track, stencil: o.stencil, width, text };
        if width <= max_w {
            return Some(run);
        }
        best = Some(run);
    }
    let mut run = best?;
    if o.min_sx < o.sx0 {
        run.sx = (o.sx0 * max_w / run.width.max(1.0)).clamp(o.min_sx, o.sx0);
        run.width = text_width(&run.font, run.px, run.sx, run.track, run.text);
        if run.width <= max_w {
            return Some(run);
        }
    }
    let k = max_w / run.width.max(1.0);
    run.px *= k;
    run.track *= k;
    run.width = text_width(&run.font, run.px, run.sx, run.track, run.text);
    Some(run)
}

fn fit<'a>(text: &'a str, wght: f32, wdth: f32, cap: f32, max_w: f32) -> Option<Run<'a>> {
    fit_ex(text, wght, wdth, cap, max_w, PLAIN)
}

impl Run<'_> {
    /// How high its capitals really are (it may have been made smaller to fit).
    fn cap(&self) -> f32 {
        self.px * cap_ratio(&self.font)
    }
}

/// The height the second line's capitals are given: `want`, but no more than the destination's
/// capitals came out in proportion (a destination made smaller to fit does not stand over a
/// second line that is not).
fn second_cap(big: Option<&Run>, big_nominal: f32, want: f32) -> f32 {
    // [ROLLBACK rollbands-1] the player's second-line size, and the destination's own size against
    // which it counts as made smaller (`fit_ex` lays the size on the result)
    let t = cur();
    let want = want * t.second;
    let nominal = big_nominal * t.size;
    match big {
        Some(b) if b.cap() < nominal => want * (b.cap() / nominal),
        _ => want,
    }
}

/// Whether a stencil's bridge cuts this pixel of a letter: the round letters are cut through
/// at the top and the bottom (O, Q, 0 in the middle; D, B, P, R beside their stem; U below), the
/// way a stencilled lettering holds its islands. `(w, h)` is the letter's box, `(x, y)` the
/// pixel in it and `cap` the capitals' height.
fn stencil_cut(c: char, w: f32, h: f32, cap: f32, x: f32, y: f32) -> bool {
    let gap = (cap * 0.075).max(1.2);
    let stroke = cap * 0.27;
    let at = |cx: f32| (x + 0.5 - cx).abs() < gap / 2.0;
    let top = y < stroke;
    let bottom = y > h - stroke;
    match c {
        'O' | 'Q' | '0' | 'Ó' | 'Ö' | 'o' | 'ó' | 'ö' => at(w / 2.0) && (top || bottom),
        'U' | 'Ü' | 'Ú' | 'u' | 'ü' | 'ú' => at(w / 2.0) && bottom,
        'D' => at(w * 0.30) && (top || bottom),
        'B' | 'P' | 'R' => at(w * 0.30) && top,
        _ => false,
    }
}

/// Draw a run with its left end at `x` and its baseline at `y`.
fn draw_run(cv: &mut Canvas, run: &Run, x: f32, y: f32, color: [f32; 3]) {
    let scale = PxScale { x: run.px * run.sx, y: run.px };
    let sf = run.font.as_scaled(scale);
    let cap = run.cap();
    let mut pen = x;
    let mut prev = None;
    for c in run.text.chars() {
        let id = run.font.glyph_id(c);
        if let Some(p) = prev {
            pen += sf.kern(p, id);
        }
        let g = id.with_scale_and_position(scale, point(pen, y));
        if let Some(o) = run.font.outline_glyph(g) {
            let b = o.px_bounds();
            let (bx, by) = (b.min.x.floor() as i32, b.min.y.floor() as i32);
            let (bw, bh) = (b.width(), b.height());
            o.draw(|gx, gy, cov| {
                if run.stencil && stencil_cut(c, bw, bh, cap, gx as f32, gy as f32) {
                    return;
                }
                cv.blend(bx + gx as i32, by + gy as i32, color, cov)
            });
        }
        pen += sf.h_advance(id) + run.track;
        prev = Some(id);
    }
}

fn centred(cv: &mut Canvas, run: &Run, centre: f32, baseline: f32, color: [f32; 3]) {
    // ([ROLLBACK rollbands-1] the player's shift of the lettering up or down)
    let baseline = baseline + cur().shift * cv.h as f32;
    draw_run(cv, run, centre - run.width / 2.0, baseline, color);
}

/// A small deterministic noise in 0..1 (the grain of paper).
fn hash01(x: u32, y: u32) -> f32 {
    let mut h = x.wrapping_mul(0x9E37_79B1) ^ y.wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h & 0xFFFF) as f32 / 65535.0
}

/// Smooth value noise in 0..1 with features about `cell` pixels across.
fn vnoise(x: f32, y: f32, cell: f32, seed: u32) -> f32 {
    let (fx, fy) = (x / cell, y / cell);
    let (ix, iy) = (fx.floor() as i32, fy.floor() as i32);
    let (tx, ty) = (fx - ix as f32, fy - iy as f32);
    let ease = |t: f32| t * t * (3.0 - 2.0 * t);
    let h = |a: i32, b: i32| hash01((a as u32).wrapping_add(seed.wrapping_mul(7919)), (b as u32) ^ seed);
    let (sx, sy) = (ease(tx), ease(ty));
    let top = h(ix, iy) * (1.0 - sx) + h(ix + 1, iy) * sx;
    let bottom = h(ix, iy + 1) * (1.0 - sx) + h(ix + 1, iy + 1) * sx;
    top * (1.0 - sy) + bottom * sy
}

/// Paper (or card) over the box `x0..x1` x `y0..y1`: `base` colour, clouds of about `cell` pixels
/// that vary it by up to `cloud` levels either way, and a fine grain of `grain` levels.
fn paper(cv: &mut Canvas, x0: i32, y0: i32, x1: i32, y1: i32, base: [f32; 3], cloud: f32, grain: f32, cell: f32, seed: u32) {
    for y in y0.max(0)..y1.min(cv.h as i32) {
        for x in x0.max(0)..x1.min(cv.w as i32) {
            let n = (vnoise(x as f32, y as f32, cell, seed) * 0.65 + vnoise(x as f32, y as f32, cell * 0.37, seed + 1) * 0.35 - 0.5) * 2.0 * cloud;
            let g = (hash01(x as u32, y as u32 ^ seed) - 0.5) * 2.0 * grain;
            let i = (y as usize * cv.w + x as usize) * 4;
            for k in 0..3 {
                cv.rgba[i + k] = (base[k] + n + g).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
}

/// A filled polygon (even-odd), edges smoothed by 3 x 3 samples a pixel.
fn fill_poly(cv: &mut Canvas, pts: &[(f32, f32)], color: [f32; 3]) {
    let inside = |px: f32, py: f32| {
        let mut c = false;
        for i in 0..pts.len() {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            if (a.1 > py) != (b.1 > py) && px < (b.0 - a.0) * (py - a.1) / (b.1 - a.1) + a.0 {
                c = !c;
            }
        }
        c
    };
    let x0 = pts.iter().map(|p| p.0).fold(f32::MAX, f32::min).floor() as i32;
    let x1 = pts.iter().map(|p| p.0).fold(f32::MIN, f32::max).ceil() as i32;
    let y0 = pts.iter().map(|p| p.1).fold(f32::MAX, f32::min).floor() as i32;
    let y1 = pts.iter().map(|p| p.1).fold(f32::MIN, f32::max).ceil() as i32;
    for y in y0..y1 {
        for x in x0..x1 {
            let mut n = 0;
            for sy in 0..3 {
                for sx in 0..3 {
                    if inside(x as f32 + (sx as f32 + 0.5) / 3.0, y as f32 + (sy as f32 + 0.5) / 3.0) {
                        n += 1;
                    }
                }
            }
            cv.blend(x, y, color, n as f32 / 9.0);
        }
    }
}

/// A double-headed vertical arrow (the stencilled "up and down"): centre `cx`, from `top` to
/// `bottom`, the heads `w` wide.
fn double_arrow(cv: &mut Canvas, cx: f32, top: f32, bottom: f32, w: f32, color: [f32; 3]) {
    let head = w * 1.15;
    let shaft = w * 0.28;
    fill_poly(cv, &[(cx, top), (cx + w / 2.0, top + head), (cx - w / 2.0, top + head)], color);
    fill_poly(cv, &[(cx, bottom), (cx + w / 2.0, bottom - head), (cx - w / 2.0, bottom - head)], color);
    fill_poly(
        cv,
        &[(cx - shaft / 2.0, top + head - 1.0), (cx + shaft / 2.0, top + head - 1.0), (cx + shaft / 2.0, bottom - head + 1.0), (cx - shaft / 2.0, bottom - head + 1.0)],
        color,
    );
}

/// The picture for `style` and `lines`, in the look the player has set for the style.
pub fn render(style: Style, lines: &Lines) -> Image {
    render_with(style, lines, &tweak(style))
}

/// The picture for `style` and `lines` in the look `tw` (the settings' preview draws the look
/// being edited).
pub fn render_with(style: Style, lines: &Lines, tw: &Tweak) -> Image {
    let before = CUR.with(|c| c.replace(*tw));
    let img = render_look(style, lines);
    CUR.with(|c| c.set(before));
    img
}

fn render_look(style: Style, lines: &Lines) -> Image {
    // (a suffix such as the "*B" of "Weeze*B" is for the depot's own use: not drawn)
    let mut lines = Lines { line: lines.line.clone(), big: cut_suffix(&lines.big), small: cut_suffix(&lines.small) };
    if cur().caps {
        lines = Lines { line: lines.line.to_uppercase(), big: lines.big.to_uppercase(), small: lines.small.to_uppercase() };
    }
    let lines = &lines;
    // (a text that came as one line, on two where the picture is made for two)
    let lines = &auto_split(style, lines);
    match style {
        Style::Sd79 => render_sd79(lines),
        Style::O307 => render_o307(lines),
        Style::Phantom => render_phantom(lines),
        Style::Mirage => render_mirage(lines),
        Style::Pr110 => render_pr110(lines),
        Style::Dykty => render_dykty(lines),
        Style::N40 => render_n40(lines),
        Style::Plate => render_plate(lines),
    }
}

/// `Rollband_SD79`: 256 x 64 as the original, drawn at four times that. White, and black text
/// inside the field from 38 to 218 across and 4 to 58 down (the original's red field was the
/// place text may go): the destination's capitals 22 high on the baseline 30 and the second
/// line's 16 high on the baseline 53, centred, never wider than 164 (the original's ink runs
/// from 47 to 208).
fn render_sd79(lines: &Lines) -> Image {
    const S: f32 = 4.0;
    let mut cv = Canvas::new((256.0 * S) as usize, (64.0 * S) as usize, bg_or(Style::Sd79.colours().0));
    let black = ink_or([0.0, 0.0, 0.0]);
    let (centre, max_w) = (128.0 * S, 164.0 * S);
    let two = !lines.small.trim().is_empty();
    let big = (!lines.big.trim().is_empty()).then(|| fit(lines.big.trim(), 650.0, 100.0, 22.0 * S, max_w)).flatten();
    if let Some(run) = &big {
        // (a destination alone stands in the middle of the field)
        let base = if two { 30.0 * S } else { 42.0 * S };
        centred(&mut cv, run, centre, base, black);
    }
    if two {
        if let Some(run) = fit(lines.small.trim(), 550.0, 100.0, second_cap(big.as_ref(), 22.0 * S, 16.0 * S), max_w) {
            centred(&mut cv, &run, centre, 53.0 * S, black);
        }
    }
    cv.into_image()
}

/// `Rollband_O307_V2`: 860 x 172 as the original, drawn at twice that. Paper, lighter in the
/// middle and darker at the corners, with a grain; a dark rule at 211; the line number centred on
/// 104 and on 327 (capitals 92 high, baseline 133, at most 140 wide: the original's are 85 high and
/// the ink 104 wide);
/// the destination centred on 627 (capitals 50 high, baseline 87) with the second line under it
/// (27 high, baseline 132), both at most 380 wide (the original's ink runs from 440 to 815).
/// Nothing is drawn beyond x 430..825, y 30..146 on the right and 36..136 for a line number.
fn render_o307(lines: &Lines) -> Image {
    const S: f32 = 2.0;
    let (w, h) = ((860.0 * S) as usize, (172.0 * S) as usize);
    let mut cv = Canvas::new(w, h, [200, 189, 178]);
    let (cx, cy) = (w as f32 * 0.5, h as f32 * 0.5);
    // ([ROLLBACK rollbands-1] the player's paper: the centre's colour, the corners darkened as they were)
    let paper_c = bg_or(Style::O307.colours().0);
    for y in 0..h {
        for x in 0..w {
            // 0 in the middle, 1 at the corners
            let (dx, dy) = ((x as f32 - cx) / cx, (y as f32 - cy) / cy);
            let t = ((dx * dx + dy * dy) / 2.0).sqrt().powf(1.4);
            let grain = (hash01(x as u32 / 2, y as u32 / 2) - 0.5) * 7.0 + (hash01(x as u32, y as u32) - 0.5) * 3.0;
            let light = [paper_c[0] as f32, paper_c[1] as f32, paper_c[2] as f32];
            let dark = [light[0] * (168.0 / 217.0), light[1] * (152.0 / 209.0), light[2] * (137.0 / 200.0)];
            let i = (y * w + x) * 4;
            for k in 0..3 {
                cv.rgba[i + k] = (light[k] + (dark[k] - light[k]) * t.min(1.0) + grain).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    // the rule between the two line numbers
    let rule = (211.0 * S) as i32;
    for dx in -3..=4i32 {
        let a = match dx {
            0 | 1 => 0.92,
            -1 | 2 => 0.35,
            _ => 0.12,
        };
        for y in 0..h as i32 {
            cv.blend(rule + dx, y, [42.0, 27.0, 17.0], a);
        }
    }
    let ink = ink_or([24.0, 17.0, 12.0]);
    let line = lines.line.trim();
    if !line.is_empty() {
        // (capitals 92 high on the baseline 133 in the field 34..139, a little larger than the
        // original's 85; a long number is narrowed before it is made smaller)
        let opts = Opts { min_sx: 0.7, ..PLAIN };
        if let Some(run) = fit_ex(line, 700.0, 75.0, 92.0 * S, 140.0 * S, opts) {
            let base = 86.0 * S + run.cap() / 2.0 + 0.5 * S;
            centred(&mut cv, &run, 104.0 * S, base, ink);
            centred(&mut cv, &run, 327.0 * S, base, ink);
        }
    }
    let two = !lines.small.trim().is_empty();
    // [ROLLBACK o307-1] with no line number the destination has the whole front window (right of the
    // rule) to itself and is centred in it, not left where the number's box would leave it
    let (dest_x, dest_w) = if line.is_empty() { (540.0, 480.0) } else { (627.0, 380.0) };
    let big = (!lines.big.trim().is_empty()).then(|| fit(lines.big.trim(), 700.0, 75.0, 50.0 * S, dest_w * S)).flatten();
    if let Some(run) = &big {
        let base = if two { 87.0 * S } else { 111.0 * S };
        centred(&mut cv, run, dest_x * S, base, ink);
    }
    if two {
        if let Some(run) = fit(lines.small.trim(), 600.0, 75.0, second_cap(big.as_ref(), 50.0 * S, 27.0 * S), dest_w * S) {
            centred(&mut cv, &run, dest_x * S, 132.0 * S, ink);
        }
    }
    cv.into_image()
}

/// `Rollband_Phantom`: 256 x 64 as the original, drawn at four times that. Near black (29), white
/// text over the whole picture: the destination's capitals 25 high on the baseline 35 and the
/// second line's 19 high on the baseline 56, centred on 127, never wider than 216 (the
/// original's ink runs from 22 to 232).
fn render_phantom(lines: &Lines) -> Image {
    const S: f32 = 4.0;
    let mut cv = Canvas::new((256.0 * S) as usize, (64.0 * S) as usize, bg_or(Style::Phantom.colours().0));
    let white = ink_or([255.0, 255.0, 255.0]);
    let (centre, max_w) = (127.0 * S, 216.0 * S);
    let two = !lines.small.trim().is_empty();
    let cap = if two { 25.0 } else { 28.0 } * S;
    let big = (!lines.big.trim().is_empty()).then(|| fit(lines.big.trim(), 430.0, 100.0, cap, max_w)).flatten();
    if let Some(run) = &big {
        let base = if two { 35.0 * S } else { 32.0 * S + run.cap() / 2.0 };
        centred(&mut cv, run, centre, base, white);
    }
    if two {
        if let Some(run) = fit(lines.small.trim(), 400.0, 100.0, second_cap(big.as_ref(), 25.0 * S, 19.0 * S), max_w) {
            centred(&mut cv, &run, centre, 56.0 * S, white);
        }
    }
    cv.into_image()
}

/// Mirage's lettering: Helvetica's capitals are wider than Roboto's, so the letters are widened
/// and spaced a little.
const MIRAGE: Opts = Opts { sx0: 1.14, min_sx: 0.8, track_em: 0.05, stencil: false };

/// `Rollband_Mirage`: 1024 x 256 as the original, drawn at twice that. Near black (20) with
/// yellow-green (193, 213, 27) text: the destination's capitals 75 high on the baseline 118,
/// the second line 64 high on the baseline 218, centred on 509, at most 780 wide (the original's
/// ink is 132..886 for a short destination): a margin of 120 stays at either side.
fn render_mirage(lines: &Lines) -> Image {
    const S: f32 = 2.0;
    let mut cv = Canvas::new((1024.0 * S) as usize, (256.0 * S) as usize, bg_or(Style::Mirage.colours().0));
    let ink = ink_or([193.0, 213.0, 27.0]);
    let (centre, max_w) = (509.0 * S, 780.0 * S);
    let two = !lines.small.trim().is_empty();
    let upper = lines.big.trim().to_string();
    let opts = MIRAGE;
    let big = (!upper.is_empty()).then(|| fit_ex(&upper, 600.0, 100.0, 75.0 * S, max_w, opts)).flatten();
    if let Some(run) = &big {
        let base = if two { 118.0 * S } else { 128.0 * S + run.cap() / 2.0 };
        centred(&mut cv, run, centre, base, ink);
    }
    if two {
        if let Some(run) = fit_ex(lines.small.trim(), 560.0, 100.0, second_cap(big.as_ref(), 75.0 * S, 64.0 * S), max_w, opts) {
            centred(&mut cv, &run, centre, 218.0 * S, ink);
        }
    }
    cv.into_image()
}

/// A text split over two lines at the space that leaves the halves most alike in length.
fn split_two(s: &str) -> (String, String) {
    split_two_ratio(s, 1.0)
}

/// A text split over two lines at the space where the first line's width comes nearest to the
/// second's when the second's letters are `k` times the first's in height (so a second line
/// that is set smaller takes more of the text).
fn split_two_ratio(s: &str, k: f32) -> (String, String) {
    let t = s.trim();
    let n = t.chars().count() as f32;
    let score = |i: usize| {
        let a = t[..i].chars().count() as f32;
        (a - k * (n - a - 1.0)).abs()
    };
    let best = t
        .char_indices()
        .filter(|(_, c)| *c == ' ')
        .map(|(i, _)| i)
        .min_by(|a, b| score(*a).partial_cmp(&score(*b)).unwrap_or(std::cmp::Ordering::Equal));
    match best {
        Some(i) => (t[..i].trim().to_string(), t[i + 1..].trim().to_string()),
        None => (t.to_string(), String::new()),
    }
}

/// How a style sets a destination that stands alone: the weight and width of the face, the
/// capitals' height and the widest line (in the picture's pixels), how it is made to fit, and the
/// second line's capitals as a share of the first's. None for a style that does not set two lines
/// out of one text by itself.
fn single_spec(style: Style) -> Option<(f32, f32, f32, f32, Opts, f32)> {
    Some(match style {
        Style::Sd79 => (650.0, 100.0, 22.0 * 4.0, 164.0 * 4.0, PLAIN, 16.0 / 22.0),
        Style::O307 => (700.0, 75.0, 50.0 * 2.0, 380.0 * 2.0, PLAIN, 27.0 / 50.0),
        Style::Phantom => (430.0, 100.0, 28.0 * 4.0, 216.0 * 4.0, PLAIN, 19.0 / 25.0),
        Style::Mirage => (600.0, 100.0, 75.0 * 2.0, 780.0 * 2.0, MIRAGE, 64.0 / 75.0),
        // (the first segment, which is set on two lines when the text does not go on one: of equal size)
        Style::N40 => (600.0, 80.0, 28.0 * 4.0, 232.0 * 4.0, N40_OPTS, 1.0),
        Style::Pr110 | Style::Dykty | Style::Plate => return None,
    })
}

/// A text that came as one line, set on two where a picture is made for two: the stencilled
/// `PR110`'s cells are all of two lines (a single word stays one, bigger; so does a short name
/// with a one or two letter first word, "U Ruhleben"), the others take two lines where one line
/// would be cramped (the destination made smaller or narrower to fit): the second line, set
/// smaller, gets the larger share. A picture whose text has two lines already is left as it is.
fn auto_split(style: Style, lines: &Lines) -> Lines {
    let a = lines.big.trim();
    if !lines.small.trim().is_empty() || !a.contains(' ') {
        return lines.clone();
    }
    let (first, second) = match style {
        Style::Pr110 => {
            let (x, y) = split_two(a);
            let orphan = x.chars().count() <= 2 || y.chars().count() <= 2;
            if orphan && a.chars().count() <= 14 {
                return lines.clone();
            }
            (x, y)
        }
        Style::Dykty | Style::Plate => return lines.clone(),
        _ => {
            let Some((wght, wdth, cap, max_w, opts, k)) = single_spec(style) else { return lines.clone() };
            // ([ROLLBACK rollbands-1] against the size and the letters the player has set)
            let cramped = fit_ex(a, wght, wdth, cap, max_w, opts).is_some_and(|r| r.cap() < cap * cur().size * 0.985 || r.sx < opts.sx0 * cur().letters - 0.01);
            if !cramped {
                return lines.clone();
            }
            split_two_ratio(a, k)
        }
    };
    if second.is_empty() {
        return lines.clone();
    }
    Lines { line: lines.line.clone(), big: first, small: second }
}

/// The lettering of the stencilled pictures: Roboto bold, narrowed to about four fifths and
/// further when the line asks for it, the round letters cut through.
const STENCIL: Opts = Opts { sx0: 0.80, min_sx: 0.55, track_em: 0.03, stencil: true };

/// The plates' lettering is narrower still (the original's "WĘGLIN D.A." is 375 wide at 70 high).
const STENCIL_NARROW: Opts = Opts { sx0: 0.70, min_sx: 0.5, track_em: 0.03, stencil: true };

/// One cell of `PR110`: the line number on the left, the destination in the middle (two lines,
/// or one that is larger) and the double arrow at the right. All measures are the original's
/// pixels (the picture is drawn at `s` times that) from the cell's corner `(x0, y0)`.
fn pr110_cell(cv: &mut Canvas, s: f32, (x0, y0): (f32, f32), big: bool, line: &str, a: &str, b: &str, ink: [f32; 3]) {
    // number: centre, capitals, widest; destination: centre, widest, capitals of two lines, gap, capitals of one;
    // arrow: centre, width, height
    let (num, dest, arrow) = if big {
        ((83.0, 113.0, 108.0), (365.0, 360.0, 51.0, 16.0, 68.0), (565.0, 20.0, 84.0))
    } else {
        ((49.0, 85.0, 78.0), (245.0, 296.0, 43.0, 13.0, 56.0), (413.5, 14.0, 65.0))
    };
    let cy = (y0 + 74.5) * s;
    if !line.is_empty() {
        if let Some(run) = fit_ex(line, 800.0, 75.0, num.1 * s, num.2 * s, STENCIL) {
            centred(cv, &run, (x0 + num.0) * s, cy + run.cap() / 2.0, ink);
        }
    }
    let (cx, max_w) = ((x0 + dest.0) * s, dest.1 * s);
    if !b.is_empty() {
        let (ra, rb) = (fit_ex(a, 800.0, 75.0, dest.2 * s, max_w, STENCIL), fit_ex(b, 800.0, 75.0, dest.2 * s, max_w, STENCIL));
        if let (Some(ra), Some(rb)) = (ra, rb) {
            // both lines the size of the smaller one
            let cap = ra.cap().min(rb.cap());
            if let (Some(ra), Some(rb)) = (fit_ex_cap(a, 800.0, 75.0, cap, max_w, STENCIL), fit_ex_cap(b, 800.0, 75.0, cap, max_w, STENCIL)) {
                let gap = dest.3 * s * cap / (dest.2 * s);
                let top = cy - (2.0 * cap + gap) / 2.0;
                centred(cv, &ra, cx, top + cap, ink);
                centred(cv, &rb, cx, top + 2.0 * cap + gap, ink);
            }
        }
    } else if let Some(run) = fit_ex(a, 800.0, 75.0, dest.4 * s, max_w, STENCIL) {
        centred(cv, &run, cx, cy + run.cap() / 2.0, ink);
    }
    double_arrow(cv, (x0 + arrow.0) * s, cy - arrow.2 * s / 2.0, cy + arrow.2 * s / 2.0, arrow.1 * s, ink);
}

/// `PR110`: 1024 x 304 as the original, drawn at twice that: beige paper in four cells (two
/// large ones at the left, two small ones at the right), parted by dark rules at 151 across and
/// 596 down; in each one the line number, the destination on two lines in stencil lettering and a
/// double arrow.
fn render_pr110(lines: &Lines) -> Image {
    const S: f32 = 2.0;
    let (w, h) = ((1024.0 * S) as usize, (304.0 * S) as usize);
    let ground = bg_or(Style::Pr110.colours().0);
    let mut cv = Canvas::new(w, h, ground);
    paper(&mut cv, 0, 0, w as i32, h as i32, [ground[0] as f32, ground[1] as f32, ground[2] as f32], 6.0, 2.5, 80.0 * S, 11);
    let ink = ink_or([16.0, 14.0, 10.0]);
    let line = lines.line.trim().to_string();
    let (a, b) = (lines.big.trim().to_string(), lines.small.trim().to_string());
    for (x0, y0, big) in [(0.0, 0.0, true), (598.0, 0.0, false), (0.0, 153.0, true), (598.0, 153.0, false)] {
        pr110_cell(&mut cv, S, (x0, y0), big, &line, &a, &b, ink);
    }
    let rule = [22, 19, 14];
    cv.rect(0, (151.0 * S) as i32, w as i32, (153.0 * S) as i32, rule);
    cv.rect((596.0 * S) as i32, 0, (598.0 * S) as i32, h as i32, rule);
    cv.into_image()
}

/// `Dykty_Juzek`: 1996 x 614, black where nothing is used. Grey plates: four large ones (x 0..570,
/// each 137 high from y 67), four small (x 570..1075, 120 high from y 135) and two blank rear ones
/// (x 1075..1219, 137 high from y 341). The destination in one line of stencil lettering, capitals 70 high,
/// centred in each large and small plate, at most 440 wide (the original's ink is 375).
fn render_dykty(lines: &Lines) -> Image {
    let (w, h) = (1996usize, 614usize);
    let mut cv = Canvas::new(w, h, [0, 0, 0]);
    let ink = ink_or([10.0, 10.0, 10.0]);
    let plate = bg_or(Style::Dykty.colours().0);
    let plate = [plate[0] as f32, plate[1] as f32, plate[2] as f32];
    let (a, b) = (lines.big.trim(), lines.small.trim());
    let text = if b.is_empty() { a.to_string() } else { format!("{a} {b}") };
    let big_rows = [67, 204, 341, 478, 614];
    let small_rows = [135, 255, 375, 495, 614];
    let rear_rows = [341, 478, 614];
    let parts: [(i32, i32, &[i32], f32, f32); 3] = [(0, 570, &big_rows, 287.0, 103.0), (570, 1075, &small_rows, 822.0, 93.0), (1075, 1219, &rear_rows, 0.0, 0.0)];
    for (x0, x1, rows, cx, base) in parts {
        for r in 0..rows.len() - 1 {
            let (y0, y1) = (rows[r], rows[r + 1]);
            paper(&mut cv, x0, y0, x1, y1, plate, 6.0, 2.0, 60.0, (r as u32 + 1) * 13 + x0 as u32);
            if r + 2 < rows.len() {
                cv.rect(x0, y1 - 1, x1, y1, [34, 34, 34]);
            }
            if cx > 0.0 && !text.is_empty() {
                if let Some(run) = fit_ex(&text, 800.0, 75.0, 70.0, 440.0, STENCIL_NARROW) {
                    centred(&mut cv, &run, cx, y0 as f32 + base, ink);
                }
            }
        }
    }
    cv.rect(569, 67, 570, h as i32, [34, 34, 34]);
    cv.rect(1074, 135, 1075, h as i32, [34, 34, 34]);
    cv.into_image()
}

/// The lettering of `Rollband_N40XX`: the narrower cuts of Roboto, squeezed a little when a line asks.
const N40_OPTS: Opts = Opts { sx0: 1.0, min_sx: 0.7, track_em: 0.0, stencil: false };

/// `Rollband_N40XX`: 774 x 64 as the original, drawn at four times that, in three segments
/// parted by a rule at 256 and at 663: the destination on two lines (capitals 19 high on the
/// baselines 24 and 54, at most 232 wide, centred on 128) - on one line, 28 high on 46, when it
/// is a single word that fits -, the destination on one line (28 high on the baseline 46,
/// centred on 460, at most 376 wide) and the line number (46 high on 55, centred on 719, at most
/// 86 wide). Grey-black (26) with the Mirage's light green; the original's boxes for the line
/// number are left out (the originals' ink: 40..218 and 369..547 for a short destination, the
/// number 684..751).
fn render_n40(lines: &Lines) -> Image {
    const S: f32 = 4.0;
    let mut cv = Canvas::new((774.0 * S) as usize, (64.0 * S) as usize, bg_or(Style::N40.colours().0));
    let ink = ink_or([193.0, 213.0, 27.0]);
    for x in [256.0f32, 663.0] {
        cv.rect((x * S) as i32, 0, ((x + 1.0) * S) as i32, (64.0 * S) as i32, [58, 60, 58]);
    }
    let (a, b) = (lines.big.trim(), lines.small.trim());
    // the first segment
    if !b.is_empty() {
        let max_w = 232.0 * S;
        let cap = 19.0 * S;
        let (ra, rb) = (fit_ex(a, 600.0, 80.0, cap, max_w, N40_OPTS), fit_ex(b, 600.0, 80.0, cap, max_w, N40_OPTS));
        if let (Some(ra), Some(rb)) = (ra, rb) {
            // both lines the size of the smaller one
            let cap = ra.cap().min(rb.cap());
            if let (Some(ra), Some(rb)) = (fit_ex_cap(a, 600.0, 80.0, cap, max_w, N40_OPTS), fit_ex_cap(b, 600.0, 80.0, cap, max_w, N40_OPTS)) {
                centred(&mut cv, &ra, 128.0 * S, 24.0 * S, ink);
                centred(&mut cv, &rb, 128.0 * S, 54.0 * S, ink);
            }
        }
    } else if !a.is_empty() {
        if let Some(run) = fit_ex(a, 600.0, 80.0, 28.0 * S, 232.0 * S, N40_OPTS) {
            centred(&mut cv, &run, 128.0 * S, 32.0 * S + run.cap() / 2.0, ink);
        }
    }
    // the second: all of it on one line
    let whole = if b.is_empty() { a.to_string() } else { format!("{a} {b}") };
    if !whole.is_empty() {
        if let Some(run) = fit_ex(&whole, 600.0, 80.0, 28.0 * S, 376.0 * S, N40_OPTS) {
            centred(&mut cv, &run, 460.0 * S, 32.0 * S + run.cap() / 2.0, ink);
        }
    }
    // the third: the line number
    let line = lines.line.trim();
    if !line.is_empty() {
        if let Some(run) = fit_ex(line, 650.0, 90.0, 46.0 * S, 86.0 * S, Opts { min_sx: 0.55, ..N40_OPTS }) {
            centred(&mut cv, &run, 719.0 * S, 32.0 * S + run.cap() / 2.0, ink);
        }
    }
    cv.into_image()
}

/// A plug-in sign (`Steckschild`): 256 x 64 as the original, drawn at four times that. Light
/// grey (228); the text is drawn in the top 35 rows only (the original's marked field, the whole
/// width): one line of capitals 27 high (its ascenders come near the original's 29) on the baseline 31, centred, at most 246 wide (the
/// original's ink is 5..250), the destination's two lines joined; a line that would be made
/// smaller than two thirds goes on two lines (12.5 high, on the baselines 15.5 and 30.5) instead. With a line number it stands at
/// the left (centred on 28, at most 48 wide) and the destination in the rest (centred on 158, at
/// most 186 wide).
fn render_plate(lines: &Lines) -> Image {
    const S: f32 = 4.0;
    let (w, h) = ((256.0 * S) as usize, (64.0 * S) as usize);
    let ground = bg_or(Style::Plate.colours().0);
    let mut cv = Canvas::new(w, h, ground);
    paper(&mut cv, 0, 0, w as i32, h as i32, [ground[0] as f32, ground[1] as f32, ground[2] as f32], 2.0, 1.2, 40.0 * S, 5);
    let black = ink_or([12.0, 12.0, 12.0]);
    let (a, b) = (lines.big.trim(), lines.small.trim());
    let text = if b.is_empty() { a.to_string() } else { format!("{a} {b}") };
    let line = lines.line.trim();
    let (centre, max_w) = if line.is_empty() { (128.0, 246.0) } else { (158.0, 186.0) };
    let opts = Opts { min_sx: 0.6, ..N40_OPTS };
    if !line.is_empty() {
        if let Some(run) = fit_ex(line, 700.0, 80.0, 27.0 * S, 48.0 * S, opts) {
            centred(&mut cv, &run, 28.0 * S, (31.0 - 13.5) * S + run.cap() / 2.0 - 0.0, black);
        }
        // a thin rule between the number and the destination
        cv.rect((55.5 * S) as i32, (3.0 * S) as i32, (56.5 * S) as i32, (32.0 * S) as i32, [60, 60, 60]);
    }
    if !text.is_empty() {
        match fit_ex(&text, 650.0, 80.0, 27.0 * S, max_w * S, opts) {
            Some(run) if run.cap() >= 27.0 * S * cur().size * 0.67 || !text.contains(' ') => {
                centred(&mut cv, &run, centre * S, (31.0 - 13.5) * S + run.cap() / 2.0 - 0.0, black)
            }
            _ => {
                let (x, y) = split_two(&text);
                let cap = 12.5 * S;
                let (ra, rb) = (fit_ex(&x, 650.0, 80.0, cap, max_w * S, opts), fit_ex(&y, 650.0, 80.0, cap, max_w * S, opts));
                if let (Some(ra), Some(rb)) = (ra, rb) {
                    let cap = ra.cap().min(rb.cap());
                    if let (Some(ra), Some(rb)) = (fit_ex_cap(&x, 650.0, 80.0, cap, max_w * S, opts), fit_ex_cap(&y, 650.0, 80.0, cap, max_w * S, opts)) {
                        centred(&mut cv, &ra, centre * S, 15.5 * S, black);
                        centred(&mut cv, &rb, centre * S, 30.5 * S, black);
                    }
                }
            }
        }
    }
    cv.into_image()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_look_is_kept_in_a_text_and_read_back() {
        let mut all = [Tweak::default(); 8];
        all[1] = Tweak { bg: Some([1, 2, 3]), ink: Some([250, 128, 0]), size: 0.8, second: 1.25, width: 0.9, letters: 1.1, weight: 150.0, track: 0.05, shift: -0.02, caps: true, font: Some(intern_font("/fonts/Some Font.ttf")) };
        let back = tweaks_from_text(&tweaks_to_text(&all));
        assert_eq!(back[1].bg, Some([1, 2, 3]));
        assert_eq!(back[1].ink, Some([250, 128, 0]));
        assert!((back[1].size - 0.8).abs() < 1e-3 && (back[1].second - 1.25).abs() < 1e-3 && (back[1].weight - 150.0).abs() < 0.1);
        assert!(back[1].caps && back[0].is_default() && back[7].is_default());
        assert_eq!(back[1].font, Some("/fonts/Some Font.ttf"));
        // (a folder left as it is is not written; nonsense is ignored, numbers are held to a range)
        assert!(!tweaks_to_text(&all).contains("[sd79]"));
        let odd = tweaks_from_text("[sd79]\nsize=99\nbg=zzz\nink=#102030\n[nothing]\nsize=2\n");
        assert_eq!((odd[0].size, odd[0].bg, odd[0].ink), (2.0, None, Some([16, 32, 48])));
        assert_eq!(parse_hex("#FFaa00"), Some([255, 170, 0]));
        assert_eq!(parse_hex("12345"), None);
        assert_eq!(hex_of([1, 171, 255]), "#01abff");
    }

    #[test]
    fn a_look_changes_the_picture_and_the_default_does_not() {
        let l = Lines { line: "76".into(), big: "Zehlendorf".into(), small: "Sachtlebenstr.".into() };
        for st in STYLES {
            let plain = render_with(st, &l, &Tweak::default());
            assert_eq!(plain.rgba, render_with(st, &l, &Tweak::default()).rgba);
            let t = Tweak { bg: Some([9, 40, 90]), ..Tweak::default() };
            let blue = render_with(st, &l, &t);
            assert_eq!((blue.width, blue.height), (plain.width, plain.height));
            // (a corner of the ground, off the lettering; the Dykty's black outside its plates is no ground)
            if st != Style::Dykty {
                let px = &blue.rgba[..3];
                assert!((px[2] as i32 - 90).abs() < 40 && px[0] < 60, "{st:?}: {px:?}");
            }
            let big = render_with(st, &l, &Tweak { size: 0.5, ink: Some([255, 0, 0]), ..Tweak::default() });
            assert_ne!(big.rgba, plain.rgba, "{st:?}");
        }
    }

    #[test]
    fn a_font_that_cannot_be_read_leaves_the_built_in_one() {
        let l = Lines { line: String::new(), big: "Zehlendorf".into(), small: String::new() };
        let plain = render_with(Style::Phantom, &l, &Tweak::default());
        let lost = render_with(Style::Phantom, &l, &Tweak { font: Some(intern_font("/no/such/font.ttf")), ..Tweak::default() });
        assert_eq!(plain.rgba, lost.rgba);
        // (the folders are looked for without a failure, whatever is in them)
        let _ = local_fonts();
    }

    #[test]
    fn capitals_and_the_size_reach_the_lettering() {
        let l = Lines { line: String::new(), big: "Zehlendorf".into(), small: String::new() };
        let a = render_with(Style::Phantom, &l, &Tweak::default());
        let b = render_with(Style::Phantom, &l, &Tweak { caps: true, ..Tweak::default() });
        let c = render_with(Style::Phantom, &l, &Tweak { size: 0.5, ..Tweak::default() });
        let ink = |i: &Image| i.rgba.chunks(4).filter(|p| p[0] > 128).count();
        assert_ne!(a.rgba, b.rgba);
        assert!(ink(&c) < ink(&a) * 2 / 3, "half the capitals, far less ink: {} against {}", ink(&c), ink(&a));
        // a text that is not cramped is not split into two lines because the size was set smaller
        let d = render_with(Style::Sd79, &Lines { line: String::new(), big: "Berlin Mitte".into(), small: String::new() }, &Tweak { size: 0.6, ..Tweak::default() });
        let rows: Vec<usize> = (0..d.height as usize).filter(|y| (0..d.width as usize).any(|x| d.rgba[(y * d.width as usize + x) * 4] < 100)).collect();
        assert!(rows.last().unwrap() - rows.first().unwrap() < d.height as usize / 2, "one line");
    }

    #[test]
    fn the_blind_is_known_by_its_folder() {
        assert_eq!(style_of(r"..\..\Anzeigen\Rollband_SD79\Zehlendorf.bmp"), Some(Style::Sd79));
        assert_eq!(style_of("Anzeigen/Rollband_O307_V2/76_x.tga"), Some(Style::O307));
        assert_eq!(style_of(r"Anzeigen\Steckschild_Berlin\a.bmp"), Some(Style::Plate));
        assert_eq!(style_of(r"..\..\Anzeigen\Rollband_Phantom\Totnes.bmp"), Some(Style::Phantom));
        assert_eq!(style_of(r"..\..\Anzeigen\Rollband_Mirage\Impingham.tga"), Some(Style::Mirage));
        assert_eq!(style_of("Anzeigen/PR110/12_Szczawienko.bmp"), Some(Style::Pr110));
        assert_eq!(style_of(r"Anzeigen\Dykty_Juzek\Weglin.bmp"), Some(Style::Dykty));
        assert_eq!(style_of(r"..\..\Anzeigen\Rollband_N40XX\191_Torow.bmp"), Some(Style::N40));
        // (a plug-in sign's folder holding a folder of its own: the inner one rules)
        assert_eq!(style_of(r"Anzeigen\Steckschild\Dykty_Juzek\Weglin.bmp"), Some(Style::Dykty));
        assert_eq!(style_of(r"Anzeigen\Steckschild\Falkensee.bmp"), Some(Style::Plate));
        assert_eq!(style_of(r"..\..\Anzeigen\Rollband_FC\Paris\17.tga"), None);
        // the folder only: a file named like a blind in no folder of one is no blind's
        assert_eq!(style_of("Rollband_SD79.bmp"), None);
        assert!(Style::O307.wants_line() && Style::Pr110.wants_line() && Style::N40.wants_line() && Style::Plate.wants_line());
        assert!(!Style::Sd79.wants_line() && !Style::Mirage.wants_line() && !Style::Dykty.wants_line());
    }

    #[test]
    fn the_lines_come_from_the_strings_of_the_destination() {
        // whole text, upper, lower, then pictures
        let s = strings(&["ZEHLENDORF SACHTLEBENSTR.", "ZEHLENDORF", "SACHTLEBENSTR.", r"Rollband_SD79\Zehlendorf.bmp"]);
        // (as the file has them: no change of case)
        assert_eq!(pick_lines(&s), ("ZEHLENDORF".to_string(), "SACHTLEBENSTR.".to_string()));
        assert_eq!(pick_lines(&strings(&["Löptener Str", "Pythagorasdorf", "via Totnes"])), ("Pythagorasdorf".to_string(), "via Totnes".to_string()));
        // one line only: string 0 is the destination
        assert_eq!(pick_lines(&strings(&["Pythagorasdorf", "", ""])), ("Pythagorasdorf".to_string(), String::new()));
        // a picture's name is no text
        assert_eq!(pick_lines(&strings(&[r"Rollband_SD79\x.bmp", "", ""])), (String::new(), String::new()));
        assert_eq!(pick_lines(&[]), (String::new(), String::new()));
    }

    #[test]
    fn a_picture_path_finds_its_destination() {
        let termini = vec![
            strings(&["A", "A", "", r"Rollband_SD79\Alpha.bmp"]),
            strings(&["B", "B", "", r"Rollband_SD79\Zehlendorf.bmp", r"Rollband_O307_V2\76_Zehlendorf.bmp"]),
            strings(&["C", "C", "", "Charlie.bmp"]),
        ];
        assert_eq!(match_terminus(&termini, r"..\..\Anzeigen\Rollband_SD79\Zehlendorf.bmp"), Some(1));
        assert_eq!(match_terminus(&termini, r"Rollband_SD79\zehlendorf.BMP"), Some(1));
        assert_eq!(match_terminus(&termini, r"..\..\Anzeigen\Rollband_O307_V2\76_Zehlendorf.bmp"), Some(1));
        // a file listed without a folder is found in any
        assert_eq!(match_terminus(&termini, r"..\Anzeigen\Rollband_SD79\Charlie.bmp"), Some(2));
        // the same file in another blind's folder is not the one listed
        assert_eq!(match_terminus(&termini, r"Rollband_O307_V2\Alpha.bmp"), None);
        assert_eq!(match_terminus(&termini, ""), None);
        assert_eq!(text_of_name(r"..\Rollband_SD79\Zehlendorf_Sachtlebenstr.bmp"), "Zehlendorf Sachtlebenstr");
    }

    fn ink_bbox(img: &Image, f: impl Fn(&[u8]) -> bool) -> Option<(usize, usize, usize, usize)> {
        let (w, h) = (img.width as usize, img.height as usize);
        let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
        for y in 0..h {
            for x in 0..w {
                if f(&img.rgba[(y * w + x) * 4..(y * w + x) * 4 + 4]) {
                    x0 = x0.min(x);
                    x1 = x1.max(x);
                    y0 = y0.min(y);
                    y1 = y1.max(y);
                }
            }
        }
        (x1 >= x0 && y1 >= y0).then_some((x0, y0, x1, y1))
    }

    #[test]
    fn the_sd79_picture_is_the_originals_layout_four_times_larger() {
        let img = render(Style::Sd79, &Lines { big: "Zehlendorf".into(), small: "Sachtlebenstr.".into(), ..Default::default() });
        assert_eq!((img.width, img.height), (1024, 256));
        // white all over but the text, which stays inside the field 38..218 x 4..58 (x4)
        assert_eq!(&img.rgba[..4], &[255, 255, 255, 255]);
        assert!(ink_bbox(&img, |p| p[1] < 200 && p[0] > 200 && p[2] < 60).is_none(), "no red");
        let black = ink_bbox(&img, |p| p[0] < 60 && p[1] < 60 && p[2] < 60).unwrap();
        assert!(black.0 > 152 && black.2 < 871 && black.1 > 16 && black.3 < 231, "{black:?}");
        // the destination's capitals reach about 8 (x4 = 32) from the top, the second line ends near 53 (x4 = 212)
        assert!((24..=48).contains(&black.1), "{black:?}");
        assert!((196..=222).contains(&black.3), "{black:?}");
        // centred: the middle of the ink is the middle of the picture
        assert!((black.0 + black.2).abs_diff(1024) < 40, "{black:?}");
    }

    #[test]
    fn a_long_destination_is_made_to_fit() {
        let long = "Sehr langer Zielname der nicht auf ein Rollband passt";
        let img = render(Style::Sd79, &Lines { big: long.into(), ..Default::default() });
        let black = ink_bbox(&img, |p| p[0] < 60 && p[1] < 60 && p[2] < 60).unwrap();
        assert!(black.0 >= 152 && black.2 <= 871, "{black:?}");
        let o = render(Style::O307, &Lines { line: "76".into(), big: long.into(), small: "x".into() });
        let dark = ink_bbox(&o, |p| p[0] < 70 && p[1] < 70 && p[2] < 70).unwrap();
        assert!(dark.2 < 1720, "{dark:?}");
        // nothing beyond the margins: the destination in x 430..825, y 30..146 (x2), a line number in 36..136
        for text in [long, "Pythagorasdorf", "Gögging Jägerstraße", "Wwwwwwwwwwwwwwwwwwwwwwwwwww"] {
            let img = render(Style::O307, &Lines { line: "100".into(), big: text.into(), small: "Qyg, Löptener Straße".into() });
            let r = ink_bbox_in(&img, 880, 1720, |p| p[0] < 70).unwrap();
            assert!(r.0 >= 860 && r.2 <= 1650 && r.1 >= 60 && r.3 <= 292, "{text}: {r:?}");
            for (from, to, centre) in [(0, 420, 208), (430, 870, 654)] {
                let n = ink_bbox_in(&img, from, to, |p| p[0] < 70).unwrap();
                assert!(n.1 >= 72 && n.3 <= 272 && n.0 >= centre - 140 && n.2 <= centre + 140, "{text}: {n:?}");
            }
        }
    }

    #[test]
    fn the_o307_picture_has_the_line_twice_and_the_destination() {
        let img = render(Style::O307, &Lines { line: "76".into(), big: "Pythagorasdorf".into(), small: "Löptener Str".into() });
        assert_eq!((img.width, img.height), (1720, 344));
        // paper, not a flat colour
        let a = &img.rgba[(10 * 1720 + 10) * 4..][..3];
        let b = &img.rgba[(172 * 1720 + 860) * 4..][..3];
        assert!(b[0] > a[0] + 20, "{a:?} {b:?}");
        // the line number in the left panel and the middle one, each about 85 (x2) high
        let left = ink_bbox_in(&img, 0, 400, |p| p[0] < 70);
        let mid = ink_bbox_in(&img, 430, 880, |p| p[0] < 70);
        let right = ink_bbox_in(&img, 880, 1720, |p| p[0] < 70);
        let (l, m, r) = (left.unwrap(), mid.unwrap(), right.unwrap());
        assert!((160..=200).contains(&(l.3 - l.1)), "{l:?}");
        assert!((l.3 as i32 - m.3 as i32).abs() <= 4, "{l:?} {m:?}");
        // the destination to the right, centred near 627 (x2)
        assert!((r.0 + r.2).abs_diff(2 * 1254) < 120, "{r:?}");
        // no line number: the panels stay empty
        let none = render(Style::O307, &Lines { big: "Pythagorasdorf".into(), ..Default::default() });
        assert!(ink_bbox_in(&none, 0, 400, |p| p[0] < 70).is_none());
        // [ROLLBACK o307-1] ... and the destination is centred in the window right of the rule
        let d = ink_bbox_in(&none, 430, 1720, |p| p[0] < 70).expect("the destination");
        assert!((d.0 + d.2).abs_diff(2 * 1080) < 120, "{d:?}");
    }

    fn ink_bbox_in(img: &Image, x_from: usize, x_to: usize, f: impl Fn(&[u8]) -> bool) -> Option<(usize, usize, usize, usize)> {
        let (w, h) = (img.width as usize, img.height as usize);
        let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
        for y in 0..h {
            for x in x_from..x_to.min(w) {
                let i = (y * w + x) * 4;
                // (not the rule: a dark one-pixel column spanning the whole height)
                if f(&img.rgba[i..i + 4]) && !((x as i32 - 422).abs() <= 6) {
                    x0 = x0.min(x);
                    x1 = x1.max(x);
                    y0 = y0.min(y);
                    y1 = y1.max(y);
                }
            }
        }
        (x1 >= x0 && y1 >= y0).then_some((x0, y0, x1, y1))
    }

    #[test]
    fn a_suffix_after_a_star_is_not_drawn() {
        assert_eq!(cut_suffix("Weeze*B"), "Weeze");
        assert_eq!(cut_suffix("Weeze *B"), "Weeze");
        assert_eq!(cut_suffix("Flughafen"), "Flughafen");
        assert_eq!(cut_suffix("  Köln Hbf*12*x "), "Köln Hbf");
        assert_eq!(cut_suffix("*B"), "");
        for style in [Style::Sd79, Style::O307, Style::Mirage, Style::N40, Style::Plate, Style::Pr110] {
            let with = render(style, &Lines { line: "123".into(), big: "Flughafen*A".into(), small: "Weeze*B".into() });
            let without = render(style, &Lines { line: "123".into(), big: "Flughafen".into(), small: "Weeze".into() });
            assert!(with.rgba == without.rgba, "{style:?}");
        }
        // the line number is not cut
        let a = render(Style::O307, &Lines { line: "N*5".into(), big: "A".into(), small: String::new() });
        let b = render(Style::O307, &Lines { line: "N".into(), big: "A".into(), small: String::new() });
        assert!(a.rgba != b.rgba);
    }

    #[test]
    fn the_o307_line_number_is_larger_than_the_originals() {
        let img = render(Style::O307, &Lines { line: "76".into(), big: "Flughafen".into(), small: "Weeze".into() });
        let dark = |p: &[u8]| p[0] < 80;
        let l = ink_bbox_in(&img, 0, 420, dark).unwrap();
        // 92 high (x2), where the original's was 85; inside the field 34..139 (x2)
        assert!((176..=196).contains(&(l.3 - l.1)), "{l:?}");
        assert!(l.1 >= 34 * 2 + 8 && l.3 <= 139 * 2 - 8, "{l:?}");
        // three digits keep the height (narrowed instead)
        let img = render(Style::O307, &Lines { line: "123".into(), big: "Flughafen".into(), ..Default::default() });
        let l = ink_bbox_in(&img, 0, 420, dark).unwrap();
        assert!((168..=196).contains(&(l.3 - l.1)) && l.2 - l.0 <= 280, "{l:?}");
    }

    #[test]
    fn the_plug_in_sign_keeps_its_text_in_the_top_rows() {
        let img = render(Style::Plate, &Lines { big: "Falkensee".into(), small: "Bahnhof".into(), ..Default::default() });
        assert_eq!((img.width, img.height), (1024, 256));
        // light grey all over (the original's 226..230), the text black in the field 0..256 x 0..35 (x4)
        let p = &img.rgba[(200 * 1024 + 500) * 4..][..3];
        assert!(p[0] > 215 && p[0] < 240, "{p:?}");
        let b = ink_bbox(&img, |p| p[0] < 90).unwrap();
        // the original's ink: 5..250 across and 2..30 down for a line that fills it
        assert!(b.0 >= 4 * 4 && b.2 <= 252 * 4 && b.1 >= 1 * 4 && b.3 <= 35 * 4, "{b:?}");
        for line in ["", "76", "N83E", "100"] {
            for text in TEXTS {
                let img = render(Style::Plate, &Lines { line: line.into(), big: text.into(), small: "Qyg via Löptener".into() });
                let b = ink_bbox(&img, |p| p[0] < 90).unwrap();
                assert!(b.0 >= 2 * 4 && b.2 <= 254 * 4 && b.1 >= 4 && b.3 <= 35 * 4, "{line} {text}: {b:?}");
            }
        }
        // the line number at the left, the destination right of it
        let img = render(Style::Plate, &Lines { line: "76".into(), big: "Falkensee Bahnhof".into(), ..Default::default() });
        let n = ink_in(&img, (0, 0, 55 * 4, 36 * 4), |p| p[0] < 90).unwrap();
        let d = ink_in(&img, (58 * 4, 0, 256 * 4, 36 * 4), |p| p[0] < 90).unwrap();
        assert!(n.2 < 55 * 4 && d.0 >= 58 * 4 && d.2 <= 254 * 4, "{n:?} {d:?}");
        // the rows under the field stay empty
        assert!(ink_in(&img, (0, 36 * 4, 1024, 256), |p| p[0] < 150).is_none());
    }

    #[test]
    fn the_n40_picture_has_three_segments_and_the_line_number_last() {
        let img = render(Style::N40, &Lines { line: "191".into(), big: "Torów".into(), small: "Śródmieście".into() });
        assert_eq!((img.width, img.height), (3096, 256));
        assert_eq!(&img.rgba[..4], &[26, 27, 26, 255]);
        let lit = |p: &[u8]| p[1] > 120;
        // (the segments: 0..256, 257..663 and 664..774, x4)
        let s1 = ink_in(&img, (0, 0, 256 * 4, 256), lit).unwrap();
        let s2 = ink_in(&img, (258 * 4, 0, 662 * 4, 256), lit).unwrap();
        let s3 = ink_in(&img, (665 * 4, 0, 3096, 256), lit).unwrap();
        // two lines in the first: it takes about the whole height; one line in the second
        assert!(s1.3 - s1.1 > 36 * 4, "{s1:?}");
        assert!(s2.3 - s2.1 < 36 * 4 && s2.3 - s2.1 > 20 * 4, "{s2:?}");
        // the whole text in the second, centred on 460 (x4)
        assert!((s2.0 + s2.2).abs_diff(2 * 460 * 4) < 30 * 4, "{s2:?}");
        // the number: 46 high (the original's 191 is 9..54), in the last segment
        assert!((40 * 4..=50 * 4).contains(&(s3.3 - s3.1)), "{s3:?}");
        assert!(s3.0 > 668 * 4 && s3.2 < 770 * 4, "{s3:?}");
        // light green
        let green = img.rgba.chunks(4).any(|p| p[0].abs_diff(193) < 6 && p[1].abs_diff(213) < 6 && p[2].abs_diff(27) < 8);
        assert!(green, "the Mirage's green");
        // never across a rule, whatever the text
        for text in TEXTS {
            for line in ["", "1", "N40E", "1234"] {
                let img = render(Style::N40, &Lines { line: line.into(), big: text.into(), small: String::new() });
                let (a, b, c) = (ink_in(&img, (0, 0, 256 * 4, 256), lit), ink_in(&img, (258 * 4, 0, 662 * 4, 256), lit), ink_in(&img, (665 * 4, 0, 3096, 256), lit));
                let (a, b) = (a.unwrap(), b.unwrap());
                assert!(a.0 >= 8 * 4 && a.2 <= 248 * 4 && a.1 >= 2 * 4 && a.3 <= 62 * 4, "{text}: {a:?}");
                assert!(b.0 >= 264 * 4 && b.2 <= 656 * 4 && b.1 >= 4 * 4 && b.3 <= 62 * 4, "{text}: {b:?}");
                match c {
                    Some(c) => assert!(c.0 >= 670 * 4 && c.2 <= 768 * 4 && c.1 >= 4 * 4 && c.3 <= 60 * 4, "{line}: {c:?}"),
                    None => assert!(line.is_empty()),
                }
            }
        }
        // a single word stays on one line in the first segment
        let one = render(Style::N40, &Lines { big: "Zehlendorf".into(), ..Default::default() });
        let s1 = ink_in(&one, (0, 0, 256 * 4, 256), lit).unwrap();
        assert!(s1.3 - s1.1 < 36 * 4, "{s1:?}");
        // a text of one line that does not fit the first segment is split in two there
        let long = render(Style::N40, &Lines { big: "Torów Śródmieście".into(), ..Default::default() });
        let s1 = ink_in(&long, (0, 0, 256 * 4, 256), lit).unwrap();
        assert!(s1.3 - s1.1 > 36 * 4, "two lines: {s1:?}");
    }

    /// The ink (pixels `f` holds) inside the box `x0..x1` x `y0..y1`.
    fn ink_in(img: &Image, (x0, y0, x1, y1): (usize, usize, usize, usize), f: impl Fn(&[u8]) -> bool) -> Option<(usize, usize, usize, usize)> {
        let w = img.width as usize;
        let (mut bx0, mut by0, mut bx1, mut by1) = (usize::MAX, usize::MAX, 0, 0);
        for y in y0..y1.min(img.height as usize) {
            for x in x0..x1.min(w) {
                let i = (y * w + x) * 4;
                if f(&img.rgba[i..i + 4]) {
                    bx0 = bx0.min(x);
                    bx1 = bx1.max(x);
                    by0 = by0.min(y);
                    by1 = by1.max(y);
                }
            }
        }
        (bx1 >= bx0 && by1 >= by0).then_some((bx0, by0, bx1, by1))
    }

    const TEXTS: [&str; 4] = ["Pythagorasdorf", "Sehr langer Zielname der nicht auf ein Rollband passt", "Wwwwwwwwwwwwwwwwwwwwwwwwwww", "Gögging Jägerstraße"];

    #[test]
    fn the_o307_ink_stays_in_the_fields_of_the_original() {
        // the original's fields (860 x 172): 19..195 and 237..413 for the line number, 431..823 for the
        // destination, all 34..139 down; the picture is twice that
        for line in ["76", "7", "100", "N5E"] {
            for text in TEXTS {
                let img = render(Style::O307, &Lines { line: line.into(), big: text.into(), small: "Qyg, Löptener Straße".into() });
                let dark = |p: &[u8]| p[0] < 80 && p[1] < 80 && p[2] < 80;
                let (l, m, r) = (ink_bbox_in(&img, 0, 420, dark).unwrap(), ink_bbox_in(&img, 430, 850, dark).unwrap(), ink_bbox_in(&img, 850, 1720, dark).unwrap());
                for (b, (fx0, fx1)) in [(l, (19, 195)), (m, (237, 413)), (r, (431, 823))] {
                    assert!(b.0 >= fx0 * 2 + 4 && b.2 <= fx1 * 2 - 4 && b.1 >= 34 * 2 && b.3 <= 139 * 2, "{line} {text}: {b:?} not in {fx0}..{fx1}");
                }
            }
        }
    }

    #[test]
    fn the_phantom_picture_uses_the_whole_picture_with_a_small_margin() {
        let img = render(Style::Phantom, &Lines { big: "Littlehempston".into(), small: "via Totnes".into(), ..Default::default() });
        assert_eq!((img.width, img.height), (1024, 256));
        assert_eq!(&img.rgba[..4], &[29, 29, 29, 255]);
        let white = ink_bbox(&img, |p| p[0] > 128).unwrap();
        // the original's ink: 22..232 across, 10..57 down (x4)
        assert!(white.0 >= 16 * 4 && white.2 <= 240 * 4 && white.1 >= 6 * 4 && white.3 <= 60 * 4, "{white:?}");
        assert!(white.2 - white.0 > 150 * 4, "a destination this long fills the picture: {white:?}");
        for text in TEXTS {
            let img = render(Style::Phantom, &Lines { big: text.into(), small: "Qyg via Löptener".into(), ..Default::default() });
            let b = ink_bbox(&img, |p| p[0] > 128).unwrap();
            assert!(b.0 >= 14 * 4 && b.2 <= 242 * 4 && b.1 >= 4 * 4 && b.3 <= 62 * 4, "{text}: {b:?}");
        }
        // a destination alone stands in the middle
        let one = render(Style::Phantom, &Lines { big: "Totnes".into(), ..Default::default() });
        let b = ink_bbox(&one, |p| p[0] > 128).unwrap();
        assert!(((b.1 + b.3) as i32 - 256).abs() < 40, "{b:?}");
    }

    #[test]
    fn the_mirage_picture_keeps_its_margins() {
        let img = render(Style::Mirage, &Lines { big: "Impingham".into(), small: "Bus Station".into(), ..Default::default() });
        assert_eq!((img.width, img.height), (2048, 512));
        assert_eq!(&img.rgba[..4], &[20, 20, 20, 255]);
        let lit = |p: &[u8]| p[1] > 100;
        let b = ink_bbox(&img, lit).unwrap();
        // the original's ink: 132..886 across, 41..219 down (x2); a margin of 100 stays at the sides
        assert!(b.0 >= 100 * 2 && b.2 <= 924 * 2 && b.1 >= 36 * 2 && b.3 <= 232 * 2, "{b:?}");
        assert!((b.0 + b.2).abs_diff(2 * 509 * 2) < 40 * 2, "centred: {b:?}");
        for text in TEXTS {
            let img = render(Style::Mirage, &Lines { big: text.into(), small: "Qyg, Löptener Straße via Gögging".into(), ..Default::default() });
            let b = ink_bbox(&img, lit).unwrap();
            assert!(b.0 >= 100 * 2 && b.2 <= 924 * 2 && b.1 >= 30 * 2 && b.3 <= 240 * 2, "{text}: {b:?}");
        }
    }

    #[test]
    fn the_pr110_cells_hold_their_texts_and_nothing_crosses_a_rule() {
        let cells = [(0usize, 0usize, 596usize, 151usize), (598, 0, 1024, 151), (0, 153, 596, 304), (598, 153, 1024, 304)];
        let texts = [("12", "DWORZEC GŁ.", "SZCZAWIENKO"), ("100", "Glinik Stary Konradów Wielki Bardzo", ""), ("N5E", "Węglin", "D.A."), ("7", "", "")];
        for (line, a, b) in texts {
            let img = render(Style::Pr110, &Lines { line: line.into(), big: a.into(), small: b.into() });
            assert_eq!((img.width, img.height), (2048, 608));
            for (x0, y0, x1, y1) in cells {
                let bx = ink_in(&img, ((x0 + 3) * 2, (y0 + 3) * 2, (x1 - 3) * 2, (y1 - 3) * 2), |p| p[0] < 90).expect("ink");
                // 4 px of paper (of the original's) at the least all round; the vertical rule and the horizontal one stay clear
                assert!(bx.0 >= (x0 + 4) * 2 && bx.2 <= (x1 - 5) * 2 && bx.1 >= (y0 + 4) * 2 && bx.3 <= (y1 - 4) * 2, "{line} {a}: {bx:?} in cell {x0},{y0}");
            }
        }
        // the beige of the original
        let img = render(Style::Pr110, &Lines::default());
        let p = &img.rgba[(20 * 2048 + 1000) * 4..][..3];
        assert!(p[0] > 190 && p[0] < 225 && p[2] < 190, "{p:?}");
    }

    #[test]
    fn a_text_of_one_line_is_set_on_two_where_the_picture_is_made_for_two() {
        assert_eq!(split_two("DWORZEC GŁ. SZCZAWIENKO"), ("DWORZEC GŁ.".to_string(), "SZCZAWIENKO".to_string()));
        assert_eq!(split_two("ONE"), ("ONE".to_string(), String::new()));
        // the second line set smaller takes more
        let (a, b) = split_two_ratio("Berlin Hauptbahnhof Europaplatz", 0.7);
        assert!(a.len() < b.len(), "{a} / {b}");
        let one = |t: &str| Lines { line: "5".into(), big: t.into(), small: String::new() };
        // PR110: always two, a single word aside, and a short name with a tiny first word too
        let l = auto_split(Style::Pr110, &one("Dworzec Gł. Szczawienko"));
        assert_eq!((l.big.as_str(), l.small.as_str(), l.line.as_str()), ("Dworzec Gł.", "Szczawienko", "5"));
        let l = auto_split(Style::Pr110, &one("Rynek Główny"));
        assert_eq!((l.big.as_str(), l.small.as_str()), ("Rynek", "Główny"));
        assert_eq!(auto_split(Style::Pr110, &one("Rynek")).small, "");
        assert_eq!(auto_split(Style::Pr110, &one("U Ruhleben")).small, "");
        // the others: where one line would be cramped
        assert_eq!(auto_split(Style::Sd79, &one("Park Ost")).small, "");
        let l = auto_split(Style::Sd79, &one("Berlin Hauptbahnhof Europaplatz"));
        assert!(!l.small.is_empty() && format!("{} {}", l.big, l.small) == "Berlin Hauptbahnhof Europaplatz", "{l:?}");
        for style in [Style::O307, Style::Phantom, Style::Mirage, Style::N40] {
            let l = auto_split(style, &one("Berlin Hauptbahnhof Europaplatz Nord Ausgang"));
            assert!(!l.small.is_empty(), "{style:?}: {l:?}");
        }
        // two lines are left as they are; plates are one line
        let two = Lines { line: String::new(), big: "A B C D E F G H I J K L M N".into(), small: "x".into() };
        assert_eq!(auto_split(Style::Pr110, &two), two);
        assert_eq!(auto_split(Style::Dykty, &one("Dworzec Gł. Szczawienko")).small, "");
        // and it shows in the picture: the long text of one line is as high as a two-line one
        let long = render(Style::Sd79, &one("Berlin Hauptbahnhof Europaplatz"));
        let b = ink_bbox(&long, |p| p[0] < 60).unwrap();
        assert!(b.3 - b.1 > 120, "two lines: {b:?}");
    }

    #[test]
    fn the_dykty_plates_carry_one_line_each_and_the_rear_ones_none() {
        let img = render(Style::Dykty, &Lines { big: "Węglin D.A.".into(), ..Default::default() });
        assert_eq!((img.width, img.height), (1996, 614));
        // black where no plate is
        assert_eq!(&img.rgba[(10 * 1996 + 10) * 4..][..4], &[0, 0, 0, 255]);
        assert_eq!(&img.rgba[(300 * 1996 + 1500) * 4..][..4], &[0, 0, 0, 255]);
        let big_rows = [67usize, 204, 341, 478, 614];
        let small_rows = [135usize, 255, 375, 495, 614];
        for (x0, x1, rows) in [(0usize, 570usize, big_rows), (570, 1075, small_rows)] {
            for r in 0..4 {
                let b = ink_in(&img, (x0 + 4, rows[r] + 3, x1 - 4, rows[r + 1] - 3), |p| p[0] == 10 || (p[0] < 40 && p[0] > 0 && p[1] < 40)).expect("text");
                assert!(b.0 >= x0 + 40 && b.2 <= x1 - 40 && b.1 >= rows[r] + 8 && b.3 <= rows[r + 1] - 6, "plate {x0} row {r}: {b:?}");
                // about 375 wide at 70 high, as the original
                assert!((300..=450).contains(&(b.2 - b.0)), "{b:?}");
            }
        }
        // nothing on the rear plates
        assert!(ink_in(&img, (1080, 345, 1215, 474), |p| p[0] < 100).is_none());
        assert!(ink_in(&img, (1080, 482, 1215, 610), |p| p[0] < 100).is_none());
        // a long text still fits in its plate
        let img = render(Style::Dykty, &Lines { big: "Sehr langer Zielname der nicht auf eine Tafel passt".into(), small: "und mehr".into(), ..Default::default() });
        let b = ink_in(&img, (4, 70, 566, 200), |p| p[0] == 10).unwrap();
        assert!(b.0 >= 40 && b.2 <= 530, "{b:?}");
    }
}
