//! Career Mode's dealership and fleet.
//!
//! The dealership offers, of every size and class the installed buses have, a few buses, new
//! or used, each in a random paint of its own list other than the bus's own
//! (`company::stock`, from a seed that changes every two real days or when "New offers" is
//! bought; the models the player left out are not offered). A card shows the bus's cached
//! thumbnail (`thumbs`); the panel on the right holds the live showroom picture of the
//! selected offer in the paint it comes in and the purchase, which asks twice.
//!
//! The fleet lists what the company owns, with service, a random repaint, the fleet number
//! and plate, and selling.

use super::career::{fleet_rows, km_text, save, vehicle_of, FleetRow, SUB_DEALERSHIP};
use super::company::{self, economy, money, Offer};
use super::theme::*;
use super::thumbs::thumb_draw;
use super::ui::{id_of, ButtonKind};
use super::Launcher;
use glam::Vec2;
use omsi_launcher_lib::catalog::{natural_cmp, CLASSES, SIZES};
use omsi_ui::paint::Align;
use omsi_ui::{Rect, Weight};
use std::collections::HashMap;

#[derive(Default)]
pub struct ShopView {
    stock: Vec<Offer>,
    stock_key: (u64, usize, u32, u32),
    search: String,
    size: usize,
    class: usize,
    kind: usize,
    sort: usize,
    maker: usize,
    sel: Option<u64>,
    pub(super) armed: bool,
    pub(super) fleet_sel: Option<u32>,
    pub(super) sell_armed: bool,
    /// The fleet page lists the buses of every depot (not only the active one).
    fleet_all: bool,
    /// The depot chosen to move the selected bus to (a place in the list of the other depots).
    move_to: usize,
    num: String,
    plate: String,
    buf_for: u32,
}

pub(super) fn dealership(l: &mut Launcher, r: Rect) {
    let Some(company) = l.career.company.clone() else { return };
    let seed = company.stock_seed();
    let key = (seed, l.state.vehicles.len(), l.career.cat_rev, l.career.ignore_rev);
    if l.career.shop.stock_key != key {
        let cat = l.career.catalog.clone().unwrap_or_default();
        l.career.shop.stock = company::stock(&l.state.vehicles, &cat, &company.ignored, seed, company::year_now());
        l.career.shop.stock_key = key;
        l.career.shop.sel = None;
        l.career.shop.armed = false;
    }
    if l.state.vehicles.is_empty() {
        l.ui.paragraph("No buses found yet. The content lists are still being read, or the OMSI 2 folder is not set up.", Vec2::new(r.x, r.y), r.w, 13.5, Weight::Regular, TEXT_DIM);
        return;
    }

    // --- the filters
    let sizes: Vec<String> = std::iter::once("All sizes".to_string()).chain(SIZES.iter().map(|c| c.to_string())).collect();
    let classes: Vec<String> = std::iter::once("All classes".to_string()).chain(CLASSES.iter().map(|c| c.to_string())).collect();
    let sorts: Vec<String> = ["By size and class", "Price, low to high", "Price, high to low", "Least worn", "Model name", "Newest first", "Oldest first"].iter().map(|s| s.to_string()).collect();
    let mut reroll = false;
    // the manufacturers on the lot
    let makers: Vec<String> = {
        let mut m: Vec<String> = l.career.shop.stock.iter().map(|o| o.maker.clone()).collect();
        m.sort_by_key(|x| x.to_lowercase());
        m.dedup();
        std::iter::once("All manufacturers".to_string()).chain(m).collect()
    };
    {
        let (ui, s) = (&mut l.ui, &mut l.career.shop);
        let mut x = r.x;
        ui.text_input("shop-search", Rect::new(x, r.y, 220.0, ROW), &mut s.search, "Search model or maker", Some("search"));
        x += 230.0;
        let mut z = s.size.min(sizes.len() - 1);
        if ui.select("shop-size", Rect::new(x, r.y, 130.0, ROW), &mut z, &sizes) {
            s.size = z;
        }
        x += 140.0;
        let mut c = s.class.min(classes.len() - 1);
        if ui.select("shop-class", Rect::new(x, r.y, 160.0, ROW), &mut c, &classes) {
            s.class = c;
        }
        x += 170.0;
        let mut k = s.kind.min(2);
        if ui.segmented("shop-kind", Rect::new(x, r.y, 200.0, ROW), &mut k, &["All", "New", "Used"]) {
            s.kind = k;
        }
        x += 210.0;
        let mut so = s.sort.min(sorts.len() - 1);
        if ui.select("shop-sort", Rect::new(x, r.y, 190.0, ROW), &mut so, &sorts) {
            s.sort = so;
        }
        // the second row: the buttons
        let y2 = r.y + ROW + 8.0;
        let mut mk = s.maker.min(makers.len() - 1);
        if ui.select("shop-maker", Rect::new(r.x, y2, 240.0, ROW), &mut mk, &makers) {
            s.maker = mk;
        }
        let b = Rect::new((r.right() - 210.0).max(r.x), y2, 210.0, ROW);
        let label = format!("New offers · {}", money(economy::REROLL_COST));
        if company.balance >= economy::REROLL_COST {
            if ui.button("shop-reroll", b, &label, Some("autorenew"), ButtonKind::Normal) {
                reroll = true;
            }
        } else {
            ui.button_off(b, &label, Some("autorenew"), ButtonKind::Normal);
        }
        if ui.button("shop-ignore", Rect::new((b.x - 230.0).max(r.x), y2, 220.0, ROW), "Choose offered buses", Some("tune"), ButtonKind::Normal) {
            l.career.ignore_open = true;
        }
    }
    if reroll {
        if let Some(c) = l.career.company.as_mut() {
            c.book("New dealer offers", -economy::REROLL_COST);
            c.rerolls += 1;
        }
        save(l);
        return;
    }

    // --- what is shown
    let q = l.career.shop.search.trim().to_lowercase();
    let (siz, cls, kind, sort) = (l.career.shop.size, l.career.shop.class, l.career.shop.kind, l.career.shop.sort);
    let maker = makers.get(l.career.shop.maker).filter(|_| l.career.shop.maker > 0).cloned();
    let sold_now: &[u64] = if company.sold_seed == seed { &company.sold } else { &[] };
    let mut visible: Vec<Offer> = l
        .career
        .shop
        .stock
        .iter()
        .filter(|o| {
            !sold_now.contains(&o.id)
                && (siz == 0 || SIZES.get(siz - 1).is_some_and(|z| o.size == *z))
                && (cls == 0 || CLASSES.get(cls - 1).is_some_and(|c| o.class == *c))
                && (kind == 0 || (kind == 1 && !o.used) || (kind == 2 && o.used))
                && maker.as_ref().map_or(true, |m| o.maker == *m)
                && (q.is_empty() || format!("{} {} {} {} {}", o.maker, o.model, o.variant, o.file, o.year).to_lowercase().contains(&q))
        })
        .cloned()
        .collect();
    match sort {
        // (0: the stock's own order, by size and then class)
        1 => visible.sort_by_key(|o| o.price),
        2 => visible.sort_by_key(|o| std::cmp::Reverse(o.price)),
        3 => visible.sort_by_key(|o| o.wear),
        4 => visible.sort_by(|a, b| natural_cmp(&a.model, &b.model)),
        // by the year it was built (a bus of an unknown year last either way), then the least worn
        5 => visible.sort_by_key(|o| (o.year == 0, std::cmp::Reverse(o.year), o.wear)),
        6 => visible.sort_by_key(|o| (o.year == 0, o.year, o.wear)),
        _ => {}
    }
    let sel_id = l.career.shop.sel.filter(|id| visible.iter().any(|o| o.id == *id)).or_else(|| visible.first().map(|o| o.id));
    if sel_id != l.career.shop.sel {
        l.career.shop.sel = sel_id;
        l.career.shop.armed = false;
    }
    for o in &visible {
        l.thumb_request(&o.file, &o.paint);
    }
    let mut owned: HashMap<String, usize> = HashMap::new();
    for b in &company.fleet {
        *owned.entry(omsi_launcher_lib::catalog::file_key(&b.file)).or_default() += 1;
    }

    // --- the layout
    let top = r.y + ROW * 2.0 + 20.0;
    let side_w = 380.0_f32.min(r.w * 0.42);
    let grid = Rect::new(r.x, top, (r.w - side_w - 16.0).max(200.0), (r.h - ROW * 2.0 - 20.0).max(100.0));
    let side = Rect::new(grid.right() + 16.0, top, (r.right() - grid.right() - 16.0).max(100.0), grid.h);

    // --- the cards
    let gap = 14.0;
    let cols = if grid.w >= 760.0 { 3 } else { 2 };
    let card_w = (grid.w - 10.0 - gap * (cols as f32 - 1.0)) / cols as f32;
    let th_h = (card_w - 24.0) * 9.0 / 16.0;
    let card_h = th_h + 150.0;
    let mut picked: Option<u64> = None;
    if visible.is_empty() {
        l.ui.text_in("No offers match. Change the filters, or buy new offers.", Rect::new(grid.x, grid.y + 8.0, grid.w, 24.0), 13.5, Weight::Regular, TEXT_DIM, Align::Left);
    }
    {
        let icons = &l.icons;
        let balance = company.balance;
        l.ui.scroll_area("shop-grid", grid, &mut |ui, v| {
            for (n, o) in visible.iter().enumerate() {
                let (cx, cy) = (n % cols, n / cols);
                let card = Rect::new(v.x + cx as f32 * (card_w + gap), v.y + cy as f32 * (card_h + gap), card_w, card_h);
                if card.bottom() < grid.y - 4.0 || card.y > grid.bottom() + 4.0 {
                    continue;
                }
                let (hv, _, clicked) = ui.interact(id_of(&format!("shop-card-{}", o.id)), card);
                if clicked {
                    picked = Some(o.id);
                }
                let sel = sel_id == Some(o.id);
                ui.p().rounded(card, RADIUS, if sel { SELECTED } else if hv { HOVER } else { PANEL });
                ui.p().rounded_border(card, RADIUS, 1.0, if sel { ACCENT.alpha(0.85) } else { EDGE });
                let th = Rect::new(card.x + 12.0, card.y + 12.0, card.w - 24.0, th_h);
                thumb_draw(ui, icons, th, &o.file, &o.paint, 6.0);
                let mut y = th.bottom() + 8.0;
                ui.text_in(&o.maker, Rect::new(th.x, y, th.w, 16.0), 12.0, Weight::Medium, TEXT_DIM, Align::Left);
                y += 16.0;
                ui.text_in(&o.model, Rect::new(th.x, y, th.w, 24.0), 18.0, Weight::Bold, TEXT, Align::Left);
                y += 24.0;
                ui.text_in(if o.variant.trim().is_empty() { "Standard" } else { o.variant.as_str() }, Rect::new(th.x, y, th.w, 16.0), 11.5, Weight::Regular, TEXT_DIM, Align::Left);
                y += 22.0;
                let mut bx = th.x;
                bx += ui.badge(Vec2::new(bx, y), if o.used { "USED" } else { "NEW" }, if o.used { TEXT_SOFT } else { ACCENT }) + 6.0;
                bx += ui.badge(Vec2::new(bx, y), &format!("{} · {}", o.size, o.class.to_uppercase()), TEXT_DIM) + 6.0;
                let have = owned.get(&omsi_launcher_lib::catalog::file_key(&o.file)).copied().unwrap_or(0);
                if have > 0 {
                    bx += ui.badge(Vec2::new(bx, y), &format!("OWNED x{have}"), OK) + 6.0;
                }
                if o.incomplete {
                    ui.badge(Vec2::new(bx, y), "MISSING PACK", WARN);
                }
                y += 24.0;
                ui.text_in(&money(o.price), Rect::new(th.x, y, th.w * 0.45, 24.0), 17.0, Weight::Bold, if o.price <= balance { ACCENT } else { DANGER }, Align::Left);
                let year = if o.year > 0 { format!("{} · ", o.year) } else { String::new() };
                let wear = if o.used { format!("{year}{}", km_text(o.km as f64)) } else { format!("{year}new") };
                ui.text_in(&wear, Rect::new(th.x + th.w * 0.35, y + 1.0, th.w * 0.65, 22.0), 13.5, Weight::Medium, TEXT_SOFT, Align::Right);
            }
            visible.len().div_ceil(cols) as f32 * (card_h + gap)
        });
    }
    if let Some(id) = picked {
        l.career.shop.sel = Some(id);
        l.career.shop.armed = false;
    }

    // --- the selected offer
    l.ui.panel(side);
    let inner = Rect::new(side.x + 16.0, side.y + 16.0, side.w - 32.0, side.h - 32.0);
    let Some(o) = sel_id.and_then(|id| visible.iter().find(|o| o.id == id).cloned()) else {
        l.ui.text_in("Nothing to show.", Rect::new(inner.x, inner.y, inner.w, 20.0), 13.0, Weight::Regular, TEXT_DIM, Align::Left);
        return;
    };
    let veh = vehicle_of(&l.state.vehicles, &o.file).cloned();
    let view = Rect::new(inner.x, inner.y, inner.w, inner.w * 9.0 / 16.0);
    l.preview_other = Some((o.file.clone(), o.paint.clone()));
    let over = l.ui.over_ui;
    l.preview(view);
    l.ui.over_ui = over;
    l.ui.p().rounded_border(view, RADIUS, 1.0, EDGE);
    let mut y = view.bottom() + 12.0;
    let variant = if o.variant.trim().is_empty() { String::new() } else { format!(" {}", o.variant) };
    l.ui.text_in(&o.maker, Rect::new(inner.x, y, inner.w, 18.0), 13.0, Weight::Medium, TEXT_DIM, Align::Left);
    y += 18.0;
    l.ui.text_in(&o.model, Rect::new(inner.x, y, inner.w, 28.0), 22.0, Weight::Bold, TEXT, Align::Left);
    y += 28.0;
    l.ui.text_in(if o.variant.trim().is_empty() { "Standard" } else { o.variant.as_str() }, Rect::new(inner.x, y, inner.w, 18.0), 12.5, Weight::Regular, TEXT_DIM, Align::Left);
    y += 22.0;
    l.ui.text_in(&format!("{} {} · {}", o.size, o.class, if o.used { "used" } else { "new" }), Rect::new(inner.x, y, inner.w, 18.0), 12.5, Weight::Regular, TEXT_DIM, Align::Left);
    y += 22.0;
    let year = if o.year > 0 { format!("Built {} · ", o.year) } else { String::new() };
    let cond = if o.used { format!("{year}{} · wear {} %", km_text(o.km as f64), o.wear) } else { format!("{year}new, 0 km") };
    l.ui.text_in(&cond, Rect::new(inner.x, y, inner.w, 22.0), 14.5, Weight::Medium, TEXT_SOFT, Align::Left);
    y += 26.0;
    l.ui.text_in(&format!("Paint: {}", o.paint_label), Rect::new(inner.x, y, inner.w, 18.0), 12.5, Weight::Regular, TEXT_SOFT, Align::Left);
    // the price at the bottom
    let total = o.price;
    let after = company.balance - total;
    let buy_r = Rect::new(inner.x, inner.bottom() - ROW, inner.w, ROW);
    // where it goes: the active depot, which must have a free place
    if let Some(d) = company.active() {
        let parked = company.parked(d.id);
        let full = parked as u32 >= d.slots;
        l.ui.text_in(&format!("Delivered to {} · {parked} of {} places taken", d.name, d.slots), Rect::new(inner.x, buy_r.y - 82.0, inner.w, 18.0), 12.0, Weight::Medium, if full { WARN } else { TEXT_SOFT }, Align::Left);
        if full {
            l.ui.text_in(&money(total), Rect::new(inner.x, buy_r.y - 56.0, inner.w, 30.0), 24.0, Weight::Bold, TEXT_DIM, Align::Left);
            l.ui.button_off(buy_r, "Depot is full: see Depots", Some("local_parking"), ButtonKind::Primary);
            return;
        }
    }
    l.ui.text_in(&format!("Balance after: {}", money(after)), Rect::new(inner.x, buy_r.y - 24.0, inner.w, 18.0), 12.5, Weight::Regular, if after >= 0 { TEXT_DIM } else { DANGER }, Align::Left);
    l.ui.text_in(&money(total), Rect::new(inner.x, buy_r.y - 56.0, inner.w, 30.0), 24.0, Weight::Bold, if after >= 0 { ACCENT } else { DANGER }, Align::Left);
    if after < 0 {
        l.ui.button_off(buy_r, "Not enough money", Some("payments"), ButtonKind::Primary);
        return;
    }
    let armed = l.career.shop.armed;
    if l.ui.button("shop-buy", buy_r, if armed { "Confirm purchase" } else { "Buy" }, Some("payments"), if armed { ButtonKind::Primary } else { ButtonKind::Normal }) {
        if !armed {
            l.career.shop.armed = true;
            return;
        }
        let numbers = veh.map(|v| v.numbers.clone()).unwrap_or_default();
        let res = l.career.company.as_mut().map(|c| c.buy(&o, &numbers, seed)).unwrap_or_else(|| Err("No company".to_string()));
        l.career.shop.armed = false;
        match res {
            Ok(()) => {
                save(l);
                l.career.shop.sel = None;
                l.state.set_status(format!("Bought {}{variant} for {}. It is in your fleet", o.model, money(total)), false);
            }
            Err(e) => l.state.set_status(e, true),
        }
    }
}

// --- the fleet ------------------------------------------------------------------------------

pub(super) fn fleet(l: &mut Launcher, r: Rect) {
    let everything = fleet_rows(l);
    if everything.is_empty() {
        let panel = Rect::new(r.x, r.y, r.w.min(640.0), 150.0);
        l.ui.panel(panel);
        l.ui.paragraph("Your fleet is empty. Buses you buy in the dealership are listed here, with their condition, paint and fleet number.", Vec2::new(panel.x + 20.0, panel.y + 20.0), panel.w - 40.0, 13.5, Weight::Regular, TEXT_SOFT);
        if l.ui.button("fleet-to-shop", Rect::new(panel.x + 20.0, panel.y + 90.0, 220.0, ROW), "Open the dealership", Some("inventory_2"), ButtonKind::Primary) {
            l.career.sub = SUB_DEALERSHIP;
        }
        return;
    }
    let Some(balance) = l.career.company.as_ref().map(|c| c.balance) else { return };
    let (active_depot, active_name, many) = l.career.company.as_ref().map(|c| (c.active_depot, c.active().map(|d| d.name.clone()).unwrap_or_default(), c.depots.len() > 1)).unwrap_or_default();
    // a bus picked on the depots page may stand in another depot: then every depot is listed
    if let Some(b) = l.career.shop.fleet_sel.and_then(|id| everything.iter().find(|b| b.id == id)) {
        if many && b.depot != active_depot {
            l.career.shop.fleet_all = true;
        }
    }
    let all_depots = !many || l.career.shop.fleet_all;
    let rows: Vec<FleetRow> = everything.iter().filter(|b| all_depots || b.depot == active_depot).cloned().collect();
    let sel_id = l.career.shop.fleet_sel.filter(|id| rows.iter().any(|b| b.id == *id)).or_else(|| rows.first().map(|b| b.id));
    let Some(sel_id) = sel_id else {
        // (the depot the player works from has no bus, and the list is narrowed to it)
        let panel = Rect::new(r.x, r.y, r.w.min(640.0), 150.0);
        l.ui.panel(panel);
        l.ui.paragraph(&format!("No bus stands in {active_name}."), Vec2::new(panel.x + 20.0, panel.y + 20.0), panel.w - 40.0, 13.5, Weight::Regular, TEXT_SOFT);
        let mut z = 0usize;
        if l.ui.segmented("fleet-filter", Rect::new(panel.x + 20.0, panel.y + 70.0, 260.0, 32.0), &mut z, &["This depot", "All depots"]) {
            l.career.shop.fleet_all = z == 1;
        }
        return;
    };
    if l.career.shop.fleet_sel != Some(sel_id) {
        l.career.shop.fleet_sel = Some(sel_id);
        l.career.shop.sell_armed = false;
    }
    for b in &rows {
        l.thumb_request(&b.file, &b.paint);
    }
    let side_w = 380.0_f32.min(r.w * 0.42);
    let list = Rect::new(r.x, r.y, (r.w - side_w - 16.0).max(200.0), r.h);
    let side = Rect::new(list.right() + 16.0, r.y, (r.right() - list.right() - 16.0).max(100.0), r.h);

    // --- the list
    l.ui.panel(list);
    let mut top_off = 0.0;
    if many {
        let mut z = l.career.shop.fleet_all as usize;
        if l.ui.segmented("fleet-filter", Rect::new(list.x + 12.0, list.y + 12.0, 260.0, 32.0), &mut z, &["This depot", "All depots"]) {
            l.career.shop.fleet_all = z == 1;
            if z == 0 {
                // (the bus chosen may not stand in this depot)
                l.career.shop.fleet_sel = None;
            }
        }
        let label = if l.career.shop.fleet_all { format!("{} buses in {} depots", rows.len(), l.career.company.as_ref().map(|c| c.depots.len()).unwrap_or(0)) } else { format!("{active_name} · {} buses", rows.len()) };
        l.ui.text_in(&label, Rect::new(list.x + 284.0, list.y + 12.0, (list.w - 296.0).max(10.0), 32.0), 12.5, Weight::Regular, TEXT_DIM, Align::Right);
        top_off = 44.0;
    }
    let inner = Rect::new(list.x + 12.0, list.y + 12.0 + top_off, list.w - 24.0, list.h - 24.0 - top_off);
    let row_h = 92.0;
    let now = company::now();
    let show_depot = all_depots && many;
    let mut picked: Option<u32> = None;
    {
        let icons = &l.icons;
        l.ui.scroll_area("fleet-list", inner, &mut |ui, v| {
            for (i, b) in rows.iter().enumerate() {
                let rr = Rect::new(v.x, v.y + i as f32 * (row_h + 6.0), v.w - 12.0, row_h);
                if ui.row(&format!("fleet-row-{}", b.id), rr, sel_id == b.id) {
                    picked = Some(b.id);
                }
                let th = Rect::new(rr.x + 10.0, rr.y + 12.0, 112.0, 63.0);
                thumb_draw(ui, icons, th, &b.file, &b.paint, 5.0);
                let x = th.right() + 14.0;
                let w = (rr.w - 136.0 - 210.0).max(40.0);
                ui.text_in(&format!("#{} · {}", b.fleet_no, b.maker), Rect::new(x, rr.y + 8.0, w, 16.0), 11.5, Weight::Medium, TEXT_DIM, Align::Left);
                ui.text_in(&b.model, Rect::new(x, rr.y + 23.0, w, 22.0), 16.0, Weight::Bold, TEXT, Align::Left);
                ui.text_in(&if show_depot { format!("{} · {}", b.desc, b.depot_name) } else { b.desc.clone() }, Rect::new(x, rr.y + 46.0, w, 16.0), 11.5, Weight::Regular, TEXT_DIM, Align::Left);
                let ky = if b.year > 0 { format!("{} · {}", b.year, km_text(b.km)) } else { km_text(b.km) };
                ui.text_in(&ky, Rect::new(x, rr.y + 64.0, w, 20.0), 13.0, Weight::Medium, TEXT_SOFT, Align::Left);
                let bar = Rect::new(rr.right() - 200.0, rr.y + 18.0, 130.0, 5.0);
                ui.progress(bar, b.condition / 100.0, false);
                ui.text_in(&format!("{:.0} %", b.condition), Rect::new(rr.right() - 60.0, rr.y + 10.0, 50.0, 20.0), 12.5, Weight::Medium, if b.condition < 40.0 { WARN } else { TEXT_SOFT }, Align::Right);
                ui.text_in(&money(b.value), Rect::new(rr.right() - 200.0, rr.y + 40.0, 190.0, 18.0), 12.5, Weight::Regular, TEXT_DIM, Align::Right);
                if b.away_until > now {
                    ui.text_in("on a job with a driver", Rect::new(rr.right() - 200.0, rr.y + 56.0, 190.0, 16.0), 10.5, Weight::Medium, ACCENT, Align::Right);
                } else {
                    ui.text_in("resale value", Rect::new(rr.right() - 200.0, rr.y + 56.0, 190.0, 16.0), 10.5, Weight::Regular, TEXT_FAINT, Align::Right);
                }
            }
            rows.len() as f32 * (row_h + 6.0)
        });
    }
    if let Some(id) = picked {
        l.career.shop.fleet_sel = Some(id);
        l.career.shop.sell_armed = false;
    }

    // --- the selected bus
    l.ui.panel(side);
    let inner = Rect::new(side.x + 16.0, side.y + 16.0, side.w - 32.0, side.h - 32.0);
    let Some(row) = rows.iter().find(|b| b.id == sel_id) else { return };
    show_bus(l, row, inner, balance);
}

fn show_bus(l: &mut Launcher, row: &FleetRow, inner: Rect, balance: i64) {
    let view = Rect::new(inner.x, inner.y, inner.w, inner.w * 9.0 / 16.0);
    l.preview_other = Some((row.file.clone(), row.paint.clone()));
    // (with the stickers the player put on this bus, or on its model)
    l.preview_owner = row.id;
    let over = l.ui.over_ui;
    l.preview(view);
    l.ui.over_ui = over;
    l.ui.p().rounded_border(view, RADIUS, 1.0, EDGE);
    let mut y = view.bottom() + 12.0;
    l.ui.text_in(&format!("#{} · {}", row.fleet_no, row.maker), Rect::new(inner.x, y, inner.w, 18.0), 13.0, Weight::Medium, TEXT_DIM, Align::Left);
    y += 18.0;
    l.ui.text_in(&row.model, Rect::new(inner.x, y, inner.w, 28.0), 22.0, Weight::Bold, TEXT, Align::Left);
    y += 28.0;
    l.ui.text_in(&row.desc, Rect::new(inner.x, y, inner.w, 18.0), 12.5, Weight::Regular, TEXT_DIM, Align::Left);
    y += 22.0;
    let ky = if row.year > 0 { format!("Built {} · {}", row.year, km_text(row.km)) } else { km_text(row.km) };
    l.ui.text_in(&ky, Rect::new(inner.x, y, inner.w, 22.0), 14.5, Weight::Medium, TEXT_SOFT, Align::Left);
    y += 22.0;
    if !row.depot_name.is_empty() {
        l.ui.text_in(&format!("Stands in {}{}", row.depot_name, if row.away_until > company::now() { " · on a job with a driver" } else { "" }), Rect::new(inner.x, y, inner.w, 18.0), 12.0, Weight::Regular, TEXT_DIM, Align::Left);
    }
    y += 22.0;

    // the fleet number and plate: loaded into the fields once per bus, written on every edit
    if l.career.shop.buf_for != row.id {
        l.career.shop.buf_for = row.id;
        if let Some(b) = l.career.company.as_ref().and_then(|c| c.fleet.iter().find(|b| b.id == row.id)) {
            l.career.shop.num = b.fleet_no.clone();
            l.career.shop.plate = b.plate.clone();
        }
    }
    let half = (inner.w - 10.0) / 2.0;
    l.ui.text_in("FLEET NUMBER", Rect::new(inner.x, y, half, 16.0), 11.0, Weight::Bold, TEXT_DIM, Align::Left);
    l.ui.text_in("PLATE", Rect::new(inner.x + half + 10.0, y, half, 16.0), 11.0, Weight::Bold, TEXT_DIM, Align::Left);
    y += 18.0;
    let (ui, s) = (&mut l.ui, &mut l.career.shop);
    let a = ui.text_input("fleet-num", Rect::new(inner.x, y, half, ROW), &mut s.num, "101", Some("badge"));
    let b = ui.text_input("fleet-plate", Rect::new(inner.x + half + 10.0, y, half, ROW), &mut s.plate, "Automatic", None);
    if a || b {
        let (num, plate) = (l.career.shop.num.trim().to_string(), l.career.shop.plate.trim().to_string());
        if let Some(bus) = l.career.company.as_mut().and_then(|c| c.fleet.iter_mut().find(|x| x.id == row.id)) {
            bus.fleet_no = num;
            bus.plate = plate;
        }
        save(l);
    }
    y += ROW + 16.0;

    // the bus's numbers: the fuel, the consumption, the inspection
    let ob = l.career.company.as_ref().and_then(|c| c.fleet.iter().find(|b| b.id == row.id)).cloned();
    if let Some(ob) = ob.as_ref() {
        let fuel = match (ob.fuel, ob.tank > 0.0) {
            (Some(f), true) => format!("Fuel {:.0} / {:.0} l ({:.0} %)", f, ob.tank, f / ob.tank * 100.0),
            (Some(f), false) => format!("Fuel {f:.0} l"),
            (None, true) => format!("Fuel full ({:.0} l)", ob.tank),
            (None, false) => "Fuel full (the tank's size is learned on the first drive)".to_string(),
        };
        let low = matches!((ob.fuel, ob.tank > 0.0), (Some(f), true) if f < ob.tank * 0.15);
        l.ui.text_in(&fuel, Rect::new(inner.x, y, inner.w, 18.0), 12.5, Weight::Regular, if low { WARN } else { TEXT_SOFT }, Align::Left);
        y += 19.0;
        let use_text = match ob.consumption() {
            Some(c) => format!("Consumption {c:.1} l/100 km · burned {} l", km_text(ob.fuel_burned).trim_end_matches(" km")),
            None => "Consumption: not measured yet".to_string(),
        };
        l.ui.text_in(&use_text, Rect::new(inner.x, y, inner.w, 18.0), 12.5, Weight::Regular, TEXT_SOFT, Align::Left);
        y += 19.0;
        let (insp, color) = if ob.inspection_failed {
            ("Failed the inspection: repair it".to_string(), DANGER)
        } else if ob.days_since_inspection >= economy::INSPECTION_DAYS {
            ("Inspection due".to_string(), DANGER)
        } else {
            (format!("Inspection in {} days of driving", ob.inspection_in()), if ob.inspection_in() <= 5 { WARN } else { TEXT_SOFT })
        };
        l.ui.text_in(&insp, Rect::new(inner.x, y, inner.w, 18.0), 12.5, Weight::Regular, color, Align::Left);
        y += 28.0;
    }

    // the depot: refuel, inspection, repairs, the livery
    let half = (inner.w - 8.0) / 2.0;
    let (cost_fuel, cost_insp, cost_repair, cost_mult) = {
        let c = l.career.company.as_ref();
        let k = c.map(|c| economy::COST_MULT[(c.difficulty as usize).min(2)]).unwrap_or(1.0);
        let missing = ob.as_ref().map(|b| if b.tank > 0.0 { (b.tank - b.fuel.unwrap_or(b.tank)).max(0.0) as f64 } else { 0.0 }).unwrap_or(0.0);
        ((missing * economy::FUEL_PRICE * k).round() as i64, (economy::INSPECTION_COST as f64 * k).round() as i64, c.map(|c| c.repair_cost(row.id)).unwrap_or(0), k)
    };
    let _ = cost_mult;
    let mut act = 0;
    let refuel_r = Rect::new(inner.x, y, half, ROW);
    let insp_r = Rect::new(inner.x + half + 8.0, y, half, ROW);
    let repair_r = Rect::new(inner.x, y + ROW + 8.0, half, ROW);
    let livery_r = Rect::new(inner.x + half + 8.0, y + ROW + 8.0, half, ROW);
    let blocked_failed = ob.as_ref().is_some_and(|b| b.inspection_failed);
    let l_fuel = format!("Refuel · {}", money(cost_fuel));
    if cost_fuel > 0 && balance >= cost_fuel {
        if l.ui.button("fleet-refuel", refuel_r, &l_fuel, Some("water_drop"), ButtonKind::Normal) {
            act = 1;
        }
    } else {
        l.ui.button_off(refuel_r, if cost_fuel > 0 { l_fuel.as_str() } else { "Tank full" }, Some("water_drop"), ButtonKind::Normal);
    }
    let l_insp = format!("Inspection · {}", money(cost_insp));
    if !blocked_failed && balance >= cost_insp {
        if l.ui.button("fleet-inspect", insp_r, &l_insp, Some("check_circle"), ButtonKind::Normal) {
            act = 2;
        }
    } else {
        l.ui.button_off(insp_r, &l_insp, Some("check_circle"), ButtonKind::Normal);
    }
    let l_rep = format!("Repair · {}", money(cost_repair));
    if row.condition < 99.0 && balance >= cost_repair {
        if l.ui.button("fleet-repair", repair_r, &l_rep, Some("construction"), ButtonKind::Normal) {
            act = 3;
        }
    } else {
        l.ui.button_off(repair_r, &l_rep, Some("construction"), ButtonKind::Normal);
    }
    if l.ui.button("fleet-livery", livery_r, "Customize", Some("palette"), ButtonKind::Normal) {
        l.career.livery = Some(super::career_livery::LiveryView::open(row));
    }
    let sell_r = Rect::new(inner.x, inner.bottom() - ROW, inner.w, ROW);
    // moving the bus to another depot (when there is room for the row)
    let mut move_op: Option<u32> = None;
    let move_y = y + 2.0 * (ROW + 8.0);
    let others: Vec<(u32, String, u32, i64)> = l
        .career
        .company
        .as_ref()
        .map(|c| c.depots.iter().filter(|d| d.id != row.depot).map(|d| (d.id, d.name.clone(), c.free_places(d.id), c.transfer_cost(row.id, d.id))).collect())
        .unwrap_or_default();
    if !others.is_empty() && move_y + ROW <= sell_r.y - 6.0 {
        let names: Vec<String> = others.iter().map(|(_, n, f, _)| format!("{n} ({f} free)")).collect();
        let mut mi = l.career.shop.move_to.min(others.len() - 1);
        if l.ui.select("fleet-move", Rect::new(inner.x, move_y, inner.w - 158.0, ROW), &mut mi, &names) {
            l.career.shop.move_to = mi;
        }
        let (tid, _, free, cost) = others[mi].clone();
        let mb = Rect::new(inner.right() - 150.0, move_y, 150.0, ROW);
        let ml = format!("Move · {}", money(cost));
        if free == 0 || row.away_until > company::now() || cost > balance {
            l.ui.button_off(mb, &ml, Some("sync_alt"), ButtonKind::Normal);
        } else if l.ui.button("fleet-move-go", mb, &ml, Some("sync_alt"), ButtonKind::Normal) {
            move_op = Some(tid);
        }
    }
    let armed = l.career.shop.sell_armed;
    let sell = l.ui.button("fleet-sell", sell_r, &if armed { "Press again to sell".to_string() } else { format!("Sell · {}", money(row.value)) }, Some("payments"), ButtonKind::Danger);

    if let Some(to) = move_op {
        let r = l.career.company.as_mut().map(|c| c.move_bus(row.id, to));
        report(l, r.map(|x| x.map(|cost| format!("#{} moved for {}", row.fleet_no, money(cost)))));
    }
    match act {
        1 => {
            let r = l.career.company.as_mut().map(|c| c.refuel_bus(row.id));
            report(l, r.map(|x| x.map(|c| format!("Refuelled for {}", money(c)))));
        }
        2 => {
            let roll = (company::mix(company::now() ^ ((row.id as u64) << 32) ^ ob.as_ref().map(|b| b.km as u64).unwrap_or(0)) >> 11) as f64 / (1u64 << 53) as f64;
            let r = l.career.company.as_mut().map(|c| c.inspect_bus(row.id, roll));
            report(l, r.map(|x| x.map(|ok| if ok { "The bus passed the inspection".to_string() } else { "The bus FAILED the inspection: repair it before it goes out".to_string() })));
        }
        3 => {
            let r = l.career.company.as_mut().map(|c| c.repair_bus(row.id));
            report(l, r.map(|x| x.map(|c| format!("Repaired for {}", money(c)))));
        }
        _ => {}
    }
    if sell {
        if armed {
            let res = l.career.company.as_mut().map(|c| c.sell(row.id)).unwrap_or_else(|| Err("No company".to_string()));
            l.career.shop.sell_armed = false;
            l.career.shop.fleet_sel = None;
            match res {
                Ok(v) => {
                    save(l);
                    l.state.set_status(format!("{} sold for {}", row.title, money(v)), false);
                }
                Err(e) => l.state.set_status(e, true),
            }
        } else {
            l.career.shop.sell_armed = true;
        }
    }
}

/// Save after a depot action and say how it went on the status line.
fn report(l: &mut Launcher, r: Option<Result<String, String>>) {
    match r {
        Some(Ok(text)) => {
            save(l);
            l.state.set_status(text, false);
        }
        Some(Err(e)) => l.state.set_status(e, true),
        None => {}
    }
}
