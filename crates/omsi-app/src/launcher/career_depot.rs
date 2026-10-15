//! Career Mode, Depots: where the company's buses stand. The company starts with one depot with
//! three parking places; more places cost more each, and a new depot (on any installed map) costs
//! money too, more with every one the company has. The depot the player works from - the
//! active one - is where the dealer delivers and which map the jobs run on; changing it is free.
//! The page lists the depots, shows the chosen one's parking places as a row of cards (the
//! bus's picture in each place that is taken) and sells an empty depot back for half the price.

use super::career::{fleet_rows, save, FleetRow, SUB_FLEET};
use super::company::{self, economy, money};
use super::theme::*;
use super::thumbs::thumb_draw;
use super::ui::ButtonKind;
use super::Launcher;
use glam::Vec2;
use omsi_ui::paint::Align;
use omsi_ui::{Rect, Weight};

#[derive(Default)]
pub struct DepotView {
    /// The depot shown on the right (0: the active one).
    sel: u32,
    new_name: String,
    new_map: usize,
    /// A button pressed once that wants a second press: (what, which depot).
    armed: Option<(u8, u32)>,
    /// The name field holds the name of this depot.
    name_for: u32,
    name: String,
}

const ARM_OPEN: u8 = 1;
const ARM_SELL: u8 = 2;

/// The map as the lists name it.
pub(super) fn map_name(l: &Launcher, file: &str) -> String {
    l.state.maps.iter().find(|m| m.file == file).map(|m| if m.friendly.is_empty() { m.name.clone() } else { m.friendly.clone() }).unwrap_or_else(|| file.to_string())
}

enum Op {
    Select(u32),
    Activate(u32),
    BuyPlace(u32),
    OpenDepot(String, String),
    Sell(u32),
    OpenBus(u32),
}

pub(super) fn draw(l: &mut Launcher, r: Rect) {
    let Some(c) = l.career.company.clone() else { return };
    let sel = if c.depot(l.career.depot.sel).is_some() { l.career.depot.sel } else { c.active_depot };
    if sel != l.career.depot.sel {
        l.career.depot.sel = sel;
        l.career.depot.armed = None;
    }
    let mut op: Option<Op> = None;
    let gap = GAP + 4.0;
    let left_w = (r.w * 0.36).clamp(300.0, 440.0);
    let new_h = 300.0_f32.min((r.h * 0.5).max(240.0));
    let list = Rect::new(r.x, r.y, left_w, (r.h - new_h - gap).max(120.0));
    let newp = Rect::new(r.x, list.bottom() + gap, left_w, (r.bottom() - list.bottom() - gap).max(0.0));
    let right = Rect::new(r.x + left_w + gap, r.y, (r.w - left_w - gap).max(200.0), r.h);

    // --- the depots
    l.ui.panel(list);
    let inner = l.ui.heading(Rect::new(list.x + 16.0, list.y + 14.0, list.w - 32.0, list.h - 28.0), "Your depots", Some("garage"));
    let rows: Vec<(u32, String, String, usize, u32)> = c.depots.iter().map(|d| (d.id, d.name.clone(), map_name(l, &d.map), c.parked(d.id), d.slots)).collect();
    let row_h = 58.0;
    l.ui.scroll_area("depot-list", inner, &mut |ui, v| {
        for (i, (id, name, map, parked, slots)) in rows.iter().enumerate() {
            let rr = Rect::new(v.x, v.y + i as f32 * (row_h + 4.0), v.w - 10.0, row_h);
            if rr.bottom() < inner.y - 4.0 || rr.y > inner.bottom() + 4.0 {
                continue;
            }
            if ui.row(&format!("depot-row-{id}"), rr, *id == sel) {
                op = Some(Op::Select(*id));
            }
            ui.icon("garage", Vec2::new(rr.x + 24.0, rr.center().y), 22.0, if *id == c.active_depot { ACCENT } else { TEXT_DIM });
            ui.text_in(name, Rect::new(rr.x + 50.0, rr.y + 8.0, rr.w - 190.0, 22.0), 14.5, Weight::Bold, TEXT, Align::Left);
            ui.text_in(map, Rect::new(rr.x + 50.0, rr.y + 31.0, rr.w - 190.0, 18.0), 11.5, Weight::Regular, TEXT_DIM, Align::Left);
            let full = *parked as u32 >= *slots;
            ui.text_in(&format!("{parked} / {slots}"), Rect::new(rr.right() - 130.0, rr.y + 8.0, 120.0, 22.0), 14.0, Weight::Bold, if full { WARN } else { TEXT_SOFT }, Align::Right);
            if *id == c.active_depot {
                ui.badge(Vec2::new(rr.right() - 74.0, rr.y + 33.0), "ACTIVE", ACCENT);
            } else {
                ui.text_in("places", Rect::new(rr.right() - 130.0, rr.y + 30.0, 120.0, 18.0), 11.0, Weight::Regular, TEXT_FAINT, Align::Right);
            }
        }
        rows.len() as f32 * (row_h + 4.0)
    });

    // --- a new depot
    l.ui.panel(newp);
    let inner = l.ui.heading(Rect::new(newp.x + 16.0, newp.y + 14.0, newp.w - 32.0, newp.h - 28.0), "Open a new depot", Some("add"));
    let maps: Vec<String> = l.state.maps.iter().map(|m| if m.friendly.is_empty() { m.name.clone() } else { m.friendly.clone() }).collect();
    let files: Vec<String> = l.state.maps.iter().map(|m| m.file.clone()).collect();
    let price = c.next_depot_price();
    let mut y = inner.y;
    l.ui.text_in("Name", Rect::new(inner.x, y, inner.w, 16.0), 11.0, Weight::Bold, TEXT_DIM, Align::Left);
    y += 18.0;
    {
        let (ui, v) = (&mut l.ui, &mut l.career.depot);
        ui.text_input("depot-new-name", Rect::new(inner.x, y, inner.w, ROW), &mut v.new_name, "e.g. North yard", Some("garage"));
    }
    y += ROW + 8.0;
    l.ui.text_in("Map", Rect::new(inner.x, y, inner.w, 16.0), 11.0, Weight::Bold, TEXT_DIM, Align::Left);
    y += 18.0;
    if maps.is_empty() {
        l.ui.text_in("No maps found.", Rect::new(inner.x, y, inner.w, ROW), 12.5, Weight::Regular, WARN, Align::Left);
    } else {
        let mut m = l.career.depot.new_map.min(maps.len() - 1);
        if l.ui.select("depot-new-map", Rect::new(inner.x, y, inner.w, ROW), &mut m, &maps) {
            l.career.depot.new_map = m;
        }
    }
    y += ROW + 10.0;
    let rent = (economy::DEPOT_RENT as f64 * economy::COST_MULT[(c.difficulty as usize).min(2)]).round() as i64;
    let btn = Rect::new(inner.x, inner.bottom() - ROW, inner.w, ROW);
    match price {
        Ok(p) => {
            l.ui.text_in(&format!("Price {} · rent {} per session you book · {} places to begin with", money(p), money(rent), economy::DEPOT_START_SLOTS), Rect::new(inner.x, y, inner.w, 18.0), 12.0, Weight::Regular, TEXT_DIM, Align::Left);
            let armed = l.career.depot.armed == Some((ARM_OPEN, 0));
            let label = if armed { format!("Confirm · {}", money(p)) } else { format!("Open depot · {}", money(p)) };
            if maps.is_empty() {
                l.ui.button_off(btn, &label, Some("add"), ButtonKind::Normal);
            } else if p > c.balance {
                l.ui.button_off(btn, &format!("Not enough money · {}", money(p)), Some("payments"), ButtonKind::Normal);
            } else if l.ui.button("depot-open", btn, &label, Some("add"), if armed { ButtonKind::Primary } else { ButtonKind::Normal }) {
                if armed {
                    let file = files.get(l.career.depot.new_map.min(files.len().saturating_sub(1))).cloned().unwrap_or_default();
                    op = Some(Op::OpenDepot(l.career.depot.new_name.clone(), file));
                } else {
                    l.career.depot.armed = Some((ARM_OPEN, 0));
                }
            }
        }
        Err(e) => {
            l.ui.text_in(&e, Rect::new(inner.x, y, inner.w, 18.0), 12.0, Weight::Regular, WARN, Align::Left);
            l.ui.button_off(btn, "No more depots", Some("add"), ButtonKind::Normal);
        }
    }

    // --- the chosen depot
    l.ui.panel(right);
    let inner = Rect::new(right.x + 18.0, right.y + 16.0, right.w - 36.0, right.h - 32.0);
    let Some(d) = c.depot(sel).cloned() else { return };
    if l.career.depot.name_for != d.id {
        l.career.depot.name_for = d.id;
        l.career.depot.name = d.name.clone();
    }
    let edited = {
        let (ui, v) = (&mut l.ui, &mut l.career.depot);
        ui.text_input("depot-name", Rect::new(inner.x, inner.y, inner.w.min(340.0), ROW), &mut v.name, "Name of the depot", Some("garage"))
    };
    if edited {
        let name = l.career.depot.name.clone();
        if let Some(co) = l.career.company.as_mut() {
            co.rename_depot(d.id, &name);
        }
        save(l);
    }
    let active = d.id == c.active_depot;
    let first = c.depots.first().map(|x| x.id) == Some(d.id);
    let map_text = map_name(l, &d.map);
    let mut y = inner.y + ROW + 8.0;
    let rent_text = if first { "no rent (the home depot)".to_string() } else { format!("rent {} per session you book", money(rent)) };
    l.ui.text_in(&format!("Map: {map_text} · {rent_text}"), Rect::new(inner.x, y, inner.w, 20.0), 13.0, Weight::Regular, TEXT_SOFT, Align::Left);
    y += 28.0;
    // make active / sell
    let bw = ((inner.w - 12.0) / 2.0).min(280.0);
    let act_r = Rect::new(inner.x, y, bw, ROW);
    if active {
        l.ui.button_off(act_r, "Active depot", Some("check_circle"), ButtonKind::Primary);
    } else if l.ui.button("depot-activate", act_r, "Work from this depot", Some("check_circle"), ButtonKind::Primary) {
        op = Some(Op::Activate(d.id));
    }
    let sell_r = Rect::new(inner.x + bw + 12.0, y, bw, ROW);
    let refund = (d.paid as f64 * economy::DEPOT_RESALE).round() as i64;
    let sell_armed = l.career.depot.armed == Some((ARM_SELL, d.id));
    if c.depots.len() <= 1 {
        l.ui.button_off(sell_r, "The last depot stays", Some("delete"), ButtonKind::Danger);
    } else if c.parked(d.id) > 0 {
        l.ui.button_off(sell_r, "Empty it to sell it", Some("delete"), ButtonKind::Danger);
    } else if l.ui.button("depot-sell", sell_r, &if sell_armed { "Press again to sell".to_string() } else { format!("Sell depot · {}", money(refund)) }, Some("delete"), ButtonKind::Danger) {
        if sell_armed {
            op = Some(Op::Sell(d.id));
        } else {
            l.career.depot.armed = Some((ARM_SELL, d.id));
        }
    }
    y += ROW + 18.0;

    // the parking places
    let parked = c.parked(d.id);
    l.ui.text_in(&format!("PARKING · {parked} of {} places taken", d.slots), Rect::new(inner.x, y, inner.w - 260.0, ROW), 11.5, Weight::Bold, TEXT_DIM, Align::Left);
    let place_r = Rect::new(inner.right() - 250.0, y, 250.0, ROW);
    if d.slots >= economy::DEPOT_MAX_SLOTS {
        l.ui.button_off(place_r, "Largest size", Some("local_parking"), ButtonKind::Normal);
    } else {
        let p = economy::slot_price(d.slots);
        let label = format!("Buy a place · {}", money(p));
        if p > c.balance {
            l.ui.button_off(place_r, &label, Some("local_parking"), ButtonKind::Normal);
        } else if l.ui.button("depot-place", place_r, &label, Some("local_parking"), ButtonKind::Normal) {
            op = Some(Op::BuyPlace(d.id));
        }
    }
    y += ROW + 10.0;
    let grid = Rect::new(inner.x, y, inner.w, (inner.bottom() - y).max(60.0));
    let buses: Vec<FleetRow> = fleet_rows(l).into_iter().filter(|b| b.depot == d.id).collect();
    for b in &buses {
        l.thumb_request(&b.file, &b.paint);
    }
    let cells = (d.slots as usize).max(buses.len());
    let gap_c = 12.0;
    let cols = (((grid.w - 10.0) / (176.0 + gap_c)).floor() as usize).max(1);
    let cw = (grid.w - 10.0 - gap_c * (cols as f32 - 1.0)) / cols as f32;
    let th_h = (cw - 16.0) * 9.0 / 16.0;
    let ch = th_h + 64.0;
    let now = company::now();
    {
        let icons = &l.icons;
        l.ui.scroll_area("depot-grid", grid, &mut |ui, v| {
            for n in 0..cells {
                let card = Rect::new(v.x + (n % cols) as f32 * (cw + gap_c), v.y + (n / cols) as f32 * (ch + gap_c), cw, ch);
                if card.bottom() < grid.y - 4.0 || card.y > grid.bottom() + 4.0 {
                    continue;
                }
                match buses.get(n) {
                    Some(b) => {
                        let (hv, _, clicked) = ui.interact(super::ui::id_of(&format!("depot-bus-{}", b.id)), card);
                        if clicked {
                            op = Some(Op::OpenBus(b.id));
                        }
                        ui.p().rounded(card, RADIUS, if hv { HOVER } else { PANEL });
                        ui.p().rounded_border(card, RADIUS, 1.0, EDGE);
                        let th = Rect::new(card.x + 8.0, card.y + 8.0, card.w - 16.0, th_h);
                        thumb_draw(ui, icons, th, &b.file, &b.paint, 5.0);
                        if b.away_until > now {
                            ui.p().rounded(th, 5.0, omsi_ui::Color::rgba(0, 0, 0, 0.55));
                            ui.text_in("ON A JOB", th, 13.0, Weight::Bold, ACCENT, Align::Center);
                        }
                        let y = th.bottom() + 6.0;
                        ui.text_in(&format!("#{} {}", b.fleet_no, b.model), Rect::new(th.x, y, th.w, 20.0), 13.0, Weight::Bold, TEXT, Align::Left);
                        ui.progress(Rect::new(th.x, y + 28.0, th.w - 44.0, 4.0), b.condition / 100.0, false);
                        ui.text_in(&format!("{:.0} %", b.condition), Rect::new(th.right() - 40.0, y + 20.0, 40.0, 18.0), 11.5, Weight::Medium, if b.condition < 40.0 { WARN } else { TEXT_SOFT }, Align::Right);
                    }
                    None => {
                        ui.p().rounded(card, RADIUS, FIELD.alpha(0.45));
                        ui.p().rounded_border(card, RADIUS, 1.0, EDGE.alpha(0.6));
                        ui.icon("local_parking", Vec2::new(card.center().x, card.y + th_h * 0.5 + 8.0), 26.0, TEXT_FAINT);
                        ui.text_in("Free place", Rect::new(card.x, card.y + th_h + 14.0, card.w, 20.0), 12.5, Weight::Regular, TEXT_FAINT, Align::Center);
                    }
                }
            }
            (cells.div_ceil(cols)) as f32 * (ch + gap_c)
        });
    }

    // --- what was pressed
    let Some(op) = op else {
        // (a press that is not the armed button's own disarms it)
        if l.ui.input.pressed && l.career.depot.armed.is_some() && !l.ui.over_ui {
            l.career.depot.armed = None;
        }
        return;
    };
    l.career.depot.armed = None;
    match op {
        Op::Select(id) => l.career.depot.sel = id,
        Op::Activate(id) => {
            if let Some(Ok(())) = l.career.company.as_mut().map(|c| c.set_active_depot(id)) {
                save(l);
                let name = l.career.company.as_ref().and_then(|c| c.depot(id)).map(|d| d.name.clone()).unwrap_or_default();
                l.state.set_status(format!("You work from {name} now: the dealer delivers there and the jobs start on its map"), false);
            }
        }
        Op::BuyPlace(id) => {
            let res = l.career.company.as_mut().map(|c| c.buy_slot(id));
            finish(l, res.map(|r| r.map(|p| format!("One more parking place for {}", money(p)))));
        }
        Op::OpenDepot(name, map) => {
            let res = l.career.company.as_mut().map(|c| c.buy_depot(&name, &map));
            if let Some(Ok(id)) = res {
                l.career.depot.sel = id;
                l.career.depot.new_name.clear();
                save(l);
                l.state.set_status("Depot opened. Press \"Work from this depot\" to deliver there and drive from its map", false);
            } else if let Some(Err(e)) = res {
                l.state.set_status(e, true);
            }
        }
        Op::Sell(id) => {
            let res = l.career.company.as_mut().map(|c| c.sell_depot(id));
            if matches!(res, Some(Ok(_))) {
                l.career.depot.sel = 0;
            }
            finish(l, res.map(|r| r.map(|p| format!("Depot sold for {}", money(p)))));
        }
        Op::OpenBus(id) => {
            l.career.shop.fleet_sel = Some(id);
            l.career.sub = SUB_FLEET;
        }
    }
}

fn finish(l: &mut Launcher, r: Option<Result<String, String>>) {
    match r {
        Some(Ok(t)) => {
            save(l);
            l.state.set_status(t, false);
        }
        Some(Err(e)) => l.state.set_status(e, true),
        None => {}
    }
}
