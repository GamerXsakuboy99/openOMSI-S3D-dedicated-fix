//! Career Mode, Jobs: choose a bus of the fleet, a line and tour of the home map and the
//! course to begin with, and start the session. The game gets the duty and a session file
//! (`career_session`); after every course it asks whether to drive the next one or to go back
//! to the garage, and reports the course into the file. When the game is gone the launcher
//! reads the file back, pays the courses (`company::settle`) and shows the report.
//!
//! The timetable comes from the same reader as the Drive page (`core::list_lines`), on a worker
//! of its own, for the map of the company's active depot and the date chosen on the Drive page.
//! Only the buses of the active depot can go out. The chosen course is shown on the map (the
//! Drive page's map picture, `mapview`, framed on its route). A hired driver can be sent instead
//! of driving: `Company::send_driver` simulates the job and pays when its time is up.

use super::career::{ensure_loaded, fleet_rows, save, vehicle_of, FleetRow, SUB_DEPOTS, SUB_STAFF};
use super::company::{self, money, settle};
use super::mapview;
use super::staff::{self, Spec};
use super::state::hhmm;
use super::theme::*;
use super::thumbs::thumb_draw;
use super::ui::ButtonKind;
use super::Launcher;
use crate::career_session::Session;
use glam::Vec2;
use omsi_launcher_lib as core;
use omsi_ui::paint::Align;
use omsi_ui::{Rect, Weight};
use std::sync::mpsc::{channel, Receiver};
use std::time::Instant;

type LinesMsg = (String, String, Result<Vec<core::LineInfo>, String>);

/// A session that has been started and is not booked yet.
struct Running {
    bus_id: u32,
    label: String,
    started: Instant,
    /// The game was seen running (it ends when it is gone again).
    seen_game: bool,
    /// What the game has reported so far, as lines to show.
    view: Vec<String>,
}

#[derive(Default)]
pub struct JobsView {
    lines: Vec<core::LineInfo>,
    for_key: Option<(String, String)>,
    rx: Option<Receiver<LinesMsg>>,
    error: String,
    bus: Option<u32>,
    line: usize,
    tour: usize,
    trip: usize,
    running: Option<Running>,
    poll: f32,
    checked_pending: bool,
    /// The last session's report, until it is closed.
    report: Option<Vec<String>>,
    /// A driver is sent instead of the player driving, and which one.
    driver_mode: bool,
    driver: Option<u32>,
    /// The last course the driver is given: (line, tour, course) in the lists as chosen.
    until: Option<(usize, usize, usize)>,
}

/// Indices of the lines a player may drive and that have a tour running that day.
fn runnable(lines: &[core::LineInfo]) -> Vec<usize> {
    lines.iter().enumerate().filter(|(_, x)| x.user_allowed && x.tours.iter().any(|t| t.runs && !t.trips.is_empty())).map(|(i, _)| i).collect()
}

/// Indices of the tours of a line that run that day.
fn runnable_tours(line: &core::LineInfo) -> Vec<usize> {
    line.tours.iter().enumerate().filter(|(_, t)| t.runs && !t.trips.is_empty()).map(|(i, _)| i).collect()
}

/// Ask for (or take over) the timetable of `map` on `date`.
fn want_lines(l: &mut Launcher, map: &str, date: &str) {
    let key = (map.to_string(), date.to_string());
    let v = &mut l.career.jobs;
    let got = v.rx.as_ref().map(|rx| rx.try_recv());
    if let Some(got) = got {
        match got {
            Ok((m, d, res)) => {
                v.rx = None;
                if m == key.0 && d == key.1 {
                    match res {
                        Ok(ls) => {
                            v.lines = ls;
                            v.error.clear();
                        }
                        Err(e) => {
                            v.lines.clear();
                            v.error = e;
                        }
                    }
                    v.for_key = Some(key);
                    v.line = 0;
                    v.tour = 0;
                    v.trip = 0;
                }
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                v.rx = None;
                v.for_key = Some(key);
                v.lines.clear();
                v.error = "the timetable reader stopped".to_string();
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
        return;
    }
    if v.for_key.as_ref() != Some(&key) {
        let (tx, rx) = channel();
        v.rx = Some(rx);
        let (m, d) = key;
        std::thread::spawn(move || {
            let r = core::list_lines(&m, &d).map_err(|e| format!("{e:#}"));
            let _ = tx.send((m, d, r));
        });
    }
}

pub(super) fn draw(l: &mut Launcher, r: Rect) {
    if l.career.jobs.report.is_some() {
        report_panel(l, r);
        return;
    }
    if l.career.jobs.running.is_some() {
        running_panel(l, r);
        return;
    }
    let Some((home, active_depot, depot_name)) = l.career.company.as_ref().map(|c| (c.active_map(), c.active_depot, c.active().map(|d| d.name.clone()).unwrap_or_default())) else { return };
    let date = l.state.choice.date.clone();
    let home_ok = l.state.maps.iter().any(|m| m.file == home);
    let home_name = l.state.maps.iter().find(|m| m.file == home).map(|m| if m.friendly.is_empty() { m.name.clone() } else { m.friendly.clone() }).unwrap_or_else(|| home.clone());
    if !home_ok {
        let panel = Rect::new(r.x, r.y, r.w.min(640.0), 110.0);
        l.ui.panel(panel);
        l.ui.paragraph(&format!("The home map of the company ({home_name}) is not installed. Install it, or start a new career on another map."), Vec2::new(panel.x + 20.0, panel.y + 20.0), panel.w - 40.0, 13.5, Weight::Regular, WARN);
        return;
    }
    want_lines(l, &home, &date);

    let all = fleet_rows(l);
    if all.is_empty() {
        let panel = Rect::new(r.x, r.y, r.w.min(640.0), 150.0);
        l.ui.panel(panel);
        l.ui.paragraph("A job needs a bus. Buy one in the dealership first: used buses are cheap.", Vec2::new(panel.x + 20.0, panel.y + 20.0), panel.w - 40.0, 13.5, Weight::Regular, TEXT_SOFT);
        if l.ui.button("jobs-to-shop", Rect::new(panel.x + 20.0, panel.y + 80.0, 220.0, ROW), "Open the dealership", Some("inventory_2"), ButtonKind::Primary) {
            l.career.sub = super::career::SUB_DEALERSHIP;
        }
        return;
    }
    // (only the buses standing in the depot the player works from can go out)
    let rows: Vec<FleetRow> = all.into_iter().filter(|b| b.depot == active_depot).collect();
    if rows.is_empty() {
        let panel = Rect::new(r.x, r.y, r.w.min(700.0), 170.0);
        l.ui.panel(panel);
        l.ui.paragraph(
            &format!("No bus stands in {depot_name}, the depot you work from. Buy one (the dealer delivers to the active depot), move one here from the Fleet page, or work from another depot."),
            Vec2::new(panel.x + 20.0, panel.y + 20.0),
            panel.w - 40.0,
            13.5,
            Weight::Regular,
            TEXT_SOFT,
        );
        if l.ui.button("jobs-to-shop2", Rect::new(panel.x + 20.0, panel.y + 100.0, 220.0, ROW), "Open the dealership", Some("inventory_2"), ButtonKind::Primary) {
            l.career.sub = super::career::SUB_DEALERSHIP;
        }
        if l.ui.button("jobs-to-depots", Rect::new(panel.x + 252.0, panel.y + 100.0, 220.0, ROW), "Depots", Some("garage"), ButtonKind::Normal) {
            l.career.sub = SUB_DEPOTS;
        }
        return;
    }
    for b in &rows {
        l.thumb_request(&b.file, &b.paint);
    }

    // --- what is chosen
    let bus_id = l.career.jobs.bus.filter(|id| rows.iter().any(|b| b.id == *id)).unwrap_or(rows[0].id);
    l.career.jobs.bus = Some(bus_id);
    let loading = l.career.jobs.rx.is_some() || l.career.jobs.for_key.is_none();
    let line_ix = runnable(&l.career.jobs.lines);
    let line_names: Vec<String> = line_ix.iter().map(|&i| l.career.jobs.lines[i].name.clone()).collect();
    let line_sel = l.career.jobs.line.min(line_ix.len().saturating_sub(1));
    let line = line_ix.get(line_sel).map(|&i| l.career.jobs.lines[i].clone());
    let tour_ix: Vec<usize> = line.as_ref().map(runnable_tours).unwrap_or_default();
    let tour_sel = l.career.jobs.tour.min(tour_ix.len().saturating_sub(1));
    let tour = line.as_ref().and_then(|x| tour_ix.get(tour_sel).map(|&i| x.tours[i].clone()));
    let trip_sel = tour.as_ref().map(|t| l.career.jobs.trip.min(t.trips.len().saturating_sub(1))).unwrap_or(0);

    // --- the layout: three columns and a bar
    let gap = 14.0;
    let bar_h = 118.0;
    let top_h = (r.h - bar_h - gap).max(160.0);
    let w1 = ((r.w - 2.0 * gap) * 0.30).max(120.0);
    let w2 = ((r.w - 2.0 * gap) * 0.28).max(120.0);
    let w3 = (r.w - 2.0 * gap - w1 - w2).max(120.0);
    let c1 = Rect::new(r.x, r.y, w1, top_h);
    let c2 = Rect::new(c1.right() + gap, r.y, w2, top_h);
    let c3 = Rect::new(c2.right() + gap, r.y, w3, top_h);
    let bar = Rect::new(r.x, r.y + top_h + gap, r.w, bar_h);
    // why the chosen bus cannot go out (an inspection due, a failed one, no fuel ...)
    let block: Option<&'static str> = l.career.company.as_ref().and_then(|c| c.fleet.iter().find(|b| b.id == bus_id)).and_then(|b| b.drive_block());

    // --- 1 · the bus
    l.ui.panel(c1);
    let inner = l.ui.heading(Rect::new(c1.x + 16.0, c1.y + 14.0, c1.w - 32.0, c1.h - 28.0), &format!("1 · Bus · {depot_name}"), Some("directions_bus"));
    let now = company::now();
    let mut pick_bus = None;
    {
        let icons = &l.icons;
        let row_h = 62.0;
        l.ui.scroll_area("jobs-bus", inner, &mut |ui, v| {
            for (i, b) in rows.iter().enumerate() {
                let rr = Rect::new(v.x, v.y + i as f32 * (row_h + 4.0), v.w - 10.0, row_h);
                if rr.bottom() < inner.y - 4.0 || rr.y > inner.bottom() + 4.0 {
                    continue;
                }
                if ui.row(&format!("jobs-bus-{}", b.id), rr, bus_id == b.id) {
                    pick_bus = Some(b.id);
                }
                let th = Rect::new(rr.x + 8.0, rr.y + 8.0, 78.0, 44.0);
                thumb_draw(ui, icons, th, &b.file, &b.paint, 4.0);
                let x = th.right() + 10.0;
                let w = rr.right() - x - 6.0;
                ui.text_in(&format!("#{} · {}", b.fleet_no, b.maker), Rect::new(x, rr.y + 6.0, w, 14.0), 10.5, Weight::Medium, TEXT_DIM, Align::Left);
                ui.text_in(&b.model, Rect::new(x, rr.y + 19.0, w, 20.0), 14.0, Weight::Bold, TEXT, Align::Left);
                if b.away_until > now {
                    ui.text_in("on a job with a driver", Rect::new(x, rr.y + 40.0, w, 14.0), 10.5, Weight::Medium, ACCENT, Align::Left);
                } else {
                    ui.text_in(&format!("condition {:.0} %", b.condition), Rect::new(x, rr.y + 40.0, w, 14.0), 10.5, Weight::Regular, if b.condition < 40.0 { WARN } else { TEXT_DIM }, Align::Left);
                }
            }
            rows.len() as f32 * (row_h + 4.0)
        });
    }
    if let Some(id) = pick_bus {
        l.career.jobs.bus = Some(id);
    }

    // --- 2 · the line and the tour
    l.ui.panel(c2);
    let inner = l.ui.heading(Rect::new(c2.x + 16.0, c2.y + 14.0, c2.w - 32.0, c2.h - 28.0), "2 · Line and tour", Some("route"));
    if line_names.is_empty() {
        let text = if loading {
            "Reading the timetable…".to_string()
        } else if !l.career.jobs.error.is_empty() {
            format!("The timetable could not be read: {}", l.career.jobs.error)
        } else {
            format!("No tour of {home_name} runs on {date}. Another date can be chosen on the Drive page.")
        };
        l.ui.paragraph(&text, Vec2::new(inner.x, inner.y), inner.w, 13.0, Weight::Regular, TEXT_DIM);
    } else {
        let mut sel = line_sel;
        if l.ui.select("jobs-line", Rect::new(inner.x, inner.y, inner.w, ROW), &mut sel, &line_names) {
            l.career.jobs.line = sel;
            l.career.jobs.tour = 0;
            l.career.jobs.trip = 0;
        }
        let list = Rect::new(inner.x, inner.y + ROW + 10.0, inner.w, (inner.h - ROW - 10.0).max(40.0));
        let mut pick_tour = None;
        let row_h = 52.0;
        if let Some(line) = line.as_ref() {
            let tours: Vec<&core::TourInfo> = tour_ix.iter().map(|&i| &line.tours[i]).collect();
            l.ui.scroll_area("jobs-tour", list, &mut |ui, v| {
                for (i, t) in tours.iter().enumerate() {
                    let rr = Rect::new(v.x, v.y + i as f32 * (row_h + 4.0), v.w - 10.0, row_h);
                    if rr.bottom() < list.y - 4.0 || rr.y > list.bottom() + 4.0 {
                        continue;
                    }
                    if ui.row(&format!("jobs-tour-{i}"), rr, tour_sel == i) {
                        pick_tour = Some(i);
                    }
                    ui.text_in(&format!("Tour {}", t.number), Rect::new(rr.x + 12.0, rr.y + 7.0, rr.w - 20.0, 20.0), 14.0, Weight::Bold, TEXT, Align::Left);
                    ui.text_in(&format!("{}–{} · {} courses · {}", hhmm(t.first), hhmm(t.last), t.trips.len(), t.days), Rect::new(rr.x + 12.0, rr.y + 28.0, rr.w - 20.0, 16.0), 11.0, Weight::Regular, TEXT_DIM, Align::Left);
                }
                tours.len() as f32 * (row_h + 4.0)
            });
        }
        if let Some(i) = pick_tour {
            l.career.jobs.tour = i;
            l.career.jobs.trip = 0;
        }
    }

    // --- 3 · the course to begin with: its route on the map, then the courses
    let over_before = l.ui.over_ui;
    l.ui.panel(c3);
    let inner = l.ui.heading(Rect::new(c3.x + 16.0, c3.y + 14.0, c3.w - 32.0, c3.h - 28.0), "3 · First course", Some("schedule"));
    if let Some(t) = tour.as_ref() {
        let map_h = (inner.h * 0.46).clamp(150.0, 330.0).min((inner.h - 120.0).max(90.0));
        let map_r = Rect::new(inner.x, inner.y, inner.w, map_h);
        route_map(l, map_r, &home, &date, &t.trips[trip_sel], over_before);
        let list = Rect::new(inner.x, map_r.bottom() + 10.0, inner.w, (inner.bottom() - map_r.bottom() - 10.0).max(40.0));
        let mut pick_trip = None;
        let row_h = 48.0;
        l.ui.scroll_area("jobs-trip", list, &mut |ui, v| {
            for (i, x) in t.trips.iter().enumerate() {
                let rr = Rect::new(v.x, v.y + i as f32 * (row_h + 4.0), v.w - 10.0, row_h);
                if rr.bottom() < list.y - 4.0 || rr.y > list.bottom() + 4.0 {
                    continue;
                }
                if ui.row(&format!("jobs-trip-{i}"), rr, trip_sel == i) {
                    pick_trip = Some(i);
                }
                ui.text_in(&format!("{} → {}", hhmm(x.departure), hhmm(x.arrival)), Rect::new(rr.x + 12.0, rr.y + 6.0, rr.w * 0.6, 20.0), 14.0, Weight::Bold, TEXT, Align::Left);
                ui.text_in(&format!("{:.1} km", x.km), Rect::new(rr.x + rr.w * 0.6, rr.y + 8.0, rr.w * 0.4 - 12.0, 18.0), 12.5, Weight::Medium, TEXT_SOFT, Align::Right);
                let from = if x.from.is_empty() { "?" } else { x.from.as_str() };
                ui.text_in(&format!("{from} → {}", x.terminus), Rect::new(rr.x + 12.0, rr.y + 27.0, rr.w - 24.0, 16.0), 11.0, Weight::Regular, TEXT_DIM, Align::Left);
            }
            t.trips.len() as f32 * (row_h + 4.0)
        });
        if let Some(i) = pick_trip {
            l.career.jobs.trip = i;
        }
    } else {
        l.ui.paragraph("Choose a line and a tour.", Vec2::new(inner.x, inner.y), inner.w, 13.0, Weight::Regular, TEXT_DIM);
    }

    // --- the bar: what will happen, who drives, and the start
    l.ui.panel(bar);
    let bus = rows.iter().find(|b| b.id == bus_id).cloned();
    let pad = 18.0;
    let drivers: Vec<(u32, String, bool)> = l
        .career
        .company
        .as_ref()
        .map(|c| {
            c.drivers
                .iter()
                .map(|d| {
                    let tag = if d.job.is_some() {
                        " · out"
                    } else if d.resting(now) {
                        " · resting"
                    } else {
                        ""
                    };
                    (d.id, format!("{} · skill {:.0}{tag}", d.name, d.skill), d.job.is_none() && !d.resting(now))
                })
                .collect()
        })
        .unwrap_or_default();
    let seg_w = 250.0;
    let mut mode = l.career.jobs.driver_mode as usize;
    if l.ui.segmented("jobs-mode", Rect::new(bar.right() - pad - seg_w, bar.y + 10.0, seg_w, ROW), &mut mode, &["Drive myself", "Send a driver"]) {
        l.career.jobs.driver_mode = mode == 1;
    }
    let by_driver = l.career.jobs.driver_mode;
    // the courses a driver is given: from the chosen one to the chosen end (a shift at most)
    let times: Vec<(f64, f64)> = tour.as_ref().map(|t| t.trips.iter().map(|x| (x.departure, x.arrival)).collect()).unwrap_or_default();
    let until_pick = l.career.jobs.until.filter(|u| u.0 == line_sel && u.1 == tour_sel).map(|u| u.2);
    let run_last = if by_driver { staff::run_end(&times, trip_sel, until_pick) } else { None };
    let mut title_w = bar.w - pad * 2.0 - seg_w - 16.0;
    if let Some(last) = run_last {
        let opts = staff::run_options(&times, trip_sel);
        let names: Vec<String> = opts.iter().map(|&k| format!("until {} · {} course(s)", hhmm(times[k].1), k - trip_sel + 1)).collect();
        let (w, mut ix) = (230.0, last - trip_sel);
        if l.ui.select("jobs-until", Rect::new(bar.right() - pad - seg_w - 12.0 - w, bar.y + 10.0, w, ROW), &mut ix, &names) {
            l.career.jobs.until = opts.get(ix).map(|&k| (line_sel, tour_sel, k));
        }
        title_w = (title_w - w - 12.0).max(80.0);
    }
    let row_b = bar.y + 10.0 + ROW + 10.0;
    let go = Rect::new(bar.right() - pad - 210.0, row_b, 210.0, ROW);
    // the driver to send
    let mut driver: Option<(u32, String, bool)> = None;
    if by_driver && !drivers.is_empty() {
        let names: Vec<String> = drivers.iter().map(|d| d.1.clone()).collect();
        let mut di = l.career.jobs.driver.and_then(|id| drivers.iter().position(|d| d.0 == id)).or_else(|| drivers.iter().position(|d| d.2)).unwrap_or(0);
        if l.ui.select("jobs-driver", Rect::new(go.x - 12.0 - 250.0, row_b, 250.0, ROW), &mut di, &names) {
            l.career.jobs.driver = drivers.get(di).map(|d| d.0);
        }
        driver = drivers.get(di).cloned();
    }
    let x = bar.x + pad;
    let note_w = (go.x - if by_driver && !drivers.is_empty() { 12.0 + 250.0 } else { 0.0 } - 16.0 - x).max(120.0);
    match (bus.as_ref(), line.as_ref(), tour.as_ref()) {
        (Some(b), Some(li), Some(t)) => {
            let first = &t.trips[trip_sel];
            l.ui.text_in(&format!("#{} {} on line {} · tour {} · from {}", b.fleet_no, b.model, li.name, t.number, hhmm(first.departure)), Rect::new(x, bar.y + 14.0, title_w, 22.0), 15.0, Weight::Bold, TEXT, Align::Left);
            let left = t.trips.len() - trip_sel;
            let (note, color) = match block {
                Some(why) => (format!("{why} (Fleet page)."), DANGER),
                None if by_driver => match driver.as_ref() {
                    Some(d) => {
                        let drv = l.career.company.as_ref().and_then(|c| c.drivers.iter().find(|x| x.id == d.0));
                        let share = drv.map(|x| x.share()).unwrap_or(0.0);
                        let resting = drv.filter(|x| x.resting(now));
                        match (run_last, resting) {
                            (None, _) => ("This course alone takes longer than a driver works in one go.".to_string(), DANGER),
                            (Some(last), rest) => {
                                let n = last - trip_sel + 1;
                                let secs = staff::job_secs(times[last].1 - first.departure);
                                let mut text = format!(
                                    "{n} course(s), {} → {} on the timetable: the driver is out for {} of real time, then it is paid. Wage {:.0} % of the takings, at least {} a course. Afterwards they rest {}.{}",
                                    hhmm(first.departure),
                                    hhmm(times[last].1),
                                    staff::span_text(secs),
                                    share * 100.0,
                                    money(staff::MIN_WAGE_PER_COURSE),
                                    staff::span_text(staff::rest_secs(secs)),
                                    if staff::is_night(first.departure) { " A night shift." } else { "" }
                                );
                                if let Some(x) = rest {
                                    text = format!("{} is resting for another {}. {text}", x.name, staff::span_text(x.rest_until.saturating_sub(now)));
                                }
                                (text, if rest.is_some() { WARN } else { TEXT_DIM })
                            }
                        }
                    }
                    None => ("No driver is employed yet: hire one on the Staff page.".to_string(), WARN),
                },
                None => (format!("{left} courses to the end of the tour. After each course the game asks: next course or back to the garage. Courses driven to their end are paid, and each one in a row pays more (x1.5, x2, x2.5 ...)."), TEXT_DIM),
            };
            l.ui.paragraph(&note, Vec2::new(x, bar.y + 44.0), note_w, 11.5, Weight::Regular, color);
        }
        _ => {
            l.ui.text_in("Choose a bus, a line, a tour and the first course.", Rect::new(x, bar.y + 14.0, note_w, 22.0), 14.0, Weight::Regular, TEXT_DIM, Align::Left);
        }
    }
    let busy = l.state.in_game();
    if by_driver {
        match (bus, line, tour, driver) {
            (_, _, _, None) if drivers.is_empty() => {
                if l.ui.button("jobs-to-staff", go, "Hire a driver", Some("groups"), ButtonKind::Primary) {
                    l.career.sub = SUB_STAFF;
                }
            }
            (Some(_), Some(_), Some(_), Some((_, _, false))) => l.ui.button_off(go, "Driver unavailable", Some("groups"), ButtonKind::Primary),
            (Some(_), _, _, _) if block.is_some() => l.ui.button_off(go, "Bus cannot go out", Some("construction"), ButtonKind::Primary),
            (Some(b), Some(li), Some(t), Some((id, _, true))) if run_last.is_some() => {
                if l.ui.button("jobs-send", go, "Send the driver", Some("route"), ButtonKind::Primary) {
                    send_driver_job(l, &b, &li.name, &t, trip_sel, run_last.unwrap_or(trip_sel), id);
                }
            }
            _ => l.ui.button_off(go, "Send the driver", Some("route"), ButtonKind::Primary),
        }
        return;
    }
    match (bus, line, tour) {
        (Some(b), Some(li), Some(t)) if !busy && block.is_none() => {
            if l.ui.button("jobs-start", go, "Start session", Some("play_arrow"), ButtonKind::Primary) {
                start(l, &b, &li.name, &t, trip_sel);
            }
        }
        (Some(_), _, _) if block.is_some() => l.ui.button_off(go, "Bus cannot go out", Some("construction"), ButtonKind::Primary),
        _ => l.ui.button_off(go, if busy { "A game is running" } else { "Start session" }, Some("play_arrow"), ButtonKind::Primary),
    }
}

/// The chosen course on the map: the Drive page's map picture framed on the route, dragged and
/// zoomed here too, with the first and the last stop written beside their points.
fn route_map(l: &mut Launcher, r: Rect, map: &str, date: &str, trip: &core::TripInfo, over_before: bool) {
    let global = omsi_cfg::find_in_roots(map).map(|(_, p)| p).unwrap_or_else(|| omsi_cfg::resolve_path(std::path::Path::new(&l.state.config.root), map));
    l.mapview.want(mapview::Look { map: map.to_string(), global, date: date.to_string(), trip: trip.name.clone(), entry: -1 });
    l.map_background(r);
    l.ui.p().rounded_border(r, RADIUS, 1.0, EDGE);
    // (the panel the map lies in has taken the mouse: the map is given it back where it is)
    let p = mapview::Pointer {
        at: l.ui.input.mouse,
        pressed: l.ui.input.pressed,
        released: l.ui.input.released,
        down: l.ui.input.down,
        wheel: l.ui.input.wheel.y,
        blocked: over_before || !r.contains(l.ui.input.mouse),
    };
    let scale = l.ui.scale;
    l.mapview.think(r, r, scale, p);
    // (a click on an entry point is the Drive page's business: not kept for it)
    let _ = l.mapview.take_clicked();
    let stops: Vec<(glam::DVec2, usize)> = l.mapview.stops_placed().to_vec();
    let (Some(first), Some(last)) = (stops.iter().map(|s| s.1).min(), stops.iter().map(|s| s.1).max()) else { return };
    for (place, k) in stops {
        if k != first && k != last {
            continue;
        }
        let Some(st) = trip.stops.get(k) else { continue };
        let at = l.mapview.project(place);
        if !r.contains(at) {
            continue;
        }
        let name: String = if st.name.chars().count() > 26 { st.name.chars().take(25).collect::<String>() + "…" } else { st.name.clone() };
        let time = hhmm(st.arr);
        let tw = l.ui.width(&time, 11.0, Weight::Bold);
        let w = l.ui.width(&name, 11.0, Weight::Medium) + tw + 22.0;
        let right = Rect::new(at.x + 9.0, at.y - 9.0, w, 18.0);
        let rr = if right.right() <= r.right() - 4.0 { right } else { Rect::new(at.x - 9.0 - w, at.y - 9.0, w, 18.0) };
        if rr.x < r.x + 4.0 {
            continue;
        }
        l.ui.p().rounded(rr, 4.0, omsi_ui::Color::rgba(10, 10, 10, 0.84));
        l.ui.text_in(&time, Rect::new(rr.x + 6.0, rr.y, tw + 2.0, rr.h), 11.0, Weight::Bold, if k == first { OK } else { ACCENT }, Align::Left);
        l.ui.text_in(&name, Rect::new(rr.x + 12.0 + tw, rr.y, rr.w - tw - 16.0, rr.h), 11.0, Weight::Medium, TEXT_SOFT, Align::Left);
    }
}

/// Send a hired driver out with this bus on the courses `first..=last` of the tour; the job
/// lasts from the first departure to the last arrival of the timetable.
fn send_driver_job(l: &mut Launcher, bus: &FleetRow, line: &str, tour: &core::TourInfo, first: usize, last: usize, driver_id: u32) {
    let last = last.min(tour.trips.len().saturating_sub(1));
    let first = first.min(last);
    let Some(rest) = tour.trips.get(first..=last) else { return };
    if rest.is_empty() {
        return;
    }
    let n = rest.len() as u32;
    let stops = (rest.iter().map(|t| t.stops.len()).sum::<usize>() as f64 / n as f64).round().max(1.0) as u32;
    let km = rest.iter().map(|t| t.km).sum::<f64>() / n as f64;
    let sched_secs = rest[rest.len() - 1].arrival - rest[0].departure;
    let Some(owned) = l.career.company.as_ref().and_then(|c| c.fleet.iter().find(|b| b.id == bus.id)).cloned() else { return };
    let size = vehicle_of(&l.state.vehicles, &owned.file).and_then(|v| l.career.catalog.as_ref().map(|cat| cat.resolve(v).size)).unwrap_or_else(|| "MAXI".to_string());
    let spec = Spec { courses: n, stops, km, night: staff::is_night(rest[0].departure), fuel_per_km: owned.consumption().map(|x| x / 100.0).unwrap_or(0.38), sched_secs };
    let seed = company::mix(company::now() ^ ((driver_id as u64) << 40) ^ ((owned.id as u64) << 20) ^ (owned.km as u64));
    let text = if n == 1 { format!("Line {line} · tour {} · {}", tour.number, hhmm(rest[0].departure)) } else { format!("Line {line} · tour {} · {}–{}", tour.number, hhmm(rest[0].departure), hhmm(rest[rest.len() - 1].arrival)) };
    let name = l.career.company.as_ref().and_then(|c| c.drivers.iter().find(|d| d.id == driver_id)).map(|d| d.name.clone()).unwrap_or_default();
    let res = l.career.company.as_mut().map(|c| c.send_driver(driver_id, bus.id, &text, &size, &spec, seed));
    match res {
        Some(Ok(secs)) => {
            save(l);
            l.state.set_status(format!("{name} is out on {text} with #{}: back in about {}", bus.fleet_no, staff::span_text(secs)), false);
        }
        Some(Err(e)) => l.state.set_status(e, true),
        None => {}
    }
}

/// Write the session file and start the game on the course.
fn start(l: &mut Launcher, bus: &FleetRow, line: &str, tour: &core::TourInfo, trip_sel: usize) {
    let Some(trip) = tour.trips.get(trip_sel) else { return };
    let Some(c) = l.career.company.as_ref() else { return };
    let Some(owned) = c.fleet.iter().find(|b| b.id == bus.id).cloned() else { return };
    if let Some(why) = owned.drive_block() {
        l.state.set_status(why, true);
        return;
    }
    if owned.depot != c.active_depot {
        l.state.set_status("That bus stands in another depot: work from that depot, or move the bus", true);
        return;
    }
    let home = c.active_map();
    let Some(veh) = vehicle_of(&l.state.vehicles, &owned.file).cloned() else {
        l.state.set_status("That bus is not installed any more", true);
        return;
    };
    let session = Session {
        version: crate::career_session::VERSION,
        id: company::now(),
        bus_id: owned.id,
        fleet_no: owned.fleet_no.clone(),
        bus_file: owned.file.clone(),
        line: line.to_string(),
        tour: tour.number.clone(),
        first_trip: trip.index,
        fuel_start: owned.fuel,
        path: Session::default_path(),
        ..Default::default()
    };
    if let Err(e) = session.save() {
        l.state.set_status(format!("The session could not be prepared: {e}"), true);
        return;
    }
    let mut d = l.state.duty();
    d.map = home;
    d.bus = veh.file.clone();
    d.paint = Some(owned.paint.clone()).filter(|p| !p.is_empty());
    d.plate = Some(owned.plate.clone()).filter(|p| !p.trim().is_empty());
    d.number = Some(owned.fleet_no.clone()).filter(|p| !p.trim().is_empty());
    d.hof = None;
    d.entry = Some(-1);
    d.line = Some(line.to_string());
    d.tour = Some(tour.number.clone());
    d.trip = Some(trip.index.to_string());
    d.whole_tour = true;
    d.schedule = Some(true);
    d.on_foot = Some(false);
    d.lan = Some("off".to_string());
    // (the clock a few minutes before the course leaves: the bus drives to its first stop)
    let start = ((trip.departure - 300.0).max(0.0) / 60.0).floor() as i64;
    d.time = format!("{:02}:{:02}", (start / 60).rem_euclid(24), start % 60);
    d.career = Some(session.path.to_string_lossy().to_string());
    if l.state.launch_duty(d) {
        let label = format!("Line {line} · tour {}", tour.number);
        l.career.jobs.running = Some(Running { bus_id: owned.id, label, started: Instant::now(), seen_game: false, view: Vec::new() });
    }
}

fn running_panel(l: &mut Launcher, r: Rect) {
    let panel = Rect::new(r.x, r.y, r.w.min(720.0), 320.0);
    l.ui.panel(panel);
    let inner = l.ui.heading(Rect::new(panel.x + 20.0, panel.y + 16.0, panel.w - 40.0, panel.h - 32.0), "Session in progress", Some("play_arrow"));
    let (label, view) = l.career.jobs.running.as_ref().map(|x| (x.label.clone(), x.view.clone())).unwrap_or_default();
    let mut y = inner.y;
    l.ui.text_in(&label, Rect::new(inner.x, y, inner.w, 22.0), 16.0, Weight::Bold, TEXT, Align::Left);
    y += 28.0;
    let h = l.ui.paragraph("The game is running. After each course it asks whether to drive the next one or to go back to the garage. When the game closes, the courses driven to their end are paid and booked here.", Vec2::new(inner.x, y), inner.w, 13.0, Weight::Regular, TEXT_DIM);
    y += h + 12.0;
    if view.is_empty() {
        l.ui.text_in("No course driven to its end yet.", Rect::new(inner.x, y, inner.w, 20.0), 13.0, Weight::Regular, TEXT_FAINT, Align::Left);
    }
    for line in view.iter().take(8) {
        l.ui.text_in(line, Rect::new(inner.x, y, inner.w, 20.0), 13.0, Weight::Regular, TEXT_SOFT, Align::Left);
        y += 22.0;
    }
}

fn report_panel(l: &mut Launcher, r: Rect) {
    let lines = l.career.jobs.report.clone().unwrap_or_default();
    let panel = Rect::new(r.x, r.y, r.w.min(640.0), 120.0 + lines.len() as f32 * 24.0 + 70.0);
    l.ui.panel(panel);
    let inner = l.ui.heading(Rect::new(panel.x + 20.0, panel.y + 16.0, panel.w - 40.0, panel.h - 32.0), "Session report", Some("receipt_long"));
    let mut y = inner.y;
    for (i, line) in lines.iter().enumerate() {
        let (px, weight, color) = if i == 0 { (15.0, Weight::Bold, TEXT) } else { (13.5, Weight::Regular, TEXT_SOFT) };
        l.ui.text_in(line, Rect::new(inner.x, y, inner.w, 22.0), px, weight, color, Align::Left);
        y += 24.0;
    }
    y += 12.0;
    if l.ui.button("jobs-report-done", Rect::new(inner.x, y, 200.0, ROW), "Done", Some("check"), ButtonKind::Primary) {
        l.career.jobs.report = None;
    }
}

/// Every frame: notice a session that ended and book it; pick up one that was left unbooked
/// (the launcher was closed while the game ran).
pub(super) fn tick(l: &mut Launcher, dt: f32) {
    if !l.career.jobs.checked_pending {
        l.career.jobs.checked_pending = true;
        // (the company is read now, so a driver's job that ended while the launcher was closed
        // is booked at once)
        ensure_loaded(l);
        pending(l);
    }
    // the drivers' jobs whose time is up
    super::career_staff::collect(l);
    if l.career.jobs.running.is_none() {
        return;
    }
    let in_game = l.state.in_game();
    let mut finished = false;
    l.career.jobs.poll -= dt;
    let poll = l.career.jobs.poll <= 0.0;
    if poll {
        l.career.jobs.poll = 1.0;
    }
    if let Some(run) = l.career.jobs.running.as_mut() {
        if in_game {
            run.seen_game = true;
            if poll {
                if let Some(s) = Session::load(&Session::default_path().to_string_lossy()) {
                    run.view = s
                        .courses
                        .iter()
                        .enumerate()
                        .map(|(i, c)| format!("Course {} · {} → {} · {:.1} km · {} of {} stops", i + 1, c.line, c.terminus, c.km, c.served, c.stops))
                        .collect();
                }
            }
        } else if run.seen_game || run.started.elapsed().as_secs() > 25 {
            finished = true;
        }
    }
    if finished {
        finish(l);
    }
}

/// An unbooked session of an earlier run of the launcher.
fn pending(l: &mut Launcher) {
    let p = Session::default_path();
    if !p.exists() || l.state.in_game() {
        return;
    }
    let Some(s) = Session::load(&p.to_string_lossy()) else { return };
    if s.settled || s.courses.is_empty() {
        return;
    }
    ensure_loaded(l);
    book(l, s);
}

/// The game is gone: read the file and book it.
fn finish(l: &mut Launcher) {
    l.career.jobs.running = None;
    let p = Session::default_path();
    match Session::load(&p.to_string_lossy()) {
        Some(s) if !s.settled && s.courses.is_empty() && !s.ended => {
            let mut s = s;
            s.settled = true;
            let _ = s.save();
            l.state.set_status("The game did not start, or ended before the first course: nothing was booked", true);
        }
        Some(s) if !s.settled => book(l, s),
        _ => l.state.set_status("The session file could not be read: nothing was booked", true),
    }
}

/// Pay a session, write it into the company's books and make the report.
fn book(l: &mut Launcher, mut s: Session) {
    let Some(c) = l.career.company.as_ref() else { return };
    let bus = c.fleet.iter().find(|b| b.id == s.bus_id).cloned();
    let size = bus
        .as_ref()
        .and_then(|b| vehicle_of(&l.state.vehicles, &b.file))
        .and_then(|v| l.career.catalog.as_ref().map(|cat| cat.resolve(v).size))
        .unwrap_or_else(|| "MAXI".to_string());
    let difficulty = l.career.company.as_ref().map(|c| c.difficulty).unwrap_or(1);
    let extras = company::Extras { ticket_cash: s.ticket_cash, fuel_added: s.fuel_added, fuel_burned: s.fuel_burned, fuel_end: s.fuel_end, tank: s.tank };
    let st = settle(&s.courses, &size, s.metres, s.crashes, difficulty, &extras);
    let text = format!("Line {} · tour {}", s.line, s.tour);
    let ok = l.career.company.as_mut().map(|c| c.settle_session(s.bus_id, &st, &text)).unwrap_or(false);
    // the depots' rent and the drivers' retainers come with every session that earned something
    let (rent, retainers) = if ok && st.courses > 0 {
        l.career.company.as_mut().map(|c| {
            let o = c.overheads();
            c.pay_overheads();
            o
        }).unwrap_or((0, 0))
    } else {
        (0, 0)
    };
    s.settled = true;
    let _ = s.save();
    if ok {
        save(l);
    }
    let mut lines = vec![format!("{text} · bus #{}", s.fleet_no)];
    if st.courses == 0 {
        lines.push("No course was driven to its end: no fares.".to_string());
        if st.km > 0.1 {
            lines.push(format!("Distance driven: {:.1} km", st.km));
        }
    } else {
        lines.push(format!("Courses driven to their end: {}", st.courses));
        lines.push(format!("Stops served: {} of {} · late {} · early {}", st.served, st.stops, st.late, st.early));
        lines.push(format!("Distance: {:.1} km (courses {:.1} km)", st.km, st.course_km));
        lines.push(format!("Fares: {}", money(st.fares)));
        if st.streak > 1.0 {
            lines.push(format!("Courses in a row: {} · up to x{:.1} a course, {} more in fares", st.courses, st.streak, money(st.streak_extra)));
        }
        lines.push(format!("Punctuality bonus: {}", money(st.bonus)));
        if st.tickets > 0 {
            lines.push(format!("Tickets sold ({}): {}", s.tickets, money(st.tickets)));
        }
        if st.fuel_cost > 0 {
            lines.push(format!("Fuel filled up on the way: -{}", money(st.fuel_cost)));
        }
        if st.damage > 0 {
            lines.push(format!("Damage from {} crash(es): -{}", st.crashes, money(st.damage)));
        }
        lines.push(format!("Total: {}", money(st.total())));
        if rent + retainers > 0 {
            lines.push(format!("Company costs: depot rent {} · drivers' retainers {}", money(rent), money(retainers)));
        }
        if st.fuel_burned > 0.0 && st.km > 0.5 {
            lines.push(format!("Fuel burned: {:.0} l ({:.1} l/100 km)", st.fuel_burned, st.fuel_burned / st.km * 100.0));
        }
        lines.push(format!("Experience +{} · reputation {:+.2} · the bus lost {:.1} % condition", st.xp, st.reputation, st.wear));
    }
    if !ok {
        lines.push("The bus is no longer in the fleet: only the report is shown.".to_string());
    }
    l.career.jobs.report = Some(lines);
    l.career.sub = super::career::SUB_JOBS;
    l.state.set_status(format!("{text}: session booked, {}", money(st.total())), false);
}
