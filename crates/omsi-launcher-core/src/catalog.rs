//! The bus catalogue: the player's own database that says under which names every installed
//! bus is shown, and what kind of bus it is.
//!
//! Vehicle packs name their buses however their authors liked: the manufacturer may be
//! "Mercedes", "MB" or empty, one model may be spread over a dozen `.bus` files that differ in
//! gearbox or engine. There are no rules in the code: the names are data, and the game comes
//! with a catalogue of the buses it knows (`data/bus_catalog_default.json`, compiled in), which
//! is what a player starts with. The player's own copy (`bus_catalog.json` in the data folder)
//! takes its place as soon as there is one, and every change is written to it. Entries for
//! buses the player has not installed (a pack he does not own) are simply never looked up. It
//! has three tables:
//!
//! * `types`: a model as the `.bus` files write it (manufacturer and type name) -> the
//!   manufacturer's and the model's name in the game, the rest of the name as a *variant*
//!   (engine, gearbox ...), and whether Career Mode's dealership leaves it out by default. The
//!   manufacturer is set for each model alone: what a `.bus` file calls its manufacturer may be
//!   the name of a pack with buses of several makes in it,
//! * `makers`: only the manufacturer a model *not yet* in `types` is shown under (a default the
//!   player can give a whole pack's manufacturer name at once; models in `types` ignore it),
//! * `models`: what a model in the game is: its *size* (MINI, MIDI, MAXI, MEGA), its *class*
//!   (City, Intercity, Long-distance, Double-decker, Minibus) and the years it was built.
//!
//! A bus that is in no table is shown with the names its files give and a guess for size and
//! class (`mapped` is false for it, so a page can show what is left to do).
//!
//! Nothing here touches the GPU or the game: [`Catalog::resolve`] is a pure function of a
//! bus's text, so the Drive page, the AI list and Career Mode can all use the same answer.
//! The catalogue can be exported to a file, imported from one (replacing everything) or merged
//! from one (only the entries that are new); the compiled-in one can be put back the same two
//! ways ([`Catalog::import_defaults`]).

use crate::{display_bus_name, vehicle_type_label, VehicleInfo};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The sizes, smallest first.
pub const SIZES: [&str; 4] = ["MINI", "MIDI", "MAXI", "MEGA"];
/// The classes, in the order the lists offer them.
pub const CLASSES: [&str; 5] = ["City", "Intercity", "Long-distance", "Double-decker", "Minibus"];

/// The file format's version. A file of another version is not read (it is kept beside as
/// `.old`).
pub const VERSION: u32 = 2;

pub const UNKNOWN_MAKER: &str = "Unknown manufacturer";

/// The catalogue the game comes with: the file `bus_catalog_export.json` of the launcher's
/// Settings > Bus catalogue > Export, kept as it was written. To change what new players start
/// with, export a catalogue and put the file here.
const DEFAULT_JSON: &str = include_str!("../data/bus_catalog_default.json");

const HELP: &str = "The bus catalogue. makers: the manufacturer as the .bus files write it -> its name in the game. types: a model as the .bus files write it (manufacturer | type name) -> its model name in the game, the rest of the name as the variant (engine, gearbox ...), and ignore = left out of Career Mode's dealership by default. models: size (MINI, MIDI, MAXI, MEGA), class (City, Intercity, Long-distance, Double-decker, Minibus) and the years built (to = 0: still built). Edit it in the launcher (Settings > Bus catalogue) rather than by hand.";

fn version() -> u32 {
    VERSION
}

/// A manufacturer as the files write it, and its name in the game.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct MakerDef {
    #[serde(default)]
    pub raw: String,
    pub name: String,
}

/// A model as the files write it, and where it goes in the game.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct TypeDef {
    #[serde(default)]
    pub raw_maker: String,
    #[serde(default)]
    pub raw_model: String,
    /// The manufacturer's name in the game, for this model only (a pack's manufacturer may be
    /// the name of the pack, with Mercedes and Setra buses in it). Empty: `makers` says.
    #[serde(default)]
    pub maker: String,
    /// The model's name in the game.
    pub model: String,
    /// The rest: engine, gearbox ... ("" none).
    #[serde(default)]
    pub variant: String,
    /// Left out of Career Mode's dealership unless the player says otherwise.
    #[serde(default)]
    pub ignore: bool,
}

/// What a model in the game is.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ModelInfo {
    pub maker: String,
    pub model: String,
    #[serde(default)]
    pub size: String,
    #[serde(default)]
    pub class: String,
    /// First and last year built; 0 unknown (for `to`: still built).
    #[serde(default)]
    pub from: u16,
    #[serde(default)]
    pub to: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Catalog {
    #[serde(default, rename = "_help")]
    pub help: String,
    #[serde(default = "version")]
    pub version: u32,
    #[serde(default)]
    pub makers: BTreeMap<String, MakerDef>,
    #[serde(default)]
    pub types: BTreeMap<String, TypeDef>,
    #[serde(default)]
    pub models: BTreeMap<String, ModelInfo>,
    /// Why the file on disk was not used, for the page to show.
    #[serde(skip)]
    pub load_error: Option<String>,
}

/// What the editor sets for one bus.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Edit {
    pub maker: String,
    pub model: String,
    pub variant: String,
    pub size: String,
    pub class: String,
    pub from: u16,
    pub to: u16,
    pub ignore: bool,
}

/// A bus in its place.
#[derive(Debug, Clone, PartialEq)]
pub struct Resolved {
    pub maker: String,
    pub model: String,
    /// "" none.
    pub variant: String,
    pub size: String,
    pub class: String,
    pub from: u16,
    pub to: u16,
    pub ignore: bool,
    /// Its model is in the `types` table (else the names are the files' own, guessed).
    pub mapped: bool,
    /// The key of its model as the files write it (what Career Mode's ignore list holds).
    pub type_key: String,
}

impl Resolved {
    /// The variant for a list ("Standard" when there is none).
    pub fn variant_text(&self) -> &str {
        if self.variant.trim().is_empty() {
            "Standard"
        } else {
            &self.variant
        }
    }

    /// "Citaro G ZF OM457" / "Citaro G".
    pub fn full_model(&self) -> String {
        if self.variant.trim().is_empty() {
            self.model.clone()
        } else {
            format!("{} {}", self.model, self.variant)
        }
    }
}

#[derive(Debug, Clone)]
pub struct VariantEntry {
    /// Index in the vehicle list the tree was built from.
    pub index: usize,
    pub file: String,
    pub variant: String,
    pub size: String,
    pub class: String,
    pub mapped: bool,
    pub ignore: bool,
}

#[derive(Debug, Clone)]
pub struct ModelGroup {
    pub model: String,
    pub variants: Vec<VariantEntry>,
}

#[derive(Debug, Clone)]
pub struct MakerGroup {
    pub maker: String,
    pub models: Vec<ModelGroup>,
}

/// A model as the files write it, for the lists of raw names.
#[derive(Debug, Clone)]
pub struct RawType {
    pub key: String,
    pub label: String,
    /// Buses (files) with this name.
    pub count: usize,
    /// The catalogue says: left out by default.
    pub ignore: bool,
    /// It is in the `types` table.
    pub mapped: bool,
    /// One of its bus files (the editor shows this one).
    pub file: String,
}

#[derive(Debug, Clone)]
pub struct RawMaker {
    pub name: String,
    pub types: Vec<RawType>,
}

/// A model as the catalogue names it. Several models of the files can come to one (the
/// catalogue corrects their spelling), so it holds all their keys: switching it on or off
/// switches every one of them.
#[derive(Debug, Clone)]
pub struct NamedType {
    pub label: String,
    /// The keys of the files' models (what Career Mode's ignore list holds).
    pub keys: Vec<String>,
    /// Buses (files) of it.
    pub count: usize,
}

#[derive(Debug, Clone)]
pub struct NamedMaker {
    pub name: String,
    pub types: Vec<NamedType>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportMode {
    /// The file replaces everything.
    Replace,
    /// Only what the catalogue has no entry for is taken.
    Merge,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ImportReport {
    pub makers: usize,
    pub types: usize,
    pub models: usize,
}

// --- text ------------------------------------------------------------------------------

/// A text as lower-case words between single spaces, with a space at both ends: every
/// character that is not a letter or a digit is a gap, so `MB_O305G.bus` and `mb o305g bus`
/// read the same and a whole-word search is a plain `contains(" word ")`.
pub fn words(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push(' ');
    let mut gap = true;
    for c in s.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
            gap = false;
        } else if !gap {
            out.push(' ');
            gap = true;
        }
    }
    if !gap {
        out.push(' ');
    }
    out
}

fn tokens(s: &str) -> Vec<String> {
    words(s).split_whitespace().map(str::to_string).collect()
}

/// `name` without the words of `drop` (the manufacturer).
fn residual(name: &str, drop: &[&str]) -> String {
    let dropped: Vec<String> = drop.iter().flat_map(|d| tokens(d)).collect();
    name.split(|c: char| c == '_' || c.is_whitespace())
        .filter(|w| !w.is_empty())
        .filter(|w| {
            let t = tokens(w);
            !(!t.is_empty() && t.iter().all(|x| dropped.contains(x)))
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The key of a bus file (for lists that go by file).
pub fn file_key(file: &str) -> String {
    file.replace('\\', "/").trim().to_lowercase()
}

/// The key of a manufacturer as the files write it.
pub fn maker_key(manufacturer: &str) -> String {
    words(manufacturer).trim().to_string()
}

/// The key of a model as the files write it: manufacturer and type name.
pub fn type_key(manufacturer: &str, type_label: &str) -> String {
    format!("{}|{}", maker_key(manufacturer), words(type_label).trim())
}

/// The type name of a bus as the files give it (the file's name when it has none).
pub fn type_label_of(v: &VehicleInfo) -> String {
    vehicle_type_label(&v.type_name, Path::new(&v.file))
}

/// The key of a bus's model as the files write it.
pub fn type_key_of(v: &VehicleInfo) -> String {
    type_key(&v.manufacturer, &type_label_of(v))
}

fn model_key(maker: &str, model: &str) -> String {
    format!("{}|{}", maker.trim().to_lowercase(), model.trim().to_lowercase())
}

/// Numbers inside names compare as numbers (DL9 before DL10), case does not matter.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let a = a.to_lowercase();
    let b = b.to_lowercase();
    let mut a = a.chars().peekable();
    let mut b = b.chars().peekable();
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let x: String = std::iter::from_fn(|| a.next_if(|c| c.is_ascii_digit())).collect();
                let y: String = std::iter::from_fn(|| b.next_if(|c| c.is_ascii_digit())).collect();
                let x = x.trim_start_matches('0');
                let y = y.trim_start_matches('0');
                let order = x.len().cmp(&y.len()).then_with(|| x.cmp(y));
                if order != Ordering::Equal {
                    return order;
                }
            }
            (Some(x), Some(y)) => {
                let order = x.cmp(&y);
                if order != Ordering::Equal {
                    return order;
                }
                a.next();
                b.next();
            }
        }
    }
}

// --- guesses -----------------------------------------------------------------------------

fn has(hay: &str, word: &str) -> bool {
    hay.contains(&format!(" {word} "))
}

fn has_any(hay: &str, list: &[&str]) -> bool {
    list.iter().any(|w| has(hay, w))
}

/// A guess of the class from the words of a bus's text; the player corrects it.
pub fn guess_class(hay: &str) -> &'static str {
    if has_any(hay, &["sprinter", "crafter", "transit", "minibus", "minibusik", "vito", "daily", "ducato", "mini bus", "kleinbus"]) {
        "Minibus"
    } else if has_any(hay, &["dd", "doppeldecker", "decker", "double", "sd202", "sd 202", "n122", "n 122", "n122 3", "olympian", "routemaster", "enviro400", "enviro 400"]) {
        "Double-decker"
    } else if has_any(hay, &["tourismo", "travego", "coach", "reisebus", "fernbus", "tourliner", "s415", "s516", "s517", "s431", "topclass", "o303", "o350", "o404", "o580", "jetliner", "touring"]) {
        "Long-distance"
    } else if has_any(hay, &["ü", "ue", "uel", "regio", "überland", "ueberland", "intercity", "interurban", "o550", "o560", "integro", "c2ü"]) {
        "Intercity"
    } else {
        "City"
    }
}

/// A guess of the size from the words of a bus's text and its class.
pub fn guess_size(hay: &str, class: &str) -> &'static str {
    if class == "Minibus" {
        "MINI"
    } else if has_any(hay, &["g", "gelenk", "gelenkbus", "articulated", "sg", "ng", "o305g", "o405g", "o405 g", "o305 g", "18m", "bi", "bigelenk", "lions city g"]) {
        "MEGA"
    } else if has_any(hay, &["midi", "citaro k", "k", "o405 n", "o405n", "city midi", "10m", "9m", "kleinbus"]) {
        "MIDI"
    } else {
        "MAXI"
    }
}

// --- the catalogue -----------------------------------------------------------------------

impl Default for Catalog {
    fn default() -> Catalog {
        Catalog::builtin()
    }
}

/// The catalogue the game comes with, made from the compiled-in file once.
static BUILTIN: OnceLock<Catalog> = OnceLock::new();

impl Catalog {
    /// An empty catalogue: everything is guessed until the player says otherwise.
    pub fn empty() -> Catalog {
        Catalog { help: HELP.to_string(), version: VERSION, makers: BTreeMap::new(), types: BTreeMap::new(), models: BTreeMap::new(), load_error: None }
    }

    /// The catalogue the game comes with (`data/bus_catalog_default.json`, compiled in): the
    /// names and the data of the buses of the packs the game knows, whether the player owns
    /// them or not. Should the compiled-in text ever not parse (a test sees to it that it
    /// does), the catalogue is empty.
    pub fn builtin() -> Catalog {
        BUILTIN
            .get_or_init(|| match serde_json::from_str::<Catalog>(DEFAULT_JSON) {
                Ok(mut c) if c.version == VERSION => {
                    c.help = HELP.to_string();
                    c.load_error = None;
                    c
                }
                _ => Catalog::empty(),
            })
            .clone()
    }

    /// The entries of the compiled-in catalogue put into this one: all of it taken in place
    /// of what there is (`Replace`), or only what this one has no entry for (`Merge`: new
    /// buses a later version knows, the player's own entries stay).
    pub fn import_defaults(&mut self, mode: ImportMode) -> ImportReport {
        self.import(Catalog::builtin(), mode)
    }

    /// Where the player's copy lives.
    pub fn path() -> PathBuf {
        crate::data_dir().join("bus_catalog.json")
    }

    /// The player's file, or the compiled-in catalogue when there is none yet.
    pub fn load() -> Catalog {
        Catalog::load_from(&Catalog::path())
    }

    /// A file that does not parse is left alone (it may be half edited) and the compiled-in
    /// catalogue is used, with the reason in `load_error`. A file of an older format is moved
    /// aside as `<name>.old` and replaced.
    pub fn load_from(path: &Path) -> Catalog {
        let Ok(text) = std::fs::read_to_string(path) else { return Catalog::builtin() };
        let mut bad = Catalog::builtin();
        let value: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                bad.load_error = Some(format!("{}: {e}", path.display()));
                return bad;
            }
        };
        let ver = value.get("version").and_then(|v| v.as_u64()).unwrap_or(1);
        if ver != VERSION as u64 {
            let old = path.with_extension("old");
            let _ = std::fs::rename(path, &old);
            bad.load_error = Some(format!("{} was a catalogue of an older format (version {ver}); it is kept as {}", path.display(), old.display()));
            return bad;
        }
        match serde_json::from_value::<Catalog>(value) {
            Ok(c) => c,
            Err(e) => {
                bad.load_error = Some(format!("{}: {e}", path.display()));
                bad
            }
        }
    }

    pub fn save(&self) -> std::io::Result<PathBuf> {
        let p = Catalog::path();
        self.save_to(&p)?;
        Ok(p)
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        // (written beside and moved over: a crash cannot leave half a catalogue)
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path)
    }

    /// Read a catalogue file into this one.
    pub fn import_file(&mut self, path: &Path, mode: ImportMode) -> Result<ImportReport, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let other: Catalog = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        if other.version != VERSION {
            return Err(format!("{} is a catalogue of version {}, this launcher reads version {VERSION}", path.display(), other.version));
        }
        Ok(self.import(other, mode))
    }

    pub fn import(&mut self, other: Catalog, mode: ImportMode) -> ImportReport {
        let mut r = ImportReport::default();
        match mode {
            ImportMode::Replace => {
                r = ImportReport { makers: other.makers.len(), types: other.types.len(), models: other.models.len() };
                self.makers = other.makers;
                self.types = other.types;
                self.models = other.models;
            }
            ImportMode::Merge => {
                for (k, v) in other.makers {
                    if let std::collections::btree_map::Entry::Vacant(e) = self.makers.entry(k) {
                        e.insert(v);
                        r.makers += 1;
                    }
                }
                for (k, v) in other.types {
                    if let std::collections::btree_map::Entry::Vacant(e) = self.types.entry(k) {
                        e.insert(v);
                        r.types += 1;
                    }
                }
                for (k, v) in other.models {
                    if let std::collections::btree_map::Entry::Vacant(e) = self.models.entry(k) {
                        e.insert(v);
                        r.models += 1;
                    }
                }
            }
        }
        r
    }

    // --- editing ---

    /// Put `v`'s model (and its manufacturer, and what the game's model is) where `e` says.
    pub fn set(&mut self, v: &VehicleInfo, e: &Edit) -> Result<(), String> {
        let (maker, model) = (e.maker.trim(), e.model.trim());
        if maker.is_empty() || model.is_empty() {
            return Err("A manufacturer and a model are needed".to_string());
        }
        let label = type_label_of(v);
        self.types.insert(type_key(&v.manufacturer, &label), TypeDef { raw_maker: display_bus_name(&v.manufacturer), raw_model: label, maker: maker.to_string(), model: model.to_string(), variant: e.variant.trim().to_string(), ignore: e.ignore });
        let size = if SIZES.contains(&e.size.as_str()) { e.size.clone() } else { String::new() };
        let class = if CLASSES.contains(&e.class.as_str()) { e.class.clone() } else { String::new() };
        self.models.insert(model_key(maker, model), ModelInfo { maker: maker.to_string(), model: model.to_string(), size, class, from: e.from, to: e.to });
        Ok(())
    }

    /// The manufacturer name that models of this raw manufacturer which are not in the
    /// catalogue yet are shown under ("" takes it away).
    pub fn set_default_maker(&mut self, raw_manufacturer: &str, name: &str) {
        let key = maker_key(raw_manufacturer);
        if name.trim().is_empty() {
            self.makers.remove(&key);
        } else {
            self.makers.insert(key, MakerDef { raw: display_bus_name(raw_manufacturer), name: name.trim().to_string() });
        }
    }

    /// Take `v`'s model out of the `types` table again (the manufacturer and the game's
    /// model stay: other buses use them).
    pub fn forget(&mut self, v: &VehicleInfo) {
        self.types.remove(&type_key_of(v));
    }

    // --- looking a bus up ---

    pub fn resolve(&self, v: &VehicleInfo) -> Resolved {
        let label = type_label_of(v);
        let tk = type_key(&v.manufacturer, &label);
        let hay = words(&format!("{} {} {} {} {}", v.manufacturer, label, v.name, v.folder, v.file));
        let raw_maker = display_bus_name(&v.manufacturer);
        let def = self.types.get(&tk);
        let maker = match def.map(|d| d.maker.trim()).filter(|m| !m.is_empty()) {
            Some(m) => m.to_string(),
            None => match self.makers.get(&maker_key(&v.manufacturer)) {
                Some(m) if !m.name.trim().is_empty() => m.name.trim().to_string(),
                _ if raw_maker.is_empty() => UNKNOWN_MAKER.to_string(),
                _ => raw_maker.clone(),
            },
        };
        let (model, variant, ignore) = match def {
            Some(d) if !d.model.trim().is_empty() => (d.model.trim().to_string(), d.variant.trim().to_string(), d.ignore),
            _ => {
                let rest = residual(&label, &[&raw_maker, &maker]);
                (if rest.is_empty() { label.clone() } else { rest }, String::new(), def.map(|d| d.ignore).unwrap_or(false))
            }
        };
        let info = self.models.get(&model_key(&maker, &model));
        let class = info.map(|i| i.class.clone()).filter(|c| !c.is_empty()).unwrap_or_else(|| guess_class(&hay).to_string());
        let size = info.map(|i| i.size.clone()).filter(|s| !s.is_empty()).unwrap_or_else(|| guess_size(&hay, &class).to_string());
        Resolved { maker, model, variant, size, class, from: info.map(|i| i.from).unwrap_or(0), to: info.map(|i| i.to).unwrap_or(0), ignore, mapped: def.is_some(), type_key: tk }
    }

    /// Every bus in its place: manufacturer, then model, then variant, each in natural
    /// order. The same manufacturer or model written in another case is one group.
    pub fn tree(&self, vehicles: &[VehicleInfo]) -> Vec<MakerGroup> {
        let mut makers: BTreeMap<String, (String, BTreeMap<String, (String, Vec<VariantEntry>)>)> = BTreeMap::new();
        for (index, v) in vehicles.iter().enumerate() {
            let r = self.resolve(v);
            let entry = VariantEntry { index, file: v.file.clone(), variant: r.variant_text().to_string(), size: r.size.clone(), class: r.class.clone(), mapped: r.mapped, ignore: r.ignore };
            let maker = makers.entry(r.maker.to_lowercase()).or_insert_with(|| (r.maker.clone(), BTreeMap::new()));
            let model = maker.1.entry(r.model.to_lowercase()).or_insert_with(|| (r.model.clone(), Vec::new()));
            model.1.push(entry);
        }
        let mut out: Vec<MakerGroup> = makers
            .into_values()
            .map(|(maker, models)| {
                let mut models: Vec<ModelGroup> = models
                    .into_values()
                    .map(|(model, mut variants)| {
                        variants.sort_by(|a, b| natural_cmp(&a.variant, &b.variant).then_with(|| a.file.cmp(&b.file)));
                        ModelGroup { model, variants }
                    })
                    .collect();
                models.sort_by(|a, b| natural_cmp(&a.model, &b.model));
                MakerGroup { maker, models }
            })
            .collect();
        out.sort_by(|a, b| natural_cmp(&a.maker, &b.maker));
        out
    }

    /// The buses by the names their files give: manufacturer, then model, in natural order.
    pub fn raw_tree(&self, vehicles: &[VehicleInfo]) -> Vec<RawMaker> {
        let mut makers: BTreeMap<String, (String, BTreeMap<String, RawType>)> = BTreeMap::new();
        for v in vehicles {
            let label = type_label_of(v);
            let mk = maker_key(&v.manufacturer);
            let name = display_bus_name(&v.manufacturer);
            let tk = type_key(&v.manufacturer, &label);
            let def = self.types.get(&tk);
            let (ignore, mapped) = (def.is_some_and(|d| d.ignore), def.is_some());
            let m = makers.entry(mk).or_insert_with(|| (if name.is_empty() { "(no manufacturer)".to_string() } else { name }, BTreeMap::new()));
            m.1.entry(tk.clone()).or_insert(RawType { key: tk, label, count: 0, ignore, mapped, file: v.file.clone() }).count += 1;
        }
        let mut out: Vec<RawMaker> = makers
            .into_values()
            .map(|(name, types)| {
                let mut types: Vec<RawType> = types.into_values().collect();
                types.sort_by(|a, b| natural_cmp(&a.label, &b.label));
                RawMaker { name, types }
            })
            .collect();
        out.sort_by(|a, b| natural_cmp(&a.name, &b.name));
        out
    }

    /// The buses by the names the catalogue gives them (corrected manufacturer and model),
    /// each model with the keys of the files' models it covers, in natural order.
    pub fn named_tree(&self, vehicles: &[VehicleInfo]) -> Vec<NamedMaker> {
        let mut makers: BTreeMap<String, (String, BTreeMap<String, NamedType>)> = BTreeMap::new();
        for v in vehicles {
            let r = self.resolve(v);
            let name = if r.maker.trim().is_empty() { "(no manufacturer)".to_string() } else { r.maker.trim().to_string() };
            let m = makers.entry(name.to_lowercase()).or_insert_with(|| (name, BTreeMap::new()));
            let label = if r.model.trim().is_empty() { type_label_of(v) } else { r.model.trim().to_string() };
            let t = m.1.entry(label.to_lowercase()).or_insert_with(|| NamedType { label, keys: Vec::new(), count: 0 });
            if !t.keys.contains(&r.type_key) {
                t.keys.push(r.type_key.clone());
            }
            t.count += 1;
        }
        let mut out: Vec<NamedMaker> = makers
            .into_values()
            .map(|(name, types)| {
                let mut types: Vec<NamedType> = types.into_values().collect();
                types.sort_by(|a, b| natural_cmp(&a.label, &b.label));
                NamedMaker { name, types }
            })
            .collect();
        out.sort_by(|a, b| natural_cmp(&a.name, &b.name));
        out
    }

    /// The keys of the models the catalogue leaves out by default.
    pub fn default_ignored(&self, vehicles: &[VehicleInfo]) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for v in vehicles {
            let k = type_key_of(v);
            if self.types.get(&k).is_some_and(|d| d.ignore) && !out.contains(&k) {
                out.push(k);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bus(manufacturer: &str, type_name: &str, file: &str) -> VehicleInfo {
        let v = serde_json::json!({ "name": type_name, "manufacturer": manufacturer, "type_name": type_name, "file": file, "folder": "", "description": "", "paints": [], "hofs": [] });
        serde_json::from_value(v).unwrap()
    }

    fn edit(maker: &str, model: &str, variant: &str) -> Edit {
        Edit { maker: maker.into(), model: model.into(), variant: variant.into(), size: "MEGA".into(), class: "City".into(), from: 1997, to: 2012, ignore: false }
    }

    #[test]
    fn words_read_alike() {
        assert_eq!(words("MB_O305G.bus"), " mb o305g bus ");
        assert_eq!(words(""), " ");
        assert_eq!(type_key("MB", "Citaro_G"), "mb|citaro g");
    }

    #[test]
    fn unmapped_buses_keep_their_own_names_and_a_guess() {
        let c = Catalog::empty();
        let r = c.resolve(&bus("Ikarus", "Ikarus 280.26", "Vehicles/Ik/280.bus"));
        assert!(!r.mapped);
        assert_eq!((r.maker.as_str(), r.model.as_str(), r.variant_text()), ("Ikarus", "280.26", "Standard"));
        let n = c.resolve(&bus("", "Foo Bar", "Vehicles/x/foo.bus"));
        assert_eq!(n.maker, UNKNOWN_MAKER);
        assert_eq!(n.model, "Foo Bar");
    }

    #[test]
    fn a_mapped_model_takes_the_players_names() {
        let mut c = Catalog::empty();
        let a = bus("MB", "Citaro G ZF OM457", "Vehicles/a/a.bus");
        let b = bus("MB", "Citaro G Voith OM906", "Vehicles/b/b.bus");
        c.set(&a, &edit("Mercedes-Benz", "Citaro G", "ZF OM457")).unwrap();
        // b: the same raw manufacturer, but its model is not in the catalogue yet: the
        // manufacturer is NOT taken over from a (each model has its own)
        let rb = c.resolve(&b);
        assert_eq!(rb.maker, "MB");
        assert!(!rb.mapped);
        c.set(&b, &edit("Mercedes-Benz", "Citaro G", "Voith OM906")).unwrap();
        let (ra, rb) = (c.resolve(&a), c.resolve(&b));
        assert_eq!((ra.model.as_str(), ra.variant.as_str()), ("Citaro G", "ZF OM457"));
        assert_eq!((rb.model.as_str(), rb.variant.as_str()), ("Citaro G", "Voith OM906"));
        assert!(ra.mapped && rb.mapped);
        // the model's data is one for both
        assert_eq!((ra.size.as_str(), ra.class.as_str(), ra.from, ra.to), ("MEGA", "City", 1997, 2012));
        let tree = c.tree(&[a, b]);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].models.len(), 1);
        assert_eq!(tree[0].models[0].variants.len(), 2);
    }

    #[test]
    fn a_pack_can_hold_several_makes() {
        let mut c = Catalog::empty();
        let mb = bus("Bus Pack DLC", "Citaro", "a.bus");
        let setra = bus("Bus Pack DLC", "S 415 HD", "b.bus");
        let other = bus("Bus Pack DLC", "Mystery", "c.bus");
        c.set(&mb, &edit("Mercedes-Benz", "Citaro", "")).unwrap();
        c.set(&setra, &edit("Setra", "S 415 HD", "")).unwrap();
        assert_eq!(c.resolve(&mb).maker, "Mercedes-Benz");
        assert_eq!(c.resolve(&setra).maker, "Setra");
        // not in the catalogue: the pack's name, until a default is given for the rest of it
        assert_eq!(c.resolve(&other).maker, "Bus Pack DLC");
        c.set_default_maker("Bus Pack DLC", "Various");
        assert_eq!(c.resolve(&other).maker, "Various");
        assert_eq!(c.resolve(&mb).maker, "Mercedes-Benz");
        let raw = c.raw_tree(&[mb.clone(), setra.clone(), other.clone()]);
        assert_eq!(raw.len(), 1);
        assert_eq!(raw[0].types.iter().filter(|t| !t.mapped).count(), 1);
        assert_eq!(raw[0].types.iter().find(|t| !t.mapped).unwrap().file, "c.bus");
    }

    #[test]
    fn needs_a_maker_and_a_model() {
        let mut c = Catalog::empty();
        let a = bus("MB", "X", "a.bus");
        assert!(c.set(&a, &edit("", "X", "")).is_err());
        assert!(c.set(&a, &edit("M", " ", "")).is_err());
        assert!(c.types.is_empty());
    }

    #[test]
    fn guesses_are_sane() {
        assert_eq!(guess_class(&words("Mercedes Sprinter 516")), "Minibus");
        assert_eq!(guess_class(&words("MAN Lions Coach R07")), "Long-distance");
        assert_eq!(guess_class(&words("Neoplan N122 3 Skyliner")), "Double-decker");
        assert_eq!(guess_class(&words("MAN NL 202")), "City");
        assert_eq!(guess_size(&words("MB O305 G"), "City"), "MEGA");
        assert_eq!(guess_size(&words("MAN NL 202"), "City"), "MAXI");
        assert_eq!(guess_size(&words("Sprinter"), "Minibus"), "MINI");
    }

    #[test]
    fn ignore_and_raw_tree() {
        let mut c = Catalog::empty();
        let a = bus("MAN", "SD 202", "a.bus");
        let b = bus("MAN", "SD 202", "b.bus");
        let d = bus("MAN", "NL 202", "d.bus");
        let mut e = edit("MAN", "SD 202", "");
        e.ignore = true;
        c.set(&a, &e).unwrap();
        let vs = [a, b, d];
        assert_eq!(c.default_ignored(&vs).len(), 1);
        let raw = c.raw_tree(&vs);
        assert_eq!(raw.len(), 1);
        assert_eq!(raw[0].types.len(), 2);
        let sd = raw[0].types.iter().find(|t| t.label == "SD 202").unwrap();
        assert_eq!(sd.count, 2);
        assert!(sd.ignore);
    }

    #[test]
    fn merge_only_adds_and_replace_replaces() {
        let a = bus("MB", "Citaro", "a.bus");
        let mut mine = Catalog::empty();
        mine.set(&a, &edit("Mercedes-Benz", "Citaro", "")).unwrap();
        let mut other = Catalog::empty();
        other.set(&a, &edit("Daimler", "Other", "x")).unwrap();
        let c = bus("MAN", "NL", "c.bus");
        other.set(&c, &edit("MAN", "NL", "")).unwrap();
        let rep = mine.import(other.clone(), ImportMode::Merge);
        assert_eq!(rep.types, 1);
        assert_eq!(mine.resolve(&a).maker, "Mercedes-Benz");
        assert!(mine.resolve(&c).mapped);
        let rep = mine.import(other, ImportMode::Replace);
        assert_eq!(rep.types, 2);
        assert_eq!(mine.resolve(&a).maker, "Daimler");
    }

    #[test]
    fn files_round_trip_and_old_ones_are_set_aside() {
        let dir = std::env::temp_dir().join(format!("catalog-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("bus_catalog.json");
        let a = bus("MB", "Citaro", "a.bus");
        let mut c = Catalog::empty();
        c.set(&a, &edit("Mercedes-Benz", "Citaro", "")).unwrap();
        c.save_to(&path).unwrap();
        let back = Catalog::load_from(&path);
        assert!(back.load_error.is_none());
        assert_eq!(back.resolve(&a).maker, "Mercedes-Benz");
        std::fs::write(&path, r#"{"version":1,"makers":[],"models":[]}"#).unwrap();
        let old = Catalog::load_from(&path);
        assert!(old.load_error.is_some());
        assert!(path.with_extension("old").is_file());
        std::fs::write(&path, "{ not json").unwrap();
        assert!(Catalog::load_from(&path).load_error.is_some());
        assert!(path.is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn natural_order() {
        assert_eq!(natural_cmp("DL9", "DL10"), Ordering::Less);
        assert_eq!(natural_cmp("abc", "ABC"), Ordering::Equal);
    }

    #[test]
    fn the_compiled_in_catalogue_is_whole() {
        let c = Catalog::builtin();
        assert_eq!(c.version, VERSION);
        assert!(c.load_error.is_none());
        assert!(c.types.len() > 1000 && c.models.len() > 200 && c.makers.len() > 20, "{} {} {}", c.types.len(), c.models.len(), c.makers.len());
        // the game model of every bus has what Career Mode needs: its size, its class and its years
        for (k, t) in &c.types {
            assert!(!t.model.trim().is_empty(), "{k}");
            let maker = Some(t.maker.trim().to_string())
                .filter(|m| !m.is_empty())
                .or_else(|| c.makers.get(k.split('|').next().unwrap_or("")).map(|m| m.name.trim().to_string()).filter(|m| !m.is_empty()))
                .unwrap_or_else(|| t.raw_maker.clone());
            assert!(c.models.contains_key(&model_key(&maker, &t.model)), "{k}: no model entry for {maker} / {}", t.model);
        }
        for (k, m) in &c.models {
            assert!(SIZES.contains(&m.size.as_str()) && CLASSES.contains(&m.class.as_str()), "{k}: {} {}", m.size, m.class);
            assert!(m.to == 0 || m.from == 0 || m.to >= m.from, "{k}");
        }
        // what a player starts with when there is no file of his own
        let nothing = std::env::temp_dir().join(format!("catalog-none-{}", std::process::id())).join("bus_catalog.json");
        let loaded = Catalog::load_from(&nothing);
        assert!(loaded.load_error.is_none() && loaded.types.len() == c.types.len());
        assert!(Catalog::default().types.len() == c.types.len());
    }

    #[test]
    fn a_bus_the_catalogue_knows_comes_with_its_names_and_the_others_are_guessed() {
        let c = Catalog::builtin();
        let r = c.resolve(&bus("Autosan SanCity", "12LF / 2D", "Vehicles/x/y.bus"));
        assert!(r.mapped);
        assert_eq!((r.maker.as_str(), r.model.as_str(), r.variant.as_str()), ("Autosan", "SanCity 12LF", "2D"));
        assert_eq!((r.size.as_str(), r.class.as_str(), r.from, r.to), ("MAXI", "City", 2020, 0));
        // a manufacturer left to the manufacturer table
        let vw = c.resolve(&bus("Volkswagen MiniBus GmbH", "Crafter / Transfer 45", "Vehicles/v/c.bus"));
        assert!(vw.mapped);
        assert_eq!((vw.maker.as_str(), vw.model.as_str(), vw.variant.as_str(), vw.size.as_str()), ("Volkswagen", "Crafter", "Transfer 45", "MINI"));
        // a pack the catalogue has never heard of keeps its own names
        let n = c.resolve(&bus("Nobody Pack", "Foo 9000", "Vehicles/n/foo.bus"));
        assert!(!n.mapped);
        assert_eq!((n.maker.as_str(), n.model.as_str()), ("Nobody Pack", "Foo 9000"));
    }

    #[test]
    fn buses_that_are_not_installed_are_never_asked_for() {
        let c = Catalog::builtin();
        // nothing installed: nothing to show, nothing hidden
        assert!(c.tree(&[]).is_empty() && c.raw_tree(&[]).is_empty() && c.default_ignored(&[]).is_empty());
        // one bus of the catalogue's thousand: only it is shown, and only its flag counts
        let fire = bus("Feuerwehr", "DLK L40A XS", "Vehicles/f/dlk.bus");
        let city = bus("Autosan SanCity", "12LF / 2D", "Vehicles/x/y.bus");
        assert!(c.resolve(&fire).ignore && !c.resolve(&city).ignore);
        assert_eq!(c.default_ignored(&[city.clone()]), Vec::<String>::new());
        assert_eq!(c.default_ignored(&[fire.clone(), city.clone()]), vec![type_key_of(&fire)]);
        assert_eq!(c.tree(&[city]).len(), 1);
    }

    #[test]
    fn the_players_edits_win_over_the_compiled_in_names() {
        let dir = std::env::temp_dir().join(format!("catalog-defaults-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("bus_catalog.json");
        let city = bus("Autosan SanCity", "12LF / 2D", "Vehicles/x/y.bus");
        let mut mine = Catalog::builtin();
        mine.set(&city, &edit("Autosan", "My SanCity", "special")).unwrap();
        mine.save_to(&path).unwrap();
        // his file takes the place of the compiled-in one
        let back = Catalog::load_from(&path);
        assert_eq!(back.resolve(&city).model, "My SanCity");
        // new entries of a later version are added, his own stay
        let mut back = back;
        back.types.remove(&type_key_of(&bus("Volkswagen MiniBus GmbH", "Crafter / Transfer 45", "c.bus")));
        let r = back.import_defaults(ImportMode::Merge);
        assert_eq!(r.types, 1);
        assert_eq!(back.resolve(&city).model, "My SanCity");
        // and the compiled-in names can be put back whole
        back.import_defaults(ImportMode::Replace);
        assert_eq!(back.resolve(&city).model, "SanCity 12LF");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
