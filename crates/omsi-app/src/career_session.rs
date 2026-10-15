//! A Career Mode session: the file the launcher and the game share.
//!
//! The launcher writes it before it starts the game (`--career <file>`): which bus of the
//! fleet drives which line and tour, from which trip. The game adds a line to `courses` every
//! time a course (a trip of the tour) is driven to its end, and marks the file `ended` with
//! the distance and time of the whole session when it closes. When the game is gone the
//! launcher reads the file back and settles the company's books (income, kilometres, wear).
//! The file is written whole, to a temporary file first, so a game that is killed leaves
//! either the old or the new one.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// One course (a trip of the tour) driven to its end.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Course {
    /// The trip's place in the duty (0 = the first driven).
    #[serde(default)]
    pub trip: usize,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub line: String,
    #[serde(default)]
    pub terminus: String,
    /// Stops on the course, and those the bus served.
    #[serde(default)]
    pub stops: u32,
    #[serde(default)]
    pub served: u32,
    /// Of those served: arrived late, left early.
    #[serde(default)]
    pub late: u32,
    #[serde(default)]
    pub early: u32,
    /// Kilometres driven on the course (from the bus's odometer).
    #[serde(default)]
    pub km: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Session {
    #[serde(default)]
    pub version: u32,
    /// When the launcher made it (seconds since 1970): the session's name.
    #[serde(default)]
    pub id: u64,
    /// The company's bus (its fleet id) and what it is called there.
    #[serde(default)]
    pub bus_id: u32,
    #[serde(default)]
    pub fleet_no: String,
    #[serde(default)]
    pub bus_file: String,
    #[serde(default)]
    pub line: String,
    #[serde(default)]
    pub tour: String,
    /// The trip of the tour the session began with (its place in the tour, 1 = the first).
    #[serde(default)]
    pub first_trip: usize,
    /// The courses driven to their end, in order.
    #[serde(default)]
    pub courses: Vec<Course>,
    /// The game has closed: `metres` and `seconds` are the whole session's.
    #[serde(default)]
    pub ended: bool,
    #[serde(default)]
    pub metres: f64,
    #[serde(default)]
    pub seconds: f64,
    #[serde(default)]
    pub crashes: i32,
    /// Fuel (litres in the tank): what the launcher puts in at the start (none: the bus's own,
    /// full), the tank's size as the bus started with it, and at the end what is left, what
    /// burned and what was filled up on the way.
    #[serde(default)]
    pub fuel_start: Option<f32>,
    #[serde(default)]
    pub tank: f32,
    #[serde(default)]
    pub fuel_end: Option<f32>,
    #[serde(default)]
    pub fuel_burned: f64,
    #[serde(default)]
    pub fuel_added: f64,
    /// Tickets sold in the session and the takings (the map's money, taken as euros).
    #[serde(default)]
    pub tickets: i32,
    #[serde(default)]
    pub ticket_cash: f64,
    /// The fuel level was set from `fuel_start` (once; not in the file).
    #[serde(skip)]
    pub fuel_set: bool,
    /// The launcher has booked it (once).
    #[serde(default)]
    pub settled: bool,
    /// Where the file lies (not in it).
    #[serde(skip)]
    pub path: PathBuf,
}

pub const VERSION: u32 = 1;

/// Courses driven one after another in a session pay more each: the first at the plain fare, the
/// second 1.5 times, the third twice, the fourth 2.5 times, and so on, a half more each time, up
/// to `STREAK_MAX` times. [ROLLBACK careerpay-1]
pub const STREAK_STEP: f64 = 0.5;
pub const STREAK_MAX: f64 = 5.0;

/// What the `n`-th course of a session (1 = the first) pays against the plain fare.
pub fn streak_mult(n: usize) -> f64 {
    (1.0 + STREAK_STEP * n.saturating_sub(1) as f64).min(STREAK_MAX)
}

impl Session {
    /// The file the launcher and the game share.
    pub fn default_path() -> PathBuf {
        omsi_launcher_lib::data_dir().join("career").join("session.json")
    }

    /// Read the session in `path` (None when there is none or it is not one).
    pub fn load(path: &str) -> Option<Session> {
        let p = Path::new(path);
        let text = std::fs::read_to_string(p).ok()?;
        let mut s: Session = serde_json::from_str(&text).ok()?;
        s.path = p.to_path_buf();
        Some(s)
    }

    /// Write the file (whole, through a temporary file).
    pub fn save(&self) -> std::io::Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("tmp");
        let text = serde_json::to_string_pretty(self).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, &self.path)
    }

    /// A course driven to its end.
    pub fn add_course(&mut self, c: Course) {
        self.courses.push(c);
        if let Err(e) = self.save() {
            log::warn!("writing the career session: {e}");
        }
    }

    /// The bus is on the road: the tank is filled to the level the company left it at (the
    /// level the bus starts with is its full tank, kept as `tank`). Once.
    pub fn apply_fuel(&mut self, bus: &mut omsi_sim::VehicleInstance) {
        if self.fuel_set {
            return;
        }
        self.fuel_set = true;
        let Some(full) = bus.var("engine_tank_content") else { return };
        self.tank = full;
        if let Some(f) = self.fuel_start {
            bus.set_var("engine_tank_content", f.clamp(0.0, full.max(f)));
        }
    }

    /// The game closes: the whole session's numbers.
    pub fn finish(&mut self, end: End) {
        if self.ended {
            return;
        }
        self.ended = true;
        self.metres = end.metres;
        self.seconds = end.seconds;
        self.crashes = end.crashes;
        self.tickets = end.tickets;
        self.ticket_cash = end.cash;
        self.fuel_end = end.fuel_end;
        self.fuel_burned = end.fuel_burned;
        self.fuel_added = end.fuel_added;
        if let Err(e) = self.save() {
            log::warn!("writing the career session: {e}");
        }
    }
}

/// What the game knows at the end of a session.
#[derive(Default, Clone, Copy)]
pub struct End {
    pub metres: f64,
    pub seconds: f64,
    pub crashes: i32,
    pub tickets: i32,
    pub cash: f64,
    pub fuel_end: Option<f32>,
    pub fuel_burned: f64,
    pub fuel_added: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_course_in_a_row_pays_more() {
        assert_eq!(streak_mult(0), 1.0);
        assert_eq!(streak_mult(1), 1.0);
        assert_eq!(streak_mult(2), 1.5);
        assert_eq!(streak_mult(3), 2.0);
        assert_eq!(streak_mult(4), 2.5);
        assert_eq!(streak_mult(9), 5.0);
        assert_eq!(streak_mult(30), STREAK_MAX);
    }

    #[test]
    fn a_session_survives_the_file() {
        let dir = std::env::temp_dir().join(format!("openomsi-session-{}", std::process::id()));
        let path = dir.join("session.json");
        let mut s = Session { version: VERSION, id: 5, bus_id: 3, fleet_no: "101".into(), line: "14".into(), tour: "1".into(), first_trip: 2, path: path.clone(), ..Default::default() };
        s.add_course(Course { trip: 0, name: "A".into(), stops: 10, served: 9, late: 1, km: 12.5, ..Default::default() });
        s.finish(End { metres: 13_000.0, seconds: 2_400.0, ..Default::default() });
        let back = Session::load(&path.to_string_lossy()).unwrap();
        assert!(back.ended && back.courses.len() == 1 && back.courses[0].km == 12.5 && back.bus_id == 3);
        std::fs::remove_dir_all(&dir).ok();
    }
}
