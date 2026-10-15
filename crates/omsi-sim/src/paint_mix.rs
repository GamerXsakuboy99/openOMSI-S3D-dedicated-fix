//! A paint that is a mix of the repaints a vehicle has.
//!
//! A repaint (a `.cti` `[item]`) swaps the textures of the `[CTCTexture]` slots and sets some
//! variables (`[setvar]`). The fleet of Career Mode wants less than that all-or-nothing: the
//! textures of another repaint on a bus that keeps the settings of its own, or one slot (the
//! destination blind, the livery's stripe) from a third repaint, and nothing of the bus's
//! configuration changed.
//!
//! The mix is written in the paint's name, which every part of the game already passes along
//! (`--paint`, the thumbnail cache, the showroom):
//!
//! ```text
//! Red                                the repaint "Red" whole (textures and settings)
//! Red||tex=Blue                      Red's settings, Blue's textures
//! ||tex=Blue                         the model's own settings, Blue's textures
//! Red||slot:Stripe=Blue              Red, but the slot "Stripe" has Blue's texture
//! Red||tex=Blue||slot:Stripe=Green   both
//! ```
//!
//! [`apply`] turns such a name into one more scheme of the loaded vehicle type (appended, named
//! as the mix), so `spawn::paint_scheme` finds it by name like any other.

use crate::vehicle::{PaintScheme, VehicleType};
use std::path::Path;

/// Between the parts of a mix.
const SEP: &str = "||";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Mix {
    /// The repaint worn (its textures and its settings); empty: the model's own.
    pub base: String,
    /// A repaint whose textures alone are put on (not its settings); empty: none.
    pub textures_of: String,
    /// `[CTCTexture]` slot, the repaint that gives that slot's texture.
    pub slots: Vec<(String, String)>,
}

impl Mix {
    /// Read a paint name; one without a mix is the plain repaint of that name.
    pub fn parse(text: &str) -> Mix {
        let mut parts = text.split(SEP);
        let mut mix = Mix { base: parts.next().unwrap_or("").trim().to_string(), ..Default::default() };
        for part in parts {
            let part = part.trim();
            if let Some(name) = part.strip_prefix("tex=") {
                mix.textures_of = name.trim().to_string();
            } else if let Some(rest) = part.strip_prefix("slot:") {
                if let Some((slot, from)) = rest.split_once('=') {
                    let (slot, from) = (slot.trim(), from.trim());
                    if !slot.is_empty() && !from.is_empty() {
                        mix.slots.push((slot.to_string(), from.to_string()));
                    }
                }
            }
        }
        mix
    }

    /// Whether it is just a repaint of that name (or the model's own).
    pub fn is_plain(&self) -> bool {
        self.textures_of.is_empty() && self.slots.is_empty()
    }

    /// The paint name for it: the base alone when the mix is plain.
    pub fn encode(&self) -> String {
        let mut out = self.base.trim().to_string();
        if !self.textures_of.is_empty() {
            out.push_str(SEP);
            out.push_str("tex=");
            out.push_str(&self.textures_of);
        }
        for (slot, from) in &self.slots {
            out.push_str(SEP);
            out.push_str("slot:");
            out.push_str(slot);
            out.push('=');
            out.push_str(from);
        }
        out
    }

    /// The repaint a slot takes its texture from, if the mix names one.
    pub fn slot_from(&self, slot: &str) -> Option<&str> {
        self.slots.iter().find(|(s, _)| s.eq_ignore_ascii_case(slot)).map(|(_, f)| f.as_str())
    }

    /// Set (or, with an empty `from`, take away) the repaint a slot comes from.
    pub fn set_slot(&mut self, slot: &str, from: &str) {
        self.slots.retain(|(s, _)| !s.eq_ignore_ascii_case(slot));
        if !from.trim().is_empty() {
            self.slots.push((slot.to_string(), from.trim().to_string()));
        }
    }
}

/// Whether a paint name is a mix (not one of a vehicle's own repaints).
pub fn is_mix(paint: &str) -> bool {
    paint.contains(SEP)
}

fn texture_of<'a>(scheme: &'a PaintScheme, slot: &str) -> Option<&'a String> {
    scheme.textures.iter().rev().find(|(s, _)| s.eq_ignore_ascii_case(slot)).map(|(_, f)| f)
}

fn put(textures: &mut Vec<(String, String)>, slot: &str, file: String) {
    textures.retain(|(s, _)| !s.eq_ignore_ascii_case(slot));
    textures.push((slot.to_string(), file));
}

/// The scheme a mix comes to, named `name`: the base's textures and settings, then the textures
/// of `textures_of`, then the slots one by one. A texture taken from a repaint that lies in
/// another folder is given whole (`resolve` makes the full path of a file of a folder), so it is
/// found there.
pub fn build(schemes: &[PaintScheme], mix: &Mix, name: &str, resolve: &dyn Fn(&Path, &str) -> String) -> PaintScheme {
    let find = |n: &str| schemes.iter().find(|s| !n.is_empty() && !is_mix(&s.name) && s.name.eq_ignore_ascii_case(n));
    let base = find(&mix.base);
    let dir = base.map(|b| b.dir.clone()).or_else(|| schemes.first().map(|s| s.dir.clone())).unwrap_or_default();
    let mut out = PaintScheme { name: name.to_string(), dir: dir.clone(), textures: Vec::new(), set_vars: Vec::new() };
    if let Some(b) = base {
        out.textures = b.textures.clone();
        out.set_vars = b.set_vars.clone();
    }
    let file_of = |from: &PaintScheme, file: &str| if from.dir == dir { file.to_string() } else { resolve(&from.dir, file) };
    if let Some(other) = find(&mix.textures_of) {
        for (slot, file) in &other.textures {
            put(&mut out.textures, slot, file_of(other, file));
        }
    }
    for (slot, from) in &mix.slots {
        let Some(other) = find(from) else { continue };
        if let Some(file) = texture_of(other, slot) {
            put(&mut out.textures, slot, file_of(other, file));
        }
    }
    out
}

/// Put the scheme of a mixed paint name into the vehicle type, and return its index. A name
/// that is no mix (or already there) is left alone: the index of the scheme of that name, if
/// there is one. With `at`, the scheme goes to that index (the list is padded with unnamed
/// schemes up to it), so the parts of an articulated bus, which wear a scheme by its number, wear
/// the same one; a list already longer takes it at its end (a rear section with more repaints than
/// the front one then wears one of its own: the parts of a bus nearly always list the same).
pub fn apply(vt: &mut VehicleType, paint: &str, at: Option<usize>) -> Option<usize> {
    let paint = paint.trim();
    if paint.is_empty() {
        return None;
    }
    if let Some(i) = vt.paint_schemes.iter().position(|s| s.name.eq_ignore_ascii_case(paint)) {
        return Some(i);
    }
    if !is_mix(paint) {
        return None;
    }
    let mix = Mix::parse(paint);
    let resolve = |dir: &Path, file: &str| {
        let p = omsi_cfg::resolve_path(dir, file);
        if omsi_cfg::vfs::is_file(&p) {
            p.to_string_lossy().into_owned()
        } else {
            file.to_string()
        }
    };
    let scheme = build(&vt.paint_schemes, &mix, paint, &resolve);
    if let Some(at) = at {
        while vt.paint_schemes.len() < at {
            vt.paint_schemes.push(PaintScheme::default());
        }
    }
    vt.paint_schemes.push(scheme);
    log::info!("{}: paint mix '{paint}' made, scheme {}", vt.def.path.display(), vt.paint_schemes.len() - 1);
    Some(vt.paint_schemes.len() - 1)
}

/// The `[CTCTexture]` slots of a vehicle type with the repaints that give each a texture (a
/// mix is not offered as a source): what the livery editor lets the player choose from.
pub fn slots(vt: &VehicleType) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    for (slot, _) in &vt.model.ctc_textures {
        if out.iter().any(|(s, _)| s.eq_ignore_ascii_case(slot)) {
            continue;
        }
        let mut names: Vec<String> = Vec::new();
        for s in &vt.paint_schemes {
            if s.name.trim().is_empty() || is_mix(&s.name) || texture_of(s, slot).is_none() || names.iter().any(|n| n.eq_ignore_ascii_case(&s.name)) {
                continue;
            }
            names.push(s.name.clone());
        }
        if !names.is_empty() {
            out.push((slot.clone(), names));
        }
    }
    out
}

/// The repaints of a vehicle type that can be worn or lent their textures: its own, in the
/// order of its files, never a mix or an unnamed one.
pub fn repaints(vt: &VehicleType) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in &vt.paint_schemes {
        if !s.name.trim().is_empty() && !is_mix(&s.name) && !out.iter().any(|n| n.eq_ignore_ascii_case(&s.name)) {
            out.push(s.name.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn scheme(name: &str, dir: &str, textures: &[(&str, &str)], vars: &[(&str, f32)]) -> PaintScheme {
        PaintScheme {
            name: name.into(),
            dir: PathBuf::from(dir),
            textures: textures.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(),
            set_vars: vars.iter().map(|(a, b)| (a.to_string(), *b)).collect(),
        }
    }

    fn list() -> Vec<PaintScheme> {
        vec![
            scheme("Red", "Vehicles/B/Texture/Red", &[("Body", "red.dds"), ("Stripe", "stripe_red.dds")], &[("Roof", 1.0)]),
            scheme("Blue", "Vehicles/B/Texture/Blue", &[("Body", "blue.dds"), ("Stripe", "stripe_blue.dds")], &[("Roof", 2.0), ("Mirror", 1.0)]),
            scheme("Green", "Vehicles/B/Texture/Red", &[("Stripe", "stripe_green.dds")], &[]),
        ]
    }

    fn whole(dir: &Path, file: &str) -> String {
        format!("{}/{}", dir.display(), file)
    }

    #[test]
    fn a_mix_is_read_and_written_back() {
        let m = Mix::parse("Red||tex=Blue||slot:Stripe=Green");
        assert_eq!(m, Mix { base: "Red".into(), textures_of: "Blue".into(), slots: vec![("Stripe".into(), "Green".into())] });
        assert_eq!(m.encode(), "Red||tex=Blue||slot:Stripe=Green");
        assert!(!m.is_plain() && is_mix("Red||tex=Blue"));
        // a plain name is the repaint itself, and writes back as it was
        let p = Mix::parse(" Red ");
        assert!(p.is_plain() && !is_mix("Red") && p.encode() == "Red");
        // no base: the model's own settings
        let o = Mix::parse("||tex=Blue");
        assert_eq!((o.base.as_str(), o.textures_of.as_str()), ("", "Blue"));
        assert_eq!(o.encode(), "||tex=Blue");
        // a slot with no repaint is not kept
        assert!(Mix::parse("Red||slot:Stripe=").slots.is_empty());
        let mut m = Mix::parse("Red");
        m.set_slot("Stripe", "Blue");
        m.set_slot("stripe", "Green");
        assert_eq!(m.slots, vec![("stripe".to_string(), "Green".to_string())]);
        assert_eq!(m.slot_from("STRIPE"), Some("Green"));
        m.set_slot("Stripe", "");
        assert!(m.is_plain());
    }

    #[test]
    fn the_textures_of_another_repaint_keep_the_settings_of_the_first() {
        let s = list();
        let got = build(&s, &Mix::parse("Red||tex=Blue"), "Red||tex=Blue", &whole);
        // Blue's textures (from Blue's folder, so given whole), Red's settings only
        assert_eq!(got.textures.len(), 2);
        assert!(got.textures.contains(&("Body".to_string(), "Vehicles/B/Texture/Blue/blue.dds".to_string())));
        assert_eq!(got.set_vars, vec![("Roof".to_string(), 1.0)]);
        assert_eq!(got.dir, PathBuf::from("Vehicles/B/Texture/Red"));
        assert_eq!(got.name, "Red||tex=Blue");
    }

    #[test]
    fn one_slot_from_a_repaint_and_the_rest_as_it_was() {
        let s = list();
        // Green lies in Red's folder: the file name stays as the folder reads it
        let got = build(&s, &Mix::parse("Red||slot:stripe=Green"), "x", &whole);
        assert_eq!(got.textures.len(), 2);
        assert!(got.textures.contains(&("Body".to_string(), "red.dds".to_string())));
        assert!(got.textures.contains(&("stripe".to_string(), "stripe_green.dds".to_string())) || got.textures.contains(&("Stripe".to_string(), "stripe_green.dds".to_string())));
        assert_eq!(got.set_vars.len(), 1);
        // a repaint with no texture for the slot, or one that is not there: nothing changes
        let same = build(&s, &Mix::parse("Red||slot:Body=Green||slot:Stripe=Nobody"), "x", &whole);
        assert_eq!(same.textures, s[0].textures);
    }

    #[test]
    fn from_the_model_s_own_look_only_the_chosen_textures_come() {
        let s = list();
        let got = build(&s, &Mix::parse("||tex=Blue"), "x", &whole);
        assert!(got.set_vars.is_empty());
        assert_eq!(got.textures.len(), 2);
        let only_slot = build(&s, &Mix::parse("||slot:Stripe=Blue"), "x", &whole);
        assert_eq!(only_slot.textures, vec![("Stripe".to_string(), "Vehicles/B/Texture/Blue/stripe_blue.dds".to_string())]);
        assert!(only_slot.set_vars.is_empty());
    }

    #[test]
    fn a_mix_is_never_a_source() {
        let mut s = list();
        let made = build(&s, &Mix::parse("Red||tex=Blue"), "Red||tex=Blue", &whole);
        s.push(made);
        // the made scheme is in the list, but asking for it by name as a base finds nothing
        let by_name = Mix { base: "Red||tex=Blue".into(), ..Default::default() };
        assert!(build(&s, &by_name, "y", &whole).textures.is_empty());
        assert_eq!(s.len(), 4);
    }
}
