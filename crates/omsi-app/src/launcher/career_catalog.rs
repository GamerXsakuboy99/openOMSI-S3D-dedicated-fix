//! The bus catalogue's editor (Settings > Bus catalogue, and Career Mode > Catalog): the
//! player's own database of how the buses are named and what they are
//! (`omsi_launcher_lib::catalog`), used by the dealership, the fleet and later the Drive page
//! and the AI list.
//!
//! Left: every installed bus in its place, manufacturer > model > variant, with a search and a
//! filter (all, not in the catalogue yet, hidden by default). Right: the selected bus with its
//! cached picture and what its files say about it (manufacturer, type name), and the fields to
//! put it in the catalogue: the manufacturer's name in the game (for every bus of that
//! manufacturer as the files write it), the model's name, the variant (the rest of the name:
//! engine, gearbox ...), the model's size and class and the years it was built, and whether
//! Career Mode's dealership leaves it out by default. A row of file buttons exports the
//! catalogue, replaces it from a file, or adds only the entries a file has that this one has
//! not; two more put the catalogue that comes with the game back (all of it, or only the
//! entries the player's has none for). A player starts with that one: the names and the data
//! of the buses of the packs the game knows, whether he owns them or not. Every change is
//! written to `bus_catalog.json` in the data folder at once.

use super::career::vehicle_of;
use super::theme::*;
use super::thumbs::{self, thumb_draw};
use super::ui::ButtonKind;
use super::Launcher;
use glam::Vec2;
use omsi_launcher_lib as core;
use omsi_launcher_lib::catalog::{file_key, type_key_of, type_label_of, Catalog, Edit, ImportMode, MakerGroup, RawMaker, VariantEntry, CLASSES, SIZES};
use omsi_ui::paint::Align;
use omsi_ui::{Rect, Weight};
use std::collections::HashSet;
use std::path::Path;

#[derive(Default)]
pub struct CatalogView {
    tree: Vec<MakerGroup>,
    /// The same buses by the names their files give (the "Not in catalogue" list is made of it).
    raw: Vec<RawMaker>,
    /// What `tree` was made from: the catalogue's revision and the number of buses.
    tree_key: Option<(u32, usize)>,
    open: HashSet<String>,
    search: String,
    filter: usize,
    /// The file of the selected bus.
    sel: Option<String>,
    /// Whose values the fields below hold.
    buf_for: Option<String>,
    maker: String,
    model: String,
    variant: String,
    /// Index in `SIZES` / `CLASSES`.
    size: usize,
    class: usize,
    from: String,
    to: String,
    ignore: bool,
    /// The file the file buttons work on.
    path: String,
    replace_armed: bool,
    /// "Defaults: restore" has been pressed once.
    defaults_armed: bool,
    /// The years copied with "Copy years" (a clipboard of its own, for the next model).
    years_clip: Option<(String, String)>,
}

enum Item {
    Maker { key: String, name: String, n: usize, open: bool },
    Model { key: String, name: String, n: usize, open: bool },
    Bus(VariantEntry),
    /// A model as the files write it that is not in the catalogue yet (one row for all the
    /// files with that name).
    Raw { label: String, files: usize, file: String },
}

impl Item {
    fn height(&self) -> f32 {
        match self {
            Item::Maker { .. } => 32.0,
            Item::Model { .. } => 30.0,
            Item::Bus(_) | Item::Raw { .. } => 34.0,
        }
    }
}

enum Click {
    Toggle(String),
    Pick(String),
}

enum Act {
    Save { file: String, edit: Edit, next: bool },
    DefaultMaker { raw_maker: String, name: String },
    Forget { file: String },
    Export,
    Import(ImportMode),
    /// The catalogue that comes with the game, put back the same two ways.
    Defaults(ImportMode),
}

fn maker_open_key(maker: &str) -> String {
    format!("m:{}", maker.to_lowercase())
}

fn model_open_key(maker: &str, model: &str) -> String {
    format!("g:{}/{}", maker.to_lowercase(), model.to_lowercase())
}

/// The tree as a flat list of rows, honouring what is folded open, the search and the filter.
fn items(tree: &[MakerGroup], raw: &[RawMaker], vehicles: &[core::VehicleInfo], open: &HashSet<String>, q: &str, mode: usize) -> Vec<Item> {
    let filtering = !q.is_empty() || mode != 0;
    let mut out = Vec::new();
    if mode == 1 {
        // what is left to do, by the names the files give: one row per model, whatever its files
        for m in raw {
            let list: Vec<_> = m.types.iter().filter(|t| !t.mapped && (q.is_empty() || format!("{} {} {}", m.name, t.label, t.file).to_lowercase().contains(q))).collect();
            if list.is_empty() {
                continue;
            }
            let mkey = format!("r:{}", m.name.to_lowercase());
            // (here the set holds the manufacturers folded shut, not open)
            let m_open = !q.is_empty() || !open.contains(&mkey);
            out.push(Item::Maker { key: mkey, name: m.name.clone(), n: list.len(), open: m_open });
            if m_open {
                out.extend(list.into_iter().map(|t| Item::Raw { label: t.label.clone(), files: t.count, file: t.file.clone() }));
            }
        }
        return out;
    }
    for mg in tree {
        let mut models: Vec<(String, &str, Vec<&VariantEntry>)> = Vec::new();
        for g in &mg.models {
            let list: Vec<&VariantEntry> = g
                .variants
                .iter()
                .filter(|v| {
                    (mode != 1 || !v.mapped)
                        && (mode != 2 || v.ignore)
                        && (q.is_empty() || {
                            let man = vehicles.get(v.index).map(|x| x.manufacturer.as_str()).unwrap_or("");
                            format!("{} {} {} {} {} {}", mg.maker, g.model, v.variant, v.file, man, v.class).to_lowercase().contains(q)
                        })
                })
                .collect();
            if !list.is_empty() {
                models.push((model_open_key(&mg.maker, &g.model), g.model.as_str(), list));
            }
        }
        if models.is_empty() {
            continue;
        }
        let mkey = maker_open_key(&mg.maker);
        let m_open = filtering || open.contains(&mkey);
        out.push(Item::Maker { key: mkey, name: mg.maker.clone(), n: models.iter().map(|m| m.2.len()).sum(), open: m_open });
        if !m_open {
            continue;
        }
        for (key, name, list) in models {
            let g_open = filtering || open.contains(&key);
            out.push(Item::Model { key, name: name.to_string(), n: list.len(), open: g_open });
            if g_open {
                out.extend(list.into_iter().map(|v| Item::Bus(v.clone())));
            }
        }
    }
    out
}

/// The next model not in the catalogue yet after `current` (a type key), round the whole
/// list: one of its files.
fn next_unmapped(raw: &[RawMaker], current: &str) -> Option<String> {
    let all: Vec<_> = raw.iter().flat_map(|m| m.types.iter()).collect();
    let me = all.iter().position(|t| t.key == current).unwrap_or(0);
    (1..=all.len()).map(|k| all[(me + k) % all.len()]).find(|t| !t.mapped && t.key != current).map(|t| t.file.clone())
}

/// A change was made to the catalogue: write it, and let everything made from it be made again.
fn commit(l: &mut Launcher, what: &str) {
    l.career.cat_rev = l.career.cat_rev.wrapping_add(1);
    l.career.cat.buf_for = None;
    l.career.cat.replace_armed = false;
    l.career.cat.defaults_armed = false;
    let Some(cat) = l.career.catalog.as_ref() else { return };
    match cat.save() {
        Ok(_) => l.state.set_status(what.to_string(), false),
        Err(e) => l.state.set_status(format!("The catalogue could not be saved: {e}"), true),
    }
}

fn years(text: &str) -> Result<u16, String> {
    let t = text.trim();
    if t.is_empty() {
        return Ok(0);
    }
    match t.parse::<u16>() {
        Ok(y) if (1880..=2100).contains(&y) => Ok(y),
        _ => Err(format!("\"{t}\" is not a year")),
    }
}

fn apply(l: &mut Launcher, act: Act) {
    match act {
        Act::Save { file, edit, next } => {
            let Some(v) = vehicle_of(&l.state.vehicles, &file).cloned() else { return };
            let Some(cat) = l.career.catalog.as_mut() else { return };
            if let Err(e) = cat.set(&v, &edit) {
                l.state.set_status(e, true);
                return;
            }
            commit(l, "Saved in the catalogue");
            if next {
                // (the lists are made again next frame; the next one is looked for in the old
                // one, where this model is still not saved but is skipped)
                if let Some(f) = next_unmapped(&l.career.cat.raw, &type_key_of(&v)) {
                    l.career.cat.sel = Some(f);
                    l.career.cat.filter = 1;
                }
            }
        }
        Act::DefaultMaker { raw_maker, name } => {
            let Some(cat) = l.career.catalog.as_mut() else { return };
            cat.set_default_maker(&raw_maker, &name);
            commit(l, "The rest of that manufacturer's models are shown under that name until you put them in");
        }
        Act::Forget { file } => {
            let Some(v) = vehicle_of(&l.state.vehicles, &file).cloned() else { return };
            let Some(cat) = l.career.catalog.as_mut() else { return };
            cat.forget(&v);
            commit(l, "Taken out of the catalogue (its names are guessed again)");
        }
        Act::Export => {
            let path = l.career.cat.path.trim().to_string();
            let Some(cat) = l.career.catalog.as_ref() else { return };
            match cat.save_to(Path::new(&path)) {
                Ok(()) => l.state.set_status(format!("Catalogue written to {path}"), false),
                Err(e) => l.state.set_status(format!("Could not write {path}: {e}"), true),
            }
        }
        Act::Defaults(mode) => {
            let Some(cat) = l.career.catalog.as_mut() else { return };
            let r = cat.import_defaults(mode);
            let what = match mode {
                ImportMode::Replace => format!("The catalogue that comes with the game is back: {} manufacturers, {} models, {} game models", r.makers, r.types, r.models),
                ImportMode::Merge => format!("Added from the catalogue that comes with the game: {} manufacturers, {} models, {} game models", r.makers, r.types, r.models),
            };
            commit(l, &what);
        }
        Act::Import(mode) => {
            let path = l.career.cat.path.trim().to_string();
            let Some(cat) = l.career.catalog.as_mut() else { return };
            match cat.import_file(Path::new(&path), mode) {
                Ok(r) => {
                    let what = match mode {
                        ImportMode::Replace => format!("Catalogue replaced from the file: {} manufacturers, {} models, {} game models", r.makers, r.types, r.models),
                        ImportMode::Merge => format!("Added from the file: {} manufacturers, {} models, {} game models", r.makers, r.types, r.models),
                    };
                    commit(l, &what);
                }
                Err(e) => l.state.set_status(e, true),
            }
        }
    }
}

/// The page when it is opened from Settings (no company needed).
pub(super) fn draw_page(l: &mut Launcher, area: Rect) {
    super::career::ensure_loaded(l);
    let body = l.page_title(area, "Bus catalogue", "How your buses are named and what they are: manufacturer, model, variant, size, class, years built.");
    if l.ui.button("cat-back", Rect::new(body.right() - 170.0, body.y - 50.0, 170.0, ROW), "Back to Settings", Some("tune"), ButtonKind::Normal) {
        l.go(super::Page::Settings);
        return;
    }
    draw(l, body);
}

pub(super) fn draw(l: &mut Launcher, r: Rect) {
    if l.career.catalog.is_none() {
        l.career.catalog = Some(Catalog::load());
    }
    let rev = l.career.cat_rev;
    let n_veh = l.state.vehicles.len();
    if l.career.cat.tree_key != Some((rev, n_veh)) {
        let tree = l.career.catalog.as_ref().map(|c| c.tree(&l.state.vehicles)).unwrap_or_default();
        let raw = l.career.catalog.as_ref().map(|c| c.raw_tree(&l.state.vehicles)).unwrap_or_default();
        l.career.cat.tree = tree;
        l.career.cat.raw = raw;
        l.career.cat.tree_key = Some((rev, n_veh));
    }
    if l.career.cat.path.is_empty() {
        l.career.cat.path = core::data_dir().join("bus_catalog_export.json").to_string_lossy().to_string();
    }
    let load_error = l.career.catalog.as_ref().and_then(|c| c.load_error.clone());

    // --- the bar
    let mut reload = false;
    let mut clear_pics = false;
    {
        let (ui, v) = (&mut l.ui, &mut l.career.cat);
        ui.text_input("cat-search", Rect::new(r.x, r.y, 260.0, ROW), &mut v.search, "Search maker, model, file", Some("search"));
        let mut f = v.filter.min(2);
        if ui.segmented("cat-filter", Rect::new(r.x + 272.0, r.y, 400.0, ROW), &mut f, &["All", "Not in catalogue", "Hidden by default"]) {
            v.filter = f;
        }
        let b2 = Rect::new(r.right() - 170.0, r.y, 170.0, ROW);
        let b1 = Rect::new(b2.x - 150.0, r.y, 140.0, ROW);
        if ui.button("cat-reload", b1, "Reload file", Some("refresh"), ButtonKind::Normal) {
            reload = true;
        }
        if ui.button("cat-clear-pics", b2, "Clear pictures", Some("restart_alt"), ButtonKind::Normal) {
            clear_pics = true;
        }
    }
    if reload {
        l.career.catalog = Some(Catalog::load());
        l.career.cat_rev = l.career.cat_rev.wrapping_add(1);
        l.career.cat.buf_for = None;
        l.state.set_status("Catalogue read from the file again", false);
        return;
    }
    if clear_pics {
        let n = thumbs::clear_cache();
        l.icons.retain(|k, _| !k.starts_with("bus:"));
        l.thumbs.reset();
        l.state.set_status(format!("{n} pictures removed; they are taken again when a page needs them"), false);
        return;
    }

    // --- the file row
    let mut file_act: Option<Act> = None;
    {
        let y = r.y + ROW + 8.0;
        let ui = &mut l.ui;
        let v = &mut l.career.cat;
        let (bw1, bw2, bw3, bw4, bw5) = (110.0, 190.0, 150.0, 190.0, 170.0);
        let pw = (r.w - bw1 - bw2 - bw3 - bw4 - bw5 - 50.0).max(160.0);
        ui.text_input("cat-path", Rect::new(r.x, y, pw, ROW), &mut v.path, "File to export to / import from", Some("folder_open"));
        let mut x = r.x + pw + 10.0;
        if ui.button("cat-export", Rect::new(x, y, bw1, ROW), "Export", Some("save"), ButtonKind::Normal) {
            file_act = Some(Act::Export);
        }
        x += bw1 + 10.0;
        if ui.button("cat-merge", Rect::new(x, y, bw2, ROW), "Add new entries only", Some("add"), ButtonKind::Normal) {
            file_act = Some(Act::Import(ImportMode::Merge));
        }
        x += bw2 + 10.0;
        let label = if v.replace_armed { "Press again" } else { "Replace all" };
        if ui.button("cat-replace", Rect::new(x, y, bw3, ROW), label, Some("restart_alt"), ButtonKind::Danger) {
            if v.replace_armed {
                file_act = Some(Act::Import(ImportMode::Replace));
            } else {
                v.replace_armed = true;
            }
        }
        x += bw3 + 10.0;
        // the catalogue that comes with the game (no file): what it has that this one has not, or all of it
        if ui.button("cat-defaults-add", Rect::new(x, y, bw4, ROW), "Game's list: add new", Some("add"), ButtonKind::Normal) {
            file_act = Some(Act::Defaults(ImportMode::Merge));
        }
        x += bw4 + 10.0;
        let label = if v.defaults_armed { "Press again" } else { "Game's list: restore" };
        if ui.button("cat-defaults-restore", Rect::new(x, y, bw5, ROW), label, Some("restart_alt"), ButtonKind::Danger) {
            if v.defaults_armed {
                file_act = Some(Act::Defaults(ImportMode::Replace));
            } else {
                v.defaults_armed = true;
            }
        }
    }
    if let Some(a) = file_act {
        apply(l, a);
        return;
    }

    // --- one line about the whole catalogue
    let (mut total, mut saved, mut hidden) = (0usize, 0usize, 0usize);
    for mg in &l.career.cat.tree {
        for g in &mg.models {
            for v in &g.variants {
                total += 1;
                saved += usize::from(v.mapped);
                hidden += usize::from(v.ignore);
            }
        }
    }
    let line = match load_error {
        Some(e) => (format!("The file was not used ({e}); the catalogue that comes with the game is shown."), DANGER),
        None => (format!("{total} buses · {saved} in the catalogue · {} still to do · {hidden} hidden by default · {}", total - saved, Catalog::path().display()), TEXT_DIM),
    };
    let info_y = r.y + (ROW + 8.0) * 2.0;
    l.ui.text_in(&line.0, Rect::new(r.x, info_y, r.w, 18.0), 12.0, Weight::Regular, line.1, Align::Left);

    // --- the rows
    let top = info_y + 26.0;
    let side_w = 420.0_f32.min(r.w * 0.42);
    let list = Rect::new(r.x, top, (r.w - side_w - 16.0).max(200.0), (r.bottom() - top).max(100.0));
    let side = Rect::new(list.right() + 16.0, top, (r.right() - list.right() - 16.0).max(100.0), list.h);
    let q = l.career.cat.search.trim().to_lowercase();
    let rows = items(&l.career.cat.tree, &l.career.cat.raw, &l.state.vehicles, &l.career.cat.open, &q, l.career.cat.filter.min(2));

    l.ui.panel(list);
    let inner = Rect::new(list.x + 10.0, list.y + 10.0, list.w - 20.0, list.h - 20.0);
    let sel = l.career.cat.sel.as_ref().map(|f| file_key(f));
    let list_view = inner;
    let mut click: Option<Click> = None;
    if rows.is_empty() {
        l.ui.text_in("No bus matches.", Rect::new(inner.x + 8.0, inner.y + 4.0, inner.w, 20.0), 13.0, Weight::Regular, TEXT_DIM, Align::Left);
    }
    l.ui.scroll_area("cat-list", inner, &mut |ui, v| {
        let mut y = v.y;
        for it in &rows {
            let h = it.height();
            let rr = Rect::new(v.x, y, v.w - 12.0, h);
            // (only the rows in view are drawn: a list of thousands of rows drawn whole filled the
            // interface's buffers and garbled the text)
            if rr.bottom() < list_view.y - 4.0 || rr.y > list_view.bottom() + 4.0 {
                y += h;
                continue;
            }
            match it {
                Item::Maker { key, name, n, open } => {
                    if ui.row(&format!("cat-{key}"), rr, false) {
                        click = Some(Click::Toggle(key.clone()));
                    }
                    ui.icon(if *open { "expand_more" } else { "chevron_right" }, Vec2::new(rr.x + 16.0, rr.center().y), 18.0, TEXT_SOFT);
                    ui.text_in(name, Rect::new(rr.x + 34.0, rr.y, rr.w - 120.0, h), 14.5, Weight::Bold, TEXT, Align::Left);
                    ui.text_in(&format!("{n}"), Rect::new(rr.right() - 70.0, rr.y, 60.0, h), 12.0, Weight::Regular, TEXT_DIM, Align::Right);
                }
                Item::Model { key, name, n, open } => {
                    if ui.row(&format!("cat-{key}"), rr, false) {
                        click = Some(Click::Toggle(key.clone()));
                    }
                    ui.icon(if *open { "expand_more" } else { "chevron_right" }, Vec2::new(rr.x + 40.0, rr.center().y), 16.0, TEXT_DIM);
                    ui.text_in(name, Rect::new(rr.x + 56.0, rr.y, rr.w - 140.0, h), 13.5, Weight::Medium, TEXT, Align::Left);
                    ui.text_in(&format!("{n}"), Rect::new(rr.right() - 70.0, rr.y, 60.0, h), 12.0, Weight::Regular, TEXT_DIM, Align::Right);
                }
                Item::Raw { label, files, file } => {
                    let k = file_key(file);
                    if ui.row(&format!("cat-r-{k}"), rr, sel.as_deref() == Some(k.as_str())) {
                        click = Some(Click::Pick(file.clone()));
                    }
                    ui.text_in(label, Rect::new(rr.x + 40.0, rr.y, (rr.w * 0.5 - 40.0).max(40.0), h), 13.0, Weight::Regular, TEXT, Align::Left);
                    ui.text_in(&if *files == 1 { "1 file".to_string() } else { format!("{files} files") }, Rect::new(rr.x + rr.w * 0.5, rr.y, 90.0, h), 12.0, Weight::Regular, TEXT_DIM, Align::Left);
                    ui.text_in(file, Rect::new(rr.x + rr.w * 0.5 + 96.0, rr.y, (rr.w * 0.5 - 190.0).max(20.0), h), 11.5, Weight::Regular, TEXT_FAINT, Align::Left);
                    ui.badge(Vec2::new(rr.right() - 84.0, rr.center().y - 8.0), "TO DO", WARN);
                }
                Item::Bus(e) => {
                    let k = file_key(&e.file);
                    if ui.row(&format!("cat-b-{k}"), rr, sel.as_deref() == Some(k.as_str())) {
                        click = Some(Click::Pick(e.file.clone()));
                    }
                    let x0 = rr.x + 64.0;
                    let split = rr.x + rr.w * 0.40;
                    ui.text_in(&e.variant, Rect::new(x0, rr.y, (split - x0).max(40.0), h), 13.0, Weight::Regular, TEXT, Align::Left);
                    ui.text_in(&format!("{} · {}", e.size, e.class), Rect::new(split, rr.y, 150.0, h), 12.0, Weight::Regular, TEXT_DIM, Align::Left);
                    ui.text_in(&e.file, Rect::new(split + 156.0, rr.y, (rr.right() - split - 156.0 - 92.0).max(20.0), h), 11.5, Weight::Regular, TEXT_FAINT, Align::Left);
                    let at = Vec2::new(rr.right() - 84.0, rr.center().y - 8.0);
                    if !e.mapped {
                        ui.badge(at, "TO DO", WARN);
                    } else if e.ignore {
                        ui.badge(at, "HIDDEN", TEXT_DIM);
                    }
                }
            }
            y += h;
        }
        y - v.y
    });
    match click {
        Some(Click::Toggle(key)) => {
            let open = &mut l.career.cat.open;
            if !open.remove(&key) {
                open.insert(key);
            }
        }
        Some(Click::Pick(file)) => l.career.cat.sel = Some(file),
        None => {}
    }

    // --- the selected bus
    l.ui.panel(side);
    let inner = Rect::new(side.x + 16.0, side.y + 16.0, side.w - 32.0, side.h - 32.0);
    let veh = l.career.cat.sel.as_ref().and_then(|f| vehicle_of(&l.state.vehicles, f)).cloned();
    let Some(veh) = veh else {
        l.ui.paragraph(
            "Pick a bus on the left. You see what its files call it, and say what it is called in the game: the manufacturer, the model, the variant (engine, gearbox ...), its size and class and the years it was built. Buses with the same name in their files are one entry. \"TO DO\" marks the buses you have not put in yet.",
            Vec2::new(inner.x, inner.y),
            inner.w,
            13.0,
            Weight::Regular,
            TEXT_DIM,
        );
        return;
    };
    let key = file_key(&veh.file);
    let Some(resolved) = l.career.catalog.as_ref().map(|c| c.resolve(&veh)) else { return };
    if l.career.cat.buf_for.as_deref() != Some(key.as_str()) {
        let v = &mut l.career.cat;
        v.buf_for = Some(key.clone());
        v.maker = resolved.maker.clone();
        v.model = resolved.model.clone();
        v.variant = resolved.variant.clone();
        v.size = SIZES.iter().position(|s| *s == resolved.size).unwrap_or(2);
        v.class = CLASSES.iter().position(|c| *c == resolved.class).unwrap_or(0);
        v.from = if resolved.from > 0 { resolved.from.to_string() } else { String::new() };
        v.to = if resolved.to > 0 { resolved.to.to_string() } else { String::new() };
        v.ignore = resolved.ignore;
    }
    l.thumb_request(&veh.file, "");

    let mut act: Option<Act> = None;
    let mut bad_input: Option<String> = None;
    let class_options: Vec<String> = CLASSES.iter().map(|c| c.to_string()).collect();
    // the other models of the same raw manufacturer that are not in the catalogue yet
    let me_key = core::catalog::maker_key(&veh.manufacturer);
    let other_unmapped = l.career.cat.raw.iter().flat_map(|m| m.types.iter()).filter(|t| !t.mapped && t.key != resolved.type_key && t.key.split('|').next() == Some(me_key.as_str())).count();
    let icons = &l.icons;
    let cv = &mut l.career.cat;
    let mapped = resolved.mapped;
    l.ui.scroll_area("cat-detail", Rect::new(inner.x - 6.0, inner.y, inner.w + 12.0, inner.h), &mut |ui, v| {
        let x = v.x + 6.0;
        let w = v.w - 12.0 - 10.0;
        let mut y = v.y;
        thumb_draw(ui, icons, Rect::new(x, y, 160.0, 90.0), &veh.file, "", 6.0);
        let tx = x + 172.0;
        let tw = (w - 172.0).max(40.0);
        ui.text_in("THE FILES SAY", Rect::new(tx, y, tw, 16.0), 10.5, Weight::Bold, TEXT_DIM, Align::Left);
        let said = if veh.manufacturer.trim().is_empty() { "(no manufacturer)".to_string() } else { veh.manufacturer.clone() };
        ui.text_in(&said, Rect::new(tx, y + 18.0, tw, 18.0), 12.5, Weight::Medium, TEXT, Align::Left);
        ui.text_in(&type_label_of(&veh), Rect::new(tx, y + 38.0, tw, 18.0), 12.5, Weight::Regular, TEXT_SOFT, Align::Left);
        let (label, col) = if mapped { ("IN THE CATALOGUE", OK) } else { ("TO DO", WARN) };
        ui.badge(Vec2::new(tx, y + 66.0), label, col);
        y += 90.0 + 8.0;
        let h = ui.paragraph(&veh.file, Vec2::new(x, y), w, 11.5, Weight::Regular, TEXT_FAINT);
        y += h + 14.0;

        let field = |ui: &mut super::ui::Ui, y: &mut f32, id: &str, label: &str, value: &mut String, hint: &str| {
            ui.text_in(label, Rect::new(x, *y, w, 16.0), 10.5, Weight::Bold, TEXT_DIM, Align::Left);
            *y += 17.0;
            ui.text_input(id, Rect::new(x, *y, w, ROW), value, hint, None);
            *y += ROW + 8.0;
        };
        field(ui, &mut y, "cat-maker", "MANUFACTURER IN THE GAME (THIS MODEL ONLY)", &mut cv.maker, "e.g. Mercedes-Benz");
        field(ui, &mut y, "cat-model", "MODEL", &mut cv.model, "e.g. Citaro G");
        field(ui, &mut y, "cat-variant", "VARIANT: THE REST OF THE NAME (ENGINE, GEARBOX ...)", &mut cv.variant, "e.g. ZF Ecomat · OM 457");

        ui.text_in("SIZE", Rect::new(x, y, w, 16.0), 10.5, Weight::Bold, TEXT_DIM, Align::Left);
        y += 17.0;
        let mut z = cv.size.min(SIZES.len() - 1);
        if ui.segmented("cat-size", Rect::new(x, y, w, ROW), &mut z, &SIZES) {
            cv.size = z;
        }
        y += ROW + 8.0;
        ui.text_in("CLASS", Rect::new(x, y, w, 16.0), 10.5, Weight::Bold, TEXT_DIM, Align::Left);
        y += 17.0;
        let mut c = cv.class.min(CLASSES.len() - 1);
        if ui.select("cat-class", Rect::new(x, y, w, ROW), &mut c, &class_options) {
            cv.class = c;
        }
        y += ROW + 8.0;

        ui.text_in("YEARS BUILT (THE MODEL'S; LEAVE THE LAST EMPTY IF STILL BUILT)", Rect::new(x, y, w, 16.0), 10.5, Weight::Bold, TEXT_DIM, Align::Left);
        y += 17.0;
        let half = (w - 10.0) / 2.0;
        ui.text_input("cat-from", Rect::new(x, y, half, ROW), &mut cv.from, "from", None);
        ui.text_input("cat-to", Rect::new(x + half + 10.0, y, half, ROW), &mut cv.to, "to", None);
        y += ROW + 6.0;
        if ui.button("cat-years-copy", Rect::new(x, y, half, ROW), "Copy years", Some("content_copy"), ButtonKind::Normal) {
            cv.years_clip = Some((cv.from.clone(), cv.to.clone()));
        }
        let paste = Rect::new(x + half + 10.0, y, half, ROW);
        match cv.years_clip.clone() {
            Some((f, t)) => {
                let label = match (f.trim(), t.trim()) {
                    ("", "") => "Paste (empty)".to_string(),
                    (a, "") => format!("Paste {a}-"),
                    ("", b) => format!("Paste -{b}"),
                    (a, b) => format!("Paste {a}-{b}"),
                };
                if ui.button("cat-years-paste", paste, &label, None, ButtonKind::Normal) {
                    cv.from = f;
                    cv.to = t;
                }
            }
            None => {
                ui.button_off(paste, "Paste years", None, ButtonKind::Normal);
            }
        }
        y += ROW + 10.0;

        ui.toggle("cat-ignore", Rect::new(x, y, w, 30.0), &mut cv.ignore, "Hidden by default in Career Mode's dealership");
        y += 38.0;

        let years_text = match (cv.from.trim(), cv.to.trim()) {
            ("", "") => String::new(),
            (a, "") => format!(" · {a}-"),
            ("", b) => format!(" · -{b}"),
            (a, b) => format!(" · {a}-{b}"),
        };
        let variant = if cv.variant.trim().is_empty() { String::new() } else { format!(" {}", cv.variant.trim()) };
        let result = format!("{} {}{variant} · {} {}{years_text}", cv.maker.trim(), cv.model.trim(), SIZES[cv.size.min(SIZES.len() - 1)], CLASSES[cv.class.min(CLASSES.len() - 1)]);
        let h = ui.paragraph(&result, Vec2::new(x, y), w, 13.0, Weight::Bold, TEXT);
        y += h + 12.0;

        let edit = || -> Result<Edit, String> {
            Ok(Edit {
                maker: cv.maker.clone(),
                model: cv.model.clone(),
                variant: cv.variant.clone(),
                size: SIZES[cv.size.min(SIZES.len() - 1)].to_string(),
                class: CLASSES[cv.class.min(CLASSES.len() - 1)].to_string(),
                from: years(&cv.from)?,
                to: years(&cv.to)?,
                ignore: cv.ignore,
            })
        };
        let half = (w - 8.0) / 2.0;
        let mut save = false;
        let mut save_next = false;
        if ui.button("cat-save", Rect::new(x, y, half, ROW), "Save", Some("save"), ButtonKind::Primary) {
            save = true;
        }
        if ui.button("cat-save-next", Rect::new(x + half + 8.0, y, half, ROW), "Save, next to do", Some("chevron_right"), ButtonKind::Normal) {
            save = true;
            save_next = true;
        }
        y += ROW + 8.0;
        if save {
            match edit() {
                Ok(e) => act = Some(Act::Save { file: veh.file.clone(), edit: e, next: save_next }),
                Err(msg) => bad_input = Some(msg),
            }
        }
        let others = other_unmapped;
        if others > 0 && !cv.maker.trim().is_empty() {
            let label = format!("Also show the other {others} not yet in the catalogue under \"{}\"", cv.maker.trim());
            if ui.button("cat-default-maker", Rect::new(x, y, w, ROW), &label, Some("add"), ButtonKind::Normal) {
                act = Some(Act::DefaultMaker { raw_maker: veh.manufacturer.clone(), name: cv.maker.clone() });
            }
            y += ROW + 8.0;
        }
        if mapped && ui.button("cat-forget", Rect::new(x, y, w, ROW), "Take this model out of the catalogue", Some("close"), ButtonKind::Danger) {
            act = Some(Act::Forget { file: veh.file.clone() });
        }
        y += ROW + 8.0;
        y - v.y
    });
    if let Some(msg) = bad_input {
        l.state.set_status(msg, true);
    }
    if let Some(a) = act {
        apply(l, a);
    }
}
