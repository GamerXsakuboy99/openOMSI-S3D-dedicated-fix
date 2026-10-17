//! `Holidays.txt` and `timezone.txt` (`TMap.loadGlobalFile` - "Load Calendar").

use omsi_cfg::CfgFile;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct HolidayRange {
    /// YYYYMMDD
    pub start: i32,
    pub end: i32,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Holiday {
    pub date: i32,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Dst {
    pub start: i32,
    pub end: i32,
    /// The hour summer time starts on `start` and ends on `end`, and the hours the clocks
    /// go forward (OMSI keeps them as floats).
    pub params: [f32; 3],
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TimeZone {
    pub offset_hours: f32,
    pub dst: Vec<Dst>,
    pub location: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Calendar {
    pub ranges: Vec<HolidayRange>,
    pub holidays: Vec<Holiday>,
}

impl Calendar {
    pub fn load(path: &Path) -> Result<Calendar, omsi_cfg::CfgError> {
        let f = CfgFile::read(path)?;
        Ok(Self::parse(&f))
    }
    pub fn parse(f: &CfgFile) -> Calendar {
        let mut c = Calendar::default();
        let mut r = f.reader();
        while let Some(k) = r.next_keyword() {
            match k.as_str() {
                "holidays" => {
                    let start = r.i32();
                    let end = r.i32();
                    let name = r.str().to_string();
                    c.ranges.push(HolidayRange { start, end, name });
                }
                "holiday" => {
                    let date = r.i32();
                    let name = r.str().to_string();
                    c.holidays.push(Holiday { date, name });
                }
                _ => {}
            }
        }
        c
    }
    /// Is `yyyymmdd` a holiday (single day or within a school-holiday range)?
    pub fn is_holiday(&self, date: i32) -> bool {
        self.holidays.iter().any(|h| h.date == date)
    }
    pub fn in_holiday_range(&self, date: i32) -> bool {
        self.ranges.iter().any(|h| date >= h.start && date <= h.end)
    }
}

/// [ROLLBACK schoolbits-2] The school bit of a tour's validity mask that counts on a day: in the
/// school holidays (a `[holidays]` range) bit 9, else bit 8. Read from Omsi.exe: the `.ttl` writes
/// and reads bit 8 (256) as the tour's byte +0x29 `NoHols` ("No Hols": runs outside the school
/// holidays, the school days) and bit 9 (512) as +0x28 `Hols` ("Hols": runs in the school
/// holidays), and the validity check wants `Hols` when the day's school-holiday name is set,
/// `NoHols` otherwise. (They had been read the other way round since Round 7, from the order
/// the record's fields are listed in, which is not the order of the bits.)
pub fn school_bit(in_school_holidays: bool) -> i32 {
    if in_school_holidays {
        1 << 9
    } else {
        1 << 8
    }
}

/// Does a tour with validity `mask` run on a day with these bits (the weekday or holiday bit, and
/// `school_bit`)? Both are needed, as in the original.
pub fn mask_runs(mask: i32, day_bit: i32, school_bit: i32) -> bool {
    mask & day_bit != 0 && mask & school_bit != 0
}

impl TimeZone {
    /// Latitude and longitude from `[location]` (north and east positive).
    pub fn lat_lon(&self) -> Option<(f64, f64)> {
        let lat = self.location.first().map(|s| omsi_cfg::parse_f64(s))?;
        let lon = self.location.get(1).map(|s| omsi_cfg::parse_f64(s))?;
        Some((lat, lon))
    }

    pub fn load(path: &Path) -> Result<TimeZone, omsi_cfg::CfgError> {
        let f = CfgFile::read(path)?;
        Ok(Self::parse(&f))
    }
    pub fn parse(f: &CfgFile) -> TimeZone {
        let mut t = TimeZone::default();
        let mut r = f.reader();
        while let Some(k) = r.next_keyword() {
            match k.as_str() {
                "timezone" => t.offset_hours = r.f32(),
                "dst" => {
                    let start = r.i32();
                    let end = r.i32();
                    let params = [r.f32(), r.f32(), r.f32()];
                    t.dst.push(Dst { start, end, params });
                }
                "location" => t.location = r.rest_of_block().into_iter().map(|s| s.to_string()).collect(),
                _ => {}
            }
        }
        t
    }
}

#[cfg(test)]
mod mask_tests {
    use super::{mask_runs, school_bit};

    #[test]
    fn school_bits() {
        // Mon-Fri, "No Hols" (bit 8): the school days; "Hols" (bit 9): the school holidays
        let (term, holidays) = (31 | 256, 31 | 512);
        assert!(mask_runs(term, 1, school_bit(false)));
        assert!(!mask_runs(term, 1, school_bit(true)));
        assert!(mask_runs(holidays, 1, school_bit(true)));
        assert!(!mask_runs(holidays, 1, school_bit(false)));
        // both ticked (1023): every day
        assert!(mask_runs(1023, 1, school_bit(true)) && mask_runs(1023, 64, school_bit(false)));
        // the day still has to be one
        assert!(!mask_runs(term, 32, school_bit(false)));
    }
}
