//! Career Mode's staff: drivers the company hires to take jobs without the player at the
//! wheel. Pure data and arithmetic - no drawing, no game: `company` keeps the drivers and books
//! the money, `career_staff` is the page, `career_jobs` sends a driver out.
//!
//! A driver has a skill (0..100, it grows with every job), traits (a careful driver crashes
//! less, one who dislikes night shifts drives worse at night ...), a wage that is a share of what
//! the job takes (never less than a minimum per course) and a retainer for every session the
//! player books. A job is simulated at once from the driver, the bus's condition, the weather
//! (rolled from the job's seed) and the courses; the result is kept in the driver's `ActiveJob`
//! and booked when the job's time has passed: the time the timetable gives the courses, from the
//! first departure to the last arrival (`job_secs`), in real seconds. A driver rests for a part
//! of a shift after it. The same seed gives the same job, so it is the same after the launcher
//! is closed and opened.

use serde::{Deserialize, Serialize};

/// Drivers the company may employ, by company level (1..=5).
pub const STAFF_LIMIT: [usize; 5] = [1, 2, 3, 5, 8];

/// Cost of asking for new applicants.
pub const APPLICANTS_COST: i64 = 60;

/// What a training course costs (plus `TRAIN_PER_SKILL` for every point of skill) and gives.
pub const TRAIN_BASE: i64 = 250;
pub const TRAIN_PER_SKILL: i64 = 4;
pub const TRAIN_GAIN: f32 = 3.0;
pub const SKILL_MAX: f32 = 95.0;

/// Real seconds of a job for a second of the timetable (1: the driver is out as long as the
/// courses take on the timetable).
pub const JOB_TIME_SCALE: f64 = 1.0;
/// The shortest job, and the longest a driver may be out (one shift).
pub const MIN_JOB_SECS: u64 = 120;
pub const MAX_SHIFT_SECS: u64 = 10 * 3600;
/// After a job the driver rests for this part of it.
pub const REST_SHARE: f64 = 1.0 / 3.0;
/// What a course costs in wages at least, whatever it took (euros).
pub const MIN_WAGE_PER_COURSE: i64 = 30;

/// How many drivers the company may employ at this level.
pub fn limit(level: u32) -> usize {
    STAFF_LIMIT[(level.clamp(1, 5) - 1) as usize]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Trait {
    Careful,
    Reckless,
    Punctual,
    Unpunctual,
    Economical,
    HeavyFoot,
    DislikesNights,
    NightOwl,
    FairWeather,
    Steady,
}

pub const TRAITS: [Trait; 10] = [
    Trait::Careful,
    Trait::Reckless,
    Trait::Punctual,
    Trait::Unpunctual,
    Trait::Economical,
    Trait::HeavyFoot,
    Trait::DislikesNights,
    Trait::NightOwl,
    Trait::FairWeather,
    Trait::Steady,
];

impl Trait {
    pub fn label(self) -> &'static str {
        match self {
            Trait::Careful => "Careful",
            Trait::Reckless => "Reckless",
            Trait::Punctual => "Punctual",
            Trait::Unpunctual => "Often late",
            Trait::Economical => "Economical",
            Trait::HeavyFoot => "Heavy foot",
            Trait::DislikesNights => "Dislikes night shifts",
            Trait::NightOwl => "Night owl",
            Trait::FairWeather => "Fair-weather driver",
            Trait::Steady => "Steady in bad weather",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Trait::Careful => "Crashes half as often",
            Trait::Reckless => "Crashes far more often",
            Trait::Punctual => "Late half as often",
            Trait::Unpunctual => "Late much more often",
            Trait::Economical => "Burns 15 % less fuel",
            Trait::HeavyFoot => "Burns 20 % more fuel",
            Trait::DislikesNights => "Drives much worse between 22:00 and 05:00",
            Trait::NightOwl => "Drives better between 22:00 and 05:00",
            Trait::FairWeather => "Drives worse in rain, fog and snow",
            Trait::Steady => "Bad weather troubles them half as much",
        }
    }

    pub fn good(self) -> bool {
        matches!(self, Trait::Careful | Trait::Punctual | Trait::Economical | Trait::NightOwl | Trait::Steady)
    }

    /// Two traits that cannot belong to one driver.
    fn clashes(self, other: Trait) -> bool {
        use Trait::*;
        matches!(
            (self, other),
            (Careful, Reckless) | (Reckless, Careful) | (Punctual, Unpunctual) | (Unpunctual, Punctual) | (Economical, HeavyFoot) | (HeavyFoot, Economical) | (DislikesNights, NightOwl) | (NightOwl, DislikesNights) | (FairWeather, Steady) | (Steady, FairWeather)
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Weather {
    #[default]
    Clear,
    Rain,
    Fog,
    Snow,
}

impl Weather {
    pub fn label(self) -> &'static str {
        match self {
            Weather::Clear => "clear",
            Weather::Rain => "rain",
            Weather::Fog => "fog",
            Weather::Snow => "snow",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Weather::Clear => "wb_sunny",
            Weather::Rain => "rainy",
            Weather::Fog => "foggy",
            Weather::Snow => "weather_snowy",
        }
    }

    /// How hard it is to drive in (0 nothing, 1 the worst).
    pub fn severity(self) -> f64 {
        match self {
            Weather::Clear => 0.0,
            Weather::Rain => 0.35,
            Weather::Fog => 0.5,
            Weather::Snow => 0.8,
        }
    }

    /// The weather of a job (`roll` 0..1): mostly clear.
    pub fn roll(roll: f64) -> Weather {
        match roll {
            x if x < 0.55 => Weather::Clear,
            x if x < 0.85 => Weather::Rain,
            x if x < 0.95 => Weather::Fog,
            _ => Weather::Snow,
        }
    }
}

/// One course of a simulated job.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct SimCourse {
    pub km: f64,
    pub stops: u32,
    pub served: u32,
    pub late: u32,
    pub early: u32,
}

/// What a simulated job came to, before the money (`company::settle` makes that of it).
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Sim {
    pub courses: Vec<SimCourse>,
    pub crashes: i32,
    pub km: f64,
    pub ticket_cash: f64,
    pub fuel_burned: f64,
}

/// A job a driver is out on.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct ActiveJob {
    pub bus_id: u32,
    /// "Line 5 · tour 2".
    pub text: String,
    /// The bus's size (the pay depends on it).
    pub size: String,
    pub started: u64,
    pub ends: u64,
    pub weather: Weather,
    pub night: bool,
    pub sim: Sim,
    /// The fuel the bus has when it is back (None: not known), its tank, and the litres the
    /// driver filled up on the way.
    pub fuel_end: Option<f32>,
    pub tank: f32,
    pub fuel_added: f64,
    /// The least the wage of this job is (`MIN_WAGE_PER_COURSE` for every course), and the share
    /// of the takings it is otherwise (`job_wage`).
    pub wage: i64,
    #[serde(default)]
    pub share: f64,
}

/// A line of the staff's log.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct LogLine {
    pub at: u64,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Driver {
    #[serde(default)]
    pub id: u32,
    #[serde(default)]
    pub name: String,
    /// 0..100; grows with every job and with training.
    #[serde(default)]
    pub skill: f32,
    #[serde(default)]
    pub traits: Vec<Trait>,
    #[serde(default)]
    pub hired: u64,
    /// Jobs done and courses driven.
    #[serde(default)]
    pub jobs: u32,
    #[serde(default)]
    pub courses: u32,
    #[serde(default)]
    pub job: Option<ActiveJob>,
    /// Resting after a shift until this time (unix seconds).
    #[serde(default)]
    pub rest_until: u64,
}

/// The share of what a job takes that is the driver's wage (0.01 steps): a green driver works
/// for about a quarter, a master for about a third, a good trait is worth two points. A better
/// driver takes more of the fares but brings in more (fewer stops missed, fewer crashes), so
/// they are worth their share.
pub fn wage_share(skill: f32, traits: &[Trait]) -> f64 {
    let good = traits.iter().filter(|t| t.good()).count() as f64;
    ((0.24 + skill as f64 * 0.0012 + good * 0.02) * 100.0).round() / 100.0
}

/// The wage of a job that took `takings` (fares, bonus, tickets) in `courses` courses: the
/// share, or the minimum for the courses when that is more.
pub fn job_wage(share: f64, takings: i64, courses: u32) -> i64 {
    ((takings.max(0) as f64 * share).round() as i64).max(MIN_WAGE_PER_COURSE * courses as i64)
}

/// What they cost for every session the player books, whatever they drive (euros).
pub fn retainer(skill: f32) -> i64 {
    30 + (skill / 2.0).round() as i64
}

/// What hiring costs: about a day's work of what they bring.
pub fn hire_cost(skill: f32) -> i64 {
    400 + (skill as i64 * 35 / 10) * 10
}

impl Driver {
    pub fn share(&self) -> f64 {
        wage_share(self.skill, &self.traits)
    }

    pub fn retainer(&self) -> i64 {
        retainer(self.skill)
    }

    pub fn has(&self, t: Trait) -> bool {
        self.traits.contains(&t)
    }

    pub fn train_cost(&self) -> i64 {
        TRAIN_BASE + self.skill as i64 * TRAIN_PER_SKILL
    }

    /// Resting after a shift at `now`.
    pub fn resting(&self, now: u64) -> bool {
        self.job.is_none() && now < self.rest_until
    }

    /// Out on a job that has not ended yet at `now`.
    pub fn busy(&self, now: u64) -> bool {
        self.job.as_ref().is_some_and(|j| j.ends > now)
    }

    /// A short word for the skill.
    pub fn rank(&self) -> &'static str {
        match self.skill as u32 {
            0..=29 => "Rookie",
            30..=49 => "Junior",
            50..=69 => "Experienced",
            70..=84 => "Veteran",
            _ => "Master",
        }
    }
}

// --- random numbers ------------------------------------------------------------------------

fn mix(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
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
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

const FIRST: [&str; 36] = [
    "Anna", "Marek", "Jan", "Katarzyna", "Piotr", "Ewa", "Tomasz", "Lena", "Hans", "Klaus", "Petra", "Jürgen", "Sofie", "Lukas", "Eva", "Martin", "Jana", "Tomáš", "Peter", "Zuzana", "Ivan", "Olga", "Pavel", "Mikael", "Greta", "Henrik", "Maria", "Jose", "Luca", "Chiara", "Oliver", "Emma", "Jack", "Sarah", "Karl", "Nora",
];
const LAST: [&str; 36] = [
    "Kowalski", "Nowak", "Wiśniewski", "Wójcik", "Lewandowski", "Zieliński", "Müller", "Schmidt", "Fischer", "Weber", "Novák", "Svoboda", "Dvořák", "Horváth", "Kovács", "Nagy", "Svensson", "Lindqvist", "Jansen", "de Vries", "Rossi", "Ferrari", "Smith", "Taylor", "Brown", "Wilson", "Petrov", "Ivanov", "García", "Silva", "Meyer", "Wagner", "Becker", "Hoffmann", "Krüger", "Bauer",
];

/// `count` people asking for a job, from `seed`. Their skill follows the company's
/// reputation (0..5): a well-known company draws better drivers.
pub fn applicants(seed: u64, reputation: f32, count: usize) -> Vec<Driver> {
    let rep = (reputation / 5.0).clamp(0.0, 1.0) as f64;
    (0..count)
        .map(|k| {
            let mut r = Rng(mix(seed ^ mix(k as u64 + 1)));
            let skill = (22.0 + r.f() * 45.0 + rep * 14.0 + r.f() * 6.0).min(88.0);
            let name = format!("{} {}", FIRST[r.below(FIRST.len())], LAST[r.below(LAST.len())]);
            let mut traits: Vec<Trait> = Vec::new();
            let want = if r.f() < 0.45 { 1 } else { 2 };
            let mut guard = 0;
            while traits.len() < want && guard < 40 {
                guard += 1;
                let t = TRAITS[r.below(TRAITS.len())];
                if !traits.contains(&t) && !traits.iter().any(|x| x.clashes(t)) {
                    traits.push(t);
                }
            }
            Driver { id: 0, name, skill: skill.round() as f32, traits, hired: 0, jobs: 0, courses: 0, job: None, rest_until: 0 }
        })
        .collect()
}

/// How many applicants a company of this reputation draws.
pub fn applicant_count(reputation: f32) -> usize {
    if reputation >= 4.0 {
        5
    } else if reputation >= 2.0 {
        4
    } else {
        3
    }
}

// --- the job ---------------------------------------------------------------------------------

/// What a job is: the courses a driver drives.
#[derive(Clone, Debug)]
pub struct Spec {
    pub courses: u32,
    /// Stops on a course, and its length.
    pub stops: u32,
    pub km: f64,
    /// The first course leaves between 22:00 and 05:00.
    pub night: bool,
    /// Litres per kilometre of the bus.
    pub fuel_per_km: f64,
    /// What the courses take on the timetable, from the first departure to the last arrival
    /// (seconds).
    pub sched_secs: f64,
}

/// Real seconds a job takes: what the timetable gives the courses (`JOB_TIME_SCALE`), at least
/// `MIN_JOB_SECS`, at most one shift.
pub fn job_secs(sched_secs: f64) -> u64 {
    ((sched_secs.max(0.0) * JOB_TIME_SCALE).round() as u64).clamp(MIN_JOB_SECS, MAX_SHIFT_SECS)
}

/// Whether courses taking this long on the timetable fit in one shift.
pub fn fits_a_shift(sched_secs: f64) -> bool {
    sched_secs * JOB_TIME_SCALE <= MAX_SHIFT_SECS as f64
}

/// The courses of a tour a driver may be given when they start at `first`, as the index of the
/// last course: every one whose arrival is within one shift of the first departure.
/// `times` are the (departure, arrival) of the tour's courses in seconds.
pub fn run_options(times: &[(f64, f64)], first: usize) -> Vec<usize> {
    let Some(f) = times.get(first) else { return Vec::new() };
    (first..times.len()).take_while(|&k| fits_a_shift(times[k].1 - f.0)).collect()
}

/// The last course of a run: `until` when it is one of the options, otherwise the longest run
/// that fits a shift (None: not even the first course fits).
pub fn run_end(times: &[(f64, f64)], first: usize, until: Option<usize>) -> Option<usize> {
    let options = run_options(times, first);
    match until {
        Some(u) if options.contains(&u) => Some(u),
        _ => options.last().copied(),
    }
}

/// A span of time in words: "45 min", "2 h 05 min", "3 h".
pub fn span_text(secs: u64) -> String {
    let m = (secs + 30) / 60;
    match (m / 60, m % 60) {
        (0, m) => format!("{} min", m.max(1)),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m:02} min"),
    }
}

/// Time left of a job: "12:05" under an hour, otherwise in words.
pub fn left_text(secs: u64) -> String {
    if secs < 3600 {
        format!("{}:{:02}", secs / 60, secs % 60)
    } else {
        span_text(secs)
    }
}

/// How long a driver rests after a job of `secs`.
pub fn rest_secs(secs: u64) -> u64 {
    (secs as f64 * REST_SHARE).round() as u64
}

/// How well a driver drives this job (0.05..1): their skill, moved by the night and the weather
/// for the traits that mind either.
fn effective(d: &Driver, weather: Weather, night: bool) -> f64 {
    let mut e = d.skill as f64 / 100.0;
    if night {
        if d.has(Trait::DislikesNights) {
            e -= 0.20;
        }
        if d.has(Trait::NightOwl) {
            e += 0.08;
        }
    }
    if weather != Weather::Clear && d.has(Trait::FairWeather) {
        e -= 0.15;
    }
    e.clamp(0.05, 1.0)
}

/// Drive the job in the head: stops served, late and early, crashes, takings, fuel. `condition`
/// is the bus's (0..100); the same seed gives the same job.
pub fn simulate(d: &Driver, condition: f32, weather: Weather, spec: &Spec, seed: u64) -> Sim {
    let mut r = Rng(mix(seed ^ 0x51ab_cafe));
    let eff = effective(d, weather, spec.night);
    let cond = (condition as f64 / 100.0).clamp(0.0, 1.0);
    let mut pen = weather.severity() * (1.0 - 0.6 * eff);
    if d.has(Trait::Steady) {
        pen *= 0.5;
    }
    let p_serve = (0.80 + 0.19 * eff - 0.12 * (1.0 - cond) - 0.10 * pen).clamp(0.40, 1.0);
    let mut p_late = (0.30 - 0.25 * eff + 0.25 * pen + 0.10 * (1.0 - cond)).clamp(0.02, 0.9);
    if d.has(Trait::Punctual) {
        p_late *= 0.5;
    }
    if d.has(Trait::Unpunctual) {
        p_late = (p_late * 1.7).min(0.95);
    }
    let mut p_crash = (0.05 + 0.20 * (1.0 - eff)) * (0.6 + 0.8 * (1.0 - cond)) * (1.0 + pen);
    if d.has(Trait::Careful) {
        p_crash *= 0.5;
    }
    if d.has(Trait::Reckless) {
        p_crash *= 1.7;
    }
    let p_crash = p_crash.clamp(0.0, 0.6);
    let mut sim = Sim::default();
    for _ in 0..spec.courses {
        let km = spec.km * (0.97 + 0.06 * r.f());
        let mut c = SimCourse { km, stops: spec.stops, ..Default::default() };
        for _ in 0..spec.stops {
            if r.f() < p_serve {
                c.served += 1;
                let x = r.f();
                if x < p_late {
                    c.late += 1;
                } else if x > 0.97 {
                    c.early += 1;
                }
            }
        }
        if r.f() < p_crash {
            sim.crashes += 1;
        }
        sim.km += km;
        sim.ticket_cash += c.served as f64 * 1.5 * (0.8 + 0.4 * r.f());
        sim.courses.push(c);
    }
    let mut fuel = spec.fuel_per_km;
    if d.has(Trait::Economical) {
        fuel *= 0.85;
    }
    if d.has(Trait::HeavyFoot) {
        fuel *= 1.2;
    }
    sim.fuel_burned = sim.km * fuel;
    sim
}

/// A driver's skill after a job: it grows a little with every course driven, more when the
/// driver is still green.
pub fn skill_after(skill: f32, courses: u32) -> f32 {
    let gain = courses as f32 * (0.35 * (1.0 - skill / 100.0)).max(0.04);
    (skill + gain).min(SKILL_MAX)
}

/// Whether the hour (seconds from midnight) is a night one (22:00..05:00).
pub fn is_night(seconds_of_day: f64) -> bool {
    let h = (seconds_of_day / 3600.0).rem_euclid(24.0);
    !(5.0..22.0).contains(&h)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn driver(skill: f32, traits: &[Trait]) -> Driver {
        Driver { id: 1, name: "T".into(), skill, traits: traits.to_vec(), hired: 0, jobs: 0, courses: 0, job: None, rest_until: 0 }
    }

    fn spec() -> Spec {
        Spec { courses: 6, stops: 12, km: 6.0, night: false, fuel_per_km: 0.4, sched_secs: 3.0 * 3600.0 }
    }

    fn totals(d: &Driver, cond: f32, w: Weather, s: &Spec, n: u64) -> (f64, f64, f64) {
        let (mut served, mut late, mut crashes) = (0.0, 0.0, 0.0);
        for seed in 0..n {
            let sim = simulate(d, cond, w, s, seed);
            served += sim.courses.iter().map(|c| c.served as f64).sum::<f64>();
            late += sim.courses.iter().map(|c| c.late as f64).sum::<f64>();
            crashes += sim.crashes as f64;
        }
        let stops = (s.courses * s.stops) as f64 * n as f64;
        (served / stops, late / served.max(1.0), crashes / (s.courses as f64 * n as f64))
    }

    #[test]
    fn the_same_seed_is_the_same_job() {
        let d = driver(55.0, &[Trait::Careful]);
        assert_eq!(simulate(&d, 80.0, Weather::Rain, &spec(), 9), simulate(&d, 80.0, Weather::Rain, &spec(), 9));
        assert_ne!(simulate(&d, 80.0, Weather::Rain, &spec(), 9), simulate(&d, 80.0, Weather::Rain, &spec(), 10));
        let sim = simulate(&d, 80.0, Weather::Clear, &spec(), 3);
        assert_eq!(sim.courses.len(), 6);
        assert!(sim.courses.iter().all(|c| c.served <= c.stops && c.late <= c.served));
        assert!((sim.km - 36.0).abs() < 2.5, "{}", sim.km);
        assert!(sim.fuel_burned > 10.0 && sim.ticket_cash > 0.0);
    }

    #[test]
    fn better_drivers_drive_better() {
        let (rs, rl, rc) = totals(&driver(15.0, &[]), 80.0, Weather::Clear, &spec(), 400);
        let (ms, ml, mc) = totals(&driver(90.0, &[]), 80.0, Weather::Clear, &spec(), 400);
        assert!(ms > rs && ml < rl && mc < rc, "{rs} {rl} {rc} / {ms} {ml} {mc}");
        assert!(ms > 0.93 && rs < 0.85, "{ms} {rs}");
    }

    #[test]
    fn wear_and_weather_cost() {
        let d = driver(50.0, &[]);
        let good = totals(&d, 95.0, Weather::Clear, &spec(), 400);
        let worn = totals(&d, 15.0, Weather::Clear, &spec(), 400);
        let snow = totals(&d, 95.0, Weather::Snow, &spec(), 400);
        assert!(worn.0 < good.0 && worn.2 > good.2, "{good:?} {worn:?}");
        assert!(snow.1 > good.1 && snow.2 > good.2, "{good:?} {snow:?}");
    }

    #[test]
    fn traits_change_the_result() {
        let base = totals(&driver(50.0, &[]), 70.0, Weather::Clear, &spec(), 600);
        let careful = totals(&driver(50.0, &[Trait::Careful]), 70.0, Weather::Clear, &spec(), 600);
        let reckless = totals(&driver(50.0, &[Trait::Reckless]), 70.0, Weather::Clear, &spec(), 600);
        assert!(careful.2 < base.2 && base.2 < reckless.2, "{} {} {}", careful.2, base.2, reckless.2);
        let punctual = totals(&driver(50.0, &[Trait::Punctual]), 70.0, Weather::Clear, &spec(), 600);
        let slow = totals(&driver(50.0, &[Trait::Unpunctual]), 70.0, Weather::Clear, &spec(), 600);
        assert!(punctual.1 < base.1 && base.1 < slow.1);
        // a night shift: the driver who dislikes it drives worse, the night owl better
        let night = Spec { night: true, ..spec() };
        let day = totals(&driver(50.0, &[Trait::DislikesNights]), 70.0, Weather::Clear, &spec(), 600);
        let nope = totals(&driver(50.0, &[Trait::DislikesNights]), 70.0, Weather::Clear, &night, 600);
        let owl = totals(&driver(50.0, &[Trait::NightOwl]), 70.0, Weather::Clear, &night, 600);
        assert!(nope.2 > day.2 && nope.1 > day.1);
        assert!(owl.1 < nope.1);
        // fuel
        let fuel = |t: &[Trait]| simulate(&driver(50.0, t), 80.0, Weather::Clear, &spec(), 1).fuel_burned;
        assert!(fuel(&[Trait::Economical]) < fuel(&[]) && fuel(&[]) < fuel(&[Trait::HeavyFoot]));
        // bad weather: the steady one minds less, the fair-weather one more
        let rain = |t: &[Trait]| totals(&driver(50.0, t), 80.0, Weather::Snow, &spec(), 600).1;
        assert!(rain(&[Trait::Steady]) < rain(&[]) && rain(&[]) < rain(&[Trait::FairWeather]));
    }

    #[test]
    fn applicants_follow_the_seed_and_the_reputation() {
        let a = applicants(42, 3.0, 4);
        assert_eq!(a.len(), 4);
        assert_eq!(a.iter().map(|d| d.name.clone()).collect::<Vec<_>>(), applicants(42, 3.0, 4).iter().map(|d| d.name.clone()).collect::<Vec<_>>());
        assert_ne!(a[0].name, applicants(43, 3.0, 4)[0].name);
        for d in &a {
            assert!((1..=2).contains(&d.traits.len()));
            assert!(!d.traits.iter().any(|x| d.traits.iter().any(|y| x.clashes(*y))));
            assert!(d.skill >= 20.0 && d.skill <= 88.0, "{}", d.skill);
        }
        let avg = |rep: f32| (0..60).flat_map(|s| applicants(s, rep, 4)).map(|d| d.skill as f64).sum::<f64>() / 240.0;
        assert!(avg(5.0) > avg(0.0) + 8.0, "{} {}", avg(5.0), avg(0.0));
        assert!(applicant_count(0.5) < applicant_count(2.5) && applicant_count(2.5) < applicant_count(4.5));
    }

    #[test]
    fn money_follows_the_skill() {
        assert!(wage_share(80.0, &[]) > wage_share(20.0, &[]));
        assert!(wage_share(50.0, &[Trait::Careful, Trait::Punctual]) > wage_share(50.0, &[]));
        assert!((wage_share(20.0, &[]) - 0.26).abs() < 1e-9 && (wage_share(90.0, &[]) - 0.35).abs() < 1e-9);
        assert!(wage_share(95.0, &[Trait::Careful, Trait::Punctual]) < 0.42);
        assert!(retainer(80.0) > retainer(20.0));
        assert!(hire_cost(80.0) > hire_cost(20.0));
        assert_eq!(hire_cost(0.0), 400);
        assert_eq!(hire_cost(60.0), 2500);
        // a wage is the share of the takings, but never less than the minimum for the courses
        assert_eq!(job_wage(0.3, 10_000, 5), 3000);
        assert_eq!(job_wage(0.3, 100, 5), 5 * MIN_WAGE_PER_COURSE);
        assert_eq!(job_wage(0.3, -50, 2), 2 * MIN_WAGE_PER_COURSE);
    }

    #[test]
    fn skill_grows_slowly_and_stops() {
        let a = skill_after(20.0, 6);
        let b = skill_after(80.0, 6);
        assert!(a > 20.0 && b > 80.0 && a - 20.0 > b - 80.0);
        assert_eq!(skill_after(94.9, 100), SKILL_MAX);
        assert_eq!(skill_after(SKILL_MAX, 5), SKILL_MAX);
    }

    #[test]
    fn jobs_take_as_long_as_the_timetable_and_nights_are_found() {
        // a job lasts what the timetable gives the courses, from the first departure to the last arrival
        assert_eq!(job_secs(40.0 * 60.0), 2400);
        assert_eq!(job_secs(3.0 * 3600.0), 10_800);
        assert_eq!(job_secs(5.0), MIN_JOB_SECS);
        assert_eq!(job_secs(-3.0), MIN_JOB_SECS);
        assert_eq!(job_secs(30.0 * 3600.0), MAX_SHIFT_SECS);
        assert!(fits_a_shift(10.0 * 3600.0) && !fits_a_shift(10.0 * 3600.0 + 1.0));
        assert_eq!(rest_secs(9000), 3000);
        assert_eq!((span_text(20), span_text(45 * 60), span_text(3 * 3600), span_text(2 * 3600 + 5 * 60)), ("1 min".into(), "45 min".into(), "3 h".into(), "2 h 05 min".into()));
        assert_eq!((left_text(725), left_text(7500)), ("12:05".to_string(), "2 h 05 min".to_string()));
        assert!(is_night(23.0 * 3600.0) && is_night(3.0 * 3600.0) && is_night(5.0 * 3600.0 - 1.0));
        assert!(!is_night(5.0 * 3600.0) && !is_night(12.0 * 3600.0) && !is_night(21.99 * 3600.0));
        assert!(is_night(25.0 * 3600.0), "past midnight wraps (a timetable runs past 24:00)");
    }

    #[test]
    fn a_run_is_a_range_of_courses_within_a_shift() {
        // a tour of courses an hour long every 90 minutes from 05:00
        let times: Vec<(f64, f64)> = (0..12).map(|k| (5.0 * 3600.0 + k as f64 * 5400.0, 6.0 * 3600.0 + k as f64 * 5400.0)).collect();
        let options = run_options(&times, 0);
        // the arrival of the course k is 1 h + k * 1.5 h after the first departure: within 10 h up to k = 6
        assert_eq!(options, (0..=6).collect::<Vec<_>>());
        assert_eq!(run_options(&times, 4), (4..=10).collect::<Vec<_>>());
        assert_eq!(run_options(&times, 11), vec![11]);
        assert!(run_options(&times, 12).is_empty() && run_options(&[], 0).is_empty());
        assert_eq!(run_end(&times, 0, Some(2)), Some(2));
        assert_eq!(run_end(&times, 0, None), Some(6), "to the end as far as a shift goes");
        assert_eq!(run_end(&times, 4, Some(2)), Some(10), "an end before the start is not one");
        assert_eq!(run_end(&times, 0, Some(9)), Some(6), "nor one beyond a shift");
        assert_eq!(run_end(&[(0.0, 11.0 * 3600.0)], 0, None), None, "one course longer than a shift");
    }

    #[test]
    fn the_staff_limit_grows_with_the_level() {
        assert_eq!(limit(1), 1);
        assert_eq!(limit(5), 8);
        assert_eq!(limit(0), 1);
        assert_eq!(limit(9), 8);
        let mut d = driver(50.0, &[]);
        assert!(!d.busy(10));
        d.job = Some(ActiveJob { ends: 100, ..Default::default() });
        assert!(d.busy(99) && !d.busy(100));
        d.job = None;
        d.rest_until = 500;
        assert!(d.resting(499) && !d.resting(500) && !d.busy(499));
        assert_eq!(driver(10.0, &[]).rank(), "Rookie");
        assert_eq!(driver(97.0, &[]).rank(), "Master");
    }

    #[test]
    fn a_driver_survives_a_round_trip_and_old_files() {
        let mut d = driver(61.0, &[Trait::Careful, Trait::NightOwl]);
        d.job = Some(ActiveJob { bus_id: 3, text: "Line 5".into(), ends: 99, weather: Weather::Fog, sim: Sim { crashes: 1, ..Default::default() }, ..Default::default() });
        let text = serde_json::to_string(&d).unwrap();
        let back: Driver = serde_json::from_str(&text).unwrap();
        assert_eq!(back.job, d.job);
        assert_eq!(back.traits, d.traits);
        let old: Driver = serde_json::from_str("{\"id\":4,\"name\":\"X\"}").unwrap();
        assert_eq!((old.id, old.skill, old.traits.len(), old.job.is_none()), (4, 0.0, 0, true));
    }
}
