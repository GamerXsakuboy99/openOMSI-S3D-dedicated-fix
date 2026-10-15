//! Career Mode's debug window: Ctrl+Shift+Alt+Backspace (anywhere in the launcher) opens it
//! over the page, the same keys or Escape close it. For trying the economy out - set the
//! money, the level, the reputation and the currency, get new offers for nothing, reset the
//! lists, delete the company. It is a panel over the launcher's window, not a window of its
//! own.

use super::career::{save, SLOT};
use super::company::{self, from_display, money, Company, CURRENCIES, LEVEL_XP};
use super::theme::*;
use super::thumbs;
use super::ui::{ButtonKind, Key};
use super::Launcher;
use glam::Vec2;
use omsi_launcher_lib::catalog::Catalog;
use omsi_ui::paint::Align;
use omsi_ui::{Rect, Weight};

enum Dbg {
    SetBalance(i64),
    Add(i64),
    Level(usize),
    Reputation(f32),
    Currency(usize),
    FreeOffers,
    ResellOffers,
    IgnoreDefaults,
    IgnoreNone,
    ServiceAll,
    FinishJobs,
    FreePlace,
    ClearPictures,
    ReloadCatalog,
    Delete,
    Close,
}

pub(super) fn draw(l: &mut Launcher) {
    let size = l.ui.size;
    let full = Rect::new(0.0, 0.0, size.x, size.y);
    l.ui.solid(full);
    l.ui.p().rect(full, omsi_ui::Color::rgba(0, 0, 0, 0.62));
    let w = (size.x - 48.0).min(640.0);
    let h = (size.y - 48.0).min(700.0);
    let panel = Rect::new((size.x - w) * 0.5, (size.y - h) * 0.5, w, h);
    l.ui.panel(panel);
    let inner = Rect::new(panel.x + 24.0, panel.y + 18.0, panel.w - 48.0, panel.h - 36.0);
    l.ui.icon("tune", Vec2::new(inner.x + 14.0, inner.y + 14.0), 26.0, ACCENT);
    l.ui.text_in("Career debug", Rect::new(inner.x + 38.0, inner.y, inner.w - 200.0, 28.0), 18.0, Weight::Bold, TEXT, Align::Left);
    l.ui.text_in("Ctrl+Shift+Alt+Backspace or Esc closes", Rect::new(inner.x + 38.0, inner.y + 26.0, inner.w - 38.0, 18.0), 11.5, Weight::Regular, TEXT_DIM, Align::Left);

    let mut op: Option<Dbg> = None;
    if l.ui.input.keys.contains(&Key::Escape) {
        op = Some(Dbg::Close);
    }
    let body = Rect::new(inner.x - 6.0, inner.y + 54.0, inner.w + 12.0, inner.h - 54.0 - 46.0);
    let close = Rect::new(inner.right() - 130.0, inner.bottom() - ROW, 130.0, ROW);
    if l.ui.button("dbg-close", close, "Close", None, ButtonKind::Primary) {
        op = Some(Dbg::Close);
    }
    let comp = l.career.company.clone();
    match comp {
        None => {
            l.ui.paragraph("There is no company yet. Found one on the Career Mode page first.", Vec2::new(inner.x, body.y), inner.w, 13.5, Weight::Regular, TEXT_SOFT);
        }
        Some(c) => {
            let cash = &mut l.career.dbg_cash;
            let currencies: Vec<String> = CURRENCIES.iter().map(|c| format!("{} ({})", c.code, c.symbol)).collect();
            l.ui.scroll_area("dbg-body", body, &mut |ui, v| {
                let x = v.x + 6.0;
                let w = v.w - 12.0 - 10.0;
                let mut y = v.y;
                let head = |ui: &mut super::ui::Ui, y: &mut f32, t: &str| {
                    ui.text_in(&t.to_uppercase(), Rect::new(x, *y, w, 16.0), 10.5, Weight::Bold, TEXT_DIM, Align::Left);
                    *y += 20.0;
                };

                head(ui, &mut y, "Money");
                ui.text_in(&format!("Balance: {}", money(c.balance)), Rect::new(x, y, w, 20.0), 14.0, Weight::Bold, TEXT, Align::Left);
                y += 26.0;
                let code = company::currency().code;
                let bw = 150.0;
                ui.text_input("dbg-cash", Rect::new(x, y, w - bw - 10.0, ROW), cash, &format!("amount in {code}"), Some("payments"));
                if ui.button("dbg-set", Rect::new(x + w - bw, y, bw, ROW), "Set balance", None, ButtonKind::Normal) {
                    if let Ok(n) = cash.trim().replace([' ', '\u{a0}'], "").parse::<i64>() {
                        op = Some(Dbg::SetBalance(from_display(n)));
                    }
                }
                y += ROW + 8.0;
                let third = (w - 16.0) / 3.0;
                for (i, amount) in [10_000i64, 100_000, 1_000_000].into_iter().enumerate() {
                    if ui.button(&format!("dbg-add{i}"), Rect::new(x + i as f32 * (third + 8.0), y, third, ROW), &format!("+ {}", money(amount)), None, ButtonKind::Normal) {
                        op = Some(Dbg::Add(amount));
                    }
                }
                y += ROW + 18.0;

                head(ui, &mut y, "Company");
                ui.text_in(&format!("Level {} · {} XP · reputation {:.1}", c.level(), c.xp, c.reputation), Rect::new(x, y, w, 18.0), 12.5, Weight::Regular, TEXT_SOFT, Align::Left);
                y += 24.0;
                let mut lv = c.level() as usize - 1;
                if ui.segmented("dbg-level", Rect::new(x, y, w, ROW), &mut lv, &["Level 1", "2", "3", "4", "5"]) {
                    op = Some(Dbg::Level(lv));
                }
                y += ROW + 8.0;
                let mut rep = (c.reputation.round() as usize).clamp(1, 5) - 1;
                if ui.segmented("dbg-rep", Rect::new(x, y, w, ROW), &mut rep, &["Reputation 1", "2", "3", "4", "5"]) {
                    op = Some(Dbg::Reputation(rep as f32 + 1.0));
                }
                y += ROW + 8.0;
                let mut cur = c.currency as usize;
                if ui.select("dbg-cur", Rect::new(x, y, w.min(300.0), ROW), &mut cur, &currencies) {
                    op = Some(Dbg::Currency(cur));
                }
                y += ROW + 18.0;

                head(ui, &mut y, "Dealership and fleet");
                let half = (w - 8.0) / 2.0;
                if ui.button("dbg-offers", Rect::new(x, y, half, ROW), "New offers (free)", Some("autorenew"), ButtonKind::Normal) {
                    op = Some(Dbg::FreeOffers);
                }
                if ui.button("dbg-resell", Rect::new(x + half + 8.0, y, half, ROW), "Bought offers back", Some("restart_alt"), ButtonKind::Normal) {
                    op = Some(Dbg::ResellOffers);
                }
                y += ROW + 8.0;
                if ui.button("dbg-ign-def", Rect::new(x, y, half, ROW), "Hidden: catalogue's", Some("tune"), ButtonKind::Normal) {
                    op = Some(Dbg::IgnoreDefaults);
                }
                if ui.button("dbg-ign-none", Rect::new(x + half + 8.0, y, half, ROW), "Hidden: none", Some("tune"), ButtonKind::Normal) {
                    op = Some(Dbg::IgnoreNone);
                }
                y += ROW + 8.0;
                if ui.button("dbg-service", Rect::new(x, y, w, ROW), "Service the whole fleet (free)", Some("construction"), ButtonKind::Normal) {
                    op = Some(Dbg::ServiceAll);
                }
                y += ROW + 18.0;

                head(ui, &mut y, "Depots and staff");
                if ui.button("dbg-jobs", Rect::new(x, y, half, ROW), "Finish driver jobs now", Some("check"), ButtonKind::Normal) {
                    op = Some(Dbg::FinishJobs);
                }
                if ui.button("dbg-place", Rect::new(x + half + 8.0, y, half, ROW), "+1 place here (free)", Some("local_parking"), ButtonKind::Normal) {
                    op = Some(Dbg::FreePlace);
                }
                y += ROW + 18.0;

                head(ui, &mut y, "Data");
                if ui.button("dbg-pics", Rect::new(x, y, half, ROW), "Clear pictures", Some("restart_alt"), ButtonKind::Normal) {
                    op = Some(Dbg::ClearPictures);
                }
                if ui.button("dbg-cat", Rect::new(x + half + 8.0, y, half, ROW), "Reload catalogue", Some("refresh"), ButtonKind::Normal) {
                    op = Some(Dbg::ReloadCatalog);
                }
                y += ROW + 18.0;

                head(ui, &mut y, "Reset");
                if ui.button("dbg-delete", Rect::new(x, y, w, ROW), "Delete the company (no question asked)", Some("restart_alt"), ButtonKind::Danger) {
                    op = Some(Dbg::Delete);
                }
                y += ROW + 8.0;
                y - v.y
            });
        }
    }

    let Some(op) = op else { return };
    match op {
        Dbg::Close => l.career.debug_open = false,
        Dbg::SetBalance(n) => {
            if let Some(c) = l.career.company.as_mut() {
                let d = n - c.balance;
                c.book("Debug: balance set", d);
            }
            save(l);
        }
        Dbg::Add(n) => {
            if let Some(c) = l.career.company.as_mut() {
                c.book("Debug: money added", n);
            }
            save(l);
        }
        Dbg::Level(i) => {
            if let Some(c) = l.career.company.as_mut() {
                c.xp = LEVEL_XP[i.min(LEVEL_XP.len() - 1)];
            }
            save(l);
        }
        Dbg::Reputation(r) => {
            if let Some(c) = l.career.company.as_mut() {
                c.reputation = r;
            }
            save(l);
        }
        Dbg::Currency(i) => {
            if let Some(c) = l.career.company.as_mut() {
                c.currency = i.min(CURRENCIES.len() - 1) as u8;
            }
            company::set_currency(i);
            l.career.new_cur = i;
            save(l);
        }
        Dbg::FreeOffers => {
            if let Some(c) = l.career.company.as_mut() {
                c.rerolls += 1;
            }
            save(l);
        }
        Dbg::ResellOffers => {
            if let Some(c) = l.career.company.as_mut() {
                c.sold.clear();
            }
            l.career.cat_rev = l.career.cat_rev.wrapping_add(1);
            save(l);
        }
        Dbg::IgnoreDefaults => {
            let list = l.career.catalog.as_ref().map(|c| c.default_ignored(&l.state.vehicles)).unwrap_or_default();
            if let Some(c) = l.career.company.as_mut() {
                c.ignored = list;
            }
            l.career.ignore_rev += 1;
            save(l);
        }
        Dbg::IgnoreNone => {
            if let Some(c) = l.career.company.as_mut() {
                c.ignored.clear();
            }
            l.career.ignore_rev += 1;
            save(l);
        }
        Dbg::ServiceAll => {
            if let Some(c) = l.career.company.as_mut() {
                for b in c.fleet.iter_mut() {
                    b.condition = 100.0;
                }
            }
            save(l);
        }
        Dbg::FinishJobs => {
            if let Some(c) = l.career.company.as_mut() {
                let lines = c.collect_jobs(u64::MAX);
                l.state.set_status(format!("{} driver job(s) booked", lines.len()), false);
            }
            save(l);
        }
        Dbg::FreePlace => {
            if let Some(c) = l.career.company.as_mut() {
                let id = c.active_depot;
                if let Some(d) = c.depots.iter_mut().find(|d| d.id == id) {
                    d.slots = (d.slots + 1).min(company::economy::DEPOT_MAX_SLOTS);
                }
            }
            save(l);
        }
        Dbg::ClearPictures => {
            let n = thumbs::clear_cache();
            l.icons.retain(|k, _| !k.starts_with("bus:"));
            l.thumbs.reset();
            l.state.set_status(format!("{n} pictures removed"), false);
        }
        Dbg::ReloadCatalog => {
            l.career.catalog = Some(Catalog::load());
            l.career.cat_rev = l.career.cat_rev.wrapping_add(1);
            l.state.set_status("Catalogue read from the file again", false);
        }
        Dbg::Delete => {
            Company::delete(SLOT);
            l.career.company = None;
            l.career.shop = Default::default();
            l.career.depot = Default::default();
            l.career.staff = Default::default();
            l.career.debug_open = false;
            l.state.set_status("Company deleted", false);
        }
    }
}
