//! Machine profiles: a name and the values that belong to one machine (the travel of the axes, the speed of the spindle, the feeds and the
//! probe), kept in the settings file so that one program can serve several machines. Choosing a profile copies its values into the
//! settings; saving one copies the settings into it. Nothing here moves a machine: the travel only changes the limits the checks and the
//! jog hold to, and the settings window asks the operator to confirm with Ok.

use crate::probe::ProbeCfg;
use crate::settings::Settings;

/// The most profiles the settings keep.
pub const MAX_PROFILES: usize = 16;
/// The longest name of a profile, in characters.
pub const NAME_MAX: usize = 40;

/// The values of one machine.
#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    pub name: String,
    /// The travel of X, Y and Z in mm (minimum, maximum).
    pub travel: [(f64, f64); 3],
    /// The speed the menu turns the spindle on with, and the speed above which the check warns.
    pub spindle_rpm: f64,
    pub max_rpm: f64,
    /// The feed of the jog, the speed of the rapid moves and the feed above which the check warns, in mm per minute.
    pub feed_mm_min: f64,
    pub rapid_mm_min: f64,
    pub max_feed_mm_min: f64,
    pub probe: ProbeCfg,
}

/// A name as it is kept: without line breaks, the signs of the file (`|` and `=`) or spaces at the ends, no longer than `NAME_MAX`.
/// A name with nothing left is no name.
pub fn clean_name(raw: &str) -> Option<String> {
    let name: String = raw
        .chars()
        .map(|c| {
            if c.is_control() || c == '|' || c == '=' {
                ' '
            } else {
                c
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let name: String = name.chars().take(NAME_MAX).collect();
    let name = name.trim().to_string();
    (!name.is_empty()).then_some(name)
}

fn num(v: &str, lo: f64, hi: f64) -> Option<f64> {
    v.trim()
        .parse::<f64>()
        .ok()
        .filter(|x| x.is_finite() && (lo..=hi).contains(x))
}

fn pair(v: &str) -> Option<(f64, f64)> {
    let (lo, hi) = v.split_once(',')?;
    let (lo, hi) = (
        lo.trim().parse::<f64>().ok()?,
        hi.trim().parse::<f64>().ok()?,
    );
    (lo.is_finite() && hi.is_finite() && hi > lo).then_some((lo, hi))
}

impl Profile {
    /// The profile that holds the values the settings have now.
    pub fn of(name: &str, s: &Settings) -> Option<Profile> {
        Some(Profile {
            name: clean_name(name)?,
            travel: s.travel,
            spindle_rpm: s.spindle_rpm,
            max_rpm: s.lint.max_rpm,
            feed_mm_min: s.feed_mm_min,
            rapid_mm_min: s.rapid_mm_min,
            max_feed_mm_min: s.lint.max_feed_mm_min,
            probe: s.probe,
        })
    }

    /// Puts the values of the profile in the settings (the name is the active one).
    pub fn apply(&self, s: &mut Settings) {
        s.travel = self.travel;
        s.spindle_rpm = self.spindle_rpm;
        s.lint.max_rpm = self.max_rpm;
        s.feed_mm_min = self.feed_mm_min;
        s.rapid_mm_min = self.rapid_mm_min;
        s.lint.max_feed_mm_min = self.max_feed_mm_min;
        s.probe = self.probe;
        s.profile = self.name.clone();
    }

    /// The line of the settings file: the name, then `key=value` pieces separated by `|`.
    pub fn to_line(&self) -> String {
        let t = &self.travel;
        let p = &self.probe;
        format!(
            "{} | tx={},{} | ty={},{} | tz={},{} | spindle={} | max_rpm={} | feed={} | rapid={} | max_feed={} | probe_feed={} | probe_depth={} | probe_retract={} | probe_plate={} | probe_clearance={}",
            self.name,
            t[0].0,
            t[0].1,
            t[1].0,
            t[1].1,
            t[2].0,
            t[2].1,
            self.spindle_rpm,
            self.max_rpm,
            self.feed_mm_min,
            self.rapid_mm_min,
            self.max_feed_mm_min,
            p.feed_mm_min,
            p.depth_mm,
            p.retract_mm,
            p.plate_mm,
            p.clearance_mm
        )
    }

    /// Reads a line of the settings file. A value that is missing or not valid keeps the default one; a line with no name is not a profile.
    pub fn parse(line: &str) -> Option<Profile> {
        let mut parts = line.split('|');
        let name = clean_name(parts.next()?)?;
        let d = Settings::default();
        let mut p = Profile::of(&name, &d)?;
        for piece in parts {
            let Some((k, v)) = piece.split_once('=') else {
                continue;
            };
            let v = v.trim();
            match k.trim().to_ascii_lowercase().as_str() {
                "tx" | "ty" | "tz" => {
                    let i = (k.trim().as_bytes()[1].to_ascii_lowercase() - b'x') as usize;
                    if let Some(t) = pair(v) {
                        p.travel[i] = t;
                    }
                }
                "spindle" => p.spindle_rpm = num(v, 0.0, 1_000_000.0).unwrap_or(p.spindle_rpm),
                "max_rpm" => p.max_rpm = num(v, 0.0, 1_000_000.0).unwrap_or(p.max_rpm),
                "feed" => p.feed_mm_min = num(v, 1.0, 100_000.0).unwrap_or(p.feed_mm_min),
                "rapid" => p.rapid_mm_min = num(v, 1.0, 1_000_000.0).unwrap_or(p.rapid_mm_min),
                "max_feed" => {
                    p.max_feed_mm_min = num(v, 0.0, 1_000_000.0).unwrap_or(p.max_feed_mm_min)
                }
                "probe_feed" => {
                    p.probe.feed_mm_min = num(v, 1.0, 1000.0).unwrap_or(p.probe.feed_mm_min)
                }
                "probe_depth" => p.probe.depth_mm = num(v, 0.1, 200.0).unwrap_or(p.probe.depth_mm),
                "probe_retract" => {
                    p.probe.retract_mm = num(v, 0.0, 100.0).unwrap_or(p.probe.retract_mm)
                }
                "probe_plate" => p.probe.plate_mm = num(v, 0.0, 100.0).unwrap_or(p.probe.plate_mm),
                "probe_clearance" => {
                    p.probe.clearance_mm = num(v, 0.5, 200.0).unwrap_or(p.probe.clearance_mm)
                }
                _ => {}
            }
        }
        Some(p)
    }
}

impl Settings {
    /// Where the profile with this name is (capitals do not matter).
    fn profile_index(&self, name: &str) -> Option<usize> {
        let name = clean_name(name)?;
        self.profiles
            .iter()
            .position(|p| p.name.eq_ignore_ascii_case(&name))
    }

    /// Keeps the values the settings have now under a name; a profile with that name is replaced. Says the name it was kept under, or nothing
    /// when the name is empty or there is no room for one more.
    pub fn save_profile(&mut self, name: &str) -> Option<String> {
        let mut p = Profile::of(name, self)?;
        match self.profile_index(name) {
            Some(i) => {
                // the name keeps the capitals it had the first time
                p.name = self.profiles[i].name.clone();
                self.profiles[i] = p.clone();
            }
            None if self.profiles.len() >= MAX_PROFILES => return None,
            None => self.profiles.push(p.clone()),
        }
        self.profile = p.name.clone();
        Some(p.name)
    }

    /// Takes the values of a profile. Says whether there is one with that name.
    pub fn use_profile(&mut self, name: &str) -> bool {
        match self.profile_index(name) {
            Some(i) => {
                let p = self.profiles[i].clone();
                p.apply(self);
                true
            }
            None => false,
        }
    }

    /// Takes a profile away (the values in use stay). Says whether there was one.
    pub fn delete_profile(&mut self, name: &str) -> bool {
        match self.profile_index(name) {
            Some(i) => {
                let gone = self.profiles.remove(i);
                if self.profile.eq_ignore_ascii_case(&gone.name) {
                    self.profile.clear();
                }
                true
            }
            None => false,
        }
    }

    /// The lines of the settings file for the profiles and the active one.
    pub fn profiles_text(&self) -> String {
        let mut out = String::new();
        for p in &self.profiles {
            out.push_str(&format!("profile = {}\n", p.to_line()));
        }
        out.push_str(&format!(
            "profile_active = {}\n",
            crate::settings::one_line(&self.profile)
        ));
        out
    }

    /// Reads the profile lines of the settings file (the lines are those of `profiles_text`; anything else is left).
    pub fn read_profiles(&mut self, text: &str) {
        let mut active = String::new();
        for line in text.trim_start_matches('\u{feff}').lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            match k.trim().to_ascii_lowercase().as_str() {
                "profile" => {
                    if let Some(p) = Profile::parse(v) {
                        let known = self
                            .profiles
                            .iter()
                            .any(|q| q.name.eq_ignore_ascii_case(&p.name));
                        if !known && self.profiles.len() < MAX_PROFILES {
                            self.profiles.push(p);
                        }
                    }
                }
                "profile_active" => active = v.trim().to_string(),
                _ => {}
            }
        }
        // the active profile is one that is there
        self.profile = self
            .profiles
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(&active))
            .map(|p| p.name.clone())
            .unwrap_or_default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lathe() -> Settings {
        Settings {
            travel: [(0.0, 300.0), (0.0, 200.0), (-60.0, 10.0)],
            spindle_rpm: 8000.0,
            feed_mm_min: 300.0,
            rapid_mm_min: 1500.0,
            probe: ProbeCfg {
                feed_mm_min: 10.0,
                depth_mm: 5.0,
                retract_mm: 1.0,
                plate_mm: 1.6,
                clearance_mm: 3.0,
            },
            ..Settings::default()
        }
    }

    #[test]
    fn a_name_is_cleaned_and_a_name_with_nothing_left_is_no_name() {
        assert_eq!(
            clean_name("  Router  big \n").as_deref(),
            Some("Router big")
        );
        assert_eq!(clean_name("a|b=c").as_deref(), Some("a b c"));
        assert_eq!(clean_name("|=\n "), None);
        assert_eq!(clean_name(""), None);
        let long = "x".repeat(100);
        assert_eq!(clean_name(&long).unwrap().chars().count(), NAME_MAX);
        // an accented name is cut on a character
        assert_eq!(
            clean_name(&"é".repeat(100)).unwrap().chars().count(),
            NAME_MAX
        );
    }

    #[test]
    fn a_profile_takes_the_values_of_the_settings_and_gives_them_back() {
        let mut s = lathe();
        assert_eq!(
            s.save_profile(" Small mill "),
            Some("Small mill".to_string())
        );
        assert_eq!(s.profile, "Small mill");
        // the settings change; the profile brings the values back
        s.travel = [(0.0, 900.0), (0.0, 900.0), (-5.0, 5.0)];
        s.spindle_rpm = 1.0;
        s.probe = ProbeCfg::default();
        s.lint.max_rpm = 99.0;
        s.lint.max_feed_mm_min = 77.0;
        assert!(s.use_profile("small MILL"));
        assert_eq!(s.travel, lathe().travel);
        assert_eq!(
            (s.spindle_rpm, s.feed_mm_min, s.rapid_mm_min),
            (8000.0, 300.0, 1500.0)
        );
        assert_eq!(s.probe, lathe().probe);
        assert_eq!(s.lint.max_rpm, lathe().lint.max_rpm);
        assert_eq!(s.lint.max_feed_mm_min, lathe().lint.max_feed_mm_min);
        assert_eq!(s.profile, "Small mill");
        // a profile that is not there changes nothing
        let before = s.clone();
        assert!(!s.use_profile("Big router"));
        assert_eq!(s, before);
    }

    #[test]
    fn saving_under_a_name_that_is_there_replaces_it_and_the_room_is_limited() {
        let mut s = Settings::default();
        s.save_profile("A");
        s.feed_mm_min = 123.0;
        assert_eq!(s.save_profile("a"), Some("A".to_string()));
        assert_eq!(s.profiles.len(), 1);
        assert_eq!(s.profiles[0].feed_mm_min, 123.0);
        for i in 1..MAX_PROFILES {
            assert!(s.save_profile(&format!("m{i}")).is_some());
        }
        assert_eq!(s.profiles.len(), MAX_PROFILES);
        // no room for another, but one that is there can still be replaced
        assert_eq!(s.save_profile("one more"), None);
        assert!(s.save_profile("m3").is_some());
        assert_eq!(s.save_profile("  |  "), None);
        assert_eq!(s.profiles.len(), MAX_PROFILES);
    }

    #[test]
    fn a_profile_is_deleted_and_the_values_in_use_stay() {
        let mut s = lathe();
        s.save_profile("Mill");
        s.save_profile("Router");
        assert_eq!(s.profile, "Router");
        assert!(s.delete_profile("router"));
        assert_eq!(s.profile, "");
        assert_eq!(s.profiles.len(), 1);
        assert_eq!(s.travel, lathe().travel);
        assert!(!s.delete_profile("Router"));
        // deleting another one leaves the active name alone
        s.use_profile("Mill");
        s.save_profile("Third");
        s.use_profile("Mill");
        assert!(s.delete_profile("Third"));
        assert_eq!(s.profile, "Mill");
    }

    #[test]
    fn the_profiles_are_written_to_the_file_and_read_back() {
        let mut s = lathe();
        s.save_profile("Small mill");
        s.travel = [(-10.5, 410.25), (0.0, 250.0), (-80.0, 20.0)];
        s.spindle_rpm = 24_000.0;
        s.save_profile("Big router");
        s.use_profile("Small mill");
        let text = s.to_text();
        assert!(
            text.contains("profile = Small mill | tx=0,300 | ty=0,200 | tz=-60,10 | spindle=8000")
        );
        assert!(text.contains("profile_active = Small mill\n"));
        let back = Settings::parse(&text);
        assert_eq!(back.profiles, s.profiles);
        assert_eq!(back.profile, "Small mill");
        assert_eq!(back, s);
        // the default settings write no profile and read back the same
        let d = Settings::default().to_text();
        assert!(!d.contains("profile = "));
        assert_eq!(Settings::parse(&d), Settings::default());
    }

    #[test]
    fn a_line_that_is_not_valid_keeps_the_defaults_and_a_bad_file_cannot_make_a_mess() {
        let d = Settings::default();
        let p = Profile::parse(
            "Mill | tx=5,1 | spindle=-3 | feed=abc | rapid=2000 | nonsense | probe_depth=0 | zz=1",
        )
        .unwrap();
        // a travel with the maximum below the minimum, a speed under zero and text are left out
        assert_eq!(p.travel, d.travel);
        assert_eq!(p.spindle_rpm, d.spindle_rpm);
        assert_eq!(p.feed_mm_min, d.feed_mm_min);
        assert_eq!(p.rapid_mm_min, 2000.0);
        assert_eq!(p.probe.depth_mm, d.probe.depth_mm);
        assert!(Profile::parse("  | tx=0,5").is_none());
        assert!(Profile::parse("").is_none());
        // two lines with the same name keep the first; an active name that is not there is dropped
        let text = "profile = Mill | feed=100\nprofile = MILL | feed=200\nprofile_active = Ghost\n";
        let s = Settings::parse(text);
        assert_eq!(s.profiles.len(), 1);
        assert_eq!(s.profiles[0].feed_mm_min, 100.0);
        assert_eq!(s.profile, "");
        let s = Settings::parse("profile = Mill\nprofile_active = mill\n");
        assert_eq!(s.profile, "Mill");
        // more lines than the room allows are not kept
        let many: String = (0..40).map(|i| format!("profile = m{i}\n")).collect();
        assert_eq!(Settings::parse(&many).profiles.len(), MAX_PROFILES);
    }

    #[test]
    fn a_name_cannot_add_a_line_to_the_file() {
        let mut s = Settings {
            profile: "x\nlang = ja".to_string(),
            ..Settings::default()
        };
        let name = s.save_profile("evil\nlang = ja | feed=1").unwrap();
        assert!(!name.contains('\n') && !name.contains('|') && !name.contains('='));
        let back = Settings::parse(&s.to_text());
        assert_eq!(back.lang, Settings::default().lang);
        assert_eq!(back.profiles.len(), 1);
    }

    #[test]
    fn using_a_profile_does_not_touch_what_does_not_belong_to_a_machine() {
        let mut s = lathe();
        s.save_profile("Mill");
        let mut other = Settings {
            server_token: "token".into(),
            recent: vec!["a.nc".into()],
            profiles: s.profiles.clone(),
            ..Settings::default()
        };
        other.use_profile("Mill");
        assert_eq!(other.server_token, "token");
        assert_eq!(other.recent, ["a.nc"]);
        assert_eq!(other.travel, lathe().travel);
    }
}
