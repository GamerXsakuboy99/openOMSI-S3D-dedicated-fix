//! Career Mode, the customize screen: stickers and small objects on a bus of the fleet.
//!
//! Opened from the fleet's "Customize" button. The bus is shown large (the showroom's
//! picture; a drag turns it, the wheel zooms towards the cursor, the buttons look at it from the
//! front, the back or a side). A new sticker, or the selected one after "Move", is carried by
//! the cursor over the bus - it is seen on the body as it goes - and put down with a click. A
//! sticker is a typed text (a font of the system, bold, italic, colour) or a picture of the
//! player's, with a size in metres, a turn and a mirror. The stickers are drawn over the bus at
//! once (`Showroom::set_live`: the bus is not read again). Saving keeps the set for this bus
//! alone or for every bus of the model; presets are named sets of a model. The engine side is
//! `decals`.

use super::career::{vehicle_of, FleetRow};
use super::theme::*;
use super::ui::ButtonKind;
use super::Launcher;
use crate::decals::{self, Decal, Kind};
use glam::{Vec2, Vec3};
use omsi_launcher_lib as core;
use omsi_ui::paint::Align;
use omsi_sim::paint_mix::Mix;
use omsi_ui::{Color, Rect, Weight};
use std::time::Instant;

const PALETTE: [[u8; 3]; 10] = [
    [255, 255, 255],
    [24, 24, 24],
    [205, 32, 32],
    [240, 130, 22],
    [245, 205, 30],
    [40, 160, 72],
    [30, 100, 200],
    [122, 62, 172],
    [150, 150, 150],
    [0, 150, 170],
];

/// The sizes an imported picture can be cut down to (longer side, pixels; 0: as it is).
const IMPORT_SIZES: [(&str, u32); 6] = [("Keep its size", 0), ("Up to 2048 px", 2048), ("Up to 1024 px", 1024), ("Up to 512 px", 512), ("Up to 256 px", 256), ("Up to 128 px", 128)];

pub struct LiveryView {
    bus_id: u32,
    file: String,
    paint: String,
    title: String,
    /// The bus's `[model]` line; empty until the showroom has the bus.
    model: String,
    loaded: bool,
    /// 0: this bus only, 1: every bus of the model.
    scope: usize,
    decals: Vec<Decal>,
    sel: Option<usize>,
    /// The selected sticker follows the cursor until a click puts it down.
    carry: bool,
    /// A new sticker goes on a window; the view is from inside.
    glass_new: bool,
    inside: bool,
    /// The spot of the bus under the cursor: position and facing.
    hover: Option<(Vec3, Vec3)>,
    last_mouse: Vec2,
    last_pick: Instant,
    new_text: String,
    sel_text: String,
    sel_for: Option<usize>,
    hex: String,
    /// The fonts (names, as the dropdown lists them) and the one chosen for a new text and
    /// the one of the selected text.
    font_names: Vec<String>,
    font_new: usize,
    font_sel: usize,
    image_ix: usize,
    images: Vec<String>,
    import_ix: usize,
    size_names: Vec<String>,
    preset_name: String,
    presets: Vec<(String, Vec<Decal>)>,
    press: Option<Vec2>,
    /// The installed assets and the one chosen in the list.
    assets: Vec<crate::assets::Asset>,
    asset_ix: usize,
    /// The mesh of the bus under the cursor (what an object put down stands on).
    hover_mesh: Option<usize>,
    /// The gizmo of a selected object: 0 move, 1 rotate, 2 off; its axes: 0 the object's own,
    /// 1 the bus's; the steps of the buttons.
    gizmo: usize,
    axes: usize,
    turn_step: usize,
    move_step: usize,
    drag: Option<Drag>,
    /// 0: customize decals (stickers), 1: customize objects.
    mode: usize,
    /// The camera: 0 turns round the bus, 1 flies freely.
    cam_mode: usize,
    /// The object browser (tiles with a picture, the author and the description) is open.
    browser: bool,
    /// Assets whose picture could not be drawn (key as in `icons`).
    thumb_failed: std::collections::HashSet<String>,
    /// The paint now (the name the showroom is asked for: a repaint, or a mix of repaints,
    /// `omsi_sim::paint_mix`) and the one the bus had when this was opened.
    mix: Mix,
    paint_open: String,
    /// The repaints of the bus and its texture slots with the repaints that give each one a
    /// texture (read from the bus once the showroom has it).
    repaints: Vec<String>,
    slots: Vec<(String, Vec<String>)>,
}

/// A handle of the gizmo being dragged.
#[derive(Clone, Copy)]
struct Drag {
    axis: usize,
    rotate: bool,
    last: Vec2,
    /// A turn: which way on the screen the ring runs where it was taken, and its radius (px).
    tangent: Vec2,
    radius_px: f32,
}

const TURN_STEPS: [(&str, f32); 5] = [("1°", 1.0), ("5°", 5.0), ("15°", 15.0), ("45°", 45.0), ("90°", 90.0)];
const MOVE_STEPS: [(&str, f32); 4] = [("1 cm", 0.01), ("5 cm", 0.05), ("20 cm", 0.2), ("1 m", 1.0)];
const AXIS_NAMES: [&str; 3] = ["X", "Y", "Z"];
const AXIS_COLORS: [[u8; 3]; 3] = [[226, 66, 66], [86, 196, 86], [74, 128, 238]];

impl LiveryView {
    pub(super) fn open(row: &FleetRow) -> LiveryView {
        let mut v = LiveryView {
            bus_id: row.id,
            file: row.file.clone(),
            paint: row.paint.clone(),
            title: row.title.clone(),
            model: String::new(),
            loaded: false,
            scope: 0,
            decals: Vec::new(),
            sel: None,
            carry: false,
            glass_new: false,
            inside: false,
            hover: None,
            last_mouse: Vec2::new(-1.0, -1.0),
            last_pick: Instant::now(),
            new_text: String::new(),
            sel_text: String::new(),
            sel_for: None,
            hex: String::new(),
            font_names: decals::system_fonts().iter().map(|f| f.0.clone()).collect(),
            font_new: 0,
            font_sel: 0,
            image_ix: 0,
            images: decals::image_files(),
            import_ix: 2,
            size_names: IMPORT_SIZES.iter().map(|s| s.0.to_string()).collect(),
            preset_name: String::new(),
            presets: Vec::new(),
            press: None,
            assets: Vec::new(),
            asset_ix: 0,
            hover_mesh: None,
            gizmo: 0,
            axes: 0,
            turn_step: 1,
            move_step: 1,
            drag: None,
            mode: 0,
            cam_mode: 0,
            browser: false,
            thumb_failed: Default::default(),
            mix: Mix::parse(&row.paint),
            paint_open: row.paint.clone(),
            repaints: Vec::new(),
            slots: Vec::new(),
        };
        v.refresh_assets();
        v
    }

    /// Read the installed assets again.
    fn refresh_assets(&mut self) {
        crate::assets::rescan();
        self.assets = crate::assets::list();
        self.asset_ix = self.asset_ix.min(self.assets.len().saturating_sub(1));
    }

    /// The font file of the font chosen at `ix` ("" is Roboto).
    fn font_file(ix: usize) -> String {
        decals::system_fonts().get(ix).map(|f| f.1.clone()).unwrap_or_default()
    }

    fn font_index(file: &str) -> usize {
        decals::system_fonts().iter().position(|f| f.1 == file).unwrap_or(0)
    }

    /// The selected object's axes: its own (turned as it is) or the bus's.
    fn axes_of(&self, d: &Decal) -> [Vec3; 3] {
        if self.axes == 0 {
            let m = d.object_matrix();
            [m.x_axis, m.y_axis, m.z_axis]
        } else {
            [Vec3::X, Vec3::Y, Vec3::Z]
        }
    }
}

#[derive(Clone, Debug)]
enum Act {
    AddText,
    AddImage(String),
    AddObject(String),
    OpenAssets,
    RescanAssets,
    OpenBrowser,
    CloseBrowser,
    MoveBy(usize, f32),
    TurnBy(usize, f32),
    ResetTurn,
    StandUp,
    Import,
    Remove(usize),
    Duplicate,
    ResetAll,
    SavePreset,
    LoadPreset(usize),
    DeletePreset(usize),
    View(f32),
    Save,
    Close,
}

pub(super) fn draw(l: &mut Launcher, r: Rect) {
    let Some(mut ed) = l.career.livery.take() else { return };
    if run(l, &mut ed, r) {
        l.career.livery = Some(ed);
    } else {
        // (the bus without the stickers, the camera as it was)
        if let Some(rd) = l.renderer.as_ref() {
            l.showroom.set_live(rd, &[]);
        }
        l.showroom.close_up(false);
        l.showroom.set_inside(false);
    }
}

/// One frame of the editor; false when it was closed.
fn run(l: &mut Launcher, ed: &mut LiveryView, r: Rect) -> bool {
    l.showroom.close_up(true);
    l.showroom.set_inside(ed.inside);
    l.showroom.set_free(ed.cam_mode == 1);
    // (what is selected belongs to the mode that is shown)
    if ed.sel.and_then(|i| ed.decals.get(i)).is_some_and(|d| d.is_object() != (ed.mode == 1)) {
        ed.sel = None;
        ed.carry = false;
    }
    if ed.mode != 1 {
        ed.browser = false;
    }
    if ed.mode == 2 {
        ed.sel = None;
        ed.carry = false;
    }
    // --- the bus's model and its saved stickers, once the showroom has the bus
    if !ed.loaded && l.showroom.shown_bus().is_some_and(|(b, p)| b == ed.file && p == ed.paint) {
        if let Some(m) = l.showroom.model_line() {
            ed.decals = decals::load_for(Some(ed.bus_id), &m);
            ed.scope = if decals::has_own(ed.bus_id) || ed.decals.is_empty() { 0 } else { 1 };
            ed.presets = decals::presets_for(&m);
            ed.model = m;
            ed.loaded = true;
            if let Some((repaints, slots)) = l.showroom.paint_options() {
                ed.repaints = repaints;
                ed.slots = slots;
            }
        }
    }
    // the paint chosen is what the showroom is asked for (a mix of repaints is made by the loader)
    ed.paint = ed.mix.encode();
    // the bus is asked for without stickers (they are drawn over it, live)
    l.preview_other = Some((ed.file.clone(), ed.paint.clone()));
    l.preview_owner = ed.bus_id;
    l.preview_decals = Some(Vec::new());
    // the fields follow the selection
    if ed.sel.is_some_and(|i| i >= ed.decals.len()) {
        ed.sel = None;
        ed.carry = false;
    }
    if ed.sel != ed.sel_for {
        ed.sel_for = ed.sel;
        ed.sel_text.clear();
        ed.hex.clear();
        if let Some(d) = ed.sel.and_then(|i| ed.decals.get(i)) {
            if let Kind::Text { text, color, font, .. } = &d.kind {
                ed.sel_text = text.replace('\n', "|");
                ed.hex = format!("#{:02X}{:02X}{:02X}", color[0], color[1], color[2]);
                ed.font_sel = LiveryView::font_index(font);
            }
        }
    }

    let side_w = if r.w < 900.0 { 320.0 } else { 372.0 };
    let view = Rect::new(r.x, r.y, (r.w - side_w - GAP).max(200.0), r.h);
    let side = Rect::new(view.right() + GAP, r.y, (r.right() - view.right() - GAP).max(0.0), r.h);
    let mut acts: Vec<Act> = Vec::new();

    // --- the side panel
    l.ui.panel(side);
    let inner = Rect::new(side.x + 14.0, side.y + 14.0, side.w - 28.0, side.h - 28.0);
    l.ui.text_in("Customize", Rect::new(inner.x, inner.y, inner.w, 24.0), 18.0, Weight::Bold, TEXT, Align::Left);
    l.ui.text_in(&ed.title, Rect::new(inner.x, inner.y + 24.0, inner.w, 18.0), 12.5, Weight::Regular, TEXT_DIM, Align::Left);
    let foot_h = ROW * 2.0 + 8.0;
    let body = Rect::new(inner.x, inner.y + 52.0, inner.w, (inner.h - 52.0 - foot_h - 10.0).max(60.0));
    let own_label = vehicle_of(&l.state.vehicles, &ed.file).map(|v| super::drive::default_livery_label(v).to_string()).unwrap_or_else(|| "Default paint".to_string());
    let images_ready = !ed.images.is_empty();
    let fov_now = l.showroom.fov();
    let mut fov_set: Option<f32> = None;
    l.ui.scroll_area("liv-scroll", body, &mut |ui, v| {
        let w = v.w - 10.0;
        let mut y = v.y;
        let label = |ui: &mut super::ui::Ui, y: f32, text: &str| {
            ui.text_in(text, Rect::new(v.x, y, w, 16.0), 11.0, Weight::Bold, TEXT_DIM, Align::Left);
        };

        // what is customized
        ui.segmented("liv-mode", Rect::new(v.x, y, w, 34.0), &mut ed.mode, &["Decals", "Objects", "Paint"]);
        y += 46.0;
        let half = (w - 8.0) / 2.0;
        let objects = ed.mode == 1;

        // the camera
        label(ui, y, "VIEW");
        y += 20.0;
        let q = (w - 4.0 * 5.0) / 5.0;
        for (k, (name, yaw)) in [("Front", 180.0f32), ("Back", 0.0), ("Left", 90.0), ("Right", 270.0), ("Reset", 215.0)].into_iter().enumerate() {
            if ui.button(&format!("liv-view-{k}"), Rect::new(v.x + k as f32 * (q + 5.0), y, q, 28.0), name, None, ButtonKind::Normal) {
                acts.push(Act::View(yaw));
            }
        }
        y += 36.0;
        ui.segmented("liv-cam", Rect::new(v.x, y, w, 30.0), &mut ed.cam_mode, &["Orbit", "Free flight"]);
        y += 36.0;
        if ed.cam_mode == 1 {
            y += ui.paragraph("Drag looks around, pressed wheel slides, W A S D Q E fly (Shift: faster). Nothing stops the camera.", Vec2::new(v.x, y), w, 12.0, Weight::Regular, TEXT_FAINT) + 6.0;
        }
        let mut lens = fov_now;
        if ui.slider("liv-fov", Rect::new(v.x, y, w, 30.0), &mut lens, 8.0, 100.0, 1.0, "Field of view (wheel)", &|v| format!("{v:.0}°")) {
            fov_set = Some(lens);
        }
        y += 36.0;
        ui.toggle("liv-inside", Rect::new(v.x, y, w, 28.0), &mut ed.inside, "Interior view (stickers inside the bus)");
        y += 40.0;

        // the paint: another repaint, other textures on the same settings, one texture
        if ed.mode == 2 {
            y += paint_panel(ui, ed, v.x, y, w, &own_label);
            return y - v.y;
        }

        // where
        label(ui, y, "APPLIES TO");
        y += 20.0;
        ui.segmented("liv-scope", Rect::new(v.x, y, w, 32.0), &mut ed.scope, &["This bus", "Every bus of this model"]);
        y += 44.0;

        // the stickers, or the objects
        label(ui, y, if objects { "OBJECTS" } else { "STICKERS" });
        y += 20.0;
        if !ed.decals.iter().any(|d| d.is_object() == objects) {
            let none = if objects { "None yet. Choose an object below: it follows the cursor over the bus until you click." } else { "None yet. Add a text or a picture below: it follows the cursor over the bus until you click." };
            y += ui.paragraph(none, Vec2::new(v.x, y), w, 12.5, Weight::Regular, TEXT_FAINT) + 8.0;
        }
        for i in 0..ed.decals.len() {
            if ed.decals[i].is_object() != objects {
                continue;
            }
            let rr = Rect::new(v.x, y, w - 34.0, 32.0);
            if ui.row(&format!("liv-row-{i}"), rr, ed.sel == Some(i)) {
                ed.sel = if ed.sel == Some(i) { None } else { Some(i) };
                ed.carry = false;
            }
            let name = ed.decals[i].label();
            ui.text_in(&name, Rect::new(rr.x + 12.0, rr.y, rr.w - 16.0, rr.h), 13.0, Weight::Regular, if ed.sel == Some(i) { TEXT } else { TEXT_SOFT }, Align::Left);
            if ui.button(&format!("liv-del-{i}"), Rect::new(rr.right() + 4.0, y + 2.0, 28.0, 28.0), "", Some("close"), ButtonKind::Ghost) {
                acts.push(Act::Remove(i));
            }
            y += 34.0;
        }
        y += 8.0;

        if !objects {
            // adding
            label(ui, y, "ADD A STICKER");
            y += 20.0;
            ui.text_input("liv-newtext", Rect::new(v.x, y, w, ROW), &mut ed.new_text, "Text (| starts a new line)", None);
            y += ROW + 6.0;
            ui.select("liv-font-new", Rect::new(v.x, y, w, ROW), &mut ed.font_new, &ed.font_names);
            y += ROW + 6.0;
            ui.toggle("liv-glass-new", Rect::new(v.x, y, w, 28.0), &mut ed.glass_new, "On a window");
            y += 34.0;
            let add_r = Rect::new(v.x, y, w, ROW);
            if !ed.new_text.trim().is_empty() {
                if ui.button("liv-addtext", add_r, "Add the text", Some("add"), ButtonKind::Primary) {
                    acts.push(Act::AddText);
                }
            } else {
                ui.button_off(add_r, "Add the text", Some("add"), ButtonKind::Primary);
            }
            y += ROW + 12.0;
            if images_ready {
                ed.image_ix = ed.image_ix.min(ed.images.len() - 1);
                ui.select("liv-image", Rect::new(v.x, y, w, ROW), &mut ed.image_ix, &ed.images);
                y += ROW + 6.0;
                if ui.button("liv-addimg", Rect::new(v.x, y, w, ROW), "Add the picture", Some("add"), ButtonKind::Normal) {
                    acts.push(Act::AddImage(ed.images[ed.image_ix].clone()));
                }
                y += ROW + 10.0;
            }
            ui.select("liv-import-size", Rect::new(v.x, y, half, ROW), &mut ed.import_ix, &ed.size_names);
            if ui.button("liv-import", Rect::new(v.x + half + 8.0, y, half, ROW), "Import…", Some("folder_open"), ButtonKind::Normal) {
                acts.push(Act::Import);
            }
            y += ROW + 16.0;

        }

        // adding an object: the browser with the tiles
        if objects {
            label(ui, y, "ADD AN OBJECT");
            y += 20.0;
            if ed.assets.is_empty() {
                y += ui.paragraph("No objects installed. Drop a folder or a zip with a .asset file onto this window, or put it in the Assets folder.", Vec2::new(v.x, y), w, 12.5, Weight::Regular, TEXT_FAINT) + 8.0;
                ui.button_off(Rect::new(v.x, y, w, ROW), "Choose an object…", Some("add"), ButtonKind::Primary);
            } else if ui.button("liv-browse", Rect::new(v.x, y, w, ROW), "Choose an object…", Some("add"), ButtonKind::Primary) {
                acts.push(Act::OpenBrowser);
            }
            y += ROW + 6.0;
            if ui.button("liv-assets-open", Rect::new(v.x, y, half, 30.0), "Assets folder", Some("folder_open"), ButtonKind::Ghost) {
                acts.push(Act::OpenAssets);
            }
            if ui.button("liv-assets-scan", Rect::new(v.x + half + 8.0, y, half, 30.0), "Read again", Some("refresh"), ButtonKind::Ghost) {
                acts.push(Act::RescanAssets);
            }
            y += 46.0;
        }

        // the selected sticker
        if let Some(i) = ed.sel.filter(|i| *i < ed.decals.len()) {
            label(ui, y, if ed.decals[i].is_object() { "SELECTED OBJECT" } else { "SELECTED STICKER" });
            y += 20.0;
            let mv = Rect::new(v.x, y, half, 30.0);
            if ui.button("liv-move", mv, if ed.carry { "Click the bus to drop" } else { "Move" }, Some("add"), if ed.carry { ButtonKind::Primary } else { ButtonKind::Normal }) {
                ed.carry = !ed.carry;
            }
            if ui.button("liv-delete", Rect::new(v.x + half + 8.0, y, half, 30.0), "Delete", Some("delete"), ButtonKind::Danger) {
                acts.push(Act::Remove(i));
            }
            y += 38.0;
            if matches!(ed.decals[i].kind, Kind::Text { .. }) {
                if ui.text_input("liv-seltext", Rect::new(v.x, y, w, ROW), &mut ed.sel_text, "Text", None) {
                    let t = ed.sel_text.replace('|', "\n");
                    if let Kind::Text { text, .. } = &mut ed.decals[i].kind {
                        *text = t;
                    }
                }
                y += ROW + 6.0;
                if ui.select("liv-font-sel", Rect::new(v.x, y, w, ROW), &mut ed.font_sel, &ed.font_names) {
                    let f = LiveryView::font_file(ed.font_sel);
                    if let Kind::Text { font, .. } = &mut ed.decals[i].kind {
                        *font = f;
                    }
                }
                y += ROW + 6.0;
                let (mut bold, mut italic) = match &ed.decals[i].kind {
                    Kind::Text { bold, italic, .. } => (*bold, *italic),
                    _ => (false, false),
                };
                let b = ui.toggle("liv-bold", Rect::new(v.x, y, w, 28.0), &mut bold, "Bold");
                y += 30.0;
                let it = ui.toggle("liv-italic", Rect::new(v.x, y, w, 28.0), &mut italic, "Italic");
                y += 34.0;
                if b || it {
                    if let Kind::Text { bold: bb, italic: ii, .. } = &mut ed.decals[i].kind {
                        *bb = bold;
                        *ii = italic;
                    }
                }
                // the colour: a palette and a hex field
                let cur = match &ed.decals[i].kind {
                    Kind::Text { color, .. } => *color,
                    _ => [255; 3],
                };
                let sw = ((w - 4.0 * (PALETTE.len() as f32 - 1.0)) / PALETTE.len() as f32).clamp(14.0, 28.0);
                for (k, c) in PALETTE.iter().enumerate() {
                    let rr = Rect::new(v.x + k as f32 * (sw + 4.0), y, sw, sw);
                    if ui.button(&format!("liv-col-{k}"), rr, "", None, ButtonKind::Ghost) {
                        if let Kind::Text { color, .. } = &mut ed.decals[i].kind {
                            *color = *c;
                        }
                        ed.hex = format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]);
                    }
                    ui.p().rounded(rr.inset(2.0), 4.0, Color::rgba(c[0], c[1], c[2], 1.0));
                    if *c == cur {
                        ui.p().rounded_border(rr, 5.0, 1.5, ACCENT);
                    }
                }
                y += sw + 8.0;
                if ui.text_input("liv-hex", Rect::new(v.x, y, w, ROW), &mut ed.hex, "#RRGGBB", None) {
                    if let Some(c) = parse_hex(&ed.hex) {
                        if let Kind::Text { color, .. } = &mut ed.decals[i].kind {
                            *color = c;
                        }
                    }
                }
                y += ROW + 10.0;
            }
            let mut width = ed.decals[i].width;
            let is_obj = ed.decals[i].is_object();
            let (lo, hi) = if is_obj { (0.05, 5.0) } else { (0.02, 4.0) };
            if ui.slider("liv-width", Rect::new(v.x, y, w, 30.0), &mut width, lo, hi, if is_obj { 0.01 } else { 0.005 }, if is_obj { "Scale" } else { "Width" }, &|v| if is_obj { format!("{v:.2}×") } else { format!("{:.1} cm", v * 100.0) }) {
                ed.decals[i].width = width;
            }
            y += 34.0;
            if !is_obj {
                // the height: the same size seen the other way (the picture keeps its proportions)
                let aspect = decals::texture(&ed.decals[i]).map(|t| t.1).unwrap_or(1.0).max(0.01);
                let mut height = ed.decals[i].width / aspect;
                if ui.slider("liv-height", Rect::new(v.x, y, w, 30.0), &mut height, lo / aspect, hi / aspect, 0.005, "Height", &|v| format!("{:.1} cm", v * 100.0)) {
                    ed.decals[i].width = (height * aspect).clamp(lo, hi);
                }
                y += 34.0;
                // exact steps of a centimetre
                let q = (w - 3.0 * 6.0) / 4.0;
                for (k, (name, dw)) in [("Wider", 0.01f32), ("Narrower", -0.01), ("Taller", 0.01 * aspect), ("Shorter", -0.01 * aspect)].into_iter().enumerate() {
                    if ui.button(&format!("liv-size-{k}"), Rect::new(v.x + k as f32 * (q + 6.0), y, q, 28.0), name, None, ButtonKind::Normal) {
                        ed.decals[i].width = (ed.decals[i].width + dw).clamp(lo, hi);
                    }
                }
                y += 34.0;
            }
            if is_obj {
                // the gizmo in the picture, and the same by buttons, to the degree and the centimetre
                ui.segmented("liv-gizmo", Rect::new(v.x, y, w, 30.0), &mut ed.gizmo, &["Move", "Rotate", "No gizmo"]);
                y += 36.0;
                ui.segmented("liv-axes", Rect::new(v.x, y, w, 30.0), &mut ed.axes, &["Object's axes", "Bus's axes"]);
                y += 40.0;
                label(ui, y, "MOVE BY");
                y += 18.0;
                ui.segmented("liv-mstep", Rect::new(v.x, y, w, 28.0), &mut ed.move_step, &[MOVE_STEPS[0].0, MOVE_STEPS[1].0, MOVE_STEPS[2].0, MOVE_STEPS[3].0]);
                y += 34.0;
                let hw = (w - 6.0) / 2.0;
                for k in 0..3 {
                    if ui.button(&format!("liv-mv-{k}-"), Rect::new(v.x, y, hw, 28.0), &format!("{} −", AXIS_NAMES[k]), None, ButtonKind::Normal) {
                        acts.push(Act::MoveBy(k, -1.0));
                    }
                    if ui.button(&format!("liv-mv-{k}+"), Rect::new(v.x + hw + 6.0, y, hw, 28.0), &format!("{} +", AXIS_NAMES[k]), None, ButtonKind::Normal) {
                        acts.push(Act::MoveBy(k, 1.0));
                    }
                    y += 32.0;
                }
                y += 6.0;
                label(ui, y, "TURN BY");
                y += 18.0;
                ui.segmented("liv-tstep", Rect::new(v.x, y, w, 28.0), &mut ed.turn_step, &[TURN_STEPS[0].0, TURN_STEPS[1].0, TURN_STEPS[2].0, TURN_STEPS[3].0, TURN_STEPS[4].0]);
                y += 34.0;
                for k in 0..3 {
                    if ui.button(&format!("liv-tn-{k}-"), Rect::new(v.x, y, hw, 28.0), &format!("{} −", AXIS_NAMES[k]), None, ButtonKind::Normal) {
                        acts.push(Act::TurnBy(k, -1.0));
                    }
                    if ui.button(&format!("liv-tn-{k}+"), Rect::new(v.x + hw + 6.0, y, hw, 28.0), &format!("{} +", AXIS_NAMES[k]), None, ButtonKind::Normal) {
                        acts.push(Act::TurnBy(k, 1.0));
                    }
                    y += 32.0;
                }
                if ui.button("liv-tn-reset", Rect::new(v.x, y + 4.0, hw, 28.0), "As modelled", Some("refresh"), ButtonKind::Ghost) {
                    acts.push(Act::ResetTurn);
                }
                if ui.button("liv-tn-stand", Rect::new(v.x + hw + 6.0, y + 4.0, hw, 28.0), "On the surface", None, ButtonKind::Ghost) {
                    acts.push(Act::StandUp);
                }
                y += 42.0;
            } else {
                let mut turn = ed.decals[i].turn;
                if ui.slider("liv-turn", Rect::new(v.x, y, w - 64.0, 30.0), &mut turn, -180.0, 180.0, 1.0, "Turn", &|v| format!("{v:.0}°")) {
                    ed.decals[i].turn = turn;
                }
                if ui.button("liv-turn-zero", Rect::new(v.x + w - 58.0, y, 58.0, 30.0), "0°", None, ButtonKind::Normal) {
                    ed.decals[i].turn = 0.0;
                }
                y += 34.0;
            }
            if !is_obj {
                let mut glass = ed.decals[i].glass;
                if ui.toggle("liv-glass", Rect::new(v.x, y, w, 28.0), &mut glass, "On a window") {
                    ed.decals[i].glass = glass;
                    ed.carry = true;
                }
                y += 34.0;
                let mut mirror = ed.decals[i].mirror;
                if ui.toggle("liv-mirror", Rect::new(v.x, y, w, 28.0), &mut mirror, "Mirror") {
                    ed.decals[i].mirror = mirror;
                }
                y += 36.0;
            }
            if !is_obj {
                // moved a little along the picture's own directions
                let frame = decals::Frame::new(&ed.decals[i], 1.0);
                let step = 0.01;
                let q = (w - 3.0 * 6.0) / 4.0;
                for (k, (icon, dir)) in [("chevron_left", -frame.right), ("chevron_right", frame.right), ("keyboard_arrow_up", frame.up), ("keyboard_arrow_down", -frame.up)].into_iter().enumerate() {
                    if ui.button(&format!("liv-nudge-{k}"), Rect::new(v.x + k as f32 * (q + 6.0), y, q, 30.0), "", Some(icon), ButtonKind::Normal) {
                        let p = Vec3::from(ed.decals[i].pos) + dir * step;
                        ed.decals[i].pos = p.to_array();
                    }
                }
                y += 38.0;
            }
            if ui.button("liv-dup", Rect::new(v.x, y, w, 30.0), "Duplicate", Some("content_copy"), ButtonKind::Ghost) {
                acts.push(Act::Duplicate);
            }
            y += 42.0;
        }

        // back to the paint the bus came in
        if !ed.decals.is_empty() {
            if ui.button("liv-reset", Rect::new(v.x, y, w, 32.0), "Reset to the original paint", Some("refresh"), ButtonKind::Danger) {
                acts.push(Act::ResetAll);
            }
            y += 44.0;
        }

        // presets
        label(ui, y, "PRESETS OF THIS MODEL");
        y += 20.0;
        let can_save = ed.loaded && !ed.decals.is_empty() && !ed.preset_name.trim().is_empty();
        ui.text_input("liv-preset-name", Rect::new(v.x, y, w - 112.0, ROW), &mut ed.preset_name, "Name", None);
        let save_r = Rect::new(v.right() - 10.0 - 104.0, y, 104.0, ROW);
        if can_save {
            if ui.button("liv-preset-save", save_r, "Save", Some("save"), ButtonKind::Normal) {
                acts.push(Act::SavePreset);
            }
        } else {
            ui.button_off(save_r, "Save", Some("save"), ButtonKind::Normal);
        }
        y += ROW + 6.0;
        if ed.presets.is_empty() {
            y += ui.paragraph("A preset is a named set of stickers. It can be put on any bus of this model.", Vec2::new(v.x, y), w, 12.0, Weight::Regular, TEXT_FAINT) + 6.0;
        }
        for (i, (name, set)) in ed.presets.iter().enumerate() {
            let rr = Rect::new(v.x, y, w - 34.0, 32.0);
            if ui.row(&format!("liv-preset-{i}"), rr, false) {
                acts.push(Act::LoadPreset(i));
            }
            ui.text_in(name, Rect::new(rr.x + 12.0, rr.y, rr.w - 110.0, rr.h), 13.0, Weight::Regular, TEXT_SOFT, Align::Left);
            ui.text_in(&format!("{} stickers", set.len()), Rect::new(rr.right() - 100.0, rr.y, 92.0, rr.h), 11.5, Weight::Regular, TEXT_FAINT, Align::Right);
            if ui.button(&format!("liv-preset-del-{i}"), Rect::new(rr.right() + 4.0, y + 2.0, 28.0, 28.0), "", Some("close"), ButtonKind::Ghost) {
                acts.push(Act::DeletePreset(i));
            }
            y += 34.0;
        }
        y += 10.0;
        y - v.y
    });

    if let Some(f) = fov_set {
        l.showroom.set_fov(f);
    }

    // --- the footer
    let fy = inner.bottom() - foot_h;
    if ed.loaded {
        if l.ui.button("liv-save", Rect::new(inner.x, fy, inner.w, ROW), "Save and close", Some("check"), ButtonKind::Primary) {
            acts.push(Act::Save);
        }
    } else {
        l.ui.button_off(Rect::new(inner.x, fy, inner.w, ROW), "Loading the bus…", None, ButtonKind::Primary);
    }
    if l.ui.button("liv-close", Rect::new(inner.x, fy + ROW + 8.0, inner.w, ROW), "Close without saving", Some("close"), ButtonKind::Normal) {
        acts.push(Act::Close);
    }

    // --- the bus
    if ed.browser {
        // (the browser lies over the picture: the bus is not turned or picked under it, and the
        // wheel scrolls the tiles instead of changing the lens)
        l.ui.solid(view);
    }
    let blocked = l.ui.over_ui;
    let kept_wheel = if ed.browser { Some(std::mem::take(&mut l.ui.input.wheel)) } else { None };
    l.preview(view);
    if let Some(wh) = kept_wheel {
        l.ui.input.wheel = wh;
    }
    l.ui.over_ui = blocked;
    l.ui.p().rounded_border(view, RADIUS, 1.0, EDGE);
    let hint = if ed.cam_mode == 1 {
        "Free flight: drag looks around, W A S D Q E fly (Shift faster), wheel changes the lens"
    } else if ed.carry {
        "Move over the bus: it follows. Click to put it down"
    } else if ed.sel.is_some() {
        "Press Move to carry the selected one. Wheel: lens, pressed wheel moves, W A S D Q E fly"
    } else if ed.mode == 2 {
        "Choose the paint on the right. Drag turns the bus, wheel changes the lens"
    } else if ed.mode == 1 {
        "Choose an object: it follows the cursor over the bus. Wheel changes the lens"
    } else {
        "Add a text or a picture: it follows the cursor over the bus. Wheel changes the lens"
    };
    let hw = l.ui.width(hint, 12.5, Weight::Medium) + 24.0;
    let hr = Rect::new(view.x + 12.0, view.y + 12.0, hw, 26.0);
    l.ui.p().rounded(hr, 6.0, Color::rgba(0, 0, 0, 0.55));
    l.ui.text_in(hint, hr, 12.5, Weight::Medium, TEXT_SOFT, Align::Center);

    // the spot of the bus under the cursor (and the carried sticker with it)
    let mouse = l.ui.input.mouse;
    let over_view = view.contains(mouse) && !blocked;
    if over_view {
        if mouse != ed.last_mouse && ed.last_pick.elapsed().as_secs_f32() > 0.03 {
            ed.last_mouse = mouse;
            ed.last_pick = Instant::now();
            let ndc = Vec2::new((mouse.x - view.x) / view.w * 2.0 - 1.0, 1.0 - (mouse.y - view.y) / view.h * 2.0);
            let glass = match (ed.carry, ed.sel.and_then(|i| ed.decals.get(i))) {
                (true, Some(d)) => d.glass,
                _ => ed.glass_new && ed.mode == 0,
            };
            let hit = l.showroom.pick_at(ndc, glass);
            ed.hover = hit.map(|h| (h.pos, h.normal));
            ed.hover_mesh = hit.map(|h| h.mesh);
            if let (true, Some((p, n)), Some(i)) = (ed.carry, ed.hover, ed.sel) {
                let host = ed.hover_mesh.map(|m| m as u32);
                if let Some(d) = ed.decals.get_mut(i) {
                    d.pos = p.to_array();
                    d.normal = n.to_array();
                    if d.is_object() {
                        d.host = host;
                    }
                }
            }
        }
    }
    gizmo(l, ed, view, blocked);
    // a ring on the selected sticker
    if let Some(d) = ed.sel.and_then(|i| ed.decals.get(i)).filter(|d| !(d.is_object() && ed.gizmo < 2 && !ed.carry)) {
        if let Some(n) = l.showroom.project(Vec3::from(d.pos)) {
            let at = Vec2::new(view.x + (n.x * 0.5 + 0.5) * view.w, view.y + (0.5 - n.y * 0.5) * view.h);
            if view.contains(at) {
                l.ui.p().arc(at, 8.0, 10.0, 0.0, std::f32::consts::TAU, ACCENT);
                l.ui.p().circle(at, 2.5, ACCENT);
            }
        }
    }
    // a click (not a drag, which turns the bus) puts a carried sticker down
    if l.ui.input.pressed && over_view {
        ed.press = Some(mouse);
    }
    if l.ui.input.released {
        if let Some(p) = ed.press.take() {
            if over_view && p.distance(mouse) < 5.0 && ed.carry && ed.hover.is_some() {
                ed.carry = false;
            }
        }
    }

    if ed.browser {
        browser(l, ed, view, &mut acts);
    }

    // --- what was asked
    for a in acts {
        match a {
            Act::AddText => {
                let (p, n) = ed.hover.unwrap_or((Vec3::new(0.0, 0.0, 1.0), Vec3::X));
                let font = LiveryView::font_file(ed.font_new);
                let mut d = Decal::text(&ed.new_text.replace('|', "\n"), &font, p, n);
                d.glass = ed.glass_new;
                ed.decals.push(d);
                ed.sel = Some(ed.decals.len() - 1);
                ed.carry = true;
            }
            Act::AddImage(name) => {
                let (p, n) = ed.hover.unwrap_or((Vec3::new(0.0, 0.0, 1.0), Vec3::X));
                let mut d = Decal::image(&name, p, n);
                d.glass = ed.glass_new;
                ed.decals.push(d);
                ed.sel = Some(ed.decals.len() - 1);
                ed.carry = true;
            }
            Act::AddObject(id) => {
                let (p, n) = ed.hover.unwrap_or((Vec3::new(0.0, 0.0, 1.0), Vec3::X));
                let mut d = Decal::object(&id, p, n);
                d.host = ed.hover_mesh.map(|m| m as u32);
                ed.decals.push(d);
                ed.sel = Some(ed.decals.len() - 1);
                ed.carry = true;
                ed.browser = false;
            }
            Act::OpenBrowser => {
                ed.browser = true;
                ed.carry = false;
                ed.asset_ix = ed.asset_ix.min(ed.assets.len().saturating_sub(1));
            }
            Act::CloseBrowser => ed.browser = false,
            Act::OpenAssets => crate::assets::open_folder(),
            Act::MoveBy(k, sign) => {
                if let Some(i) = ed.sel.filter(|i| ed.decals.get(*i).is_some_and(|d| d.is_object())) {
                    let axis = ed.axes_of(&ed.decals[i])[k];
                    let p = Vec3::from(ed.decals[i].pos) + axis * (sign * MOVE_STEPS[ed.move_step.min(MOVE_STEPS.len() - 1)].1);
                    ed.decals[i].pos = p.to_array();
                }
            }
            Act::TurnBy(k, sign) => {
                if let Some(i) = ed.sel.filter(|i| ed.decals.get(*i).is_some_and(|d| d.is_object())) {
                    let axis = ed.axes_of(&ed.decals[i])[k];
                    let a = (sign * TURN_STEPS[ed.turn_step.min(TURN_STEPS.len() - 1)].1).to_radians();
                    ed.decals[i].turn_about(axis, a);
                }
            }
            Act::StandUp => {
                if let Some(d) = ed.sel.and_then(|i| ed.decals.get_mut(i)) {
                    d.stand_on_surface();
                }
            }
            Act::ResetTurn => {
                if let Some(d) = ed.sel.and_then(|i| ed.decals.get_mut(i)) {
                    d.rot = [0.0, 0.0, 0.0, 1.0];
                }
            }
            Act::RescanAssets => {
                ed.refresh_assets();
                l.state.set_status(format!("{} objects installed", ed.assets.len()), false);
            }
            Act::Import => {
                if let Some(path) = core::pick_file("Choose a picture") {
                    match decals::import_image(&path, IMPORT_SIZES[ed.import_ix.min(IMPORT_SIZES.len() - 1)].1) {
                        Ok(name) => {
                            ed.images = decals::image_files();
                            ed.image_ix = ed.images.iter().position(|n| *n == name).unwrap_or(0);
                            l.state.set_status(format!("{name} imported"), false);
                        }
                        Err(e) => l.state.set_status(e, true),
                    }
                }
            }
            Act::Remove(i) => {
                if i < ed.decals.len() {
                    ed.decals.remove(i);
                    ed.sel = None;
                    ed.carry = false;
                }
            }
            Act::Duplicate => {
                if let Some(d) = ed.sel.and_then(|i| ed.decals.get(i)).cloned() {
                    let mut d = d;
                    let f = decals::Frame::new(&d, 1.0);
                    let p = Vec3::from(d.pos) - f.up * (d.width * 0.5).min(0.4);
                    d.pos = p.to_array();
                    ed.decals.push(d);
                    ed.sel = Some(ed.decals.len() - 1);
                    ed.carry = true;
                }
            }
            Act::ResetAll => {
                ed.decals.clear();
                ed.sel = None;
                ed.carry = false;
                l.state.set_status("All stickers removed: Save and close keeps the original paint", false);
            }
            Act::View(yaw) => {
                // (a button looks at the bus from outside: the orbit camera again)
                ed.cam_mode = 0;
                l.showroom.set_free(false);
                // (the last button is the usual three-quarter view)
                l.showroom.view_from(yaw, if yaw == 215.0 { 8.0 } else { 0.0 });
            }
            Act::SavePreset => {
                let name = ed.preset_name.trim().to_string();
                match decals::save_preset(&name, &ed.model, &ed.decals) {
                    Ok(()) => {
                        ed.presets = decals::presets_for(&ed.model);
                        l.state.set_status(format!("Preset {name} saved"), false);
                    }
                    Err(e) => l.state.set_status(format!("The preset could not be saved: {e}"), true),
                }
            }
            Act::LoadPreset(i) => {
                if let Some((_, set)) = ed.presets.get(i) {
                    ed.decals = set.clone();
                    ed.sel = None;
                    ed.carry = false;
                }
            }
            Act::DeletePreset(i) => {
                if let Some((name, _)) = ed.presets.get(i) {
                    decals::delete_preset(name);
                    ed.presets = decals::presets_for(&ed.model);
                }
            }
            Act::Save => {
                // (the paint first: it is kept even if the decals cannot be written)
                if ed.paint != ed.paint_open {
                    let paint = ed.paint.clone();
                    let done = l.career.company.as_mut().and_then(|c| c.fleet.iter_mut().find(|b| b.id == ed.bus_id)).map(|b| b.paint = paint).is_some();
                    if done {
                        super::career::save(l);
                        ed.paint_open = ed.paint.clone();
                    }
                }
                let res = if ed.scope == 0 {
                    decals::save_bus(ed.bus_id, &ed.model, &ed.decals)
                } else {
                    decals::forget_bus(ed.bus_id);
                    decals::save_model(&ed.model, &ed.decals)
                };
                match res {
                    Ok(()) => {
                        let what = if ed.scope == 0 { "this bus" } else { "every bus of the model" };
                        l.state.set_status(format!("Decals saved for {what}"), false);
                        return false;
                    }
                    Err(e) => l.state.set_status(format!("The decals could not be saved: {e}"), true),
                }
            }
            Act::Close => return false,
        }
    }

    // --- the stickers over the bus
    if let Some(rd) = l.renderer.as_ref() {
        l.showroom.set_live(rd, &ed.decals);
    }
    true
}

/// The Paint tab: the whole repaint (its pictures and its settings), the pictures of another
/// repaint on the same settings, and single texture slots from any repaint - the bus's own
/// configuration stays as it is for the last two. Every choice changes `ed.mix` (the picture
/// follows at once, the bus is read again with the paint it names). Returns the height used.
fn paint_panel(ui: &mut super::ui::Ui, ed: &mut LiveryView, x: f32, y0: f32, w: f32, own: &str) -> f32 {
    let mut y = y0;
    let label = |ui: &mut super::ui::Ui, y: f32, text: &str| {
        ui.text_in(text, Rect::new(x, y, w, 16.0), 11.0, Weight::Bold, TEXT_DIM, Align::Left);
    };
    if ed.repaints.is_empty() {
        let text = if ed.loaded { "This bus has no repaints besides its own paint." } else { "Loading the bus…" };
        y += ui.paragraph(text, Vec2::new(x, y), w, 12.5, Weight::Regular, TEXT_FAINT) + 8.0;
        return y - y0;
    }

    // the whole repaint
    label(ui, y, "REPAINT");
    y += 20.0;
    y += ui.paragraph("The whole repaint: its pictures and its settings (what it tells the bus, such as a roof or mirrors it adds).", Vec2::new(x, y), w, 12.0, Weight::Regular, TEXT_FAINT) + 6.0;
    let mut names: Vec<String> = vec![own.to_string()];
    names.extend(ed.repaints.iter().cloned());
    let mut ix = ed.repaints.iter().position(|r| r.eq_ignore_ascii_case(&ed.mix.base)).map_or(0, |i| i + 1);
    if ui.select("liv-paint-base", Rect::new(x, y, w, ROW), &mut ix, &names) {
        ed.mix.base = if ix == 0 { String::new() } else { ed.repaints[ix - 1].clone() };
    }
    y += ROW + 18.0;

    // the pictures of another repaint, the settings as they are
    label(ui, y, "PICTURES ONLY");
    y += 20.0;
    y += ui.paragraph("The pictures of another repaint on the settings above. Nothing of the bus's configuration changes.", Vec2::new(x, y), w, 12.0, Weight::Regular, TEXT_FAINT) + 6.0;
    let mut names: Vec<String> = vec!["Those of the repaint".to_string()];
    names.extend(ed.repaints.iter().cloned());
    let mut ix = ed.repaints.iter().position(|r| r.eq_ignore_ascii_case(&ed.mix.textures_of)).map_or(0, |i| i + 1);
    if ui.select("liv-paint-tex", Rect::new(x, y, w, ROW), &mut ix, &names) {
        ed.mix.textures_of = if ix == 0 { String::new() } else { ed.repaints[ix - 1].clone() };
    }
    y += ROW + 18.0;

    // single texture slots
    label(ui, y, "SINGLE TEXTURES");
    y += 20.0;
    if ed.slots.is_empty() {
        y += ui.paragraph("No texture of this bus can be chosen on its own.", Vec2::new(x, y), w, 12.0, Weight::Regular, TEXT_FAINT) + 8.0;
    } else {
        y += ui.paragraph("One texture (a stripe, a destination display) from any repaint; the rest stays as above.", Vec2::new(x, y), w, 12.0, Weight::Regular, TEXT_FAINT) + 6.0;
        for k in 0..ed.slots.len() {
            let (slot, from) = (ed.slots[k].0.clone(), ed.slots[k].1.clone());
            ui.text_in(&slot, Rect::new(x, y, w, 16.0), 12.0, Weight::Medium, TEXT_SOFT, Align::Left);
            y += 18.0;
            let mut names: Vec<String> = vec!["As above".to_string()];
            names.extend(from.iter().cloned());
            let mut ix = ed.mix.slot_from(&slot).and_then(|f| from.iter().position(|n| n.eq_ignore_ascii_case(f))).map_or(0, |i| i + 1);
            if ui.select(&format!("liv-paint-slot-{k}"), Rect::new(x, y, w, ROW), &mut ix, &names) {
                let pick = if ix == 0 { String::new() } else { from[ix - 1].clone() };
                ed.mix.set_slot(&slot, &pick);
            }
            y += ROW + 10.0;
        }
    }
    y += 8.0;

    // back to the paint the bus came in
    if ed.mix.encode() != ed.paint_open.trim() {
        if ui.button("liv-paint-reset", Rect::new(x, y, w, 32.0), "Back to the paint it came in", Some("refresh"), ButtonKind::Ghost) {
            ed.mix = Mix::parse(&ed.paint_open);
        }
        y += 44.0;
    }
    y += ui.paragraph("Save and close keeps the paint for this bus. Stickers stay as they are.", Vec2::new(x, y), w, 12.0, Weight::Regular, TEXT_FAINT) + 8.0;
    y - y0
}

/// The object browser: a panel over the picture with a tile for every installed asset (its
/// picture, name and author), and for the chosen one a larger picture and the description.
fn browser(l: &mut Launcher, ed: &mut LiveryView, view: Rect, acts: &mut Vec<Act>) {
    let panel = view.inset(18.0);
    l.ui.panel(panel);
    l.ui.solid(panel);
    let inner = panel.inset(18.0);
    l.ui.text_in("Choose an object", Rect::new(inner.x, inner.y, inner.w - 120.0, 26.0), 18.0, Weight::Bold, TEXT, Align::Left);
    l.ui.text_in(&format!("{} installed", ed.assets.len()), Rect::new(inner.x, inner.y + 26.0, inner.w - 120.0, 16.0), 12.0, Weight::Regular, TEXT_DIM, Align::Left);
    if l.ui.button("liv-browser-close", Rect::new(inner.right() - 104.0, inner.y, 104.0, 32.0), "Close", Some("close"), ButtonKind::Normal) {
        acts.push(Act::CloseBrowser);
    }
    let body = Rect::new(inner.x, inner.y + 52.0, inner.w, (inner.h - 52.0).max(60.0));
    if ed.assets.is_empty() {
        l.ui.paragraph("No objects installed. Drop a folder or a zip with a .asset file onto this window, or put it in the Assets folder, then press Read again.", Vec2::new(body.x, body.y), body.w.min(520.0), 13.0, Weight::Regular, TEXT_FAINT);
        if l.ui.button("liv-browser-open", Rect::new(body.x, body.y + 64.0, 160.0, 32.0), "Assets folder", Some("folder_open"), ButtonKind::Normal) {
            acts.push(Act::OpenAssets);
        }
        if l.ui.button("liv-browser-scan", Rect::new(body.x + 170.0, body.y + 64.0, 160.0, 32.0), "Read again", Some("refresh"), ButtonKind::Normal) {
            acts.push(Act::RescanAssets);
        }
        return;
    }
    ed.asset_ix = ed.asset_ix.min(ed.assets.len() - 1);
    // --- the pictures not drawn yet (a couple a frame: the one chosen first)
    let rev = crate::assets::revision();
    let key_of = |id: &str| format!("asset:{id}:{rev}");
    let tail = format!(":{rev}");
    l.icons.retain(|k, _| !k.starts_with("asset:") || k.ends_with(&tail));
    let mut order: Vec<usize> = vec![ed.asset_ix];
    order.extend((0..ed.assets.len()).filter(|i| *i != ed.asset_ix));
    let mut made = 0;
    for i in order {
        if made >= 2 {
            break;
        }
        let key = key_of(&ed.assets[i].id);
        if l.icons.contains_key(&key) || l.icons_pending.iter().any(|(k, _)| *k == key) || ed.thumb_failed.contains(&key) {
            continue;
        }
        made += 1;
        match crate::assets::thumbnail(&ed.assets[i].id, 320, 240) {
            Some(img) => l.icons_pending.push((key, img)),
            None => {
                ed.thumb_failed.insert(key);
            }
        }
    }
    // --- the tiles and the detail
    let detail_w = (body.w * 0.34).clamp(240.0, 340.0);
    let grid = Rect::new(body.x, body.y, (body.w - detail_w - 14.0).max(120.0), body.h);
    let detail = Rect::new(grid.right() + 14.0, body.y, (body.right() - grid.right() - 14.0).max(100.0), body.h);
    let icons = &l.icons;
    let assets = &ed.assets;
    let sel_ix = &mut ed.asset_ix;
    let icon_of = |id: &str| icons.get(&key_of(id)).copied();
    l.ui.scroll_area("liv-tiles", grid, &mut |ui, v| {
        let gw = v.w - 12.0;
        let cols = ((gw + 10.0) / 180.0).floor().max(1.0) as usize;
        let tw = (gw - (cols as f32 - 1.0) * 10.0) / cols as f32;
        let th = tw * 0.75;
        let tile_h = th + 12.0 + 20.0 + 18.0 + 6.0;
        for (i, a) in assets.iter().enumerate() {
            let r = Rect::new(v.x + (i % cols) as f32 * (tw + 10.0), v.y + (i / cols) as f32 * (tile_h + 10.0), tw, tile_h);
            ui.p().rounded(r, 8.0, FIELD);
            if ui.row(&format!("liv-tile-{i}"), r, *sel_ix == i) {
                *sel_ix = i;
            }
            let pic = Rect::new(r.x + 6.0, r.y + 6.0, r.w - 12.0, th);
            match icon_of(&a.id) {
                Some(tex) => ui.image(pic, tex, 6.0),
                None => {
                    ui.p().rounded(pic, 6.0, Color::rgba(44, 44, 48, 1.0));
                    ui.icon("deployed_code", pic.center(), (pic.h * 0.35).clamp(14.0, 40.0), TEXT_FAINT);
                }
            }
            ui.text_in(&a.name, Rect::new(r.x + 8.0, pic.bottom() + 6.0, r.w - 16.0, 20.0), 13.0, Weight::Bold, TEXT, Align::Left);
            let by = if a.author.is_empty() { String::new() } else { format!("by {}", a.author) };
            ui.text_in(&by, Rect::new(r.x + 8.0, pic.bottom() + 26.0, r.w - 16.0, 18.0), 11.5, Weight::Regular, TEXT_DIM, Align::Left);
        }
        let rows = assets.len().div_ceil(cols);
        rows as f32 * (tile_h + 10.0) + 4.0
    });
    // the chosen one
    let a = ed.assets[ed.asset_ix].clone();
    let pic = Rect::new(detail.x, detail.y, detail.w, detail.w * 0.75);
    match l.icons.get(&key_of(&a.id)).copied() {
        Some(tex) => l.ui.image(pic, tex, 8.0),
        None => {
            l.ui.p().rounded(pic, 8.0, FIELD);
            l.ui.icon("deployed_code", pic.center(), 40.0, TEXT_FAINT);
        }
    }
    let mut y = pic.bottom() + 12.0;
    l.ui.text_in(&a.name, Rect::new(detail.x, y, detail.w, 22.0), 16.0, Weight::Bold, TEXT, Align::Left);
    y += 24.0;
    if !a.author.is_empty() {
        l.ui.text_in(&format!("by {}", a.author), Rect::new(detail.x, y, detail.w, 18.0), 12.5, Weight::Regular, TEXT_DIM, Align::Left);
        y += 22.0;
    }
    let foot = 30.0 + 6.0 + ROW + 4.0;
    let desc = Rect::new(detail.x, y + 4.0, detail.w, (detail.bottom() - foot - y - 8.0).max(30.0));
    let text = if a.description.is_empty() { "No description.".to_string() } else { a.description.clone() };
    l.ui.scroll_area("liv-desc", desc, &mut |ui, v| ui.paragraph(&text, Vec2::new(v.x, v.y), v.w - 10.0, 12.5, Weight::Regular, TEXT_SOFT) + 6.0);
    let by = detail.bottom() - ROW;
    if l.ui.button("liv-browser-add", Rect::new(detail.x, by, detail.w, ROW), "Add to the bus", Some("add"), ButtonKind::Primary) {
        acts.push(Act::AddObject(a.id.clone()));
    }
    let hw = (detail.w - 8.0) / 2.0;
    if l.ui.button("liv-browser-open", Rect::new(detail.x, by - 36.0, hw, 30.0), "Assets folder", Some("folder_open"), ButtonKind::Ghost) {
        acts.push(Act::OpenAssets);
    }
    if l.ui.button("liv-browser-scan", Rect::new(detail.x + hw + 8.0, by - 36.0, hw, 30.0), "Read again", Some("refresh"), ButtonKind::Ghost) {
        acts.push(Act::RescanAssets);
    }
}

/// Distance from a point to a segment (px), and where on it (0..1).
fn segment_distance(p: Vec2, a: Vec2, b: Vec2) -> (f32, f32) {
    let ab = b - a;
    let len2 = ab.length_squared();
    let t = if len2 < 1e-6 { 0.0 } else { ((p - a).dot(ab) / len2).clamp(0.0, 1.0) };
    (p.distance(a + ab * t), t)
}

/// The selected object's gizmo in the picture: arrows along its three axes (Move) or rings
/// about them (Rotate), the axes being the object's own or the bus's. A handle is taken with
/// the left button and dragged; the bus is not turned by that drag (`Launcher::gizmo_hot`).
fn gizmo(l: &mut Launcher, ed: &mut LiveryView, view: Rect, blocked: bool) {
    let Some(i) = ed.sel.filter(|i| ed.decals.get(*i).is_some_and(|d| d.is_object())) else {
        ed.drag = None;
        return;
    };
    if ed.carry || ed.gizmo >= 2 {
        ed.drag = None;
        return;
    }
    let d = ed.decals[i].clone();
    let c = Vec3::from(d.pos);
    let (Some(ppm), Some(nc)) = (l.showroom.px_per_metre(c, view.h), l.showroom.project(c)) else { return };
    let to_px = |n: Vec2| Vec2::new(view.x + (n.x * 0.5 + 0.5) * view.w, view.y + (0.5 - n.y * 0.5) * view.h);
    let origin = to_px(nc);
    if !view.contains(origin) {
        return;
    }
    let axes = ed.axes_of(&d);
    let rotate = ed.gizmo == 1;
    // the arrows are about 90 px long, the rings 72 px across (in metres here)
    let len = (90.0 / ppm).clamp(0.02, 4.0);
    let ring_r = len * 0.8;
    let mouse = l.ui.input.mouse;
    const N: usize = 48;
    // the handles on the screen: an arrow is origin -> tip; a ring is a loop of points
    let mut tips: [Option<Vec2>; 3] = [None; 3];
    let mut rings: Vec<Vec<Option<Vec2>>> = vec![Vec::new(); 3];
    for k in 0..3 {
        if rotate {
            let u = axes[k].any_orthonormal_vector();
            let w = axes[k].cross(u);
            rings[k] = (0..=N)
                .map(|s| {
                    let a = s as f32 / N as f32 * std::f32::consts::TAU;
                    l.showroom.project(c + (u * a.cos() + w * a.sin()) * ring_r).map(to_px)
                })
                .collect();
        } else {
            tips[k] = l.showroom.project(c + axes[k] * len).map(to_px);
        }
    }
    // which handle the mouse is on (the nearest within 9 px), and for a ring where
    let mut near: Option<(f32, usize, Vec2)> = None;
    if !blocked && view.contains(mouse) {
        for k in 0..3 {
            if rotate {
                for s in 0..N {
                    if let (Some(a), Some(b)) = (rings[k][s], rings[k][s + 1]) {
                        let (dist, _) = segment_distance(mouse, a, b);
                        if dist < 9.0 && near.is_none_or(|n| dist < n.0) {
                            near = Some((dist, k, (b - a).normalize_or_zero()));
                        }
                    }
                }
            } else if let Some(tip) = tips[k] {
                let (dist, _) = segment_distance(mouse, origin, tip);
                if dist < 9.0 && near.is_none_or(|n| dist < n.0) {
                    near = Some((dist, k, Vec2::ZERO));
                }
            }
        }
    }
    let active = ed.drag.map(|dr| dr.axis).or(near.map(|n| n.1));
    if active.is_some() {
        l.gizmo_hot = true;
    }
    // a press on a handle takes it
    if ed.drag.is_none() && l.ui.input.pressed {
        if let Some((_, k, tangent)) = near {
            ed.drag = Some(Drag { axis: k, rotate, last: mouse, tangent, radius_px: ring_r * ppm });
        }
    }
    // the drag
    if let Some(dr) = ed.drag {
        if !l.ui.input.down || dr.rotate != rotate {
            ed.drag = None;
        } else {
            let delta = mouse - dr.last;
            if delta != Vec2::ZERO {
                if dr.rotate {
                    // (the way the ring runs where the mouse is now, not where it was taken)
                    let mut tangent = dr.tangent;
                    let mut best = f32::MAX;
                    for s in 0..N {
                        if let (Some(a), Some(b)) = (rings[dr.axis][s], rings[dr.axis][s + 1]) {
                            let (dist, _) = segment_distance(mouse, a, b);
                            if dist < best {
                                best = dist;
                                tangent = (b - a).normalize_or_zero();
                            }
                        }
                    }
                    let ang = delta.dot(tangent) / dr.radius_px.max(10.0);
                    ed.decals[i].turn_about(axes[dr.axis], ang);
                } else if let Some(tip) = tips[dr.axis] {
                    let along = tip - origin;
                    let px_per_m = along.length() / len;
                    // (an arrow pointing at the camera cannot be dragged: nothing to follow)
                    if px_per_m > 4.0 {
                        let m = delta.dot(along.normalize_or_zero()) / px_per_m;
                        let p = Vec3::from(ed.decals[i].pos) + axes[dr.axis] * m;
                        ed.decals[i].pos = p.to_array();
                    }
                }
                if let Some(dr) = ed.drag.as_mut() {
                    dr.last = mouse;
                }
            }
        }
    }
    // drawing
    let col = |k: usize| {
        let c = AXIS_COLORS[k];
        let base = Color::rgba(c[0], c[1], c[2], 1.0);
        if active == Some(k) { base.lighten(0.45) } else { base }
    };
    let p = l.ui.p();
    for k in 0..3 {
        if rotate {
            let pts: Vec<Option<Vec2>> = rings[k].clone();
            for s in 0..N {
                if let (Some(a), Some(b)) = (pts[s], pts[s + 1]) {
                    p.line(a, b, if active == Some(k) { 4.0 } else { 2.5 }, col(k));
                }
            }
        } else if let Some(tip) = tips[k] {
            let dir = (tip - origin).normalize_or_zero();
            let side = Vec2::new(-dir.y, dir.x);
            p.line(origin, tip - dir * 8.0, if active == Some(k) { 4.5 } else { 3.0 }, col(k));
            p.convex(&[tip, tip - dir * 14.0 + side * 6.0, tip - dir * 14.0 - side * 6.0], col(k));
        }
    }
    p.circle(origin, 4.0, Color::rgba(255, 255, 255, 1.0));
}

/// "#RRGGBB" (the # is optional).
fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let t = s.trim().trim_start_matches('#');
    if t.len() != 6 || !t.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let v = u32::from_str_radix(t, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

#[cfg(test)]
mod tests {
    use super::parse_hex;

    #[test]
    fn hex_colours() {
        assert_eq!(parse_hex("#FF8000"), Some([255, 128, 0]));
        assert_eq!(parse_hex("00ff00"), Some([0, 255, 0]));
        assert_eq!(parse_hex("#12345"), None);
        assert_eq!(parse_hex("#GGGGGG"), None);
    }
}
