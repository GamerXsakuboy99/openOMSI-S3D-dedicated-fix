//! [ROLLBACK careerhired-1] The hired drivers' buses on the road. A driver sent out on a job from
//! the garage (Career Mode > Jobs) drives courses of a tour "in the head" of the company; so that a
//! player starting a session meanwhile passes that bus too, the launcher writes the jobs under
//! way into a file, and the game's timetable runs those courses of the tour with the company's
//! own bus (its model, livery and fleet number) instead of a bus drawn from the map's depot.
//!
//! The file is `<data folder>/career/hired.json`, written by the launcher before it starts a
//! Career Mode session (and emptied before any other start), read by the game only when it was
//! started with a career session (`--career`).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// One job under way.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Hired {
    /// The map the job is on (the company's active depot's map).
    pub map: String,
    pub line: String,
    pub tour: String,
    /// The courses driven, as places in the tour's list (`first..=last`).
    pub first: usize,
    pub last: usize,
    /// The company's bus: its file (as the vehicle list has it), repaint ("" the bus's own paint),
    /// fleet number and plate.
    pub bus_file: String,
    #[serde(default)]
    pub paint: String,
    #[serde(default)]
    pub fleet_no: String,
    #[serde(default)]
    pub plate: String,
    /// The driver's name (for the log) and when the job ends (seconds since 1970).
    #[serde(default)]
    pub driver: String,
    pub ends: u64,
}

pub fn path() -> PathBuf {
    omsi_launcher_lib::data_dir().join("career").join("hired.json")
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Write the list (an empty one removes the file).
pub fn write(list: &[Hired]) {
    let p = path();
    if list.is_empty() {
        let _ = std::fs::remove_file(&p);
        return;
    }
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match serde_json::to_string_pretty(list) {
        Ok(text) => {
            if let Err(e) = std::fs::write(&p, text) {
                log::warn!("hired drivers: {} not written: {e}", p.display());
            }
        }
        Err(e) => log::warn!("hired drivers: {e}"),
    }
}

/// The jobs still under way.
pub fn read() -> Vec<Hired> {
    let Ok(text) = std::fs::read_to_string(path()) else { return Vec::new() };
    let t = now();
    serde_json::from_str::<Vec<Hired>>(&text).unwrap_or_default().into_iter().filter(|h| h.ends > t && !h.bus_file.is_empty()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_job_survives_the_file_text() {
        let h = Hired { map: "Maps/X/global.cfg".into(), line: "5".into(), tour: "2".into(), first: 1, last: 3, bus_file: "Vehicles/A/A.bus".into(), ends: 5, ..Default::default() };
        let text = serde_json::to_string(&vec![h.clone()]).unwrap();
        assert_eq!(serde_json::from_str::<Vec<Hired>>(&text).unwrap(), vec![h]);
    }
}
