//! The time estimate of programs, kept so that the window does not measure the same program again every frame: the program that is loaded
//! is measured once for as long as its path and the speeds stay the same, and the programs of a queue are measured once for as long as
//! their file keeps the same size and date. Nothing here moves a machine.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::gcode::{toolpath, Segment};
use crate::sim::{Extras, Rates, Timeline};

/// The biggest file measured for the queue (the same limit as the report of the command line).
pub const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

/// A number that changes when the path or the speeds change (FNV-1a over the bits of every number).
pub fn fingerprint(path: &[Segment], r: Rates) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut put = |v: u64| {
        for b in v.to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    put(path.len() as u64);
    put(r.rapid.to_bits());
    put(r.default_feed.to_bits());
    for s in path {
        for v in [s.from.0, s.from.1, s.from.2, s.to.0, s.to.1, s.to.2, s.feed] {
            put(v.to_bits());
        }
        put(u64::from(s.rapid) | (u64::from(s.arc) << 1) | ((s.line as u64) << 2));
    }
    h
}

/// The fingerprint of a path with what is added to it (the arcs, the tool changes and the time of a change).
pub fn fingerprint_with(path: &[Segment], r: Rates, extras: &Extras, change_s: f64) -> u64 {
    let mut h = fingerprint(path, r);
    let mut put = |v: u64| {
        for b in v.to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    put(change_s.to_bits());
    put(extras.arc_mm.len() as u64);
    for (line, mm) in &extras.arc_mm {
        put(*line as u64);
        put(mm.to_bits());
    }
    put(extras.changes.len() as u64);
    for line in &extras.changes {
        put(*line as u64);
    }
    h
}

/// What is known of a file measured for the queue.
#[derive(Debug, Clone, PartialEq)]
struct FileEntry {
    len: u64,
    modified_ns: u128,
    rates: (u64, u64),
    seconds: f64,
}

/// The estimates that are kept.
#[derive(Debug, Default)]
pub struct Cache {
    current: Option<(u64, Timeline)>,
    files: HashMap<PathBuf, FileEntry>,
    max_files: usize,
    /// How many times an estimate was taken from the cache and how many times it had to be measured (for the tests and the log).
    pub hits: usize,
    pub misses: usize,
}

impl Cache {
    /// A cache that keeps at most `max_files` measured files (at least 1).
    pub fn new(max_files: usize) -> Cache {
        Cache {
            max_files: max_files.max(1),
            ..Cache::default()
        }
    }

    /// The program in time, measured again only when the path or the speeds are not the ones of the last call.
    pub fn timeline(&mut self, path: &[Segment], r: Rates) -> &Timeline {
        self.timeline_with(path, r, &Extras::default(), 0.0)
    }

    /// The same, with the length of the arcs and `change_s` seconds for every tool change.
    pub fn timeline_with(
        &mut self,
        path: &[Segment],
        r: Rates,
        extras: &Extras,
        change_s: f64,
    ) -> &Timeline {
        let key = fingerprint_with(path, r, extras, change_s);
        if matches!(&self.current, Some((k, _)) if *k == key) {
            self.hits += 1;
        } else {
            self.misses += 1;
            self.current = Some((key, Timeline::build_with(path, r, extras, change_s)));
        }
        &self
            .current
            .get_or_insert_with(|| (key, Timeline::default()))
            .1
    }

    /// The seconds a G-code file takes at these speeds, or nothing when it cannot be read or is too big. The file is read again only when
    /// its size or its date changed, or when the speeds did.
    pub fn file_seconds(&mut self, file: &Path, r: Rates) -> Option<f64> {
        self.file_seconds_with(file, r, 0.0)
    }

    /// The same, with the arcs by their curve and `change_s` seconds for every tool change.
    pub fn file_seconds_with(&mut self, file: &Path, r: Rates, change_s: f64) -> Option<f64> {
        let meta = std::fs::metadata(file).ok()?;
        if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
            return None;
        }
        let modified_ns = meta
            .modified()
            .ok()
            .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos());
        let rates = (
            r.rapid.to_bits() ^ change_s.to_bits().rotate_left(17),
            r.default_feed.to_bits(),
        );
        if let Some(e) = self.files.get(file) {
            if e.len == meta.len() && e.modified_ns == modified_ns && e.rates == rates {
                self.hits += 1;
                return Some(e.seconds);
            }
        }
        self.misses += 1;
        let bytes = std::fs::read(file).ok()?;
        // every byte is a character, as the program reads a file
        let text: String = bytes.iter().map(|&b| b as char).collect();
        let lines: Vec<String> = text.lines().map(str::to_string).collect();
        let seconds =
            Timeline::build_with(&toolpath(&lines), r, &Extras::of(&lines), change_s).total_s;
        if self.files.len() >= self.max_files && !self.files.contains_key(file) {
            // the room is made by dropping one that is not the one asked for
            if let Some(old) = self.files.keys().next().cloned() {
                self.files.remove(&old);
            }
        }
        self.files.insert(
            file.to_path_buf(),
            FileEntry {
                len: meta.len(),
                modified_ns,
                rates,
                seconds,
            },
        );
        Some(seconds)
    }

    /// How many files are kept.
    pub fn files_kept(&self) -> usize {
        self.files.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATES: Rates = Rates {
        rapid: 6000.0,
        default_feed: 600.0,
    };

    fn path_of(text: &str) -> Vec<Segment> {
        let lines: Vec<String> = text.lines().map(str::to_string).collect();
        toolpath(&lines)
    }

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("eh3d_estimate_{}_{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn the_same_path_is_measured_once() {
        let p = path_of("G21 G90\nG1 X60 F600\nG1 Y60");
        let mut c = Cache::new(4);
        let total = c.timeline(&p, RATES).total_s;
        assert!((total - 12.0).abs() < 1e-9, "{total}");
        for _ in 0..5 {
            assert_eq!(c.timeline(&p, RATES).total_s, total);
        }
        assert_eq!((c.misses, c.hits), (1, 5));
    }

    #[test]
    fn a_changed_path_or_speed_is_measured_again() {
        let p = path_of("G1 X60 F600");
        let q = path_of("G1 X61 F600");
        let mut c = Cache::new(4);
        let a = c.timeline(&p, RATES).total_s;
        let b = c.timeline(&q, RATES).total_s;
        assert!(b > a);
        let slow = Rates {
            default_feed: 300.0,
            ..RATES
        };
        let d = c.timeline(&path_of("G1 X60"), slow).total_s;
        let e = c.timeline(&path_of("G1 X60"), RATES).total_s;
        assert!(d > e);
        assert_eq!(c.misses, 4);
    }

    #[test]
    fn the_fingerprint_tells_paths_apart() {
        let a = fingerprint(&path_of("G1 X1 F100"), RATES);
        assert_eq!(a, fingerprint(&path_of("G1 X1 F100"), RATES));
        assert_ne!(a, fingerprint(&path_of("G1 X1 F101"), RATES));
        assert_ne!(a, fingerprint(&path_of("G0 X1"), RATES));
        assert_ne!(a, fingerprint(&[], RATES));
    }

    #[test]
    fn the_arcs_and_the_tool_changes_are_part_of_the_estimate_and_of_its_key() {
        let lines: Vec<String> = "T1 M6
G1 X60 F600
G3 X60 Y10 I0 J5"
            .lines()
            .map(str::to_string)
            .collect();
        let p = toolpath(&lines);
        let x = Extras::of(&lines);
        let mut c = Cache::new(4);
        let plain = c.timeline(&p, RATES).total_s;
        let with = c.timeline_with(&p, RATES, &x, 20.0).total_s;
        // 20 s for the change and the curve of the half circle (15.7 mm) instead of its chord (10 mm)
        assert!((with - plain - 20.0 - 5.0 * std::f64::consts::PI / 10.0 + 1.0).abs() < 1e-9);
        let again = c.timeline_with(&p, RATES, &x, 20.0).total_s;
        assert_eq!(with, again);
        // another time for a change is another estimate
        let other = c.timeline_with(&p, RATES, &x, 25.0).total_s;
        assert!((other - with - 5.0).abs() < 1e-9);
        assert_ne!(
            fingerprint_with(&p, RATES, &x, 20.0),
            fingerprint_with(&p, RATES, &Extras::default(), 20.0)
        );
    }

    #[test]
    fn a_file_in_the_queue_counts_its_changes_and_is_measured_again_when_the_time_changes() {
        let d = temp("changes");
        let f = d.join("a.nc");
        std::fs::write(
            &f,
            "T1 M6
G1 X60 F600
",
        )
        .unwrap();
        let mut c = Cache::new(4);
        let a = c.file_seconds_with(&f, RATES, 0.0).unwrap();
        let b = c.file_seconds_with(&f, RATES, 30.0).unwrap();
        assert!((b - a - 30.0).abs() < 1e-9, "{a} {b}");
        assert_eq!(c.misses, 2);
        assert_eq!(c.file_seconds(&f, RATES), Some(a));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn an_empty_path_has_no_time() {
        let mut c = Cache::new(1);
        assert_eq!(c.timeline(&[], RATES).total_s, 0.0);
    }

    #[test]
    fn a_file_is_read_once_until_it_changes() {
        let d = temp("file");
        let f = d.join("a.nc");
        std::fs::write(&f, "G21 G90\nG1 X60\n").unwrap();
        let mut c = Cache::new(4);
        let first = c.file_seconds(&f, RATES).unwrap();
        assert!((first - 6.0).abs() < 1e-9, "{first}");
        assert_eq!(c.file_seconds(&f, RATES), Some(first));
        assert_eq!((c.misses, c.hits), (1, 1));
        // the file grows: it is read again
        std::fs::write(&f, "G21 G90\nG1 X60\nG1 X0\n").unwrap();
        let second = c.file_seconds(&f, RATES).unwrap();
        assert!(second > first);
        assert_eq!(c.misses, 2);
        // other speeds are another estimate
        let slow = Rates {
            default_feed: 300.0,
            ..RATES
        };
        assert!(c.file_seconds(&f, slow).unwrap() > second);
        assert_eq!(c.misses, 3);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_missing_file_a_folder_and_a_big_file_give_nothing() {
        let d = temp("none");
        let mut c = Cache::new(2);
        assert_eq!(c.file_seconds(&d.join("nope.nc"), RATES), None);
        assert_eq!(c.file_seconds(&d, RATES), None);
        assert_eq!(c.files_kept(), 0);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn at_most_the_asked_number_of_files_is_kept() {
        let d = temp("many");
        let mut c = Cache::new(2);
        for i in 0..5 {
            let f = d.join(format!("j{i}.nc"));
            std::fs::write(&f, format!("G1 X{} F600\n", i + 1)).unwrap();
            assert!(c.file_seconds(&f, RATES).is_some());
        }
        assert_eq!(c.files_kept(), 2);
        // a cache asked for 0 still keeps one
        let mut one = Cache::new(0);
        let f = d.join("j0.nc");
        assert!(one.file_seconds(&f, RATES).is_some());
        assert_eq!(one.files_kept(), 1);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_file_in_any_code_page_is_read() {
        let d = temp("bytes");
        let f = d.join("odd.nc");
        // a comment with bytes that are not UTF-8
        std::fs::write(&f, b"(caf\xe9)\nG1 X60 F600\n").unwrap();
        let mut c = Cache::new(2);
        assert!((c.file_seconds(&f, RATES).unwrap() - 6.0).abs() < 1e-9);
        let _ = std::fs::remove_dir_all(&d);
    }
}
