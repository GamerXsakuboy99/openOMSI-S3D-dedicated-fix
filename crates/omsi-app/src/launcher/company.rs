//! Career Mode's data: the player's bus company (money, fleet, ledger, level) and the
//! dealership's stock. No drawing here and nothing of the game: `career.rs` is the page.
//!
//! The company is one JSON file per slot in `<data folder>/career/`. The dealer's stock is
//! not stored: it is made from a seed (the real day divided by `STOCK_DAYS`, the company's
//! birth and a "new offers" counter), so the same stock comes back until the seed changes,
//! and what was bought from it stays gone (`sold`).
//!
//! All the money numbers are in the `economy` block so they can be tuned in one place.

use omsi_launcher_lib as core;
use omsi_launcher_lib::catalog::{Catalog, CLASSES, SIZES};
use super::staff::{self, ActiveJob, Driver, LogLine, Spec};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The economy's numbers.
pub mod economy {
    /// Money at the start, by difficulty (Relaxed, Standard, Hardcore).
    pub const START: [i64; 3] = [40_000, 25_000, 12_000];
    /// The dealer's new stock every this many real days.
    pub const STOCK_DAYS: u64 = 2;
    /// Offers of one size and class on the lot, at most (every category the installed buses
    /// have is on it, with this many buses when there are).
    pub const PER_CATEGORY: usize = 6;
    /// Offers of one model at most.
    pub const PER_MODEL: usize = 3;
    /// What "new offers" costs.
    pub const REROLL_COST: i64 = 20;
    /// The SD200 and the SD202 come in dozens of files (every pack has its own) and filled the
    /// lot and the search with them. All of them of one size and class together are drawn as
    /// often as this many ordinary buses (1.0: as one of them): at 0.5 about one stock in four
    /// has one at all, and the rest of the lot is drawn as it was.
    pub const CLUTTER_SHARE: f64 = 0.5;
    /// What a kilometre of a course earns (a MAXI bus), the punctuality bonus on top (share of the
    /// fare), and what a crash costs.
    pub const FARE_PER_KM: f64 = 10.8;
    /// What a stop served earns (a MAXI bus): a city bus earns at its stops.
    pub const FARE_PER_STOP: f64 = 21.0;
    pub const PUNCTUAL_BONUS: f64 = 0.25;
    pub const CRASH_COST: i64 = 200;
    /// Income and costs by difficulty (Relaxed, Standard, Hardcore): the fares and the
    /// punctuality bonus are multiplied by the first, crashes, fuel and the workshop by the
    /// second. Ticket takings are not multiplied.
    pub const INCOME_MULT: [f64; 3] = [1.4, 1.0, 0.6];
    pub const COST_MULT: [f64; 3] = [0.8, 1.0, 1.3];
    /// Diesel, euros a litre.
    pub const FUEL_PRICE: f64 = 1.55;
    /// A bus must be inspected after this many days it was driven.
    pub const INSPECTION_DAYS: u32 = 21;
    pub const INSPECTION_COST: i64 = 150;
    /// Below this condition (%) the inspector may fail the bus.
    pub const INSPECTION_SAFE: f32 = 50.0;
    /// Repairs after a failed inspection: this per missing point of condition, plus a base.
    pub const REPAIR_BASE: i64 = 300;
    pub const REPAIR_PER_POINT: i64 = 40;
    /// Interest per booked session on the loan, by difficulty, and the share of the fares
    /// that goes to repay it.
    pub const LOAN_INTEREST: [f64; 3] = [0.01, 0.02, 0.03];
    pub const LOAN_REPAY_SHARE: f64 = 0.25;
    /// The most the bank lends, by company level.
    pub const LOAN_LIMIT: [i64; 5] = [10_000, 25_000, 60_000, 120_000, 250_000];
    /// A service: condition back up by this much, for this much money.
    pub const SERVICE_GAIN: f32 = 35.0;
    pub const SERVICE_COST: i64 = 420;
    /// Kilometres that wear a bus by one point of condition.
    pub const KM_PER_POINT: f64 = 20.0;
    /// Depots. The company starts with one of this many parking places; more places cost
    /// `slot_price` each, up to `DEPOT_MAX_SLOTS` in one depot.
    pub const DEPOT_START_SLOTS: u32 = 3;
    pub const DEPOT_MAX_SLOTS: u32 = 20;
    /// Depots a company may have at most.
    pub const DEPOT_MAX: usize = 8;
    /// What the next depot costs, by the number of depots the company has (the first is free).
    pub const DEPOT_PRICES: [i64; 7] = [15_000, 30_000, 50_000, 80_000, 120_000, 170_000, 230_000];
    pub fn depot_price(have: usize) -> i64 {
        DEPOT_PRICES[have.saturating_sub(1).min(DEPOT_PRICES.len() - 1)]
    }
    /// What one more parking place costs a depot with this many.
    pub fn slot_price(slots: u32) -> i64 {
        2_500 + 1_000 * slots.saturating_sub(DEPOT_START_SLOTS) as i64
    }
    /// What a depot fetches when it is sold (share of what was paid).
    pub const DEPOT_RESALE: f64 = 0.5;
    /// Rent of every depot beyond the first, per booked session (times the difficulty's costs).
    pub const DEPOT_RENT: i64 = 40;
    /// Moving a bus to another depot (twice that to a depot on another map).
    pub const TRANSFER_COST: i64 = 300;
    /// The share of the experience a driver's job earns against one the player drove.
    pub const DRIVER_XP_SHARE: f64 = 0.5;
    /// The price of a new bus by size and class (euros, whatever currency is shown).
    pub fn base_price(size: &str, class: &str) -> i64 {
        let by_size = match size {
            "MINI" => 35_000.0,
            "MIDI" => 55_000.0,
            "MEGA" => 115_000.0,
            _ => 70_000.0,
        };
        let by_class = match class {
            "Intercity" => 1.15,
            "Long-distance" => 1.6,
            "Double-decker" => 1.5,
            _ => 1.0,
        };
        (by_size * by_class) as i64
    }
    /// What a used bus is worth against a new one: falls steeply in the first years and then
    /// slowly (a ten-year-old bus about a fifth, an eighteen-year-old one under a tenth), never
    /// to nothing.
    pub fn used_factor(age: u16) -> f64 {
        0.04 + 0.96 * (-(age as f64) / 6.0).exp()
    }
    /// The kilometres a bus of this class does in a year.
    pub fn km_per_year(class: &str) -> f64 {
        match class {
            "Intercity" => 70_000.0,
            "Long-distance" => 95_000.0,
            "Double-decker" => 45_000.0,
            "Minibus" => 50_000.0,
            _ => 55_000.0,
        }
    }
    /// The most a bus is credited with (it has had an engine and a gearbox since).
    pub const KM_MAX: f64 = 1_400_000.0;
    /// What the dealer pays for a bus of the company: its price when bought, less its wear.
    pub fn resale(bought: i64, condition: f32) -> i64 {
        let f = 0.30 + 0.45 * (condition as f64 / 100.0).clamp(0.0, 1.0);
        (bought as f64 * f / 10.0).round() as i64 * 10
    }
}

/// Experience needed for each level (1-based): level n is reached at `LEVEL_XP[n-1]`.
pub const LEVEL_XP: [u32; 5] = [0, 800, 2_500, 6_000, 14_000];
pub const LEVEL_NAMES: [&str; 5] = ["Sole driver", "Local subcontractor", "Regional carrier", "City operator", "Transport group"];

pub fn level_name(level: u32) -> &'static str {
    LEVEL_NAMES[(level.clamp(1, 5) - 1) as usize]
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct OwnedBus {
    pub id: u32,
    pub file: String,
    /// The paint ("" the bus's own).
    pub paint: String,
    pub fleet_no: String,
    pub plate: String,
    pub price: i64,
    /// 0..100.
    pub condition: f32,
    pub km: f64,
    pub used: bool,
    pub bought: u64,
    /// The year it was built (0 unknown).
    #[serde(default)]
    pub year: u16,
    /// Litres in the tank (none: not known yet - a full tank), the tank's size (0 unknown).
    #[serde(default)]
    pub fuel: Option<f32>,
    #[serde(default)]
    pub tank: f32,
    /// Litres burned in the sessions where the fuel was measured, and the kilometres of those.
    #[serde(default)]
    pub fuel_burned: f64,
    #[serde(default)]
    pub fuel_km: f64,
    /// Days driven since the last inspection, and whether the last one was failed (the bus
    /// stays in the yard until it is repaired).
    #[serde(default)]
    pub days_since_inspection: u32,
    #[serde(default)]
    pub inspection_failed: bool,
    #[serde(default)]
    pub inspections: u32,
    /// The depot it stands in (`Depot::id`; 0: not known yet - the first depot).
    #[serde(default)]
    pub depot: u32,
    /// Out on a driver's job until this time (seconds since 1970; 0: not).
    #[serde(default)]
    pub away_until: u64,
}

/// A place the company keeps its buses: on a map, with a number of parking places. The
/// jobs of the player start from the active one, on its map.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Depot {
    pub id: u32,
    pub name: String,
    /// The map file the depot is on.
    pub map: String,
    pub slots: u32,
    /// What the depot cost (euros; 0 for the first), what a sale pays half of.
    #[serde(default)]
    pub paid: i64,
    #[serde(default)]
    pub opened: u64,
}

impl OwnedBus {
    /// Why this bus cannot go out (None: it can).
    pub fn drive_block(&self) -> Option<&'static str> {
        if self.away_until > now() {
            return Some("Out on a job with a driver");
        }
        if self.inspection_failed {
            return Some("Failed the inspection: repair it first");
        }
        if self.days_since_inspection >= economy::INSPECTION_DAYS {
            return Some("Inspection due");
        }
        if self.condition < 5.0 {
            return Some("Worn out: repair it first");
        }
        if let (Some(f), true) = (self.fuel, self.tank > 0.0) {
            if f < self.tank * 0.05 {
                return Some("Out of fuel: refuel it first");
            }
        }
        None
    }

    /// Days of driving left before the inspection.
    pub fn inspection_in(&self) -> u32 {
        economy::INSPECTION_DAYS.saturating_sub(self.days_since_inspection)
    }

    /// Litres per 100 km over the measured sessions (None: not enough yet).
    pub fn consumption(&self) -> Option<f64> {
        (self.fuel_km >= 2.0 && self.fuel_burned > 0.0).then(|| self.fuel_burned / self.fuel_km * 100.0)
    }
}

/// Whether the inspector passes a bus of this condition. From 50 % up it always does; below,
/// the chance falls with the condition (`roll` is 0..1).
pub fn inspection_passes(condition: f32, roll: f64) -> bool {
    if condition >= economy::INSPECTION_SAFE {
        return true;
    }
    let p = (condition / economy::INSPECTION_SAFE * 0.9).clamp(0.0, 0.9) as f64;
    roll < p
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Entry {
    pub at: u64,
    pub text: String,
    pub amount: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Company {
    pub version: u32,
    pub name: String,
    /// The map file the company starts on.
    pub home_map: String,
    /// 0 relaxed, 1 standard, 2 hardcore.
    pub difficulty: u8,
    pub created: u64,
    pub balance: i64,
    pub xp: u32,
    pub reputation: f32,
    pub next_id: u32,
    pub fleet: Vec<OwnedBus>,
    pub ledger: Vec<Entry>,
    /// "New offers" bought so far (part of the stock's seed).
    #[serde(default)]
    pub rerolls: u32,
    /// Offers of the current stock already bought, and the seed they belong to.
    #[serde(default)]
    pub sold: Vec<u64>,
    #[serde(default)]
    pub sold_seed: u64,
    /// The money is shown in this currency (`CURRENCIES`); the numbers are euros.
    #[serde(default)]
    pub currency: u8,
    /// What is owed to the bank (euros).
    #[serde(default)]
    pub loan: i64,
    /// The models (by the names their files give, `catalog::type_key_of`) the dealership
    /// leaves out.
    #[serde(default)]
    pub ignored: Vec<String>,
    /// The depots, the one the player works from (`Depot::id`) and the next id.
    #[serde(default)]
    pub depots: Vec<Depot>,
    #[serde(default)]
    pub active_depot: u32,
    #[serde(default)]
    pub next_depot: u32,
    /// The drivers, the next id, the log of what they did, and the applicants: asked for so
    /// many times (part of their seed), those of the current pool already hired (by place), and
    /// the seed that pool belongs to.
    #[serde(default)]
    pub drivers: Vec<Driver>,
    #[serde(default)]
    pub next_driver: u32,
    #[serde(default)]
    pub staff_log: Vec<LogLine>,
    #[serde(default)]
    pub applicant_rerolls: u32,
    #[serde(default)]
    pub applicant_taken: Vec<usize>,
    #[serde(default)]
    pub applicant_seed: u64,
}

pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub fn slot_path(slot: usize) -> PathBuf {
    core::data_dir().join("career").join(format!("slot{slot}.json"))
}

/// A currency the money can be shown in. The company's numbers are euros; a currency only
/// changes how they are written, at a fixed rate (rounded to ten, or a hundred for the
/// forint, once they are big, so a price reads like a price).
pub struct Currency {
    pub code: &'static str,
    pub symbol: &'static str,
    /// The symbol after the number ("48 250 zł") or before it ("€ 48 250").
    pub after: bool,
    /// Units per euro.
    pub rate: f64,
}

pub const CURRENCIES: [Currency; 8] = [
    Currency { code: "EUR", symbol: "€", after: false, rate: 1.0 },
    Currency { code: "PLN", symbol: "zł", after: true, rate: 4.30 },
    Currency { code: "USD", symbol: "$", after: false, rate: 1.08 },
    Currency { code: "GBP", symbol: "£", after: false, rate: 0.85 },
    Currency { code: "CZK", symbol: "Kč", after: true, rate: 25.0 },
    Currency { code: "CHF", symbol: "CHF", after: false, rate: 0.95 },
    Currency { code: "SEK", symbol: "kr", after: true, rate: 11.5 },
    Currency { code: "HUF", symbol: "Ft", after: true, rate: 395.0 },
];

static CURRENT: AtomicUsize = AtomicUsize::new(0);

/// The currency the money is shown in from now on.
pub fn set_currency(i: usize) {
    CURRENT.store(i.min(CURRENCIES.len() - 1), Ordering::Relaxed);
}

pub fn currency() -> &'static Currency {
    &CURRENCIES[CURRENT.load(Ordering::Relaxed).min(CURRENCIES.len() - 1)]
}

/// Euros in the shown currency, rounded the way prices are.
pub fn to_display(euros: i64) -> i64 {
    let c = currency();
    if c.rate == 1.0 {
        return euros;
    }
    let x = (euros as f64 * c.rate).round() as i64;
    if x.abs() >= 10_000 {
        let step = if c.rate >= 100.0 { 100 } else { 10 };
        (x as f64 / step as f64).round() as i64 * step
    } else {
        x
    }
}

/// An amount typed in the shown currency, in euros.
pub fn from_display(shown: i64) -> i64 {
    (shown as f64 / currency().rate).round() as i64
}

/// 48250 (euros) -> "€ 48 250", or "207 480 zł" in zloty; -300 -> "- € 300".
pub fn money(v: i64) -> String {
    let c = currency();
    let shown = to_display(v);
    let digits = shown.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(' ');
        }
        out.push(ch);
    }
    let body = if c.after { format!("{out} {}", c.symbol) } else { format!("{} {out}", c.symbol) };
    if shown < 0 {
        format!("- {body}")
    } else {
        body
    }
}

/// The year now (the real calendar's).
pub fn year_now() -> u16 {
    (1970 + now() / 31_556_952) as u16
}

impl Company {
    pub fn new(name: &str, home_map: &str, difficulty: u8, currency: u8, ignored: Vec<String>) -> Company {
        let d = (difficulty as usize).min(2);
        let mut c = Company {
            version: 2,
            name: name.trim().to_string(),
            home_map: home_map.to_string(),
            difficulty: d as u8,
            created: now(),
            balance: 0,
            xp: 0,
            reputation: 3.0,
            next_id: 1,
            fleet: Vec::new(),
            ledger: Vec::new(),
            rerolls: 0,
            sold: Vec::new(),
            sold_seed: 0,
            currency: currency.min(CURRENCIES.len() as u8 - 1),
            loan: 0,
            ignored,
            depots: Vec::new(),
            active_depot: 0,
            next_depot: 1,
            drivers: Vec::new(),
            next_driver: 1,
            staff_log: Vec::new(),
            applicant_rerolls: 0,
            applicant_taken: Vec::new(),
            applicant_seed: 0,
        };
        c.ensure_depots();
        c.book("Company founded", economy::START[d]);
        c
    }

    pub fn load(slot: usize) -> Option<Company> {
        let text = std::fs::read_to_string(slot_path(slot)).ok()?;
        let mut c: Company = serde_json::from_str(&text).ok()?;
        c.ensure_depots();
        set_currency(c.currency as usize);
        Some(c)
    }

    /// A company of an earlier version has no depots: it gets the one on its home map, with
    /// room for the buses it has, and its buses stand in it. Also makes sure the active depot
    /// and the buses' depots are ones that exist.
    pub fn ensure_depots(&mut self) {
        if self.depots.is_empty() {
            let id = self.next_depot.max(1);
            self.depots.push(Depot { id, name: "Home depot".to_string(), map: self.home_map.clone(), slots: economy::DEPOT_START_SLOTS.max(self.fleet.len() as u32), paid: 0, opened: self.created });
            self.next_depot = id + 1;
            self.active_depot = id;
        }
        if !self.depots.iter().any(|d| d.id == self.active_depot) {
            self.active_depot = self.depots[0].id;
        }
        let first = self.depots[0].id;
        let ids: Vec<u32> = self.depots.iter().map(|d| d.id).collect();
        for b in &mut self.fleet {
            if !ids.contains(&b.depot) {
                b.depot = first;
            }
        }
        if self.next_driver == 0 {
            self.next_driver = self.drivers.iter().map(|d| d.id).max().unwrap_or(0) + 1;
        }
        if self.next_depot <= self.depots.iter().map(|d| d.id).max().unwrap_or(0) {
            self.next_depot = self.depots.iter().map(|d| d.id).max().unwrap_or(0) + 1;
        }
    }

    pub fn save(&self, slot: usize) -> std::io::Result<()> {
        let path = slot_path(slot);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path)
    }

    pub fn delete(slot: usize) {
        let _ = std::fs::remove_file(slot_path(slot));
    }

    /// The level the experience comes to (1..=5).
    pub fn level(&self) -> u32 {
        LEVEL_XP.iter().rposition(|&x| self.xp >= x).map(|i| i as u32 + 1).unwrap_or(1)
    }

    /// Share of the way from this level to the next (1.0 at the top) and the experience the
    /// next one needs.
    pub fn level_progress(&self) -> (f32, Option<u32>) {
        let l = self.level() as usize;
        if l >= LEVEL_XP.len() {
            return (1.0, None);
        }
        let (a, b) = (LEVEL_XP[l - 1], LEVEL_XP[l]);
        (((self.xp - a) as f32 / (b - a).max(1) as f32).clamp(0.0, 1.0), Some(b))
    }

    /// Write a line in the ledger and change the balance by `amount`.
    pub fn book(&mut self, text: &str, amount: i64) {
        self.balance += amount;
        self.ledger.push(Entry { at: now(), text: text.to_string(), amount });
        // (the last 500 lines are enough to look back on)
        if self.ledger.len() > 500 {
            let cut = self.ledger.len() - 500;
            self.ledger.drain(..cut);
        }
    }

    pub fn owned(&self, file: &str) -> usize {
        let k = core::catalog::file_key(file);
        self.fleet.iter().filter(|b| core::catalog::file_key(&b.file) == k).count()
    }

    /// The seed of the dealer's stock now.
    pub fn stock_seed(&self) -> u64 {
        mix(now() / (economy::STOCK_DAYS * 86_400) ^ mix(self.created) ^ mix(self.rerolls as u64 + 1))
    }

    /// Whether the dealership leaves this model out.
    pub fn is_ignored(&self, type_key: &str) -> bool {
        self.ignored.iter().any(|k| k == type_key)
    }

    pub fn set_ignored(&mut self, type_key: &str, ignored: bool) {
        let at = self.ignored.iter().position(|k| k == type_key);
        match (ignored, at) {
            (true, None) => self.ignored.push(type_key.to_string()),
            (false, Some(i)) => {
                self.ignored.remove(i);
            }
            _ => {}
        }
    }

    /// Buy an offer, in the paint it comes in. The fleet number and plate are the first free
    /// of the bus's own list.
    pub fn buy(&mut self, offer: &Offer, numbers: &[(String, String)], seed: u64) -> Result<(), String> {
        let price = offer.price;
        if price > self.balance {
            return Err(format!("Not enough money: {} needed, {} in the bank", money(price), money(self.balance)));
        }
        if let Some(why) = self.parking_block(self.active_depot) {
            return Err(why);
        }
        if self.sold_seed != seed {
            self.sold.clear();
            self.sold_seed = seed;
        }
        if self.sold.contains(&offer.id) {
            return Err("That bus has been sold already".to_string());
        }
        let taken: Vec<String> = self.fleet.iter().filter(|b| b.file == offer.file).map(|b| b.fleet_no.clone()).collect();
        let (fleet_no, plate) = numbers
            .iter()
            .find(|(n, _)| !taken.iter().any(|t| t == n))
            .map(|(n, p)| (n.trim().to_string(), p.trim().to_string()))
            .unwrap_or_else(|| (format!("{}", 100 + self.next_id), String::new()));
        let bus = OwnedBus {
            id: self.next_id,
            file: offer.file.clone(),
            paint: offer.paint.clone(),
            fleet_no,
            plate,
            price,
            condition: (100 - offer.wear as i32).clamp(1, 100) as f32,
            km: offer.km as f64,
            used: offer.used,
            bought: now(),
            year: offer.year,
            depot: self.active_depot,
            ..Default::default()
        };
        self.next_id += 1;
        self.sold.push(offer.id);
        let what = format!("Bought {} {} ({})", offer.model, offer.variant, if offer.used { "used" } else { "new" });
        self.fleet.push(bus);
        self.book(&what, -price);
        Ok(())
    }

    /// Sell a bus of the fleet; returns what it fetched.
    pub fn sell(&mut self, id: u32) -> Result<i64, String> {
        let i = self.fleet.iter().position(|b| b.id == id).ok_or("No such bus")?;
        if self.fleet[i].away_until > now() {
            return Err("That bus is out on a job: sell it when it is back".to_string());
        }
        let b = self.fleet.remove(i);
        let v = economy::resale(b.price, b.condition);
        self.book(&format!("Sold bus #{}", b.fleet_no), v);
        Ok(v)
    }
}

// --- settling a session ------------------------------------------------------------------

/// What a session came to: the courses driven to their end, paid; the kilometres and the
/// crashes wear the bus.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Settlement {
    pub courses: u32,
    pub stops: u32,
    pub served: u32,
    pub late: u32,
    pub early: u32,
    pub crashes: i32,
    /// Kilometres of the courses, and of the whole session (the bus's odometer grows by this).
    pub course_km: f64,
    pub km: f64,
    /// Euros: the fares, the punctuality bonus, the crashes' cost. The fares include what the
    /// series of courses added (`streak_extra`).
    pub fares: i64,
    /// The most a course paid against the plain fare (1: no series), and the money it added.
    pub streak: f64,
    pub streak_extra: i64,
    pub bonus: i64,
    /// Ticket takings (not multiplied by the difficulty).
    pub tickets: i64,
    pub damage: i64,
    /// What the fuel filled up on the way cost.
    pub fuel_cost: i64,
    pub fuel_burned: f64,
    pub fuel_end: Option<f32>,
    pub tank: f32,
    pub xp: u32,
    /// Points of condition the bus lost, and the reputation change.
    pub wear: f32,
    pub reputation: f32,
}

impl Settlement {
    pub fn total(&self) -> i64 {
        self.fares + self.bonus + self.tickets - self.damage - self.fuel_cost
    }
}

/// What the game reports besides the courses.
#[derive(Clone, Copy, Debug, Default)]
pub struct Extras {
    /// Ticket takings of the session (euros).
    pub ticket_cash: f64,
    /// Litres filled up on the way, burned, left in the tank at the end, and the tank's size.
    pub fuel_added: f64,
    pub fuel_burned: f64,
    pub fuel_end: Option<f32>,
    pub tank: f32,
}

/// Pay for a session of a bus of this size: `courses` are the ones driven to their end,
/// `metres` the whole session's distance (0: the courses' own), `crashes` the number of crashes.
/// A course pays for its stops served and its kilometres (less when stops were left out); the
/// bonus is a quarter more for a punctual one. The difficulty (0 relaxed, 1 standard, 2
/// hardcore) multiplies the fares and the costs. Only the courses are paid: the way to the
/// first stop and back to the garage wears the bus and earns nothing.
pub fn settle(courses: &[crate::career_session::Course], size: &str, metres: f64, crashes: i32, difficulty: u8, extra: &Extras) -> Settlement {
    settle_with(courses, size, metres, crashes, difficulty, extra, true)
}

/// `settle`, with the pay for courses in a row (`streak`) on or off. The player's sessions have
/// it; a driver's jobs do not (the series rewards the player for staying at the wheel).
pub fn settle_with(courses: &[crate::career_session::Course], size: &str, metres: f64, crashes: i32, difficulty: u8, extra: &Extras, streak: bool) -> Settlement {
    let mult = match size {
        "MINI" => 0.7,
        "MIDI" => 0.85,
        "MEGA" => 1.3,
        _ => 1.0,
    };
    let d = (difficulty as usize).min(2);
    let (income, cost) = (economy::INCOME_MULT[d], economy::COST_MULT[d]);
    let mut s = Settlement { crashes, streak: 1.0, ..Default::default() };
    let (mut fares, mut bonus, mut punct, mut series_extra) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for (k, c) in courses.iter().enumerate() {
        s.courses += 1;
        s.stops += c.stops;
        s.served += c.served;
        s.late += c.late;
        s.early += c.early;
        s.course_km += c.km;
        let done = if c.stops == 0 { 0.0 } else { c.served as f64 / c.stops as f64 };
        let plain = (c.km * economy::FARE_PER_KM * done + c.served as f64 * economy::FARE_PER_STOP) * mult * income;
        // [ROLLBACK careerpay-1] the courses driven one after another pay more each
        let series = if streak { crate::career_session::streak_mult(k + 1) } else { 1.0 };
        let fare = plain * series;
        series_extra += fare - plain;
        s.streak = s.streak.max(series);
        let off = if c.served == 0 { 1.0 } else { ((c.late + c.early) as f64 / c.served as f64).min(1.0) };
        fares += fare;
        bonus += fare * economy::PUNCTUAL_BONUS * (1.0 - off);
        punct += (1.0 - off) * c.served as f64;
    }
    s.fares = fares.round() as i64;
    s.streak_extra = series_extra.round() as i64;
    s.bonus = bonus.round() as i64;
    s.tickets = extra.ticket_cash.max(0.0).round() as i64;
    s.damage = (crashes.max(0) as f64 * economy::CRASH_COST as f64 * cost).round() as i64;
    s.fuel_cost = (extra.fuel_added.max(0.0) * economy::FUEL_PRICE * cost).round() as i64;
    s.fuel_burned = extra.fuel_burned.max(0.0);
    s.fuel_end = extra.fuel_end;
    s.tank = extra.tank;
    s.km = (metres / 1000.0).max(s.course_km);
    s.xp = (s.course_km * 8.0 + s.served as f64 * 3.0).round() as u32;
    s.wear = (s.km / economy::KM_PER_POINT) as f32 + crashes.max(0) as f32 * 5.0;
    // the punctual share of the stops served moves the reputation up, a crash moves it down
    let share = if s.served == 0 { 0.0 } else { punct / s.served as f64 };
    s.reputation = if s.courses == 0 { 0.0 } else { (share as f32 - 0.6) * 0.2 * s.courses as f32 - crashes.max(0) as f32 * 0.25 };
    s
}

impl Company {
    fn diff(&self) -> usize {
        (self.difficulty as usize).min(2)
    }

    /// Book a settled session on bus `bus_id`: the money, the experience, the reputation, and
    /// the bus's kilometres, condition, fuel and days driven; the loan's interest and its
    /// share of the fares. Returns false when the bus is no longer there.
    pub fn settle_session(&mut self, bus_id: u32, s: &Settlement, text: &str) -> bool {
        let d = self.diff();
        let Some(b) = self.fleet.iter_mut().find(|b| b.id == bus_id) else { return false };
        b.km += s.km;
        b.condition = (b.condition - s.wear).clamp(1.0, 100.0);
        if s.tank > 0.0 {
            b.tank = s.tank.max(b.tank);
        }
        if let Some(f) = s.fuel_end {
            b.fuel = Some(f.max(0.0));
        }
        if s.fuel_burned > 0.0 {
            b.fuel_burned += s.fuel_burned;
            b.fuel_km += s.km;
        }
        if s.km > 0.5 {
            b.days_since_inspection += 1;
        }
        self.xp += s.xp;
        self.reputation = (self.reputation + s.reputation).clamp(0.0, 5.0);
        let income = s.fares + s.bonus + s.tickets;
        if income != 0 {
            self.book(&format!("{text}: fares and tickets"), income);
        }
        if s.damage != 0 {
            self.book(&format!("{text}: damage"), -s.damage);
        }
        if s.fuel_cost != 0 {
            self.book(&format!("{text}: fuel"), -s.fuel_cost);
        }
        if self.loan > 0 {
            let interest = (self.loan as f64 * economy::LOAN_INTEREST[d]).ceil() as i64;
            self.book("Loan interest", -interest);
            let repay = ((s.fares + s.bonus).max(0) as f64 * economy::LOAN_REPAY_SHARE).round() as i64;
            let repay = repay.min(self.loan);
            if repay > 0 {
                self.loan -= repay;
                self.book("Loan repayment", -repay);
            }
        }
        true
    }

    /// The most the bank lends this company.
    pub fn loan_limit(&self) -> i64 {
        economy::LOAN_LIMIT[(self.level().clamp(1, 5) - 1) as usize]
    }

    pub fn take_loan(&mut self, amount: i64) -> Result<(), String> {
        let room = self.loan_limit() - self.loan;
        if amount <= 0 || amount > room {
            return Err(format!("The bank lends up to {} more", money(room.max(0))));
        }
        self.loan += amount;
        self.book("Loan taken", amount);
        Ok(())
    }

    pub fn repay_loan(&mut self, amount: i64) -> Result<i64, String> {
        let pay = amount.min(self.loan).min(self.balance);
        if pay <= 0 {
            return Err("Nothing to repay, or no money for it".to_string());
        }
        self.loan -= pay;
        self.book("Loan repayment", -pay);
        Ok(pay)
    }

    /// The periodic inspection: costs money, passes by the condition and a roll (0..1). Fails
    /// leave the bus in the yard until it is repaired. Returns whether it passed.
    pub fn inspect_bus(&mut self, bus_id: u32, roll: f64) -> Result<bool, String> {
        let cost = (economy::INSPECTION_COST as f64 * economy::COST_MULT[self.diff()]).round() as i64;
        if self.balance < cost {
            return Err("Not enough money for the inspection".to_string());
        }
        let b = self.fleet.iter_mut().find(|b| b.id == bus_id).ok_or("No such bus")?;
        if b.away_until > now() {
            return Err("The bus is out on a job".to_string());
        }
        if b.inspection_failed {
            return Err("It failed the last inspection: repair it first".to_string());
        }
        let ok = inspection_passes(b.condition, roll);
        let no = b.fleet_no.clone();
        if ok {
            b.days_since_inspection = 0;
            b.inspections += 1;
        } else {
            b.inspection_failed = true;
        }
        self.book(&format!("Inspection of #{no}: {}", if ok { "passed" } else { "failed" }), -cost);
        Ok(ok)
    }

    /// What repairs to the safe condition cost (the workshop after a failed inspection, or a
    /// service by choice: it brings the condition up to 70 %).
    pub fn repair_cost(&self, bus_id: u32) -> i64 {
        let Some(b) = self.fleet.iter().find(|b| b.id == bus_id) else { return 0 };
        let missing = (70.0 - b.condition).max(0.0) as i64;
        ((economy::REPAIR_BASE + missing * economy::REPAIR_PER_POINT) as f64 * economy::COST_MULT[self.diff()]).round() as i64
    }

    /// Repair: condition up to at least 70 %, the failed inspection cleared.
    pub fn repair_bus(&mut self, bus_id: u32) -> Result<i64, String> {
        let cost = self.repair_cost(bus_id);
        if self.balance < cost {
            return Err("Not enough money for the repairs".to_string());
        }
        let b = self.fleet.iter_mut().find(|b| b.id == bus_id).ok_or("No such bus")?;
        if b.away_until > now() {
            return Err("The bus is out on a job".to_string());
        }
        b.condition = b.condition.max(70.0);
        b.inspection_failed = false;
        let no = b.fleet_no.clone();
        self.book(&format!("Repairs of #{no}"), -cost);
        Ok(cost)
    }

    /// Fill the tank at the depot; returns what it cost (the level is not known before the
    /// first session).
    pub fn refuel_bus(&mut self, bus_id: u32) -> Result<i64, String> {
        let k = economy::COST_MULT[self.diff()];
        let b = self.fleet.iter().find(|b| b.id == bus_id).ok_or("No such bus")?;
        if b.away_until > now() {
            return Err("The bus is out on a job".to_string());
        }
        if b.tank <= 0.0 {
            return Err("The tank is full - its size is learned on the first drive".to_string());
        }
        let missing = (b.tank - b.fuel.unwrap_or(b.tank)).max(0.0) as f64;
        let cost = (missing * economy::FUEL_PRICE * k).round() as i64;
        if missing < 0.5 {
            return Err("The tank is full".to_string());
        }
        if self.balance < cost {
            return Err("Not enough money for the fuel".to_string());
        }
        let b = self.fleet.iter_mut().find(|b| b.id == bus_id).ok_or("No such bus")?;
        b.fuel = Some(b.tank);
        let no = b.fleet_no.clone();
        self.book(&format!("Fuel for #{no}"), -cost);
        Ok(cost)
    }
}

// --- depots ---------------------------------------------------------------------------------

impl Company {
    pub fn depot(&self, id: u32) -> Option<&Depot> {
        self.depots.iter().find(|d| d.id == id)
    }

    /// The depot the player works from.
    pub fn active(&self) -> Option<&Depot> {
        self.depot(self.active_depot).or(self.depots.first())
    }

    /// The map the player's jobs run on: the active depot's.
    pub fn active_map(&self) -> String {
        self.active().map(|d| d.map.clone()).unwrap_or_else(|| self.home_map.clone())
    }

    /// Buses standing in a depot.
    pub fn parked(&self, depot: u32) -> usize {
        self.fleet.iter().filter(|b| b.depot == depot).count()
    }

    pub fn free_places(&self, depot: u32) -> u32 {
        self.depot(depot).map(|d| d.slots.saturating_sub(self.parked(depot) as u32)).unwrap_or(0)
    }

    /// Why nothing more can be parked in this depot (None: there is room).
    pub fn parking_block(&self, depot: u32) -> Option<String> {
        let Some(d) = self.depot(depot) else { return Some("No such depot".to_string()) };
        let n = self.parked(depot);
        (n as u32 >= d.slots).then(|| format!("The depot \"{}\" is full ({n} of {} places): buy a place, move a bus or sell one", d.name, d.slots))
    }

    pub fn set_active_depot(&mut self, id: u32) -> Result<(), String> {
        if self.depot(id).is_none() {
            return Err("No such depot".to_string());
        }
        self.active_depot = id;
        Ok(())
    }

    pub fn rename_depot(&mut self, id: u32, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        if let Some(d) = self.depots.iter_mut().find(|d| d.id == id) {
            d.name = name.chars().take(32).collect();
        }
    }

    /// What the next depot costs (Err: the company has all it may).
    pub fn next_depot_price(&self) -> Result<i64, String> {
        if self.depots.len() >= economy::DEPOT_MAX {
            return Err(format!("A company has at most {} depots", economy::DEPOT_MAX));
        }
        Ok(economy::depot_price(self.depots.len()))
    }

    /// Open a depot on `map` (the player makes it the active one themselves).
    pub fn buy_depot(&mut self, name: &str, map: &str) -> Result<u32, String> {
        let price = self.next_depot_price()?;
        if map.trim().is_empty() {
            return Err("Choose a map for the depot".to_string());
        }
        if price > self.balance {
            return Err(format!("Not enough money: {} needed, {} in the bank", money(price), money(self.balance)));
        }
        let id = self.next_depot.max(self.depots.iter().map(|d| d.id).max().unwrap_or(0) + 1);
        self.next_depot = id + 1;
        let name: String = if name.trim().is_empty() { format!("Depot {}", self.depots.len() + 1) } else { name.trim().chars().take(32).collect() };
        self.depots.push(Depot { id, name: name.clone(), map: map.to_string(), slots: economy::DEPOT_START_SLOTS, paid: price, opened: now() });
        self.book(&format!("Opened depot \"{name}\""), -price);
        Ok(id)
    }

    /// One more parking place in a depot; returns what it cost.
    pub fn buy_slot(&mut self, id: u32) -> Result<i64, String> {
        let d = self.depot(id).ok_or("No such depot")?;
        if d.slots >= economy::DEPOT_MAX_SLOTS {
            return Err(format!("A depot holds at most {} buses", economy::DEPOT_MAX_SLOTS));
        }
        let price = economy::slot_price(d.slots);
        if price > self.balance {
            return Err(format!("Not enough money: {} needed, {} in the bank", money(price), money(self.balance)));
        }
        let name = d.name.clone();
        if let Some(d) = self.depots.iter_mut().find(|d| d.id == id) {
            d.slots += 1;
        }
        self.book(&format!("Parking place in \"{name}\""), -price);
        Ok(price)
    }

    /// Sell an empty depot (not the last one); returns what it fetched (the places are not paid back).
    pub fn sell_depot(&mut self, id: u32) -> Result<i64, String> {
        if self.depots.len() <= 1 {
            return Err("The company's last depot cannot be sold".to_string());
        }
        let i = self.depots.iter().position(|d| d.id == id).ok_or("No such depot")?;
        if self.parked(id) > 0 {
            return Err("Move the buses out of the depot first".to_string());
        }
        let d = self.depots.remove(i);
        let got = (d.paid as f64 * economy::DEPOT_RESALE).round() as i64;
        if self.active_depot == id {
            self.active_depot = self.depots[0].id;
        }
        self.book(&format!("Sold depot \"{}\"", d.name), got);
        Ok(got)
    }

    /// What moving a bus to a depot costs.
    pub fn transfer_cost(&self, bus_id: u32, to: u32) -> i64 {
        let from = self.fleet.iter().find(|b| b.id == bus_id).map(|b| b.depot).unwrap_or(0);
        let far = matches!((self.depot(from), self.depot(to)), (Some(a), Some(b)) if a.map != b.map);
        ((economy::TRANSFER_COST * if far { 2 } else { 1 }) as f64 * economy::COST_MULT[self.diff()]).round() as i64
    }

    /// Move a bus to another depot (it needs a free place there); returns the cost.
    pub fn move_bus(&mut self, bus_id: u32, to: u32) -> Result<i64, String> {
        let b = self.fleet.iter().find(|b| b.id == bus_id).ok_or("No such bus")?;
        if b.depot == to {
            return Err("The bus is there already".to_string());
        }
        if b.away_until > now() {
            return Err("The bus is out on a job".to_string());
        }
        let no = b.fleet_no.clone();
        if let Some(why) = self.parking_block(to) {
            return Err(why);
        }
        let cost = self.transfer_cost(bus_id, to);
        if cost > self.balance {
            return Err(format!("Not enough money for the transfer: {} needed", money(cost)));
        }
        let to_name = self.depot(to).map(|d| d.name.clone()).unwrap_or_default();
        if let Some(b) = self.fleet.iter_mut().find(|b| b.id == bus_id) {
            b.depot = to;
        }
        self.book(&format!("Moved #{no} to \"{to_name}\""), -cost);
        Ok(cost)
    }

    /// What the company costs for a session the player booked: the rent of every depot beyond
    /// the first and the drivers' retainers.
    pub fn overheads(&self) -> (i64, i64) {
        let rent = (self.depots.len().saturating_sub(1) as f64 * economy::DEPOT_RENT as f64 * economy::COST_MULT[self.diff()]).round() as i64;
        let retainers = self.drivers.iter().map(|d| d.retainer()).sum();
        (rent, retainers)
    }

    /// Book those costs; returns the sum.
    pub fn pay_overheads(&mut self) -> i64 {
        let (rent, retainers) = self.overheads();
        let n = self.drivers.len();
        if rent > 0 {
            self.book("Depot rent", -rent);
        }
        if retainers > 0 {
            self.book(&format!("Retainers of {n} driver(s)"), -retainers);
        }
        rent + retainers
    }
}

// --- staff ----------------------------------------------------------------------------------

impl Company {
    /// How many drivers the company may employ at its level.
    pub fn staff_limit(&self) -> usize {
        staff::limit(self.level())
    }

    /// The seed of the applicants now: changes with the dealer's stock, or when new ones are
    /// asked for.
    pub fn applicant_seed_now(&self) -> u64 {
        mix((now() / (economy::STOCK_DAYS * 86_400)) ^ mix(self.created ^ 0xa11) ^ mix(self.applicant_rerolls as u64 + 7))
    }

    /// The applicants not hired yet, with their place in the pool.
    pub fn applicants(&self) -> Vec<(usize, Driver)> {
        let seed = self.applicant_seed_now();
        let rep = self.reputation.round();
        let taken: &[usize] = if self.applicant_seed == seed { &self.applicant_taken } else { &[] };
        staff::applicants(seed, rep, staff::applicant_count(rep)).into_iter().enumerate().filter(|(i, _)| !taken.contains(i)).collect()
    }

    pub fn reroll_applicants(&mut self) -> Result<(), String> {
        if self.balance < staff::APPLICANTS_COST {
            return Err("Not enough money for a new round of applicants".to_string());
        }
        self.book("New applicants", -staff::APPLICANTS_COST);
        self.applicant_rerolls += 1;
        Ok(())
    }

    fn log_staff(&mut self, text: String) {
        self.staff_log.push(LogLine { at: now(), text });
        if self.staff_log.len() > 40 {
            let cut = self.staff_log.len() - 40;
            self.staff_log.drain(..cut);
        }
    }

    /// Hire the applicant at `place` of the pool; returns the new driver's id.
    pub fn hire(&mut self, place: usize) -> Result<u32, String> {
        if self.drivers.len() >= self.staff_limit() {
            return Err(format!("At level {} the company employs at most {} driver(s)", self.level(), self.staff_limit()));
        }
        let seed = self.applicant_seed_now();
        let (_, mut d) = self.applicants().into_iter().find(|(i, _)| *i == place).ok_or("That applicant has found another job")?;
        let cost = staff::hire_cost(d.skill);
        if cost > self.balance {
            return Err(format!("Not enough money: hiring costs {}", money(cost)));
        }
        if self.applicant_seed != seed {
            self.applicant_taken.clear();
            self.applicant_seed = seed;
        }
        self.applicant_taken.push(place);
        let id = self.next_driver.max(1);
        self.next_driver = id + 1;
        d.id = id;
        d.hired = now();
        let name = d.name.clone();
        self.drivers.push(d);
        self.book(&format!("Hired {name}"), -cost);
        self.log_staff(format!("{name} joined the company"));
        Ok(id)
    }

    pub fn fire(&mut self, id: u32) -> Result<String, String> {
        let i = self.drivers.iter().position(|d| d.id == id).ok_or("No such driver")?;
        if self.drivers[i].job.is_some() {
            return Err("They are out on a job: wait until they are back".to_string());
        }
        let d = self.drivers.remove(i);
        self.log_staff(format!("{} left the company", d.name));
        Ok(d.name)
    }

    /// A training course; returns what it cost.
    pub fn train(&mut self, id: u32) -> Result<i64, String> {
        let i = self.drivers.iter().position(|d| d.id == id).ok_or("No such driver")?;
        if self.drivers[i].job.is_some() {
            return Err("They are out on a job".to_string());
        }
        if self.drivers[i].skill >= staff::SKILL_MAX {
            return Err("They have nothing more to learn".to_string());
        }
        let cost = self.drivers[i].train_cost();
        if cost > self.balance {
            return Err(format!("Not enough money: the course costs {}", money(cost)));
        }
        let d = &mut self.drivers[i];
        d.skill = (d.skill + staff::TRAIN_GAIN).min(staff::SKILL_MAX);
        let name = d.name.clone();
        self.book(&format!("Training of {name}"), -cost);
        Ok(cost)
    }

    /// Send a driver out on a job with a bus of the active depot. The job is driven in the head
    /// now (`staff::simulate`); the money is booked when its time is up (`collect_jobs`). Returns
    /// the seconds it takes.
    pub fn send_driver(&mut self, driver_id: u32, bus_id: u32, text: &str, size: &str, spec: &Spec, seed: u64) -> Result<u64, String> {
        let t = now();
        let di = self.drivers.iter().position(|d| d.id == driver_id).ok_or("No such driver")?;
        if self.drivers[di].job.is_some() {
            return Err("The driver is out on a job already".to_string());
        }
        if self.drivers[di].resting(t) {
            return Err(format!("{} is resting after the last shift", self.drivers[di].name));
        }
        if !staff::fits_a_shift(spec.sched_secs) {
            return Err(format!("A driver works {} hours at most in one go", staff::MAX_SHIFT_SECS / 3600));
        }
        let bi = self.fleet.iter().position(|b| b.id == bus_id).ok_or("No such bus")?;
        let b = &self.fleet[bi];
        if let Some(why) = b.drive_block() {
            return Err(why.to_string());
        }
        if b.depot != self.active_depot {
            return Err("That bus stands in another depot".to_string());
        }
        let weather = staff::Weather::roll((mix(seed ^ 0x7e47) >> 11) as f64 / (1u64 << 53) as f64);
        let sim = staff::simulate(&self.drivers[di], b.condition, weather, spec, seed);
        // the fuel: the driver fills up on the way when the tank would run nearly dry
        let tank = b.tank;
        let (mut fuel_end, mut fuel_added) = (None, 0.0f64);
        if tank > 0.0 {
            let after = (b.fuel.unwrap_or(tank) as f64 - sim.fuel_burned).max(0.0);
            if after < tank as f64 * 0.10 {
                fuel_added = tank as f64 - after;
                fuel_end = Some(tank);
            } else {
                fuel_end = Some(after as f32);
            }
        }
        let wage = staff::MIN_WAGE_PER_COURSE * spec.courses as i64;
        let share = self.drivers[di].share();
        let secs = staff::job_secs(spec.sched_secs);
        let ends = t + secs;
        self.drivers[di].job = Some(ActiveJob { bus_id, text: text.to_string(), size: size.to_string(), started: t, ends, weather, night: spec.night, sim, fuel_end, tank, fuel_added, wage, share });
        self.drivers[di].rest_until = ends + staff::rest_secs(secs);
        self.fleet[bi].away_until = ends;
        Ok(secs)
    }

    /// Book the jobs whose time is up at `t`: the fares, the bus's wear and fuel, the driver's
    /// wage and what they learned. Returns a line for each.
    pub fn collect_jobs(&mut self, t: u64) -> Vec<String> {
        let mut out = Vec::new();
        let ready: Vec<usize> = self.drivers.iter().enumerate().filter(|(_, d)| d.job.as_ref().is_some_and(|j| j.ends <= t)).map(|(i, _)| i).collect();
        for i in ready {
            let Some(job) = self.drivers[i].job.take() else { continue };
            let name = self.drivers[i].name.clone();
            let courses: Vec<crate::career_session::Course> = job
                .sim
                .courses
                .iter()
                .enumerate()
                .map(|(k, c)| crate::career_session::Course { trip: k, km: c.km, stops: c.stops, served: c.served, late: c.late, early: c.early, ..Default::default() })
                .collect();
            let extras = Extras { ticket_cash: job.sim.ticket_cash, fuel_added: job.fuel_added, fuel_burned: job.sim.fuel_burned, fuel_end: job.fuel_end, tank: job.tank };
            let mut st = settle_with(&courses, &job.size, job.sim.km * 1000.0, job.sim.crashes, self.difficulty, &extras, false);
            st.xp = (st.xp as f64 * economy::DRIVER_XP_SHARE).round() as u32;
            let text = format!("{} ({name})", job.text);
            let _ = self.settle_session(job.bus_id, &st, &text);
            if let Some(b) = self.fleet.iter_mut().find(|b| b.id == job.bus_id) {
                b.away_until = 0;
            }
            let n = courses.len() as u32;
            let wage = staff::job_wage(job.share, st.fares + st.bonus + st.tickets, n).max(job.wage);
            self.book(&format!("Wage: {name}"), -wage);
            {
                let d = &mut self.drivers[i];
                d.jobs += 1;
                d.courses += n;
                d.skill = staff::skill_after(d.skill, n);
            }
            let crash = if st.crashes > 0 { format!(" · {} crash(es)", st.crashes) } else { String::new() };
            let line = format!("{name} · {} · {n} course(s){crash} · {} after the wage", job.text, money(st.total() - wage));
            self.log_staff(line.clone());
            out.push(line);
        }
        out
    }

    /// Jobs out and the time the nearest one ends (None: no driver is out).
    pub fn next_job_end(&self) -> Option<u64> {
        self.drivers.iter().filter_map(|d| d.job.as_ref().map(|j| j.ends)).min()
    }
}

// --- the dealer's stock ------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Offer {
    /// Stable for the stock's seed: what `sold` remembers.
    pub id: u64,
    pub file: String,
    /// The paint it comes in ("" the bus's own) and its name.
    pub paint: String,
    pub paint_label: String,
    pub used: bool,
    pub km: u32,
    /// Wear in per cent (0 new).
    pub wear: u8,
    pub price: i64,
    pub maker: String,
    pub model: String,
    pub variant: String,
    pub size: String,
    pub class: String,
    /// The year it was built (0 unknown).
    pub year: u16,
    /// The model's key as the files write it (what the ignore list holds).
    pub type_key: String,
    /// The bus borrows parts from a pack that is not installed.
    pub incomplete: bool,
}

/// A mixing step (splitmix64) for seeds.
pub fn mix(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn hash_str(s: &str, seed: u64) -> u64 {
    let mut h = seed ^ 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    mix(h)
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = mix(self.0);
        self.0
    }
    /// 0.0..1.0
    fn f(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Whether a model built in `from..=to` (0 unknown; `to` 0 still built) can be bought new
/// this year: it is still built, or ended a year ago.
pub fn can_be_new(to: u16, this_year: u16) -> bool {
    to == 0 || to + 1 >= this_year
}

/// The year a bus of a model built in `from..=to` was built: a new one in the last year of the
/// model (this year while it is built), a used one any year of the model, newer ones more often
/// (`luck` is 0..1).
pub fn build_year(from: u16, to: u16, used: bool, luck: f64, this_year: u16) -> u16 {
    let hi = if to > 0 { to.min(this_year) } else { this_year };
    let lo = if from > 0 { from.min(hi) } else { hi.saturating_sub(25) };
    if !used {
        return hi;
    }
    let back = (luck.clamp(0.0, 1.0).powf(1.5) * (hi - lo) as f64).round() as u16;
    hi - back
}

/// The kilometres of a bus of this age: its class's yearly mileage, more or less (`luck` 0..1),
/// a few thousand even for a bus of this year.
pub fn km_for(age: u16, class: &str, luck: f64) -> u32 {
    let years = (age as f64).max(0.25);
    let km = years * economy::km_per_year(class) * (0.7 + 0.6 * luck.clamp(0.0, 1.0));
    km.min(economy::KM_MAX).max(3_000.0) as u32
}

/// Whether the text names an SD200 or an SD202 (any spelling: "SD 202", "man_sd202_ak", "SD200L"),
/// not a longer number that merely starts so.
pub fn names_sd200_family(text: &str) -> bool {
    let t: Vec<char> = text.to_lowercase().chars().collect();
    (0..t.len().saturating_sub(1)).any(|i| {
        if t[i] != 's' || t[i + 1] != 'd' {
            return false;
        }
        // (a space, an underscore, a dash or a dot may stand between the letters and the number)
        let mut j = i + 2;
        while j < t.len() && matches!(t[j], ' ' | '_' | '-' | '.') {
            j += 1;
        }
        let number = j + 2 < t.len() && t[j] == '2' && t[j + 1] == '0' && matches!(t[j + 2], '0' | '2');
        number && !t.get(j + 3).is_some_and(|c| c.is_ascii_digit())
    })
}

/// Whether a bus is one of the models that crowd the dealership: the SD200 and the SD202 (the
/// player has dozens of them installed, and they bury everything else in the search).
fn is_cluttering(v: &core::VehicleInfo, r: &core::catalog::Resolved) -> bool {
    [r.model.as_str(), r.variant.as_str(), r.type_key.as_str(), v.name.as_str(), v.type_name.as_str(), v.file.as_str()].iter().any(|t| names_sd200_family(t))
}

/// The place of a bus in the draw: the smaller, the sooner it is drawn. A weighted draw without
/// replacement (the time an event of rate `weight` takes: -ln(u) / weight), so a bus of a twentieth
/// of the weight is drawn ahead of one of the full weight about one time in twenty-one, and still
/// comes when a category has nothing else. Fixed by the hash: the same stock for the same seed.
pub fn draw_key(hash: u64, weight: f64) -> f64 {
    let u = ((hash >> 11) as f64 + 1.0) / (1u64 << 53) as f64;
    -u.ln() / weight.max(1e-6)
}

/// The dealer's stock for `seed`: of every size and class the installed buses have, up to
/// `PER_CATEGORY` buses (no more than `PER_MODEL` of one model), each new or used, in a
/// random paint of its own list that is not the bus's default. The models in `ignored` are
/// left out. Listed by size, then class.
pub fn stock(vehicles: &[core::VehicleInfo], catalog: &Catalog, ignored: &[String], seed: u64, this_year: u16) -> Vec<Offer> {
    // (hash, place in `vehicles`, what the catalogue says, whether it is one of the crowding models)
    type Cand = (u64, usize, core::catalog::Resolved, bool);
    let mut groups: BTreeMap<(usize, usize), Vec<Cand>> = BTreeMap::new();
    for (i, v) in vehicles.iter().enumerate() {
        if v.file.is_empty() {
            continue;
        }
        let r = catalog.resolve(v);
        if ignored.contains(&r.type_key) {
            continue;
        }
        let size = SIZES.iter().position(|s| *s == r.size).unwrap_or(2);
        let class = CLASSES.iter().position(|c| *c == r.class).unwrap_or(0);
        let crowding = is_cluttering(v, &r);
        groups.entry((size, class)).or_default().push((hash_str(&v.file, seed), i, r, crowding));
    }
    let mut out = Vec::new();
    for (_, list) in groups {
        // (the order of the draw: an ordinary bus by its hash, the crowding ones - however many
        // files they come in - far down the list, sharing the weight of half a bus)
        let crowd = list.iter().filter(|c| c.3).count().max(1) as f64;
        let mut list: Vec<(f64, u64, usize, core::catalog::Resolved)> =
            list.into_iter().map(|(h, i, r, crowding)| (draw_key(h, if crowding { economy::CLUTTER_SHARE / crowd } else { 1.0 }), h, i, r)).collect();
        list.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut per_model: HashMap<String, usize> = HashMap::new();
        let mut taken = 0;
        for (_, h, i, r) in list {
            if taken >= economy::PER_CATEGORY {
                break;
            }
            let n = per_model.entry(format!("{}|{}", r.maker.to_lowercase(), r.model.to_lowercase())).or_default();
            if *n >= economy::PER_MODEL {
                continue;
            }
            *n += 1;
            taken += 1;
            let v = &vehicles[i];
            let mut rng = Rng(h);
            let used = !can_be_new(r.to, this_year) || rng.f() < 0.55;
            let year = build_year(r.from, r.to, used, rng.f(), this_year);
            let age = this_year.saturating_sub(year);
            let km = if used { km_for(age, &r.class, rng.f()) } else { 0 };
            let wear = if used { (km as f64 / economy::KM_MAX * 70.0 + age as f64 * 0.7 + rng.f() * 8.0).clamp(3.0, 90.0) as u8 } else { 0 };
            // the random paint: one of the bus's list, never its default (the bus's own) - that
            // is only the paint of a bus that has no other
            let (paint, paint_label) = if v.paints.is_empty() {
                (String::new(), super::drive::default_livery_label(v).to_string())
            } else {
                let p = &v.paints[(rng.next() % v.paints.len() as u64) as usize];
                (p.clone(), p.clone())
            };
            let base = economy::base_price(&r.size, &r.class) as f64 * (0.9 + 0.2 * rng.f());
            let factor = if used { economy::used_factor(age) * (1.0 - 0.2 * wear as f64 / 100.0) } else { 1.0 };
            let price = (base * factor / 100.0).round() as i64 * 100;
            out.push(Offer {
                id: h,
                file: v.file.clone(),
                paint,
                paint_label,
                used,
                km,
                wear,
                price: price.max(1_500),
                maker: r.maker,
                model: r.model,
                variant: r.variant,
                size: r.size,
                class: r.class,
                year,
                type_key: r.type_key,
                incomplete: !v.missing_packs.is_empty(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money_has_spaces_and_a_currency() {
        set_currency(0);
        assert_eq!(money(48250), "€ 48 250");
        assert_eq!(money(0), "€ 0");
        assert_eq!(money(-300), "- € 300");
        assert_eq!(money(1_234_567), "€ 1 234 567");
        set_currency(1);
        assert_eq!(money(25_000), "107 500 zł");
        assert_eq!(money(-300), "- 1 290 zł");
        assert_eq!(from_display(107_500), 25_000);
        set_currency(2);
        assert_eq!(money(1_000), "$ 1 080");
        set_currency(0);
    }

    #[test]
    fn levels_follow_experience() {
        let mut c = Company::new("X", "m", 1, 0, Vec::new());
        assert_eq!(c.level(), 1);
        c.xp = 800;
        assert_eq!(c.level(), 2);
        c.xp = 20_000;
        assert_eq!(c.level(), 5);
        assert_eq!(c.level_progress().1, None);
    }

    fn offer(price: i64) -> Offer {
        Offer { id: 7, file: "Vehicles/a/b.bus".into(), paint: "Red".into(), paint_label: "Red".into(), used: true, km: 100_000, wear: 20, price, maker: "M".into(), model: "Mo".into(), variant: "V".into(), size: "MAXI".into(), class: "City".into(), year: 2008, type_key: "m|mo".into(), incomplete: false }
    }

    #[test]
    fn the_sd200_and_sd202_are_found_in_any_spelling() {
        for yes in ["MAN SD202", "man_sd202_ak", "SD 200 L", "Vehicles/MAN_SD200/x.bus", "sd-202", "MAN SD202 Stadtbus", "man_sd202_2010", "SD200"] {
            assert!(names_sd200_family(yes), "{yes}");
        }
        for no in ["Citaro", "O305", "SD2020", "MAN SD2002", "SD20", "Lion's City"] {
            assert!(!names_sd200_family(no), "{no}");
        }
    }

    #[test]
    fn a_cluttering_bus_is_drawn_rarely_and_a_lone_one_still_comes() {
        // 4000 hashes of each: how often the cluttering one is drawn ahead of the other
        let mut ahead = 0;
        for n in 0..4000u64 {
            let (a, b) = (mix(n * 2 + 1), mix(n * 2 + 2));
            if draw_key(a, 0.05) < draw_key(b, 1.0) {
                ahead += 1;
            }
        }
        // (1 in 21 for a weight of 0.05)
        assert!((100..=320).contains(&ahead), "{ahead}");
        // the same hash and weight: the same place
        assert_eq!(draw_key(77, 0.05), draw_key(77, 0.05));
        assert!(draw_key(0, 1.0).is_finite() && draw_key(u64::MAX, 1.0) >= 0.0);
    }

    #[test]
    fn buying_and_selling_move_money() {
        let mut c = Company::new("X", "m", 1, 0, Vec::new());
        let o = offer(10_000);
        let start = c.balance;
        c.buy(&o, &[("101".into(), "AB-1".into())], 5).unwrap();
        assert_eq!(c.balance, start - 10_000);
        assert_eq!(c.fleet[0].fleet_no, "101");
        assert_eq!(c.fleet[0].paint, "Red");
        assert_eq!(c.fleet[0].year, 2008);
        assert!(c.buy(&o, &[], 5).is_err());
        let got = c.sell(c.fleet[0].id).unwrap();
        assert!(got > 0 && got < 10_000);
        assert!(c.fleet.is_empty());
    }

    #[test]
    fn too_poor_cannot_buy() {
        let mut c = Company::new("X", "m", 2, 0, Vec::new());
        assert!(c.buy(&offer(500_000), &[], 1).is_err());
    }

    #[test]
    fn the_ignore_list_toggles() {
        let mut c = Company::new("X", "m", 1, 0, vec!["a|b".into()]);
        assert!(c.is_ignored("a|b"));
        c.set_ignored("a|b", false);
        c.set_ignored("c|d", true);
        c.set_ignored("c|d", true);
        assert!(!c.is_ignored("a|b") && c.is_ignored("c|d") && c.ignored.len() == 1);
    }

    fn offer_n(id: u64, price: i64) -> Offer {
        Offer { id, ..offer(price) }
    }

    #[test]
    fn a_new_company_has_a_home_depot_with_three_places() {
        let c = Company::new("X", "Maps/A/global.cfg", 1, 0, Vec::new());
        assert_eq!(c.depots.len(), 1);
        assert_eq!((c.depots[0].slots, c.depots[0].paid), (3, 0));
        assert_eq!(c.active_map(), "Maps/A/global.cfg");
        assert_eq!(c.free_places(c.active_depot), 3);
    }

    #[test]
    fn a_depot_holds_as_many_buses_as_it_has_places() {
        let mut c = Company::new("X", "m", 1, 0, Vec::new());
        for i in 0..3 {
            c.buy(&offer_n(i, 2_000), &[], 5).unwrap();
        }
        assert_eq!(c.parked(c.active_depot), 3);
        let before = c.balance;
        let err = c.buy(&offer_n(9, 2_000), &[], 5).unwrap_err();
        assert!(err.contains("full"), "{err}");
        assert_eq!(c.balance, before, "nothing is paid for a bus that cannot be parked");
        // a place costs money and makes room
        let id = c.active_depot;
        assert_eq!(c.buy_slot(id), Ok(economy::slot_price(3)));
        assert_eq!(c.depots[0].slots, 4);
        assert!(c.buy(&offer_n(9, 2_000), &[], 5).is_ok());
        // each next place costs more, and there is a limit
        assert!(economy::slot_price(4) > economy::slot_price(3));
        c.depots[0].slots = economy::DEPOT_MAX_SLOTS;
        assert!(c.buy_slot(id).is_err());
    }

    #[test]
    fn depots_cost_more_each_and_can_be_moved_into_and_sold() {
        let mut c = Company::new("X", "map-a", 1, 0, Vec::new());
        assert_eq!((economy::depot_price(1), economy::depot_price(2), economy::depot_price(99)), (15_000, 30_000, 230_000));
        let start = c.balance;
        let b = c.buy_depot("North", "map-b").unwrap();
        assert_eq!(c.balance, start - 15_000);
        assert_eq!(c.depots.len(), 2);
        assert_eq!((c.depot(b).unwrap().slots, c.depot(b).unwrap().map.as_str()), (3, "map-b"));
        // (the second one is beyond the means: 30 000 against 10 000)
        assert!(c.buy_depot("", "map-c").unwrap_err().contains("Not enough"));
        // the active depot is the player's choice
        assert_eq!(c.active_depot, c.depots[0].id);
        c.set_active_depot(b).unwrap();
        assert_eq!(c.active_map(), "map-b");
        assert!(c.set_active_depot(99).is_err());
        // a bus bought now stands in the active depot, and moving one costs (twice to another map)
        c.balance = 50_000;
        c.buy(&offer_n(1, 2_000), &[], 5).unwrap();
        let bus = c.fleet[0].id;
        assert_eq!(c.fleet[0].depot, b);
        let home = c.depots[0].id;
        let cost = c.move_bus(bus, home).unwrap();
        assert_eq!(cost, 600);
        assert_eq!(c.fleet[0].depot, home);
        assert!(c.move_bus(bus, home).is_err());
        // a full depot takes no bus
        c.depots[1].slots = 0;
        assert!(c.move_bus(bus, b).unwrap_err().contains("full"));
        c.depots[1].slots = 3;
        // selling: only an empty one, never the last, half of the price
        assert!(c.sell_depot(home).is_err(), "a bus stands in it");
        c.move_bus(bus, b).unwrap();
        let money_before = c.balance;
        c.set_active_depot(b).unwrap();
        assert_eq!(c.sell_depot(home), Ok(0), "the home depot was free");
        assert_eq!(c.balance, money_before);
        assert!(c.sell_depot(b).is_err(), "the last depot stays");
        let c2 = c.buy_depot("Third", "map-a").unwrap();
        assert_eq!(c.move_bus(bus, c2), Ok(600));
        assert_eq!(c.depots.len(), 2);
    }

    #[test]
    fn a_sold_depot_pays_half_and_the_active_one_moves() {
        let mut c = Company::new("X", "map-a", 1, 0, Vec::new());
        let b = c.buy_depot("North", "map-b").unwrap();
        c.set_active_depot(b).unwrap();
        let before = c.balance;
        assert_eq!(c.sell_depot(b), Ok(7_500));
        assert_eq!(c.balance, before + 7_500);
        assert_eq!(c.active_depot, c.depots[0].id);
    }

    #[test]
    fn an_old_company_gets_a_depot_that_holds_its_buses() {
        let bus = |id: u32| format!("{{\"id\":{id},\"file\":\"v/{id}.bus\",\"paint\":\"\",\"fleet_no\":\"{id}\",\"plate\":\"\",\"price\":1000,\"condition\":50.0,\"km\":0.0,\"used\":true,\"bought\":1}}");
        let buses: Vec<String> = (1..=5).map(bus).collect();
        let json = format!("{{\"version\":2,\"name\":\"Old\",\"home_map\":\"maps/old\",\"difficulty\":1,\"created\":5,\"balance\":100,\"xp\":0,\"reputation\":3.0,\"next_id\":6,\"fleet\":[{}],\"ledger\":[]}}", buses.join(","));
        let mut c: Company = serde_json::from_str(&json).unwrap();
        assert!(c.depots.is_empty());
        c.ensure_depots();
        assert_eq!(c.depots.len(), 1);
        assert_eq!(c.depots[0].map, "maps/old");
        assert_eq!(c.depots[0].slots, 5, "room for the buses it has");
        assert!(c.fleet.iter().all(|b| b.depot == c.depots[0].id));
        assert_eq!(c.active_depot, c.depots[0].id);
        assert_eq!((c.next_depot, c.next_driver), (2, 1));
        // the same again changes nothing
        let before = serde_json::to_string(&c).unwrap();
        c.ensure_depots();
        assert_eq!(serde_json::to_string(&c).unwrap(), before);
    }

    #[test]
    fn the_company_costs_money_per_session() {
        let mut c = Company::new("X", "a", 1, 0, Vec::new());
        assert_eq!(c.overheads(), (0, 0));
        assert_eq!(c.pay_overheads(), 0);
        c.buy_depot("B", "b").unwrap();
        assert_eq!(c.overheads(), (economy::DEPOT_RENT, 0));
        c.balance = 50_000;
        c.hire(c.applicants()[0].0).unwrap();
        let (rent, ret) = c.overheads();
        assert!(rent == 40 && ret > 10, "{rent} {ret}");
        let b = c.balance;
        assert_eq!(c.pay_overheads(), rent + ret);
        assert_eq!(c.balance, b - rent - ret);
    }

    #[test]
    fn drivers_are_hired_within_the_limit_and_leave_the_pool() {
        let mut c = Company::new("X", "a", 1, 0, Vec::new());
        c.reputation = 3.0;
        let pool = c.applicants();
        assert_eq!(pool.len(), 4);
        let first = pool[0].clone();
        let start = c.balance;
        let id = c.hire(first.0).unwrap();
        assert_eq!(c.balance, start - staff::hire_cost(first.1.skill));
        assert_eq!(c.drivers[0].name, first.1.name);
        assert_eq!(c.applicants().len(), 3, "the hired one is out of the pool");
        // level 1: one driver
        let err = c.hire(c.applicants()[0].0).unwrap_err();
        assert!(err.contains("at most 1"), "{err}");
        c.xp = 800;
        let second = c.applicants()[0].0;
        assert!(c.hire(second).is_ok());
        assert_eq!(c.drivers.len(), 2);
        assert_eq!(c.drivers[1].id, id + 1);
        // new applicants cost money and change the pool
        let names: Vec<String> = c.applicants().iter().map(|(_, d)| d.name.clone()).collect();
        c.reroll_applicants().unwrap();
        let names2: Vec<String> = c.applicants().iter().map(|(_, d)| d.name.clone()).collect();
        assert_ne!(names, names2);
        // firing, training
        let skill = c.drivers[0].skill;
        let cost = c.drivers[0].train_cost();
        let b = c.balance;
        assert_eq!(c.train(id), Ok(cost));
        assert_eq!(c.balance, b - cost);
        assert!(c.drivers[0].skill > skill);
        assert!(c.fire(id).is_ok() && c.fire(id).is_err());
        assert_eq!(c.drivers.len(), 1);
    }

    #[test]
    fn a_driver_does_a_job_that_is_booked_when_its_time_is_up() {
        let mut c = Company::new("X", "a", 1, 0, Vec::new());
        c.buy(&offer_n(1, 2_000), &[], 5).unwrap();
        c.fleet[0].condition = 80.0;
        c.fleet[0].tank = 250.0;
        c.fleet[0].fuel = Some(200.0);
        c.hire(c.applicants()[0].0).unwrap();
        let (did, bid) = (c.drivers[0].id, c.fleet[0].id);
        let spec = Spec { courses: 4, stops: 10, km: 5.0, night: false, fuel_per_km: 0.4, sched_secs: 2.0 * 3600.0 };
        let secs = c.send_driver(did, bid, "Line 5 · tour 1", "MAXI", &spec, 7).unwrap();
        assert_eq!(secs, 7200, "as long as the timetable takes");
        assert_eq!(c.fleet[0].drive_block(), Some("Out on a job with a driver"));
        assert!(c.sell(bid).is_err() && c.repair_bus(bid).is_err() && c.refuel_bus(bid).is_err());
        assert!(c.send_driver(did, bid, "x", "MAXI", &spec, 8).is_err(), "the driver is out");
        let (balance, xp, share0) = (c.balance, c.xp, c.drivers[0].share());
        assert!(c.collect_jobs(now() + secs - 5).is_empty());
        assert_eq!(c.balance, balance, "nothing is paid before the time is up");
        let lines = c.collect_jobs(now() + secs + 5);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].contains("Line 5"), "{}", lines[0]);
        assert_eq!(c.fleet[0].away_until, 0);
        assert_eq!(c.fleet[0].drive_block(), None);
        let d = &c.drivers[0];
        assert_eq!((d.jobs, d.courses), (1, 4));
        assert!(d.job.is_none());
        assert!(c.xp > xp, "the company learns from it, a little less than when the player drives");
        assert!(c.ledger.iter().any(|e| e.text.starts_with("Wage:") && e.amount < 0));
        assert!(c.fleet[0].condition < 80.0 && c.fleet[0].km > 15.0);
        assert!(c.fleet[0].fuel.unwrap() < 200.0, "the bus burned fuel");
        assert_eq!(c.staff_log.len(), 2, "joined, and the job");
        assert_eq!(c.next_job_end(), None);
        // the wage is a share of the takings (at least the minimum for the courses), no series
        let wage = -c.ledger.iter().find(|e| e.text.starts_with("Wage:")).unwrap().amount;
        let takings = c.ledger.iter().find(|e| e.text.contains("fares and tickets")).unwrap().amount;
        assert!(wage >= 4 * staff::MIN_WAGE_PER_COURSE && wage < takings, "{wage} {takings}");
        assert_eq!(wage, staff::job_wage(share0, takings, 4).max(4 * staff::MIN_WAGE_PER_COURSE));
        // and the driver rests for a third of the shift before the next one
        let t = now();
        assert!(c.drivers[0].rest_until > t + secs / 4);
        assert!(c.send_driver(did, bid, "x", "MAXI", &spec, 9).unwrap_err().contains("resting"));
        c.drivers[0].rest_until = 0;
        assert!(c.send_driver(did, bid, "x", "MAXI", &spec, 9).is_ok());
    }

    #[test]
    fn a_driver_gets_no_series_and_no_shift_longer_than_ten_hours() {
        use crate::career_session::Course;
        let courses: Vec<Course> = (0..9).map(|k| Course { trip: k, km: 6.0, stops: 14, served: 14, ..Default::default() }).collect();
        let me = settle(&courses, "MAXI", 0.0, 0, 1, &Extras::default());
        let they = settle_with(&courses, "MAXI", 0.0, 0, 1, &Extras::default(), false);
        assert!(me.streak == 5.0 && they.streak == 1.0 && they.streak_extra == 0);
        let one = settle_with(&courses[..1], "MAXI", 0.0, 0, 1, &Extras::default(), false);
        assert!((they.fares - 9 * one.fares).abs() <= 9, "{} {}", they.fares, one.fares);
        assert!(me.fares > 2 * they.fares, "{} {}", me.fares, they.fares);
        let mut c = Company::new("X", "a", 1, 0, Vec::new());
        c.buy(&offer_n(1, 2_000), &[], 5).unwrap();
        c.hire(c.applicants()[0].0).unwrap();
        let (did, bid) = (c.drivers[0].id, c.fleet[0].id);
        let long = Spec { courses: 20, stops: 10, km: 5.0, night: false, fuel_per_km: 0.4, sched_secs: 11.0 * 3600.0 };
        assert!(c.send_driver(did, bid, "x", "MAXI", &long, 1).unwrap_err().contains("hours"));
        assert!(c.drivers[0].job.is_none() && c.fleet[0].away_until == 0 && c.drivers[0].rest_until == 0);
    }

    #[test]
    fn a_bus_of_another_depot_or_one_that_cannot_go_out_cannot_be_sent() {
        let mut c = Company::new("X", "a", 1, 0, Vec::new());
        c.buy(&offer_n(1, 2_000), &[], 5).unwrap();
        c.hire(c.applicants()[0].0).unwrap();
        let (did, bid) = (c.drivers[0].id, c.fleet[0].id);
        let spec = Spec { courses: 1, stops: 5, km: 3.0, night: true, fuel_per_km: 0.4, sched_secs: 1800.0 };
        c.fleet[0].days_since_inspection = 30;
        assert!(c.send_driver(did, bid, "x", "MAXI", &spec, 1).unwrap_err().contains("Inspection"));
        c.fleet[0].days_since_inspection = 0;
        c.balance = 50_000;
        let b = c.buy_depot("N", "b").unwrap();
        c.set_active_depot(b).unwrap();
        assert!(c.send_driver(did, bid, "x", "MAXI", &spec, 1).unwrap_err().contains("another depot"));
        assert!(c.send_driver(did, 99, "x", "MAXI", &spec, 1).is_err() && c.send_driver(99, bid, "x", "MAXI", &spec, 1).is_err());
        assert!(c.drivers[0].job.is_none() && c.fleet[0].away_until == 0);
    }

    #[test]
    fn a_driver_who_is_out_comes_back_through_a_save() {
        let mut c = Company::new("X", "a", 1, 0, Vec::new());
        c.buy(&offer_n(1, 2_000), &[], 5).unwrap();
        c.hire(c.applicants()[0].0).unwrap();
        let (did, bid) = (c.drivers[0].id, c.fleet[0].id);
        let spec = Spec { courses: 2, stops: 8, km: 4.0, night: false, fuel_per_km: 0.4, sched_secs: 3600.0 };
        let secs = c.send_driver(did, bid, "Line 9 · tour 2", "MIDI", &spec, 3).unwrap();
        let text = serde_json::to_string_pretty(&c).unwrap();
        let mut back: Company = serde_json::from_str(&text).unwrap();
        back.ensure_depots();
        assert!(back.drivers[0].busy(now()));
        assert_eq!(back.fleet[0].away_until, c.fleet[0].away_until);
        assert_eq!(back.next_job_end(), c.next_job_end());
        assert_eq!(back.collect_jobs(now() + secs + 1).len(), 1);
        assert!(back.drivers[0].job.is_none());
    }

    #[test]
    fn years_follow_the_model_and_the_wear() {
        assert_eq!(build_year(1997, 2012, false, 0.5, 2026), 2012);
        assert_eq!(build_year(1997, 0, false, 0.5, 2026), 2026);
        for luck in [0.0, 0.3, 0.5, 1.0] {
            let y = build_year(1997, 2012, true, luck, 2026);
            assert!((1997..=2012).contains(&y), "{luck} -> {y}");
        }
        assert!(!can_be_new(2012, 2026) && can_be_new(0, 2026) && can_be_new(2025, 2026));
        let y = build_year(0, 0, true, 0.3, 2026);
        assert!((2001..=2026).contains(&y));
        assert!(year_now() >= 2025);
    }

    #[test]
    fn old_buses_are_cheap_and_have_covered_a_lot() {
        let km = km_for(18, "City", 0.5);
        assert!((850_000..=1_150_000).contains(&km), "{km}");
        let price = economy::base_price("MAXI", "City") as f64 * economy::used_factor(18);
        assert!((4_000.0..9_000.0).contains(&price), "{price}");
        assert!(km_for(2, "City", 0.5) < 150_000);
        assert!(economy::used_factor(2) > 0.65);
        assert!(economy::used_factor(40) < economy::used_factor(18));
        assert!(km_for(60, "Long-distance", 1.0) <= 1_400_000);
    }

    #[test]
    fn prices_rise_with_size_and_class() {
        assert!(economy::base_price("MINI", "Minibus") < economy::base_price("MIDI", "City"));
        assert!(economy::base_price("MAXI", "City") < economy::base_price("MEGA", "City"));
        assert!(economy::base_price("MAXI", "City") < economy::base_price("MAXI", "Long-distance"));
    }

    #[test]
    fn a_session_is_paid_by_its_courses() {
        use crate::career_session::Course;
        let c = |km: f64, stops: u32, served: u32, late: u32| Course { km, stops, served, late, ..Default::default() };
        let none = Extras::default();
        // two punctual courses of 10 km, 10 stops, on a MAXI bus (standard difficulty)
        let s = settle(&[c(10.0, 10, 10, 0), c(10.0, 10, 10, 0)], "MAXI", 24_000.0, 0, 1, &none);
        assert_eq!(s.courses, 2);
        // (318 for the first, 1.5 times that for the second; the bonus a quarter of the fares)
        assert_eq!(s.fares, 795);
        assert_eq!(s.bonus, 199);
        assert_eq!((s.streak, s.streak_extra), (1.5, 159));
        assert!((s.km - 24.0).abs() < 1e-9);
        // late and unserved stops pay less
        let bad = settle(&[c(10.0, 10, 5, 5)], "MAXI", 0.0, 1, 1, &none);
        assert!(bad.fares < 318 && bad.bonus == 0 && bad.damage == economy::CRASH_COST);
        assert!(bad.total() < s.total());
        // a smaller bus earns less
        assert!(settle(&[c(10.0, 10, 10, 0)], "MINI", 0.0, 0, 1, &none).fares < settle(&[c(10.0, 10, 10, 0)], "MEGA", 0.0, 0, 1, &none).fares);
        // nothing driven: nothing booked, no change of reputation
        let nothing = settle(&[], "MAXI", 0.0, 0, 1, &none);
        assert_eq!(nothing.total(), 0);
        assert_eq!(nothing.reputation, 0.0);
        // the difficulty: easier pays more, harder costs more; tickets are never multiplied
        let ex = Extras { ticket_cash: 20.0, fuel_added: 100.0, ..Default::default() };
        let (easy, std, hard) = (settle(&[c(10.0, 10, 10, 0)], "MAXI", 0.0, 1, 0, &ex), settle(&[c(10.0, 10, 10, 0)], "MAXI", 0.0, 1, 1, &ex), settle(&[c(10.0, 10, 10, 0)], "MAXI", 0.0, 1, 2, &ex));
        assert!(easy.fares > std.fares && std.fares > hard.fares);
        assert!(easy.fuel_cost < std.fuel_cost && std.fuel_cost < hard.fuel_cost);
        assert!(easy.tickets == 20 && hard.tickets == 20);
        assert_eq!(std.fuel_cost, 155);
    }

    #[test]
    fn a_city_course_pays_something_worth_driving() {
        use crate::career_session::Course;
        // 5.8 km and 11 stops, all served, a bit late once: some 330-380 euros (about 1500 zl)
        let s = settle(&[Course { km: 5.8, stops: 11, served: 11, late: 1, ..Default::default() }], "MAXI", 0.0, 0, 1, &Extras::default());
        let t = s.total();
        assert!((300..=420).contains(&t), "{t}");
        // and the longer the series, the more a course pays
        let c = Course { km: 5.8, stops: 11, served: 11, ..Default::default() };
        let one = settle(&[c.clone()], "MAXI", 0.0, 0, 1, &Extras::default()).fares;
        let four = settle(&[c.clone(), c.clone(), c.clone(), c], "MAXI", 0.0, 0, 1, &Extras::default());
        assert!(four.fares > 4 * one && four.streak == 2.5, "{} {}", four.fares, one);
    }

    #[test]
    fn booking_a_session_changes_the_bus_and_the_books() {
        use crate::career_session::Course;
        let mut co = Company::new("T", "m", 1, 0, Vec::new());
        co.fleet.push(OwnedBus { id: 4, condition: 80.0, km: 1000.0, ..Default::default() });
        let before = co.balance;
        let ex = Extras { fuel_burned: 6.0, fuel_end: Some(190.0), tank: 250.0, ..Default::default() };
        let s = settle(&[Course { km: 12.0, stops: 8, served: 8, ..Default::default() }], "MAXI", 15_000.0, 0, 1, &ex);
        assert!(co.settle_session(4, &s, "Line 14"));
        assert_eq!(co.balance, before + s.total());
        let b = &co.fleet[0];
        assert!((b.km - 1015.0).abs() < 1e-6);
        assert!(b.condition < 80.0 && co.xp > 0);
        assert_eq!((b.fuel, b.tank, b.days_since_inspection), (Some(190.0), 250.0, 1));
        assert!((b.consumption().unwrap() - 40.0).abs() < 1e-6);
        assert!(!co.settle_session(99, &s, "x"));
    }

    #[test]
    fn inspections_follow_the_condition() {
        assert!(inspection_passes(50.0, 0.99) && inspection_passes(100.0, 0.99));
        assert!(!inspection_passes(0.0, 0.0));
        // below 50 % it is a throw of the dice, the better the condition the likelier a pass
        assert!(inspection_passes(25.0, 0.3) && !inspection_passes(25.0, 0.6));
        let mut co = Company::new("T", "m", 1, 0, Vec::new());
        co.fleet.push(OwnedBus { id: 1, condition: 20.0, days_since_inspection: 21, ..Default::default() });
        assert_eq!(co.fleet[0].drive_block(), Some("Inspection due"));
        assert_eq!(co.inspect_bus(1, 0.99), Ok(false));
        assert_eq!(co.fleet[0].drive_block(), Some("Failed the inspection: repair it first"));
        assert!(co.inspect_bus(1, 0.0).is_err());
        assert!(co.repair_bus(1).is_ok());
        assert!(co.fleet[0].condition >= 70.0);
        assert_eq!(co.inspect_bus(1, 0.5), Ok(true));
        assert_eq!(co.fleet[0].drive_block(), None);
        assert_eq!(co.fleet[0].inspection_in(), 21);
    }

    #[test]
    fn fuel_and_loans() {
        let mut co = Company::new("T", "m", 1, 0, Vec::new());
        co.fleet.push(OwnedBus { id: 1, condition: 90.0, tank: 200.0, fuel: Some(100.0), ..Default::default() });
        let b = co.balance;
        assert_eq!(co.refuel_bus(1), Ok(155));
        assert_eq!(co.balance, b - 155);
        assert!(co.refuel_bus(1).is_err());
        co.fleet[0].fuel = Some(5.0);
        assert_eq!(co.fleet[0].drive_block(), Some("Out of fuel: refuel it first"));
        // a loan: limited by the level, repaid out of the fares with interest
        assert!(co.take_loan(co.loan_limit() + 1).is_err());
        assert!(co.take_loan(5_000).is_ok() && co.loan == 5_000);
        let s = Settlement { fares: 400, km: 10.0, ..Default::default() };
        let before = co.balance;
        assert!(co.settle_session(1, &s, "x"));
        // +400 fares, -100 interest (2 % of 5000), -100 repaid (a quarter of the fares)
        assert_eq!(co.balance, before + 400 - 100 - 100);
        assert_eq!(co.loan, 4_900);
        assert_eq!(co.repay_loan(1_000), Ok(1_000));
        assert_eq!(co.loan, 3_900);
    }
}
