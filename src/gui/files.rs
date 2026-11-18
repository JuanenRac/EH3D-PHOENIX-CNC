//! The files the window reads and writes besides the G-code programs: the height map (saved to a file and loaded from one, through the same
//! file windows as the programs) and the log of the session (exported as text, CSV or JSON). A height map is never written over a file without
//! the operator saying yes, and the log is never written over a file at all. The map is `heightmap`, the log is `logbook`; the words are those
//! of `words`.

use super::*;
use crate::heightmap::HeightMap;
use crate::logbook::classify;
use crate::words::Msg;
use eh3d_filedialog::Mode;

/// What the file window that is open is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Purpose {
    /// A G-code program (open, save, queue).
    Program,
    MapSave,
    MapLoad,
    LogSave,
}

/// The biggest height map file read (a grid of 100 x 100 points is a few hundred kilobytes).
const MAX_MAP_BYTES: u64 = 8 * 1024 * 1024;

impl App {
    /// The title of the file window when it is not for a program (the menu entry without its dots).
    pub(super) fn purpose_title(&self) -> Option<&'static str> {
        let m = match self.file_purpose {
            Purpose::Program => return None,
            Purpose::MapSave => Msg::MenuMapSave,
            Purpose::MapLoad => Msg::MenuMapLoad,
            Purpose::LogSave => Msg::MenuLogExport,
        };
        Some(self.w(m).trim_end_matches('.'))
    }

    /// The name of the first type in the list of types of the file window.
    pub(super) fn first_filter_label(&self) -> &'static str {
        match self.file_purpose {
            Purpose::Program => self.t(Text::McFilterGcode),
            Purpose::MapSave | Purpose::MapLoad => self.w(Msg::FilterMap),
            Purpose::LogSave => self.w(Msg::FilterLog),
        }
    }

    /// The operator chose a file in the file window: it is opened or saved as what the window is for.
    pub(super) fn file_chosen(&mut self, mode: Mode, path: &Path) {
        match (self.file_purpose, mode) {
            (Purpose::Program, Mode::Open) => self.open(path),
            (Purpose::Program, _) => {
                if let Err(e) = self.save_program(path) {
                    self.session.log.push(e);
                }
            }
            (Purpose::MapLoad, _) => self.load_height_map(path),
            (Purpose::MapSave, _) => self.save_height_map(path),
            (Purpose::LogSave, _) => self.export_log(path),
        }
    }

    /// Writes the height map of the session to a file (the window asked the operator before it was written over a file).
    pub(super) fn save_height_map(&mut self, path: &Path) {
        let Some(map) = self.session.height_map.as_ref() else {
            let said = self.w(Msg::MapNone).to_string();
            self.session.log.push(said);
            return;
        };
        match std::fs::write(path, map.to_text()) {
            Ok(()) => {
                let said = self.ws(Msg::MapSaved, &[path.display().to_string()]);
                self.session.log.push(said);
            }
            Err(e) => self
                .session
                .log
                .push(format!("heightmap: {}: {e}", path.display())),
        }
    }

    /// Takes a height map from a file. What is not a map, or does not fit its grid, changes nothing and the reason goes to the log.
    pub(super) fn load_height_map(&mut self, path: &Path) {
        let text = match std::fs::metadata(path) {
            Ok(m) if m.len() > MAX_MAP_BYTES => {
                self.session.log.push(format!(
                    "heightmap: {} is too big for a height map",
                    path.display()
                ));
                return;
            }
            Ok(_) => std::fs::read_to_string(path),
            Err(e) => Err(e),
        };
        match text
            .map_err(|e| e.to_string())
            .and_then(|t| HeightMap::parse(&t))
        {
            Ok(map) => {
                let said = self.ws(
                    Msg::MapLoaded,
                    &[map.z.len().to_string(), path.display().to_string()],
                );
                self.session.height_map = Some(map);
                self.session.log.push(said);
            }
            Err(e) => self
                .session
                .log
                .push(format!("heightmap: {}: {e}", path.display())),
        }
    }

    /// Writes the log of the session in the format the extension names (`.csv`, `.json`, otherwise text). A file that is there is never
    /// written over.
    pub(super) fn export_log(&mut self, path: &Path) {
        self.log_collect();
        match self.logbook.write_new(path) {
            Ok(()) => {
                let said = self.ws(Msg::LogExported, &[path.display().to_string()]);
                self.session.log.push(said);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let said = self.ws(Msg::LogExists, &[path.display().to_string()]);
                self.session.log.push(said);
            }
            Err(e) => self
                .session
                .log
                .push(format!("log: {}: {e}", path.display())),
        }
    }

    /// Takes the lines the session added to its log since the last call into the log that can be exported, with the time since the
    /// window opened and a kind guessed from the line.
    pub(super) fn log_collect(&mut self) {
        let len = self.session.log.len();
        let from = self
            .log_seen
            .saturating_sub(self.session.log_dropped)
            .min(len);
        let now = self.started.elapsed().as_secs_f64();
        for line in &self.session.log[from..] {
            self.logbook.push(now, classify(line), line);
        }
        self.log_seen = self.session.log_dropped + len;
    }

    /// The entries of the Machine menu that save and load the height map.
    pub(super) fn map_files_menu(&mut self, ui: &mut egui::Ui) {
        let idle = !self.session.running && !self.session.busy();
        if ui
            .add_enabled(
                self.session.height_map.is_some(),
                egui::Button::new(self.w(Msg::MenuMapSave)),
            )
            .clicked()
        {
            self.win = Some(Win::Save(self.browser_for(Mode::Save, Purpose::MapSave)));
            ui.close();
        }
        if ui
            .add_enabled(idle, egui::Button::new(self.w(Msg::MenuMapLoad)))
            .clicked()
        {
            self.win = Some(Win::Open(self.browser_for(Mode::Open, Purpose::MapLoad)));
            ui.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("eh3d_files_{}_{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn a_map() -> HeightMap {
        let readings: Vec<f64> = (0..6).map(|i| f64::from(i) * 0.1).collect();
        HeightMap::from_readings([0.0, 0.0, 10.0, 5.0], 3, 2, &readings).unwrap()
    }

    #[test]
    fn a_height_map_is_written_and_read_back_through_the_files_of_the_window() {
        let d = folder("map");
        let file = d.join("board.map");
        let mut a = App::isolated();
        // nothing to save yet
        a.save_height_map(&file);
        assert!(!file.exists());
        assert!(a
            .session
            .log
            .iter()
            .any(|l| l.contains("there is none to save")));
        a.session.height_map = Some(a_map());
        a.file_purpose = Purpose::MapSave;
        a.file_chosen(Mode::Save, &file);
        assert!(file.is_file());
        assert!(a.session.log.iter().any(|l| l.contains("saved to")));
        // another window reads it
        let mut b = App::isolated();
        b.file_purpose = Purpose::MapLoad;
        b.file_chosen(Mode::Open, &file);
        assert_eq!(b.session.height_map, a.session.height_map);
        assert!(b
            .session
            .log
            .iter()
            .any(|l| l.contains("6 points read from")));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_file_that_is_not_a_height_map_changes_nothing_and_says_why() {
        let d = folder("bad");
        let mut a = App::isolated();
        a.session.height_map = Some(a_map());
        let before = a.session.height_map.clone();
        a.file_purpose = Purpose::MapLoad;
        for (name, text) in [
            ("text.map", "G21\nG1 X1\n".to_string()),
            ("cut.map", "heightmap 0 0 10 5 3 2\n0 0.1 0.2\n".to_string()),
            (
                "nan.map",
                "heightmap 0 0 10 5 2 2\n0 NaN\n0 0\n".to_string(),
            ),
            ("empty.map", String::new()),
        ] {
            let f = d.join(name);
            std::fs::write(&f, text).unwrap();
            a.file_chosen(Mode::Open, &f);
            assert_eq!(a.session.height_map, before, "{name}");
        }
        assert_eq!(
            a.session
                .log
                .iter()
                .filter(|l| l.starts_with("heightmap:"))
                .count(),
            4
        );
        // a file that is not there, and one that is too big
        a.load_height_map(&d.join("missing.map"));
        assert_eq!(a.session.height_map, before);
        let big = d.join("big.map");
        std::fs::write(&big, vec![b'0'; 9 * 1024 * 1024]).unwrap();
        a.load_height_map(&big);
        assert!(a.session.log.iter().any(|l| l.contains("too big")));
        assert_eq!(a.session.height_map, before);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_window_knows_what_its_file_is_for() {
        let mut a = App::isolated();
        assert_eq!(a.purpose_title(), None);
        a.file_purpose = Purpose::MapSave;
        assert_eq!(a.purpose_title(), Some("Save the height map"));
        assert_eq!(a.first_filter_label(), "Height maps");
        a.file_purpose = Purpose::LogSave;
        assert_eq!(a.purpose_title(), Some("Export the session log"));
        assert_eq!(a.first_filter_label(), "Session logs");
        // a window for a program resets it
        let _ = a.browser_for(Mode::Open, Purpose::MapLoad);
        assert_eq!(a.file_purpose, Purpose::MapLoad);
        let _ = a.browser_for(Mode::Open, Purpose::Program);
        assert_eq!(a.file_purpose, Purpose::Program);
    }

    #[test]
    fn the_log_of_the_session_is_exported_in_the_format_of_the_extension_and_never_over_a_file() {
        let d = folder("log");
        let mut a = App::isolated();
        a.session.mdi("M5");
        a.session.log.push("heightmap: something odd".to_string());
        let txt = d.join("session.txt");
        a.file_purpose = Purpose::LogSave;
        a.file_chosen(Mode::Save, &txt);
        let text = std::fs::read_to_string(&txt).unwrap();
        assert!(
            text.contains("sent > M5") || text.contains("sent M5"),
            "{text}"
        );
        assert!(text.contains("heightmap: something odd"), "{text}");
        let csv = d.join("session.csv");
        a.file_chosen(Mode::Save, &csv);
        assert!(std::fs::read_to_string(&csv)
            .unwrap()
            .starts_with("time,kind,text\n"));
        let json = d.join("session.json");
        a.file_chosen(Mode::Save, &json);
        assert!(std::fs::read_to_string(&json).unwrap().starts_with('['));
        // the same name again is refused and the file stays as it was
        std::fs::write(&txt, "mine").unwrap();
        a.file_chosen(Mode::Save, &txt);
        assert_eq!(std::fs::read_to_string(&txt).unwrap(), "mine");
        assert!(a.session.log.iter().any(|l| l.contains("already exists")));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_log_is_taken_once_and_the_lines_dropped_from_the_session_do_not_repeat_it() {
        let mut a = App::isolated();
        a.session.log.push("one".into());
        a.session.log.push("two".into());
        a.log_collect();
        a.log_collect();
        assert_eq!(a.logbook.len(), 2);
        a.session.log.push("three".into());
        a.log_collect();
        assert_eq!(a.logbook.len(), 3);
        // the session drops its oldest lines when the log gets long: what was taken is not taken again
        a.session.log = (0..150).map(|i| format!("line {i}")).collect();
        a.session.log_dropped = 100;
        a.log_seen = 100 + 3;
        a.log_collect();
        // lines 3.. of the new vector are new: 147 lines
        assert_eq!(a.logbook.len(), 3 + 147);
        assert_eq!(a.log_seen, 250);
    }

    #[test]
    fn the_log_kept_for_export_is_limited_by_the_settings() {
        let mut a = App::isolated();
        a.logbook = crate::logbook::Logbook::new(10);
        a.session.log = (0..30).map(|i| format!("n{i}")).collect();
        a.log_collect();
        assert_eq!(a.logbook.len(), 10);
        assert_eq!(a.logbook.dropped(), 20);
    }
}
