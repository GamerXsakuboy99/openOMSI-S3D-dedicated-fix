//! Career Mode: which buses the dealership offers. Shown after founding a company and from
//! the dealership ("Choose offered buses").
//!
//! The list can be read by two sets of names: the ones the `.bus` files give (manufacturer,
//! then model, as the packs wrote them - the same list whatever the catalogue says) or the
//! ones the catalogue corrects them to (several spellings of a model come to one row, which
//! switches all of them). A model switched off is not offered (`Company::ignored`, which
//! always holds the files' own keys); the catalogue can mark models as hidden by default for
//! new companies. Rows are tinted by whether they are offered, and the list can be narrowed
//! to the offered or to the hidden ones.
//!
//! The list is laid out as one flat run of rows (a manufacturer's header, then its models), each
//! with its own place, and every row is drawn when it touches the view - not the block of a
//! manufacturer as a whole, which left the models of a manufacturer whose header had scrolled
//! out of sight undrawn.

use super::career::save;
use super::theme::*;
use super::ui::{id_of, ButtonKind};
use super::Launcher;
use glam::Vec2;
use omsi_launcher_lib::catalog::Catalog;
use omsi_ui::paint::Align;
use omsi_ui::{Color, Rect, Weight};
use std::collections::HashSet;

const MAKER_H: f32 = 34.0;
const ITEM_H: f32 = 30.0;

/// One model of the list: the name shown, the keys (of the files' models) it switches, and
/// the number of bus files it comes to.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Item {
    pub label: String,
    pub keys: Vec<String>,
    pub count: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Maker {
    pub name: String,
    pub items: Vec<Item>,
}

#[derive(Default)]
pub struct IgnoreView {
    tree: Vec<Maker>,
    tree_key: Option<(u32, usize, bool)>,
    open: HashSet<String>,
    search: String,
    /// 0 all, 1 only the offered, 2 only the hidden.
    view: usize,
    /// Read by the catalogue's names (false: by the names the files give).
    by_catalog: bool,
}

/// What a row of the list is.
enum Kind {
    Maker { name: String, mkey: String, keys: Vec<String>, shown: usize, offered: usize, open: bool },
    Item { m: usize, i: usize },
}

struct Line {
    y: f32,
    h: f32,
    kind: Kind,
}

enum Op {
    Fold(String),
    /// Set these models (and no others) to this: true = offered.
    Set(Vec<String>, bool),
}

/// How many of the keys are offered, out of how many.
fn offered_of(ignored: &[String], keys: &[String]) -> (usize, usize) {
    (keys.iter().filter(|k| !ignored.contains(k)).count(), keys.len())
}

/// Whether a model with `on` of its `total` keys offered belongs to the view (0 all, 1 the
/// offered - also the partly offered, 2 the hidden - also the partly hidden).
pub(super) fn passes(view: usize, on: usize, total: usize) -> bool {
    match view {
        1 => on > 0,
        2 => on < total,
        _ => true,
    }
}

/// Whether a row at `y` (height `h`) touches the view `top..bottom`, with a little margin.
pub(super) fn in_view(y: f32, h: f32, top: f32, bottom: f32) -> bool {
    y + h >= top - 4.0 && y <= bottom + 4.0
}

/// The rows of the list for the search and the view: a manufacturer's header, then its
/// models when it is open (or the list is narrowed), with their places from the top.
fn layout(tree: &[Maker], open: &HashSet<String>, q: &str, view: usize, ignored: &[String]) -> (Vec<Line>, f32) {
    let narrowed = !q.is_empty() || view != 0;
    let mut lines = Vec::new();
    let mut y = 0.0;
    for (mi, m) in tree.iter().enumerate() {
        let maker_hit = !q.is_empty() && m.name.to_lowercase().contains(q);
        let shown: Vec<usize> = (0..m.items.len())
            .filter(|&i| {
                let t = &m.items[i];
                let (on, total) = offered_of(ignored, &t.keys);
                (q.is_empty() || maker_hit || t.label.to_lowercase().contains(q)) && passes(view, on, total)
            })
            .collect();
        if shown.is_empty() {
            continue;
        }
        let mkey = format!("ign-m:{}", m.name.to_lowercase());
        let is_open = narrowed || open.contains(&mkey);
        let keys: Vec<String> = shown.iter().flat_map(|&i| m.items[i].keys.iter().cloned()).collect();
        let offered = shown.iter().filter(|&&i| offered_of(ignored, &m.items[i].keys).0 > 0).count();
        lines.push(Line { y, h: MAKER_H, kind: Kind::Maker { name: m.name.clone(), mkey, keys, shown: shown.len(), offered, open: is_open } });
        y += MAKER_H;
        if is_open {
            for i in shown {
                lines.push(Line { y, h: ITEM_H, kind: Kind::Item { m: mi, i } });
                y += ITEM_H;
            }
        }
    }
    (lines, y)
}

fn build(cat: &Catalog, vehicles: &[omsi_launcher_lib::VehicleInfo], by_catalog: bool) -> Vec<Maker> {
    if by_catalog {
        cat.named_tree(vehicles)
            .into_iter()
            .map(|m| Maker { name: m.name, items: m.types.into_iter().map(|t| Item { label: t.label, keys: t.keys, count: t.count }).collect() })
            .collect()
    } else {
        cat.raw_tree(vehicles)
            .into_iter()
            .map(|m| Maker { name: m.name, items: m.types.into_iter().map(|t| Item { label: t.label, keys: vec![t.key], count: t.count }).collect() })
            .collect()
    }
}

pub(super) fn draw(l: &mut Launcher, r: Rect) {
    let Some(cat) = l.career.catalog.as_ref() else { return };
    let key = (l.career.cat_rev, l.state.vehicles.len(), l.career.ign.by_catalog);
    if l.career.ign.tree_key != Some(key) {
        let tree = build(cat, &l.state.vehicles, l.career.ign.by_catalog);
        l.career.ign.tree = tree;
        l.career.ign.tree_key = Some(key);
    }
    let Some(company) = l.career.company.clone() else { return };

    // --- the top
    let panel = Rect::new(r.x, r.y, r.w, 88.0);
    l.ui.panel(panel);
    let inner = Rect::new(panel.x + 18.0, panel.y + 12.0, panel.w - 36.0, panel.h - 24.0);
    l.ui.text_in("Which buses should the dealer offer?", Rect::new(inner.x, inner.y, inner.w, 24.0), 16.0, Weight::Bold, TEXT, Align::Left);
    let note = if l.career.ign.by_catalog {
        "Switch off the models you never want to see in the dealership. The names are the catalogue's (corrected manufacturer and model; one row can cover several spellings from the bus files). You can change this later: Dealership > Choose offered buses."
    } else {
        "Switch off the models you never want to see in the dealership. The names are the ones the bus files give (manufacturer and model as the pack wrote them). You can change this later: Dealership > Choose offered buses."
    };
    l.ui.paragraph(note, Vec2::new(inner.x, inner.y + 28.0), inner.w - 150.0, 12.5, Weight::Regular, TEXT_DIM);
    if l.ui.button("ign-done", Rect::new(inner.right() - 130.0, inner.y + 6.0, 130.0, ROW), "Done", Some("check"), ButtonKind::Primary) {
        l.career.ignore_open = false;
        l.career.sub = super::career::SUB_DEALERSHIP;
        return;
    }

    // --- the bar: search, the view, the names; then the bulk buttons and the count
    let bar_y = panel.bottom() + 12.0;
    let q = l.career.ign.search.trim().to_lowercase();
    l.ui.text_input("ign-search", Rect::new(r.x, bar_y, 260.0, ROW), &mut l.career.ign.search, "Search manufacturer or model", Some("search"));
    let mut view = l.career.ign.view.min(2);
    if l.ui.segmented("ign-view", Rect::new(r.x + 272.0, bar_y, 270.0, ROW), &mut view, &["All", "Offered", "Hidden"]) {
        l.career.ign.view = view;
    }
    let mut names = l.career.ign.by_catalog as usize;
    let names_w = (r.right() - (r.x + 554.0)).clamp(200.0, 340.0);
    if l.ui.segmented("ign-names", Rect::new(r.x + 554.0, bar_y, names_w, ROW), &mut names, &["File names", "Catalogue names"]) {
        l.career.ign.by_catalog = names == 1;
    }
    let row2 = bar_y + ROW + 8.0;
    let ignored = company.ignored.clone();
    let (lines, content_h) = layout(&l.career.ign.tree, &l.career.ign.open, &q, l.career.ign.view.min(2), &ignored);
    let mut shown_keys: Vec<String> = Vec::new();
    for line in &lines {
        if let Kind::Maker { keys, .. } = &line.kind {
            shown_keys.extend(keys.iter().cloned());
        }
    }
    let mut bulk: Option<bool> = None;
    if l.ui.button("ign-all-on", Rect::new(r.x, row2, 170.0, ROW), "Offer all shown", Some("check"), ButtonKind::Normal) {
        bulk = Some(true);
    }
    if l.ui.button("ign-all-off", Rect::new(r.x + 180.0, row2, 170.0, ROW), "Hide all shown", Some("close"), ButtonKind::Normal) {
        bulk = Some(false);
    }
    let total: usize = l.career.ign.tree.iter().map(|m| m.items.len()).sum();
    let offered_n = l.career.ign.tree.iter().flat_map(|m| m.items.iter()).filter(|t| offered_of(&ignored, &t.keys).0 > 0).count();
    l.ui.text_in(&format!("{offered_n} of {total} models offered"), Rect::new(r.right() - 300.0, row2, 300.0, ROW), 12.5, Weight::Regular, TEXT_DIM, Align::Right);

    // --- the list
    let top = row2 + ROW + 12.0;
    let list = Rect::new(r.x, top, r.w, (r.bottom() - top).max(100.0));
    l.ui.panel(list);
    let inner = Rect::new(list.x + 10.0, list.y + 10.0, list.w - 20.0, list.h - 20.0);
    let mut ops: Vec<Op> = Vec::new();
    if lines.is_empty() {
        let text = if l.career.ign.view == 1 {
            "No model is offered with these filters."
        } else if l.career.ign.view == 2 {
            "No model is hidden with these filters."
        } else {
            "No model matches the search."
        };
        l.ui.text_in(text, Rect::new(inner.x + 6.0, inner.y + 6.0, inner.w, 22.0), 13.0, Weight::Regular, TEXT_DIM, Align::Left);
    }
    {
        let tree = &l.career.ign.tree;
        let ignored = &ignored;
        let lines = &lines;
        l.ui.scroll_area("ign-list", inner, &mut |ui, v| {
            for line in lines.iter() {
                let y = v.y + line.y;
                // (every row on its own: a header out of sight does not take its models with it)
                if !in_view(y, line.h, inner.y, inner.bottom()) {
                    continue;
                }
                match &line.kind {
                    Kind::Maker { name, mkey, keys, shown, offered, open } => {
                        let rr = Rect::new(v.x, y, v.w - 12.0, MAKER_H);
                        if ui.row(mkey, Rect::new(rr.x, rr.y, rr.w - 200.0, rr.h), false) {
                            ops.push(Op::Fold(mkey.clone()));
                        }
                        ui.icon(if *open { "expand_more" } else { "chevron_right" }, Vec2::new(rr.x + 16.0, rr.center().y), 18.0, TEXT_SOFT);
                        let none = *offered == 0;
                        ui.text_in(name, Rect::new(rr.x + 34.0, rr.y, rr.w - 400.0, rr.h), 14.5, Weight::Bold, if none { TEXT_DIM } else { TEXT }, Align::Left);
                        ui.text_in(&format!("{offered} of {shown} offered"), Rect::new(rr.right() - 380.0, rr.y, 170.0, rr.h), 12.0, Weight::Regular, if none { DANGER } else { TEXT_DIM }, Align::Right);
                        if ui.button(&format!("{mkey}-on"), Rect::new(rr.right() - 196.0, rr.y + 4.0, 92.0, 26.0), "All on", None, ButtonKind::Normal) {
                            ops.push(Op::Set(keys.clone(), true));
                        }
                        if ui.button(&format!("{mkey}-off"), Rect::new(rr.right() - 98.0, rr.y + 4.0, 92.0, 26.0), "All off", None, ButtonKind::Normal) {
                            ops.push(Op::Set(keys.clone(), false));
                        }
                    }
                    Kind::Item { m, i } => {
                        let (maker, t) = (&tree[*m], &tree[*m].items[*i]);
                        let rr = Rect::new(v.x + 34.0, y + 1.0, v.w - 12.0 - 34.0, ITEM_H - 2.0);
                        let (on, total) = offered_of(ignored, &t.keys);
                        // 2 offered, 1 partly (a catalogue row over several models of the files), 0 hidden
                        let state = if on == total { 2 } else if on == 0 { 0 } else { 1 };
                        let id = id_of(&format!("ign-t:{}|{}", maker.name.to_lowercase(), t.label.to_lowercase()));
                        let (hv, _, clicked) = ui.interact(id, rr);
                        if clicked {
                            ops.push(Op::Set(t.keys.clone(), state != 2));
                        }
                        let (tint, edge, text_c, tag) = match state {
                            2 => (OK.alpha(0.10), OK, TEXT, "OFFERED"),
                            1 => (WARN.alpha(0.10), WARN, TEXT_SOFT, "PARTLY"),
                            _ => (DANGER.alpha(0.07), DANGER, TEXT_FAINT, "HIDDEN"),
                        };
                        ui.p().rounded(rr, 6.0, tint);
                        if hv {
                            ui.p().rounded(rr, 6.0, HOVER.alpha(0.6));
                        }
                        ui.p().rounded(Rect::new(rr.x, rr.y + 5.0, 3.0, rr.h - 10.0), 1.5, edge.alpha(0.9));
                        ui.text_in(&t.label, Rect::new(rr.x + 14.0, rr.y, rr.w - 14.0 - 250.0, rr.h), 13.0, if state == 2 { Weight::Medium } else { Weight::Regular }, text_c, Align::Left);
                        ui.badge(Vec2::new(rr.right() - 226.0, rr.y + (rr.h - 16.0) * 0.5), tag, edge);
                        let files = if t.count == 1 { "1 file".to_string() } else { format!("{} files", t.count) };
                        ui.text_in(&files, Rect::new(rr.right() - 150.0, rr.y, 90.0, rr.h), 11.5, Weight::Regular, TEXT_FAINT, Align::Right);
                        // the switch
                        let a = ui.anim(id, match state { 2 => 1.0, 1 => 0.5, _ => 0.0 }, 0.07);
                        let (tw, th) = (34.0, 18.0);
                        let track = Rect::new(rr.right() - tw - 10.0, rr.y + (rr.h - th) * 0.5, tw, th);
                        ui.p().rounded(track, th * 0.5, Color::rgba(62, 62, 62, 1.0).mix(ACCENT, a));
                        let kx = track.x + th * 0.5 + (tw - th) * a;
                        ui.p().circle(Vec2::new(kx, track.center().y), th * 0.5 - 3.0, Color::rgba(240, 240, 240, 1.0));
                    }
                }
            }
            content_h
        });
    }
    if let Some(on) = bulk {
        ops.push(Op::Set(shown_keys, on));
    }
    if ops.is_empty() {
        return;
    }
    let Some(c) = l.career.company.as_mut() else { return };
    let mut changed = false;
    for op in ops {
        match op {
            Op::Fold(k) => {
                let open = &mut l.career.ign.open;
                if !open.remove(&k) {
                    open.insert(k);
                }
            }
            Op::Set(keys, offered) => {
                for k in keys {
                    c.set_ignored(&k, !offered);
                }
                changed = true;
            }
        }
    }
    if changed {
        l.career.ignore_rev += 1;
        save(l);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> Vec<Maker> {
        let it = |n: &str, k: &[&str]| Item { label: n.into(), keys: k.iter().map(|s| s.to_string()).collect(), count: 1 };
        vec![
            Maker { name: "Mercedes".into(), items: (0..40).map(|i| it(&format!("O{i}"), &[&format!("merc|o{i}")])).collect() },
            Maker { name: "MAN".into(), items: vec![it("Lion's City", &["man|lions city", "man|lion s city"]), it("SD202", &["man|sd202"])] },
        ]
    }

    #[test]
    fn models_stay_drawn_when_their_header_is_scrolled_away() {
        let t = tree();
        let open: HashSet<String> = ["ign-m:mercedes".to_string()].into_iter().collect();
        let (lines, h) = layout(&t, &open, "", 0, &[]);
        // header, 40 models, MAN's header (closed)
        assert_eq!(lines.len(), 1 + 40 + 1);
        assert_eq!(h, MAKER_H * 2.0 + ITEM_H * 40.0);
        // the view is scrolled 300 px down and 200 px tall: the header (y 0) is out of it
        let (top, bottom, off) = (0.0, 200.0, 300.0);
        let drawn: Vec<usize> = lines.iter().enumerate().filter(|(_, ln)| in_view(ln.y - off, ln.h, top, bottom)).map(|(i, _)| i).collect();
        assert!(!drawn.contains(&0), "the header is out of sight");
        assert!(drawn.len() >= 6, "the models that are in view are drawn: {drawn:?}");
        assert!(drawn.windows(2).all(|w| w[1] == w[0] + 1));
        // the first model whose bottom is below the top edge is drawn
        let first = lines.iter().position(|ln| ln.y - off + ln.h >= top - 4.0).unwrap();
        assert_eq!(drawn[0], first);
    }

    #[test]
    fn views_narrow_the_list() {
        let t = tree();
        let ignored = vec!["merc|o3".to_string(), "man|lions city".to_string()];
        let n = |view: usize, q: &str| {
            let (lines, _) = layout(&t, &HashSet::new(), q, view, &ignored);
            lines.iter().filter(|l| matches!(l.kind, Kind::Item { .. })).count()
        };
        // a narrowed list opens every manufacturer
        assert_eq!(n(0, ""), 0);
        assert_eq!(n(1, ""), 39 + 2, "offered: all but O3, and the lion's city that is only partly hidden");
        assert_eq!(n(2, ""), 1 + 1, "hidden: O3 and the partly hidden lion's city");
        assert_eq!(n(0, "sd2"), 1);
        assert_eq!(n(0, "man"), 2, "a manufacturer's name takes all its models");
        assert_eq!(n(2, "zzz"), 0);
    }

    #[test]
    fn partly_offered_rows_count_both_ways() {
        let keys: Vec<String> = vec!["a".into(), "b".into()];
        assert_eq!(offered_of(&[], &keys), (2, 2));
        assert_eq!(offered_of(&["a".to_string()], &keys), (1, 2));
        assert_eq!(offered_of(&["a".to_string(), "b".to_string()], &keys), (0, 2));
        assert!(passes(1, 1, 2) && passes(2, 1, 2));
        assert!(passes(1, 2, 2) && !passes(2, 2, 2));
        assert!(!passes(1, 0, 2) && passes(2, 0, 2));
    }
}
