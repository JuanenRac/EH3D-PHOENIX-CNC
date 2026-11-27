//! The work that is saved while a program runs: if the window, the computer or the link goes down in the middle of a program, the file that is left tells at the next
//! start which program it was and the line it had reached, and the operator chooses to go on from there or to leave it. Nothing here touches the controller.

use std::path::{Path, PathBuf};

/// What a program that was running needs to go on.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Recovery {
    /// The path of the G-code file that was running.
    pub file: String,
    /// The next line to send (the lines before it were sent).
    pub next: usize,
    /// How many lines the program has.
    pub total: usize,
    /// The id of the job on the EH3D server when the program was sent to it (empty: the program ran on the controller of this computer).
    pub job: String,
    /// The SHA-256 of the program (its lines joined by line breaks) when it was saved; empty in a file that an older version wrote.
    pub sha256: String,
    /// The work zero had been shifted by hand (`G92`): a reset of the controller takes the shift away, so the program is not resumed.
    pub shifted: bool,
}

/// The SHA-256 of a program, as the recovery file keeps it.
pub fn program_hash(lines: &[String]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    for line in lines {
        h.update(line.as_bytes());
        h.update(b"\n");
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

impl Recovery {
    pub fn to_text(&self) -> String {
        let mut text = format!(
            "file={}\nnext={}\ntotal={}\njob={}\nsha256={}\n",
            self.file.replace(['\r', '\n'], " "),
            self.next,
            self.total,
            self.job.replace(['\r', '\n'], " "),
            self.sha256.replace(['\r', '\n'], " ")
        );
        if self.shifted {
            text.push_str("shifted=yes\n");
        }
        text
    }

    /// Reads what `to_text` wrote; `None` when it is not one (a file that is cut or from another program).
    pub fn parse(text: &str) -> Option<Recovery> {
        let mut r = Recovery::default();
        let mut file = false;
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            match k.trim() {
                "file" => {
                    r.file = v.to_string();
                    file = true;
                }
                "next" => r.next = v.trim().parse().ok()?,
                "total" => r.total = v.trim().parse().ok()?,
                "job" => r.job = v.trim().to_string(),
                "sha256" => r.sha256 = v.trim().to_string(),
                "shifted" => r.shifted = v.trim() != "no",
                _ => {}
            }
        }
        (file && !r.file.is_empty() && r.total > 0 && r.next < r.total).then_some(r)
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        // written to a side file and renamed, so a crash in the middle of saving never leaves half of it
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, self.to_text())?;
        std::fs::rename(&tmp, path)
    }

    pub fn load(path: &Path) -> Option<Recovery> {
        Recovery::parse(&std::fs::read_to_string(path).ok()?)
    }

    pub fn clear(path: &Path) {
        let _ = std::fs::remove_file(path);
    }
}

/// Whether a line puts a move into the buffer of the controller: it has a word of an axis (or of the centre of an arc) and is not one that takes those
/// words to set an offset (`G10`, `G43.1`, `G92`).
fn moves(line: &str) -> bool {
    let words = crate::gcode::words(line);
    let offset = words
        .iter()
        .any(|&(l, v)| l == 'G' && matches!((v * 10.0).round() as i32, 100 | 431 | 920));
    !offset
        && words
            .iter()
            .any(|&(l, _)| matches!(l, 'X' | 'Y' | 'Z' | 'A' | 'B' | 'C' | 'I' | 'J' | 'K'))
}

/// The line a program that stopped at `reached` goes on from: `margin` lines that move before it. The controller says `ok` when it takes a line, not
/// when it has run it, so the last moves may not have been made; comments, blank lines and lines that move nothing take no room in its buffer and are
/// not counted.
pub fn backed_off(program: &[String], reached: usize, margin: usize) -> usize {
    let mut at = reached.min(program.len());
    let mut back = 0;
    while at > 0 && back < margin {
        at -= 1;
        if moves(&program[at]) {
            back += 1;
        }
    }
    at
}

/// The first `G10` line (from 0) among those that may have run already after `next`, where the program goes on: it changed an offset that the controller
/// keeps through a reset, so the lines before it would be run again with the new one. `margin` is the one [`backed_off`] took and `unsaved` how many
/// lines the program may have gone past the last save.
pub fn offset_set_again(
    program: &[String],
    next: usize,
    margin: usize,
    unsaved: usize,
) -> Option<usize> {
    let (mut end, mut ahead) = (next.min(program.len()), 0);
    while end < program.len() && ahead < margin {
        if moves(&program[end]) {
            ahead += 1;
        }
        end += 1;
    }
    let end = end.saturating_add(unsaved).min(program.len());
    (next.min(end)..end).find(|&n| {
        crate::gcode::words(&program[n])
            .iter()
            .any(|&(l, v)| l == 'G' && (v * 10.0).round() as i32 == 100)
    })
}

/// Where the recovery is kept: next to the settings.
pub fn recovery_path(settings: &Path) -> PathBuf {
    settings.with_file_name("recovery.cfg")
}

/// Is it time to save again: when the program has gone `every` lines past the last save, or when nothing has been saved yet.
pub fn due(saved: Option<usize>, next: usize, every: usize) -> bool {
    match saved {
        None => true,
        Some(s) => next.abs_diff(s) >= every,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_written_is_read_back() {
        let r = Recovery {
            file: "C:\\jobs\\board one.nc".to_string(),
            next: 120,
            total: 400,
            job: "j1a2b3c4d".to_string(),
            sha256: program_hash(&["G0 X1".to_string()]),
            shifted: false,
        };
        assert_eq!(Recovery::parse(&r.to_text()), Some(r));
    }

    #[test]
    fn the_hash_tells_a_changed_program_from_the_same_one() {
        let a = vec!["G0 X1".to_string(), "G1 Z-1".to_string()];
        let mut b = a.clone();
        assert_eq!(program_hash(&a), program_hash(&b));
        assert_eq!(program_hash(&a).len(), 64);
        b[1] = "G1 Z-2".to_string();
        assert_ne!(program_hash(&a), program_hash(&b));
        // a file written before the hash existed still reads, with no hash
        let old = Recovery::parse("file=a.nc\nnext=1\ntotal=3\n").unwrap();
        assert!(old.sha256.is_empty());
    }

    #[test]
    fn a_file_that_makes_no_sense_is_not_a_recovery() {
        assert!(Recovery::parse("").is_none());
        assert!(Recovery::parse("file=a.nc\nnext=x\ntotal=3\n").is_none());
        assert!(Recovery::parse("file=a.nc\nnext=3\ntotal=3\n").is_none());
        assert!(Recovery::parse("next=1\ntotal=3\n").is_none());
        assert!(Recovery::parse("file=a.nc\nnext=1\n").is_none());
    }

    #[test]
    fn a_name_with_a_line_break_cannot_add_keys() {
        let r = Recovery {
            file: "a.nc\nnext=0".to_string(),
            next: 5,
            total: 9,
            job: String::new(),
            sha256: String::new(),
            shifted: false,
        };
        let back = Recovery::parse(&r.to_text()).unwrap();
        assert_eq!(back.next, 5);
    }

    fn lines(s: &str) -> Vec<String> {
        s.lines().map(str::to_string).collect()
    }

    #[test]
    fn only_the_lines_that_move_count_in_the_margin_of_a_program_that_stopped() {
        let p =
            lines("G0 X1\n(a note)\nG1 X2 F100\n\nM8\nG92 X0\nG1 X3\n; more\nI1 J0\nF200\nG1 Y4");
        assert_eq!(backed_off(&p, p.len(), 0), p.len());
        assert_eq!(backed_off(&p, p.len(), 1), 10);
        assert_eq!(backed_off(&p, p.len(), 2), 8);
        assert_eq!(backed_off(&p, p.len(), 3), 6);
        // the comments, the blank line, the coolant and the offset take no room in the buffer
        assert_eq!(backed_off(&p, p.len(), 4), 2);
        assert_eq!(backed_off(&p, 99, 100), 0);
    }

    #[test]
    fn an_offset_set_in_the_lines_that_may_have_run_is_found() {
        let p = lines("G0 X1\nG1 X2\nG1 X3\nG10 L20 P1 X0\nG1 X4\nG1 X5");
        assert_eq!(offset_set_again(&p, 1, 3, 0), Some(3));
        assert_eq!(offset_set_again(&p, 1, 1, 0), None);
        assert_eq!(offset_set_again(&p, 1, 1, 2), Some(3));
        // before the line the program goes on from it stays as it is
        assert_eq!(offset_set_again(&p, 4, 16, 20), None);
        assert_eq!(offset_set_again(&p, 99, 16, 20), None);
    }

    #[test]
    fn it_is_saved_every_so_many_lines_and_cleared() {
        assert!(due(None, 0, 20));
        assert!(!due(Some(10), 25, 20));
        assert!(due(Some(10), 30, 20));
        let dir = std::env::temp_dir().join(format!("eh3d-phoenix-cnc-rec-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = recovery_path(&dir.join("settings.cfg"));
        let r = Recovery {
            file: "a.nc".into(),
            next: 3,
            total: 9,
            job: String::new(),
            sha256: String::new(),
            shifted: false,
        };
        r.save(&p).unwrap();
        assert_eq!(Recovery::load(&p), Some(r));
        Recovery::clear(&p);
        assert!(Recovery::load(&p).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_shift_of_the_zero_given_by_hand_goes_with_the_recovery() {
        let r = Recovery {
            file: "a.nc".into(),
            next: 1,
            total: 4,
            job: String::new(),
            sha256: String::new(),
            shifted: true,
        };
        assert!(r.to_text().contains("shifted=yes\n"));
        assert_eq!(Recovery::parse(&r.to_text()), Some(r));
        // a file written before says nothing of it
        assert!(
            !Recovery::parse("file=a.nc\nnext=1\ntotal=4\n")
                .unwrap()
                .shifted
        );
    }
}
