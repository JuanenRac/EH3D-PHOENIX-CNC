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
        format!(
            "file={}\nnext={}\ntotal={}\njob={}\nsha256={}\n",
            self.file.replace(['\r', '\n'], " "),
            self.next,
            self.total,
            self.job.replace(['\r', '\n'], " "),
            self.sha256.replace(['\r', '\n'], " ")
        )
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
        };
        let back = Recovery::parse(&r.to_text()).unwrap();
        assert_eq!(back.next, 5);
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
        };
        r.save(&p).unwrap();
        assert_eq!(Recovery::load(&p), Some(r));
        Recovery::clear(&p);
        assert!(Recovery::load(&p).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
