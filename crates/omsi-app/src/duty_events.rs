//! Private server-side duty tracker for the optional Discord Bridge.
//! Only observes a player's published LAN pose (line, tour, bus, name).
//! It does not and cannot claim a trip was successfully completed.
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

static TRACKER: OnceLock<Mutex<Tracker>> = OnceLock::new();

#[derive(Clone, Debug)]
struct Duty {
    name: String,
    line: String,
    tour: String,
    bus: String,
    route: String,
}
impl Duty {
    fn from_pose(p: &omsi_net::Pose) -> Duty {
        let tour = p.tour.trim();
        let duty = tour.split_once('/').filter(|(line, tour)| !line.trim().is_empty() && !tour.trim().is_empty());
        let (line, tour) = duty.map(|(line,tour)| (line.trim(),tour.trim())).unwrap_or(("", ""));
        Duty {
            name: clip(&p.name), line: clip(line), tour: clip(tour),
            bus: clip(&p.bus), route: clip(&p.line),
        }
    }
    fn active(&self) -> bool { !self.line.is_empty() && !self.tour.is_empty() }
}
fn clip(s: &str) -> String {
    let mut value=String::new();
    for ch in s.trim().chars().filter(|c| !c.is_control()) {
        if value.len()+ch.len_utf8() > 100 { break; }
        value.push(ch);
    }
    value
}

struct Tracker {
    file: PathBuf,
    peers: HashMap<u32,Duty>,
}
pub(crate) fn configure(server_cfg: &Path) {
    let dir=server_cfg.parent().unwrap_or_else(||Path::new("."));
    let file=dir.join("duty-events.jsonl");
    let _=TRACKER.set(Mutex::new(Tracker { file,peers:HashMap::new() }));
}
fn event(t: &Tracker, kind: &str, id: u32, p: &Duty) {
    let data=serde_json::json!({
        "kind":kind,"player_id":id,"player":p.name,
        "line":p.line,"tour":p.tour,"route":p.route,"bus":p.bus,
        "timestamp_unix":std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).map(|d|d.as_secs()).unwrap_or(0)
    });
    let result=std::fs::OpenOptions::new().create(true).append(true).open(&t.file)
        .and_then(|mut f|writeln!(f,"{data}"));
    if let Err(err)=result { log::warn!("server: private duty journal write failed: {err}"); }
}

/// Poll once per second from the host loop; no player coordinates leave the host.
pub(crate) fn tick(lan: &omsi_net::LanSession) {
    let Some(lock)=TRACKER.get() else { return };
    let mut t=lock.lock().unwrap_or_else(|e|e.into_inner());
    let current:HashMap<u32,Duty>=lan.peers()
        .filter(|peer|peer.has_info && peer.has_pose)
        .map(|peer|(peer.pose.id,Duty::from_pose(&peer.pose))).collect();
    for (&id,prev) in &t.peers {
        if !current.contains_key(&id) && prev.active() {
            event(&t,"duty_disconnected",id,prev);
        }
    }
    for (&id,now) in &current {
        match t.peers.get(&id) {
            None => { if now.active() { event(&t,"duty_started",id,now); } }
            Some(prev) => {
                if prev.active() && !now.active() { event(&t,"duty_ended",id,prev); }
                else if !prev.active() && now.active() { event(&t,"duty_started",id,now); }
                else if prev.active() && now.active() && (prev.line!=now.line || prev.tour!=now.tour) {
                    event(&t,"duty_switched",id,prev);
                    event(&t,"duty_started",id,now);
                }
                if !now.route.is_empty() && prev.route!=now.route {
                    event(&t,"line_changed",id,now);
                }
                if !now.bus.is_empty() && prev.bus!=now.bus {
                    event(&t,"bus_changed",id,now);
                }
            }
        }
    }
    t.peers=current;
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duty_requires_both_line_and_tour() {
        assert!(!Duty { name:"".into(),line:"37".into(),tour:"".into(),route:"".into(),bus:"".into() }.active());
        assert!(Duty { name:"".into(),line:"37".into(),tour:"01".into(),route:"".into(),bus:"".into() }.active());
    }
    #[test]
    fn player_text_is_bounded_without_splitting_utf8() {
        assert!(clip(&"ä".repeat(100)).len()<=100);
        assert!(!clip("driver\r\nname").contains('\n'));
    }
}
