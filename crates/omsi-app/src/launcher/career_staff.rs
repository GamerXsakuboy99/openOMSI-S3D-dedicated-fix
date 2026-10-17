//! Career Mode, Staff: the drivers the company employs and the people asking for a job.
//!
//! A driver is sent out from the Jobs page (a bus, a line, a tour and the courses of it, as the
//! player would take them); the job is driven in the head (`staff`), lasts as long as the
//! timetable gives the courses and pays when it is over (`Company::collect_jobs`, called every
//! frame from `career_jobs::tick`). Hiring costs money, a driver takes a share of what a job
//! brings (at least a minimum for every course) and a retainer for every session the player
//! books, rests after a shift, and the number of drivers is not limited (career-staff-1).
//! Training raises a skill.

use super::career::{date_time, save};
use super::company::{self, money};
use super::staff::{self, Driver};
use super::theme::*;
use super::ui::ButtonKind;
use super::Launcher;
use glam::Vec2;
use omsi_ui::paint::Align;
use omsi_ui::{Rect, Weight};

#[derive(Default)]
pub struct StaffView {
    /// A driver whose "Dismiss" was pressed once.
    fire_armed: Option<u32>,
}

enum Op {
    Train(u32),
    Fire(u32),
    Arm(u32),
    Hire(usize),
    Reroll,
}

pub(super) fn draw(l: &mut Launcher, r: Rect) {
    let Some(c) = l.career.company.clone() else { return };
    let now = company::now();
    let gap = GAP + 4.0;
    let right_w = (r.w * 0.38).clamp(320.0, 470.0);
    let left = Rect::new(r.x, r.y, (r.w - right_w - gap).max(300.0), r.h);
    let right = Rect::new(left.right() + gap, r.y, r.right() - left.right() - gap, r.h);
    let log_h = (r.h * 0.30).clamp(130.0, 230.0);
    let drivers_p = Rect::new(left.x, left.y, left.w, (left.h - log_h - gap).max(140.0));
    let log_p = Rect::new(left.x, drivers_p.bottom() + gap, left.w, (left.bottom() - drivers_p.bottom() - gap).max(0.0));
    let mut ops: Vec<Op> = Vec::new();

    // --- the drivers
    l.ui.panel(drivers_p);
    let inner = l.ui.heading(Rect::new(drivers_p.x + 16.0, drivers_p.y + 14.0, drivers_p.w - 32.0, drivers_p.h - 28.0), &format!("Drivers · {}", c.drivers.len()), Some("groups"));
    if c.drivers.is_empty() {
        l.ui.paragraph(
            "No drivers yet. A driver takes a job without you at the wheel: their skill, the bus's condition, the weather and their traits decide how it goes. Hire one from the applicants on the right, then send them out on the Jobs page.",
            Vec2::new(inner.x, inner.y + 4.0),
            inner.w,
            13.0,
            Weight::Regular,
            TEXT_DIM,
        );
    }
    let armed = l.career.staff.fire_armed;
    let row_h = 98.0;
    // (the tags' widths are measured before the list is drawn)
    let widths: Vec<Vec<f32>> = c.drivers.iter().map(|d| d.traits.iter().map(|t| l.ui.width(t.label(), 10.0, Weight::Bold) + 10.0).collect()).collect();
    l.ui.scroll_area("staff-list", inner, &mut |ui, v| {
        for (i, d) in c.drivers.iter().enumerate() {
            let rr = Rect::new(v.x, v.y + i as f32 * (row_h + 6.0), v.w - 10.0, row_h);
            if rr.bottom() < inner.y - 4.0 || rr.y > inner.bottom() + 4.0 {
                continue;
            }
            ui.p().rounded(rr, 6.0, FIELD.alpha(0.55));
            ui.p().rounded_border(rr, 6.0, 1.0, EDGE);
            ui.text_in(&d.name, Rect::new(rr.x + 14.0, rr.y + 8.0, rr.w - 330.0, 22.0), 15.0, Weight::Bold, TEXT, Align::Left);
            let name_w = ui.width(&d.name, 15.0, Weight::Bold);
            ui.badge(Vec2::new(rr.x + 20.0 + name_w, rr.y + 11.0), &d.rank().to_uppercase(), ACCENT);
            // the state at the right
            match d.job.as_ref() {
                Some(j) if j.ends > now => {
                    ui.text_in(&format!("{} · {} · {} left", j.text, j.weather.label(), staff::left_text(j.ends - now)), Rect::new(rr.right() - 330.0, rr.y + 8.0, 320.0, 20.0), 12.0, Weight::Medium, ACCENT, Align::Right);
                    let frac = (now.saturating_sub(j.started)) as f32 / (j.ends - j.started).max(1) as f32;
                    ui.progress(Rect::new(rr.right() - 230.0, rr.y + 34.0, 220.0, 5.0), frac, true);
                }
                Some(_) => {
                    ui.text_in("Back, settling up…", Rect::new(rr.right() - 330.0, rr.y + 8.0, 320.0, 20.0), 12.0, Weight::Medium, OK, Align::Right);
                }
                None if d.resting(now) => {
                    ui.text_in(&format!("Resting · {} left", staff::left_text(d.rest_until - now)), Rect::new(rr.right() - 330.0, rr.y + 8.0, 320.0, 20.0), 12.5, Weight::Medium, WARN, Align::Right);
                }
                None => {
                    ui.text_in("Free", Rect::new(rr.right() - 330.0, rr.y + 8.0, 320.0, 20.0), 12.5, Weight::Medium, OK, Align::Right);
                }
            }
            // the traits
            let mut x = rr.x + 14.0;
            let max_x = rr.right() - 240.0;
            for (k, t) in d.traits.iter().enumerate() {
                let w = widths.get(i).and_then(|w| w.get(k)).copied().unwrap_or(80.0);
                if x + w > max_x {
                    break;
                }
                ui.badge(Vec2::new(x, rr.y + 36.0), t.label(), if t.good() { OK } else { WARN });
                ui.tooltip(Rect::new(x, rr.y + 36.0, w, 16.0), t.blurb());
                x += w + 6.0;
            }
            ui.text_in(
                &format!("Skill {:.0} · wage {:.0} % of the takings · retainer {} per session", d.skill, d.share() * 100.0, money(d.retainer())),
                Rect::new(rr.x + 14.0, rr.y + 58.0, rr.w - 14.0 - 280.0, 18.0),
                11.5,
                Weight::Regular,
                TEXT_SOFT,
                Align::Left,
            );
            ui.text_in(&format!("{} job(s), {} course(s) driven", d.jobs, d.courses), Rect::new(rr.x + 14.0, rr.y + 76.0, rr.w - 14.0 - 280.0, 16.0), 11.0, Weight::Regular, TEXT_FAINT, Align::Left);
            // the buttons
            let free = d.job.is_none();
            let train_r = Rect::new(rr.right() - 268.0, rr.y + 60.0, 168.0, 28.0);
            let fire_r = Rect::new(rr.right() - 92.0, rr.y + 60.0, 84.0, 28.0);
            let cost = d.train_cost();
            let tl = format!("Train · {}", money(cost));
            if !free || d.skill >= staff::SKILL_MAX || cost > c.balance {
                ui.button_off(train_r, if d.skill >= staff::SKILL_MAX { "Fully trained" } else { tl.as_str() }, None, ButtonKind::Normal);
            } else if ui.button(&format!("staff-train-{}", d.id), train_r, &tl, None, ButtonKind::Normal) {
                ops.push(Op::Train(d.id));
            }
            if !free {
                ui.button_off(fire_r, "Dismiss", None, ButtonKind::Danger);
            } else if ui.button(&format!("staff-fire-{}", d.id), fire_r, if armed == Some(d.id) { "Sure?" } else { "Dismiss" }, None, ButtonKind::Danger) {
                ops.push(if armed == Some(d.id) { Op::Fire(d.id) } else { Op::Arm(d.id) });
            }
        }
        c.drivers.len() as f32 * (row_h + 6.0)
    });

    // --- the log
    l.ui.panel(log_p);
    let inner = l.ui.heading(Rect::new(log_p.x + 16.0, log_p.y + 14.0, log_p.w - 32.0, log_p.h - 28.0), "What the drivers did", Some("receipt_long"));
    if c.staff_log.is_empty() {
        l.ui.text_in("Nothing yet.", Rect::new(inner.x, inner.y, inner.w, 20.0), 12.5, Weight::Regular, TEXT_FAINT, Align::Left);
    }
    let lines: Vec<(u64, String)> = c.staff_log.iter().rev().map(|e| (e.at, e.text.clone())).collect();
    l.ui.scroll_area("staff-log", Rect::new(inner.x - 4.0, inner.y, inner.w + 8.0, inner.h), &mut |ui, v| {
        for (i, (at, text)) in lines.iter().enumerate() {
            let y = v.y + i as f32 * 24.0;
            if y + 24.0 < inner.y - 4.0 || y > inner.bottom() + 4.0 {
                continue;
            }
            ui.text_in(&date_time(*at), Rect::new(v.x + 4.0, y, 130.0, 22.0), 11.5, Weight::Regular, TEXT_DIM, Align::Left);
            ui.text_in(text, Rect::new(v.x + 140.0, y, v.w - 160.0, 22.0), 12.5, Weight::Regular, TEXT_SOFT, Align::Left);
        }
        lines.len() as f32 * 24.0
    });

    // --- the applicants
    l.ui.panel(right);
    let inner = l.ui.heading(Rect::new(right.x + 16.0, right.y + 14.0, right.w - 32.0, right.h - 28.0), "Applicants", Some("person"));
    let foot_h = ROW + 38.0;
    let list = Rect::new(inner.x, inner.y, inner.w, (inner.h - foot_h).max(80.0));
    let pool = c.applicants();
    let full = false; // [ROLLBACK career-staff-1] hire as many as you like
    let cw = ((inner.w - 10.0) - 150.0).max(80.0);
    let card_h = 112.0;
    let widths: Vec<Vec<f32>> = pool.iter().map(|(_, d)| d.traits.iter().map(|t| l.ui.width(t.label(), 10.0, Weight::Bold) + 10.0).collect()).collect();
    if pool.is_empty() {
        l.ui.paragraph("Nobody is asking for a job now. New applicants come every two days, or ask for some below.", Vec2::new(list.x, list.y + 4.0), list.w, 13.0, Weight::Regular, TEXT_DIM);
    }
    l.ui.scroll_area("staff-applicants", list, &mut |ui, v| {
        for (i, (place, d)) in pool.iter().enumerate() {
            let rr = Rect::new(v.x, v.y + i as f32 * (card_h + 8.0), v.w - 10.0, card_h);
            if rr.bottom() < list.y - 4.0 || rr.y > list.bottom() + 4.0 {
                continue;
            }
            ui.p().rounded(rr, 6.0, FIELD.alpha(0.55));
            ui.p().rounded_border(rr, 6.0, 1.0, EDGE);
            ui.text_in(&d.name, Rect::new(rr.x + 12.0, rr.y + 8.0, cw, 22.0), 14.5, Weight::Bold, TEXT, Align::Left);
            ui.text_in(&format!("Skill {:.0} · {}", d.skill, d.rank()), Rect::new(rr.x + 12.0, rr.y + 30.0, cw, 18.0), 12.0, Weight::Medium, TEXT_SOFT, Align::Left);
            let mut x = rr.x + 12.0;
            let mut y = rr.y + 54.0;
            for (k, t) in d.traits.iter().enumerate() {
                let w = widths.get(i).and_then(|w| w.get(k)).copied().unwrap_or(80.0);
                if x + w > rr.right() - 10.0 {
                    x = rr.x + 12.0;
                    y += 20.0;
                }
                ui.badge(Vec2::new(x, y), t.label(), if t.good() { OK } else { WARN });
                ui.tooltip(Rect::new(x, y, w, 16.0), t.blurb());
                x += w + 6.0;
            }
            ui.text_in(&format!("wage {:.0} % of the takings · retainer {} per session", d.share() * 100.0, money(d.retainer())), Rect::new(rr.x + 12.0, rr.bottom() - 26.0, rr.w - 170.0, 18.0), 11.0, Weight::Regular, TEXT_DIM, Align::Left);
            let cost = staff::hire_cost(d.skill);
            let hb = Rect::new(rr.right() - 148.0, rr.y + 8.0, 138.0, 28.0);
            let hl = format!("Hire · {}", money(cost));
            if full {
                ui.button_off(hb, "Staff is full", None, ButtonKind::Normal);
            } else if cost > c.balance {
                ui.button_off(hb, &hl, None, ButtonKind::Normal);
            } else if ui.button(&format!("staff-hire-{place}"), hb, &hl, Some("check"), ButtonKind::Primary) {
                ops.push(Op::Hire(*place));
            }
        }
        pool.len() as f32 * (card_h + 8.0)
    });
    let foot = Rect::new(inner.x, list.bottom() + 6.0, inner.w, foot_h - 6.0);
    l.ui.text_in(
        &format!("{} driver(s) now, as many as you like · a better reputation draws better applicants", c.drivers.len()),
        Rect::new(foot.x, foot.y, foot.w, 18.0),
        11.5,
        Weight::Regular,
        TEXT_DIM,
        Align::Left,
    );
    let rl = format!("New applicants · {}", money(staff::APPLICANTS_COST));
    let rb = Rect::new(foot.x, foot.y + 24.0, foot.w, ROW);
    if c.balance < staff::APPLICANTS_COST {
        l.ui.button_off(rb, &rl, Some("autorenew"), ButtonKind::Normal);
    } else if l.ui.button("staff-reroll", rb, &rl, Some("autorenew"), ButtonKind::Normal) {
        ops.push(Op::Reroll);
    }

    // --- what was pressed
    if ops.is_empty() {
        return;
    }
    l.career.staff.fire_armed = None;
    for op in ops {
        let res: Option<Result<String, String>> = match op {
            Op::Arm(id) => {
                l.career.staff.fire_armed = Some(id);
                None
            }
            Op::Train(id) => l.career.company.as_mut().map(|c| {
                let name = c.drivers.iter().find(|d| d.id == id).map(|d| d.name.clone()).unwrap_or_default();
                c.train(id).map(|cost| format!("{name} took a training course for {}", money(cost)))
            }),
            Op::Fire(id) => l.career.company.as_mut().map(|c| c.fire(id).map(|n| format!("{n} has left the company"))),
            Op::Hire(place) => l.career.company.as_mut().map(|c| {
                c.hire(place).map(|id| {
                    let d: Option<&Driver> = c.drivers.iter().find(|d| d.id == id);
                    format!("Hired {}", d.map(|d| d.name.as_str()).unwrap_or("a driver"))
                })
            }),
            Op::Reroll => l.career.company.as_mut().map(|c| c.reroll_applicants().map(|_| "New applicants are waiting".to_string())),
        };
        match res {
            Some(Ok(t)) => {
                save(l);
                l.state.set_status(t, false);
            }
            Some(Err(e)) => l.state.set_status(e, true),
            None => {}
        }
    }
}

/// Every frame: book the jobs whose time is up.
pub(super) fn collect(l: &mut Launcher) {
    let Some(c) = l.career.company.as_mut() else { return };
    let lines = c.collect_jobs(company::now());
    if lines.is_empty() {
        return;
    }
    save(l);
    let text = if lines.len() == 1 { lines[0].clone() } else { format!("{} driver jobs finished", lines.len()) };
    l.state.set_status(text, false);
}
