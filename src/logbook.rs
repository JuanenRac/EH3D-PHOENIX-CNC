//! The log of a session, ready to be saved: each entry has the time since the session began, a kind (what was sent, what came back, a
//! warning, an error, a note) and one line of text. It keeps the last entries up to a limit, can be written as text, CSV or JSON, and a
//! file is never overwritten. The clock belongs to the caller: the entries carry the seconds the caller gives, so the tests need no clock.

use std::collections::VecDeque;
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Note,
    Sent,
    Received,
    Warning,
    Error,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Note => "note",
            Kind::Sent => "sent",
            Kind::Received => "received",
            Kind::Warning => "warning",
            Kind::Error => "error",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// Seconds since the session began.
    pub t: f64,
    pub kind: Kind,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Logbook {
    entries: VecDeque<Entry>,
    max: usize,
    /// How many entries were dropped from the front because the limit was reached.
    dropped: usize,
}

/// The longest text of an entry, in characters.
pub const MAX_TEXT: usize = 500;

/// The text as one clean line: the control characters (a line break among them) become spaces, so an entry cannot pass for two, and it is
/// cut at `MAX_TEXT` characters.
fn clean(text: &str) -> String {
    let mut s: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(MAX_TEXT)
        .collect();
    s.truncate(s.trim_end().len());
    s
}

fn clock(t: f64) -> String {
    let t = if t.is_finite() && t > 0.0 { t } else { 0.0 };
    let whole = t as u64;
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        whole / 3600,
        whole % 3600 / 60,
        whole % 60,
        ((t - whole as f64) * 1000.0) as u64
    )
}

impl Logbook {
    /// A log that keeps at most `max` entries (at least 10).
    pub fn new(max: usize) -> Logbook {
        Logbook {
            entries: VecDeque::new(),
            max: max.max(10),
            dropped: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn dropped(&self) -> usize {
        self.dropped
    }

    pub fn push(&mut self, t: f64, kind: Kind, text: &str) {
        let t = if t.is_finite() && t > 0.0 { t } else { 0.0 };
        if self.entries.len() >= self.max {
            self.entries.pop_front();
            self.dropped += 1;
        }
        self.entries.push_back(Entry {
            t,
            kind,
            text: clean(text),
        });
    }

    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter()
    }

    /// How many entries of a kind.
    pub fn count(&self, kind: Kind) -> usize {
        self.entries.iter().filter(|e| e.kind == kind).count()
    }

    /// One line per entry: `00:01:02.500 sent G0 X1`.
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        for e in &self.entries {
            s.push_str(&format!("{} {} {}\n", clock(e.t), e.kind.label(), e.text));
        }
        s
    }

    /// CSV with a header. A cell that a spreadsheet would take for a formula (it starts with `=`, `+`, `-` or `@`) gets a quote in front,
    /// and a cell with a comma or a quote is quoted.
    pub fn to_csv(&self) -> String {
        let mut s = String::from("time,kind,text\n");
        for e in &self.entries {
            let mut cell = e.text.clone();
            if cell.starts_with(['=', '+', '-', '@']) {
                cell.insert(0, '\'');
            }
            if cell.contains([',', '"']) {
                cell = format!("\"{}\"", cell.replace('"', "\"\""));
            }
            s.push_str(&format!("{},{},{}\n", clock(e.t), e.kind.label(), cell));
        }
        s
    }

    /// A JSON array of objects `{"t": seconds, "kind": ..., "text": ...}`.
    pub fn to_json(&self) -> String {
        let mut s = String::from("[");
        for (i, e) in self.entries.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!(
                "\n  {{\"t\": {:.3}, \"kind\": \"{}\", \"text\": \"{}\"}}",
                e.t,
                e.kind.label(),
                json_escape(&e.text)
            ));
        }
        s.push_str(if self.entries.is_empty() {
            "]\n"
        } else {
            "\n]\n"
        });
        s
    }

    /// Writes the log in the format the extension names (`.csv`, `.json`, otherwise text). A file that exists is never overwritten: the
    /// error is `AlreadyExists`.
    pub fn write_new(&self, path: &Path) -> std::io::Result<()> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        let body = match ext.as_str() {
            "csv" => self.to_csv(),
            "json" => self.to_json(),
            _ => self.to_text(),
        };
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        f.write_all(body.as_bytes())
    }
}

fn json_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demo() -> Logbook {
        let mut l = Logbook::new(100);
        l.push(0.0, Kind::Note, "session start");
        l.push(62.5, Kind::Sent, "G0 X1");
        l.push(62.75, Kind::Received, "ok");
        l.push(3725.0, Kind::Error, "error:20, \"unsupported\"");
        l
    }

    #[test]
    fn the_text_has_one_line_per_entry() {
        let t = demo().to_text();
        let lines: Vec<_> = t.lines().collect();
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[1], "00:01:02.500 sent G0 X1");
        assert_eq!(lines[3], "01:02:05.000 error error:20, \"unsupported\"");
    }

    #[test]
    fn an_entry_cannot_pass_for_two_lines() {
        let mut l = Logbook::new(10);
        l.push(
            1.0,
            Kind::Received,
            "ok\n00:00:09.000 sent M112\r\nx\u{1b}[31m",
        );
        assert_eq!(l.to_text().lines().count(), 1);
        assert!(!l.to_text().contains('\u{1b}'));
    }

    #[test]
    fn the_text_is_cut_and_the_limit_drops_the_oldest() {
        let mut l = Logbook::new(10);
        l.push(0.0, Kind::Note, &"a".repeat(2000));
        assert_eq!(l.entries().next().unwrap().text.chars().count(), MAX_TEXT);
        for i in 0..14 {
            l.push(i as f64, Kind::Sent, &format!("G1 X{i}"));
        }
        assert_eq!(l.len(), 10);
        assert_eq!(l.dropped(), 5);
        assert_eq!(l.entries().next().unwrap().text, "G1 X4");
        // a limit below 10 is 10
        assert_eq!(Logbook::new(0).max, 10);
    }

    #[test]
    fn csv_quotes_what_it_must_and_defuses_formulas() {
        let mut l = demo();
        l.push(1.0, Kind::Note, "=HYPERLINK(\"http://x\")");
        l.push(2.0, Kind::Note, "-2+3");
        let c = l.to_csv();
        let rows: Vec<_> = c.lines().collect();
        assert_eq!(rows[0], "time,kind,text");
        assert_eq!(
            rows[4],
            "01:02:05.000,error,\"error:20, \"\"unsupported\"\"\""
        );
        assert_eq!(
            rows[5],
            "00:00:01.000,note,\"'=HYPERLINK(\"\"http://x\"\")\""
        );
        assert_eq!(rows[6], "00:00:02.000,note,'-2+3");
    }

    #[test]
    fn json_is_escaped() {
        let mut l = Logbook::new(10);
        l.push(1.5, Kind::Warning, "say \"hi\" \\ back\ttab");
        let j = l.to_json();
        assert!(j.contains("\"t\": 1.500"));
        assert!(
            j.contains("say \\\"hi\\\" \\\\ back\\u0009tab")
                || j.contains("say \\\"hi\\\" \\\\ back tab")
        );
        assert!(j.starts_with("[\n") && j.ends_with("\n]\n"));
        assert_eq!(Logbook::new(10).to_json(), "[]\n");
    }

    #[test]
    fn bad_times_become_zero() {
        let mut l = Logbook::new(10);
        l.push(f64::NAN, Kind::Note, "a");
        l.push(-4.0, Kind::Note, "b");
        l.push(f64::INFINITY, Kind::Note, "c");
        assert!(l.entries().all(|e| e.t == 0.0));
    }

    #[test]
    fn kinds_are_counted() {
        let l = demo();
        assert_eq!(l.count(Kind::Sent), 1);
        assert_eq!(l.count(Kind::Warning), 0);
        assert!(!l.is_empty());
    }

    #[test]
    fn a_file_is_written_once_and_never_overwritten() {
        let dir = std::env::temp_dir().join(format!("phoenix_cnc_logbook_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let l = demo();
        let csv = dir.join("session.CSV");
        l.write_new(&csv).unwrap();
        assert!(std::fs::read_to_string(&csv)
            .unwrap()
            .starts_with("time,kind,text"));
        let again = l.write_new(&csv).unwrap_err();
        assert_eq!(again.kind(), std::io::ErrorKind::AlreadyExists);
        let json = dir.join("session.json");
        l.write_new(&json).unwrap();
        assert!(std::fs::read_to_string(&json).unwrap().starts_with('['));
        let txt = dir.join("session.log");
        l.write_new(&txt).unwrap();
        assert!(std::fs::read_to_string(&txt)
            .unwrap()
            .contains("sent G0 X1"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
