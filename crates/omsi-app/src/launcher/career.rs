//! The Career Mode page: a bus company that starts small. This file is the frame (the
//! page's title, the sub menu, starting a company) and the plain screens: Overview and
//! Finance. The jobs are in `career_jobs`, the dealership and the fleet in `career_shop`, the
//! depots (where the buses stand, and which one the player works from) in `career_depot`, the
//! drivers in `career_staff`, the list of buses the dealer leaves out in `career_ignore`, the
//! manufacturer / model / variant catalogue's editor in `career_catalog`, the debug window
//! (Ctrl+Shift+Alt+Backspace) in `career_debug`; the data is `company`, `staff` and
//! `omsi_launcher_lib::catalog`.
//!
//! One company, in slot 0 (`company::slot_path`). The page works on `Launcher::career`;
//! every change that matters is written to the file at once.

use super::career_catalog::CatalogView;
use super::career_depot::DepotView;
use super::career_ignore::IgnoreView;
use super::career_jobs::JobsView;
use super::career_livery::LiveryView;
use super::career_shop::ShopView;
use super::career_staff::StaffView;
use super::company::{self, economy, money, Company, Entry, CURRENCIES};
use super::theme::*;
use super::ui::ButtonKind;
use super::Launcher;
use glam::Vec2;
use omsi_launcher_lib as core;
use omsi_launcher_lib::catalog::Catalog;
use omsi_ui::paint::Align;
use omsi_ui::{Rect, Weight};

/// The sub menu, in order. The indices are what `CareerView::sub` holds.
pub const SUBS: [&str; 8] = ["Overview", "Jobs", "Dealership", "Fleet", "Depots", "Staff", "Finance", "Catalog"];
pub const SUB_JOBS: usize = 1;
pub const SUB_DEALERSHIP: usize = 2;
pub const SUB_FLEET: usize = 3;
pub const SUB_DEPOTS: usize = 4;
pub const SUB_STAFF: usize = 5;
pub const SUB_FINANCE: usize = 6;

/// The only slot for now.
pub const SLOT: usize = 0;

#[derive(Default)]
pub struct CareerView {
    pub sub: usize,
    pub(super) company: Option<Company>,
    loaded: bool,
    pub(super) catalog: Option<Catalog>,
    /// Bumped whenever the catalogue changed: what was worked out from it is made again.
    pub(super) cat_rev: u32,
    new_name: String,
    new_map: usize,
    new_diff: usize,
    pub(super) new_cur: usize,
    pub(super) shop: ShopView,
    pub(super) jobs: JobsView,
    /// The livery editor, while it is open (it takes the fleet page's place).
    pub(super) livery: Option<LiveryView>,
    pub(super) cat: CatalogView,
    pub(super) ign: IgnoreView,
    pub(super) depot: DepotView,
    pub(super) staff: StaffView,
    /// The list of buses the dealer leaves out is open (after founding, or from the dealership).
    pub(super) ignore_open: bool,
    /// Bumped whenever that list changed: the dealer's stock is made again.
    pub(super) ignore_rev: u32,
    /// The debug window is open, and the amount typed in it.
    pub debug_open: bool,
    pub(super) dbg_cash: String,
}

/// Write the company; a failure goes to the status line.
pub(super) fn save(l: &mut Launcher) {
    let Some(c) = l.career.company.as_ref() else { return };
    if let Err(e) = c.save(SLOT) {
        l.state.set_status(format!("The company could not be saved: {e}"), true);
    }
}

/// The bus of the vehicle list that `file` names (the list's paths and the company's may
/// differ in case and slashes).
pub(super) fn vehicle_of<'a>(vehicles: &'a [core::VehicleInfo], file: &str) -> Option<&'a core::VehicleInfo> {
    let k = core::catalog::file_key(file);
    vehicles.iter().find(|v| core::catalog::file_key(&v.file) == k)
}

pub(super) fn ensure_loaded(l: &mut Launcher) {
    let v = &mut l.career;
    if v.loaded {
        return;
    }
    v.loaded = true;
    v.company = Company::load(SLOT);
    v.catalog = Some(Catalog::load());
    v.new_diff = 1;
    if let Some(c) = v.company.as_ref() {
        v.new_cur = c.currency as usize;
        if let Some(i) = l.state.maps.iter().position(|m| m.file == c.home_map) {
            v.new_map = i;
        }
    }
}

pub fn draw(l: &mut Launcher, area: Rect) {
    ensure_loaded(l);
    let sub_line = match l.career.company.as_ref() {
        Some(c) => format!("{} · {} · {}", c.name, company::level_name(c.level()), money(c.balance)),
        None => "A bus company that starts small: buy buses, take jobs, grow.".to_string(),
    };
    let body = l.page_title(area, "Career Mode", &sub_line);
    if l.career.company.is_none() {
        new_career(l, body);
        return;
    }
    if l.career.ignore_open {
        super::career_ignore::draw(l, body);
        return;
    }
    let mut sub = l.career.sub.min(SUBS.len() - 1);
    let strip = Rect::new(body.x, body.y, body.w.min(SUBS.len() as f32 * 112.0), 34.0);
    if l.ui.segmented("career-sub", strip, &mut sub, &SUBS) {
        l.career.sub = sub;
        l.career.shop.armed = false;
        l.career.shop.sell_armed = false;
    }
    if let Some(s) = l.thumbs_status() {
        l.ui.text_in(&s, Rect::new(strip.right() + 16.0, body.y, (body.right() - strip.right() - 40.0).max(0.0), 34.0), 12.0, Weight::Regular, TEXT_DIM, Align::Right);
        l.thumbs_spinner(Vec2::new(body.right() - 12.0, body.y + 17.0));
    }
    let r = Rect::new(body.x, body.y + 48.0, body.w, (body.h - 48.0).max(0.0));
    if sub != SUB_FLEET && l.career.livery.is_some() {
        // (the decal editor is open but another page is shown: the bus as it was)
        if let Some(rd) = l.renderer.as_ref() {
            l.showroom.set_live(rd, &[]);
        }
        l.showroom.close_up(false);
        l.showroom.set_inside(false);
    }
    match sub {
        0 => overview(l, r),
        SUB_JOBS => super::career_jobs::draw(l, r),
        SUB_DEALERSHIP => super::career_shop::dealership(l, r),
        SUB_FLEET => {
            if l.career.livery.is_some() {
                super::career_livery::draw(l, r)
            } else {
                super::career_shop::fleet(l, r)
            }
        }
        SUB_DEPOTS => super::career_depot::draw(l, r),
        SUB_STAFF => super::career_staff::draw(l, r),
        SUB_FINANCE => finance(l, r),
        _ => super::career_catalog::draw(l, r),
    }
}

// --- starting a company ------------------------------------------------------------------

fn new_career(l: &mut Launcher, body: Rect) {
    let maps: Vec<String> = l.state.maps.iter().map(|m| m.friendly.clone()).collect();
    let files: Vec<String> = l.state.maps.iter().map(|m| m.file.clone()).collect();
    let w = body.w.min(640.0);
    let panel = Rect::new(body.x, body.y, w, 510.0);
    l.ui.panel(panel);
    let inner = l.ui.heading(Rect::new(panel.x + 20.0, panel.y + 14.0, panel.w - 40.0, panel.h - 28.0), "New career", Some("commute"));
    let (ui, v) = (&mut l.ui, &mut l.career);
    let mut y = inner.y;
    ui.text_in("Company name", Rect::new(inner.x, y, inner.w, 18.0), 11.5, Weight::Bold, TEXT_DIM, Align::Left);
    y += 20.0;
    ui.text_input("career-name", Rect::new(inner.x, y, inner.w, ROW), &mut v.new_name, "e.g. Nord Express", Some("badge"));
    y += ROW + 14.0;
    ui.text_in("Home map", Rect::new(inner.x, y, inner.w, 18.0), 11.5, Weight::Bold, TEXT_DIM, Align::Left);
    y += 20.0;
    if maps.is_empty() {
        ui.text_in("No maps found: set up the OMSI 2 folder on the Setup page.", Rect::new(inner.x, y, inner.w, ROW), 13.0, Weight::Regular, WARN, Align::Left);
    } else {
        v.new_map = v.new_map.min(maps.len() - 1);
        let mut m = v.new_map;
        if ui.select("career-map", Rect::new(inner.x, y, inner.w, ROW), &mut m, &maps) {
            v.new_map = m;
        }
    }
    y += ROW + 14.0;
    ui.text_in("Difficulty", Rect::new(inner.x, y, inner.w, 18.0), 11.5, Weight::Bold, TEXT_DIM, Align::Left);
    y += 20.0;
    let mut d = v.new_diff.min(2);
    if ui.segmented("career-diff", Rect::new(inner.x, y, inner.w.min(420.0), ROW), &mut d, &["Relaxed", "Standard", "Hardcore"]) {
        v.new_diff = d;
    }
    y += ROW + 14.0;
    ui.text_in("Currency", Rect::new(inner.x, y, inner.w, 18.0), 11.5, Weight::Bold, TEXT_DIM, Align::Left);
    y += 20.0;
    let currencies: Vec<String> = CURRENCIES.iter().map(|c| format!("{} ({})", c.code, c.symbol)).collect();
    let mut cur = v.new_cur.min(currencies.len() - 1);
    if ui.select("career-cur", Rect::new(inner.x, y, inner.w.min(300.0), ROW), &mut cur, &currencies) {
        v.new_cur = cur;
    }
    // (the amounts below are written in the currency chosen)
    company::set_currency(v.new_cur);
    y += ROW + 10.0;
    let d = v.new_diff.min(2);
    let text = format!("You start with {} in the bank (fixed exchange rates; the game's prices are in euros underneath). The company begins with one depot (three parking places). You buy buses, take the jobs yourself or send hired drivers, and grow: more places, more depots, more staff. Next you can choose which buses the dealer offers.", money(economy::START[d]));
    let h = ui.paragraph(&text, Vec2::new(inner.x, y), inner.w, 13.0, Weight::Regular, TEXT_DIM);
    y += h + 16.0;
    let ready = !v.new_name.trim().is_empty() && !maps.is_empty();
    let btn = Rect::new(inner.x, y, 200.0, ROW);
    if ready {
        if ui.button("career-start", btn, "Start career", Some("play_arrow"), ButtonKind::Primary) {
            let map = files.get(v.new_map).cloned().unwrap_or_default();
            let ignored = v.catalog.as_ref().map(|c| c.default_ignored(&l.state.vehicles)).unwrap_or_default();
            let c = Company::new(&v.new_name, &map, v.new_diff as u8, v.new_cur as u8, ignored);
            v.company = Some(c);
            v.shop = ShopView::default();
            v.depot = DepotView::default();
            v.staff = StaffView::default();
            v.ignore_open = true;
            v.ignore_rev += 1;
            save(l);
            if !map.is_empty() && l.state.choice.map.is_empty() {
                l.state.choice.map = map;
            }
            l.state.set_status("Company founded. Choose which buses the dealer offers, then buy your first one", false);
        }
    } else {
        ui.button_off(btn, "Start career", Some("play_arrow"), ButtonKind::Primary);
    }
}

// --- small helpers ------------------------------------------------------------------------

/// A label over a value in a panel.
fn stat(ui: &mut super::ui::Ui, r: Rect, label: &str, value: &str, note: &str, note_color: omsi_ui::Color) {
    ui.panel(r);
    ui.text_in(&label.to_uppercase(), Rect::new(r.x + 16.0, r.y + 12.0, r.w - 32.0, 16.0), 11.0, Weight::Bold, TEXT_DIM, Align::Left);
    ui.text_in(value, Rect::new(r.x + 16.0, r.y + 32.0, r.w - 32.0, 34.0), 24.0, Weight::Bold, TEXT, Align::Left);
    if !note.is_empty() {
        ui.text_in(note, Rect::new(r.x + 16.0, r.y + 68.0, r.w - 32.0, 18.0), 12.0, Weight::Regular, note_color, Align::Left);
    }
}

/// A bus of the fleet as a list shows it.
#[derive(Clone)]
pub(super) struct FleetRow {
    pub id: u32,
    pub file: String,
    pub paint: String,
    /// "#101 Citaro G" (for messages and the book).
    pub title: String,
    pub fleet_no: String,
    /// The two lines of the name: manufacturer, model; and the description below them.
    pub maker: String,
    pub model: String,
    pub desc: String,
    pub year: u16,
    pub condition: f32,
    pub km: f64,
    pub value: i64,
    /// The depot it stands in, and that depot's name.
    pub depot: u32,
    pub depot_name: String,
    /// Out on a driver's job until this time (0: not).
    pub away_until: u64,
}

pub(super) fn fleet_rows(l: &Launcher) -> Vec<FleetRow> {
    let (Some(c), Some(cat)) = (l.career.company.as_ref(), l.career.catalog.as_ref()) else { return Vec::new() };
    c.fleet
        .iter()
        .map(|b| {
            let (title, maker, model, desc) = match vehicle_of(&l.state.vehicles, &b.file) {
                Some(v) => {
                    let r = cat.resolve(v);
                    let desc = r.variant_text().to_string();
                    (format!("#{} {}", b.fleet_no, r.full_model()), r.maker.clone(), r.model.clone(), desc)
                }
                None => (format!("#{} {}", b.fleet_no, core::display_bus_name(&b.file)), String::new(), core::display_bus_name(&b.file).to_string(), "Not installed any more".to_string()),
            };
            let depot_name = c.depot(b.depot).map(|d| d.name.clone()).unwrap_or_default();
            FleetRow { id: b.id, file: b.file.clone(), paint: b.paint.clone(), title, fleet_no: b.fleet_no.clone(), maker, model, desc, year: b.year, condition: b.condition, km: b.km, value: economy::resale(b.price, b.condition), depot: b.depot, depot_name, away_until: b.away_until }
        })
        .collect()
}

pub(super) fn km_text(km: f64) -> String {
    let digits = (km.round() as u64).to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(' ');
        }
        out.push(c);
    }
    format!("{out} km")
}

/// Days since 1970-01-01 to a calendar date (the proleptic Gregorian calendar).
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub(super) fn date_time(secs: u64) -> String {
    let (y, m, d) = civil((secs / 86_400) as i64);
    let rest = secs % 86_400;
    format!("{y}-{m:02}-{d:02} {:02}:{:02}", rest / 3600, rest % 3600 / 60)
}

// --- the screens --------------------------------------------------------------------------

fn overview(l: &mut Launcher, r: Rect) {
    let Some(c) = l.career.company.clone() else { return };
    let gap = GAP + 4.0;
    let kw = (r.w - gap * 3.0) / 4.0;
    let kh = 92.0;
    let (progress, next) = c.level_progress();
    let level = c.level();
    let worn = c.fleet.iter().filter(|b| b.condition < 50.0).count();
    let up = c.ledger.iter().rev().take(20).map(|e| e.amount).sum::<i64>();
    stat(&mut l.ui, Rect::new(r.x, r.y, kw, kh), "Balance", &money(c.balance), &format!("{} over the last 20 entries", money(up)), if up >= 0 { OK } else { DANGER });
    stat(&mut l.ui, Rect::new(r.x + (kw + gap), r.y, kw, kh), "Level", &format!("{level} · {}", company::level_name(level)), &match next {
        Some(n) => format!("{} / {n} XP", c.xp),
        None => "Top level".to_string(),
    }, TEXT_DIM);
    stat(&mut l.ui, Rect::new(r.x + (kw + gap) * 2.0, r.y, kw, kh), "Reputation", &format!("{:.1} / 5", c.reputation), "Grows with punctual, careful jobs", TEXT_DIM);
    stat(&mut l.ui, Rect::new(r.x + (kw + gap) * 3.0, r.y, kw, kh), "Fleet", &format!("{} buses", c.fleet.len()), &if worn > 0 { format!("{worn} below 50 % condition") } else { "All in good shape".to_string() }, if worn > 0 { WARN } else { TEXT_DIM });
    // the level's bar under the Level tile
    l.ui.progress(Rect::new(r.x + (kw + gap) + 16.0, r.y + kh - 10.0, kw - 32.0, 4.0), progress, false);

    let top = r.y + kh + gap;
    let h = (r.h - kh - gap).max(120.0);
    let left = Rect::new(r.x, top, (r.w - gap) * 0.62, h);
    let right = Rect::new(left.right() + gap, top, r.w - left.w - gap, h);

    // --- the fleet
    l.ui.panel(left);
    let inner = l.ui.heading(Rect::new(left.x + 18.0, left.y + 14.0, left.w - 36.0, left.h - 28.0), "Your fleet", Some("garage"));
    let rows = fleet_rows(l);
    if rows.is_empty() {
        l.ui.paragraph("No buses yet. Open the Dealership and buy your first one: used buses are cheap, and every offer comes in a random paint.", Vec2::new(inner.x, inner.y), inner.w, 13.0, Weight::Regular, TEXT_DIM);
        if l.ui.button("career-to-shop", Rect::new(inner.x, inner.y + 60.0, 200.0, ROW), "Open the dealership", Some("inventory_2"), ButtonKind::Primary) {
            l.career.sub = SUB_DEALERSHIP;
        }
    }
    let th_h = 54.0;
    let th_w = th_h * 16.0 / 9.0;
    let multi = c.depots.len() > 1;
    let now = company::now();
    for (i, row) in rows.iter().enumerate() {
        let y = inner.y + i as f32 * (th_h + 10.0);
        if y + th_h > inner.bottom() {
            break;
        }
        l.draw_thumb(Rect::new(inner.x, y, th_w, th_h), &row.file, &row.paint, 5.0);
        let x = inner.x + th_w + 14.0;
        l.ui.text_in(&row.title, Rect::new(x, y + 4.0, inner.w - th_w - 200.0, 20.0), 14.0, Weight::Bold, TEXT, Align::Left);
        let place = if row.away_until > now {
            " · on a job".to_string()
        } else if multi {
            format!(" · {}", row.depot_name)
        } else {
            String::new()
        };
        l.ui.text_in(&format!("{}{} · {}{place}", row.maker, if row.year > 0 { format!(" · {}", row.year) } else { String::new() }, km_text(row.km)), Rect::new(x, y + 26.0, inner.w - th_w - 200.0, 18.0), 12.0, Weight::Regular, if row.away_until > now { ACCENT } else { TEXT_DIM }, Align::Left);
        l.ui.progress(Rect::new(inner.right() - 170.0, y + 14.0, 120.0, 5.0), row.condition / 100.0, false);
        l.ui.text_in(&format!("{:.0} %", row.condition), Rect::new(inner.right() - 44.0, y + 6.0, 44.0, 20.0), 12.5, Weight::Medium, TEXT_SOFT, Align::Right);
        l.ui.text_in(&money(row.value), Rect::new(inner.right() - 170.0, y + 26.0, 170.0, 18.0), 12.0, Weight::Regular, TEXT_DIM, Align::Right);
    }

    // --- the dealership's list
    l.ui.panel(right);
    let inner = l.ui.heading(Rect::new(right.x + 18.0, right.y + 14.0, right.w - 36.0, right.h - 28.0), "Dealership", Some("inventory_2"));
    let mut y = inner.y;
    let h = l.ui.paragraph("The dealer offers a few buses of every size and class your installed buses have, new or used, each in a random paint other than the bus's own. You choose which models it leaves out.", Vec2::new(inner.x, y), inner.w, 12.5, Weight::Regular, TEXT_DIM);
    y += h + 8.0;
    let (depot_name, parked, slots) = c.active().map(|d| (d.name.clone(), c.parked(d.id), d.slots)).unwrap_or_default();
    l.ui.text_in(&format!("Deliveries go to {depot_name} · {parked} of {slots} places taken"), Rect::new(inner.x, y, inner.w, 18.0), 12.5, Weight::Medium, if parked as u32 >= slots { WARN } else { TEXT_SOFT }, Align::Left);
    y += 24.0;
    let hidden = c.ignored.len();
    if l.ui.button("career-ignore", Rect::new(inner.x, y, inner.w, ROW), &format!("Choose offered buses · {hidden} hidden"), Some("tune"), ButtonKind::Normal) {
        l.career.ignore_open = true;
    }
    y += ROW + 8.0;
    if l.ui.button("career-to-catalog", Rect::new(inner.x, y, inner.w, ROW), "Bus catalogue", Some("directions_bus"), ButtonKind::Normal) {
        l.career.sub = SUBS.len() - 1;
    }
    y += ROW + 8.0;
    if l.ui.button("career-to-depots", Rect::new(inner.x, y, inner.w, ROW), &format!("Depots · {} · places and new depots", c.depots.len()), Some("garage"), ButtonKind::Normal) {
        l.career.sub = SUB_DEPOTS;
    }
    y += ROW + 8.0;
    let out = c.drivers.iter().filter(|d| d.job.is_some()).count();
    if l.ui.button("career-to-staff", Rect::new(inner.x, y, inner.w, ROW), &format!("Staff · {} driver(s){}", c.drivers.len(), if out > 0 { format!(", {out} out") } else { String::new() }), Some("groups"), ButtonKind::Normal) {
        l.career.sub = SUB_STAFF;
    }
    y += ROW + 14.0;
    l.ui.paragraph("Testing the economy: Ctrl+Shift+Alt+Backspace opens the debug window (money, level, reset ...).", Vec2::new(inner.x, y), inner.w, 12.0, Weight::Regular, TEXT_FAINT);
}

fn finance(l: &mut Launcher, r: Rect) {
    let Some(c) = l.career.company.clone() else { return };
    let gap = GAP + 4.0;
    let kw = ((r.w - gap * 2.0) / 3.0).min(340.0);
    let income: i64 = c.ledger.iter().filter(|e| e.amount > 0).map(|e| e.amount).sum();
    let spent: i64 = c.ledger.iter().filter(|e| e.amount < 0).map(|e| -e.amount).sum();
    stat(&mut l.ui, Rect::new(r.x, r.y, kw, 92.0), "Balance", &money(c.balance), "", TEXT_DIM);
    stat(&mut l.ui, Rect::new(r.x + kw + gap, r.y, kw, 92.0), "Total in", &money(income), "", TEXT_DIM);
    stat(&mut l.ui, Rect::new(r.x + (kw + gap) * 2.0, r.y, kw, 92.0), "Total out", &money(spent), "", TEXT_DIM);
    // --- the bank
    let bank = Rect::new(r.x, r.y + 92.0 + gap, r.w, 104.0);
    l.ui.panel(bank);
    let inner = l.ui.heading(Rect::new(bank.x + 18.0, bank.y + 12.0, bank.w - 36.0, bank.h - 24.0), "Bank loan", Some("payments"));
    let d = (c.difficulty as usize).min(2);
    let room = (c.loan_limit() - c.loan).max(0);
    l.ui.text_in(
        &format!("Owed {} · the bank lends up to {} at your level · interest {:.0} % per session, {:.0} % of the fares go to the loan", money(c.loan), money(c.loan_limit()), economy::LOAN_INTEREST[d] * 100.0, economy::LOAN_REPAY_SHARE * 100.0),
        Rect::new(inner.x, inner.y, inner.w, 18.0),
        12.5,
        Weight::Regular,
        TEXT_SOFT,
        Align::Left,
    );
    let mut x = inner.x;
    let by = inner.y + 26.0;
    let mut loan_act: Option<(bool, i64)> = None;
    for (i, amount) in [5_000i64, 10_000, 25_000].iter().enumerate() {
        let b = Rect::new(x, by, 150.0, ROW);
        let label = format!("Borrow {}", money(*amount));
        if *amount <= room {
            if l.ui.button(&format!("loan-take-{i}"), b, &label, None, ButtonKind::Normal) {
                loan_act = Some((true, *amount));
            }
        } else {
            l.ui.button_off(b, &label, None, ButtonKind::Normal);
        }
        x += 158.0;
    }
    x += 14.0;
    let pay_all = c.loan.min(c.balance);
    for (i, amount) in [5_000i64, i64::MAX].iter().enumerate() {
        let b = Rect::new(x, by, 150.0, ROW);
        let part = (*amount).min(c.loan).min(c.balance);
        let label = if i == 0 { format!("Repay {}", money(5_000i64.min(c.loan))) } else { "Repay all".to_string() };
        if c.loan > 0 && part > 0 {
            if l.ui.button(&format!("loan-pay-{i}"), b, &label, None, ButtonKind::Normal) {
                loan_act = Some((false, if i == 0 { part } else { pay_all }));
            }
        } else {
            l.ui.button_off(b, &label, None, ButtonKind::Normal);
        }
        x += 158.0;
    }
    if let Some((take, amount)) = loan_act {
        let res = l.career.company.as_mut().map(|c| if take { c.take_loan(amount).map(|_| format!("Loan taken: {}", money(amount))) } else { c.repay_loan(amount).map(|p| format!("Repaid {}", money(p))) });
        match res {
            Some(Ok(t)) => {
                save(l);
                l.state.set_status(t, false);
            }
            Some(Err(e)) => l.state.set_status(e, true),
            None => {}
        }
    }
    let list = Rect::new(r.x, bank.bottom() + gap, r.w, (r.h - 92.0 - gap * 2.0 - bank.h).max(80.0));
    l.ui.panel(list);
    let inner = l.ui.heading(Rect::new(list.x + 18.0, list.y + 14.0, list.w - 36.0, list.h - 28.0), "Ledger", Some("receipt_long"));
    let entries: Vec<Entry> = c.ledger.iter().rev().cloned().collect();
    if entries.is_empty() {
        l.ui.text_in("Nothing booked yet.", Rect::new(inner.x, inner.y, inner.w, 20.0), 13.0, Weight::Regular, TEXT_DIM, Align::Left);
        return;
    }
    l.ui.scroll_area("career-ledger", Rect::new(inner.x - 6.0, inner.y, inner.w + 12.0, inner.h), &mut |ui, v| {
        for (i, e) in entries.iter().enumerate() {
            let y = v.y + i as f32 * 34.0;
            if i % 2 == 0 {
                ui.p().rounded(Rect::new(v.x + 6.0, y, v.w - 22.0, 32.0), 5.0, FIELD.alpha(0.5));
            }
            ui.text_in(&date_time(e.at), Rect::new(v.x + 16.0, y, 150.0, 32.0), 12.5, Weight::Regular, TEXT_DIM, Align::Left);
            ui.text_in(&e.text, Rect::new(v.x + 176.0, y, v.w - 360.0, 32.0), 13.0, Weight::Regular, TEXT, Align::Left);
            ui.text_in(&money(e.amount), Rect::new(v.right() - 190.0, y, 170.0, 32.0), 13.0, Weight::Medium, if e.amount >= 0 { OK } else { DANGER }, Align::Right);
        }
        entries.len() as f32 * 34.0
    });
}

/// The company's levels. Not in the menu any more (the level shows on the overview); kept
/// for when it comes back.
#[allow(dead_code)]
fn progress(l: &mut Launcher, r: Rect) {
    let Some(c) = l.career.company.clone() else { return };
    let w = r.w.min(760.0);
    let panel = Rect::new(r.x, r.y, w, r.h.min(470.0));
    l.ui.panel(panel);
    let inner = l.ui.heading(Rect::new(panel.x + 20.0, panel.y + 14.0, panel.w - 40.0, panel.h - 28.0), "Company levels", Some("trending_up"));
    let level = c.level();
    let unlocks = [
        "Loans up to 10 000 · every map is open to you",
        "Loans up to 25 000",
        "Loans up to 60 000, hiring drivers (later)",
        "Loans up to 120 000, contracts with a city (later)",
        "Loans up to 250 000, several maps at once (later)",
    ];
    let (frac, next) = c.level_progress();
    for (i, name) in company::LEVEL_NAMES.iter().enumerate() {
        let n = i as u32 + 1;
        let y = inner.y + i as f32 * 62.0;
        let here = n == level;
        let done = n < level;
        let row = Rect::new(inner.x, y, inner.w, 56.0);
        if here {
            l.ui.p().rounded(row, 6.0, SELECTED);
            l.ui.p().rounded(Rect::new(row.x, row.y + 8.0, 2.0, row.h - 16.0), 1.0, ACCENT);
        }
        l.ui.icon(if done { "check_circle" } else if here { "star" } else { "lock" }, Vec2::new(row.x + 24.0, row.center().y), 20.0, if done { OK } else if here { ACCENT } else { TEXT_FAINT });
        let col = if n > level { TEXT_DIM } else { TEXT };
        l.ui.text_in(&format!("{n} · {name}"), Rect::new(row.x + 48.0, row.y + 8.0, row.w - 200.0, 20.0), 14.0, Weight::Bold, col, Align::Left);
        l.ui.text_in(unlocks[i], Rect::new(row.x + 48.0, row.y + 30.0, row.w - 200.0, 18.0), 12.0, Weight::Regular, TEXT_DIM, Align::Left);
        let xp = company::LEVEL_XP[i];
        l.ui.text_in(&format!("{xp} XP"), Rect::new(row.right() - 130.0, row.y, 120.0, 56.0), 12.5, Weight::Regular, TEXT_DIM, Align::Right);
        if here {
            if let Some(nx) = next {
                l.ui.progress(Rect::new(row.right() - 230.0, row.y + 40.0, 220.0, 4.0), frac, false);
                l.ui.text_in(&format!("{} / {nx}", c.xp), Rect::new(row.right() - 230.0, row.y + 8.0, 100.0, 18.0), 11.5, Weight::Regular, TEXT_SOFT, Align::Left);
            }
        }
    }
}
