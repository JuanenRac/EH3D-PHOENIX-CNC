//! The entries of the menu File for a project file of the suite (`.eh3d`): open it, save it and save it under another name without
//! losing a branch, import a program or a drawing keeping the original, take a file of another branch and extract a branch.
//!
//! The work is done by the functions of the command `project` (`eh3dfile`); here it is decided what the window holds, what goes into
//! the branch and what the operator has to agree to first. Only the branch `cnc` is written. A project that was not opened here, a
//! branch that changed since it was read and a signature that a write removes are asked about before anything is written. The table of
//! tools and the motion file of a project are kept in it and never applied to the machine, and nothing here moves it.

use super::*;
use crate::eh3dfile::{self, Contents, Held, Identity, Opened, BRANCH};
use crate::suitecore::{self, name_of, ProjectView};
use crate::suitetext::{Said, S};
use eh3d_filedialog::Mode;

/// The line of the motion file that holds the credential of a link: a motion file with it never goes into a project.
const CREDENTIAL_KEY: &str = "klipper.api_key";

/// Something that writes a project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Action {
    Save(PathBuf),
    Import(PathBuf),
    Take(String, String),
}

/// A write that waits for the operator to agree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Pending {
    action: Action,
    pub warnings: Vec<Said>,
}

/// How an entry ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Step {
    Done,
    /// The window has no project yet: a name has to be asked for.
    NeedsName,
    /// Nothing was written yet: the operator has to agree.
    Asked,
    /// It was refused; the log says why.
    Refused,
}

/// The list of the project that is on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SuiteList {
    Branches,
    Take,
    Extract,
}

/// What the window keeps about its project file of the suite.
#[derive(Debug, Default)]
pub(super) struct SuiteState {
    /// The project the window works on.
    pub file: Option<PathBuf>,
    view: Option<ProjectView>,
    notes: Vec<Said>,
    extra: Vec<String>,
    pub pending: Option<Pending>,
    /// The content hash of the branch of the program when it was read or written.
    seen: Option<String>,
    /// The name the program has in the project.
    program_name: Option<String>,
    /// The operator asked for the table of tools, or the motion file, to go into the project.
    pub put_tools: bool,
    pub put_motion: bool,
    pub extract_branch: String,
    list: Option<SuiteList>,
    was_focused: bool,
}

/// What an import or a take gives: the notices of the write, the lines to tell and what was done.
type Brought = (Vec<String>, Vec<Said>, Said);

/// The bytes of a program as they are written into a project: a file that was read byte by byte goes back byte by byte.
fn program_bytes(lines: &[String]) -> Vec<u8> {
    let mut text = lines.join("\n");
    text.push('\n');
    if text.chars().all(|c| u32::from(c) < 256) {
        text.chars().map(|c| u32::from(c) as u8).collect()
    } else {
        text.into_bytes()
    }
}

/// Whether a program of a branch has the lines of the one in the window.
fn same_lines(bytes: &[u8], lines: &[String]) -> bool {
    let held = eh3dfile::program_lines(bytes);
    held.len() == lines.len() && held.iter().zip(lines).all(|(a, b)| a.trim_end() == b)
}

/// Leaves out what the branch already holds, so a file taken from another branch stays where it is with its reference.
fn drop_unchanged(contents: &mut Contents, held: &Opened, lines: &[String]) {
    let file = |rel: &Option<String>| rel.as_ref().and_then(|r| held.files.get(r));
    if contents.program.is_some() && file(&held.program).is_some_and(|b| same_lines(b, lines)) {
        contents.program = None;
    }
    if contents
        .tools
        .as_ref()
        .is_some_and(|h| file(&held.tools) == Some(&h.bytes))
    {
        contents.tools = None;
    }
    if contents
        .motion
        .as_ref()
        .is_some_and(|h| file(&held.motion) == Some(&h.bytes))
    {
        contents.motion = None;
    }
}

impl App {
    fn suite_code(&self) -> &'static str {
        self.lang.code()
    }

    /// Says a message in the log, in the language of the window.
    fn suite_tell(&mut self, said: &Said) {
        let line = said.text(self.suite_code());
        self.session.log.push(line);
    }

    fn suite_fail(&mut self, why: String) -> Step {
        self.session.log.push(why);
        Step::Refused
    }

    fn suite_refuse(&mut self, word: S) -> Step {
        self.suite_tell(&Said::plain(word));
        Step::Refused
    }

    /// Reads the project again to tell what is out of date now. What the window holds does not change.
    pub(super) fn suite_refresh(&mut self) {
        if let Some(file) = &self.suite.file {
            self.suite.view = suitecore::view(file, BRANCH).ok();
        }
    }

    /// The lines that stay in sight while the window works on a project; the flag marks the ones that ask for attention.
    pub(super) fn suite_banner(&self) -> Vec<(String, bool)> {
        let Some(file) = &self.suite.file else {
            return Vec::new();
        };
        let code = self.suite_code();
        let mut lines = suitecore::banner(file, self.suite.view.as_ref(), BRANCH, code);
        lines.extend(self.suite.notes.iter().map(|n| (n.text(code), false)));
        lines.extend(self.suite.extra.iter().map(|n| (n.clone(), true)));
        lines
    }

    /// Opens a project. A queue that was there is dropped, as when a program is opened.
    pub(super) fn suite_open(&mut self, path: &Path) -> bool {
        if !self.machine_idle() {
            self.suite_refuse(S::Busy);
            return false;
        }
        self.queue = Queue::default();
        self.queue_seen = false;
        self.queue_gate = None;
        self.load_project(path)
    }

    /// Takes the program of the branch `cnc` of a project as the program of the window; a project without one leaves the window with
    /// no program, so that saving never puts the program of before into it. A project that cannot be read changes nothing.
    pub(super) fn load_project(&mut self, path: &Path) -> bool {
        if !self.machine_idle() {
            self.suite_refuse(S::Busy);
            return false;
        }
        let found = match suitecore::view(path, BRANCH) {
            Ok(found) => found,
            Err(e) => {
                self.suite_fail(e);
                return false;
            }
        };
        let has_branch = found.branch(BRANCH).is_some();
        let held = if has_branch {
            match eh3dfile::held(path) {
                Ok(held) => held,
                Err(e) => {
                    self.suite_fail(e);
                    return false;
                }
            }
        } else {
            Opened::default()
        };
        if let Err(e) = self.suite_show_program(path, &held) {
            self.suite_fail(e);
            return false;
        }
        let mut notes = Vec::new();
        if held.tools.is_some() || held.motion.is_some() {
            notes.push(Said::plain(S::MachineFilesNotApplied));
        }
        if let Some(report) = &held.report {
            notes.extend(suitecore::report_lines(report));
        }
        self.suite.notes = notes;
        self.suite.extra = held
            .warnings
            .iter()
            .filter(|w| !w.starts_with("OUT OF DATE"))
            .cloned()
            .collect();
        self.suite.pending = None;
        self.suite.put_tools = false;
        self.suite.put_motion = false;
        self.suite.seen = found.own_hash.clone();
        self.suite.view = Some(found);
        self.suite.file = Some(path.to_path_buf());
        self.remember(path);
        let said = if has_branch {
            Said(S::Opened, vec![name_of(path)])
        } else {
            Said(S::NoWork, vec![name_of(path), BRANCH.to_owned()])
        };
        self.suite_tell(&said);
        true
    }

    /// Puts the program of the branch in the window, or none. The path of the program is the project, so that the recovery of a run
    /// and the bookmarks find it again.
    fn suite_show_program(&mut self, path: &Path, held: &Opened) -> Result<(), String> {
        let program = match &held.program {
            Some(rel) => {
                let bytes = held
                    .files
                    .get(rel)
                    .ok_or_else(|| format!("{BRANCH}/{rel}: the program is not in the branch"))?;
                let text: String = bytes.iter().map(|&b| char::from(b)).collect();
                Some((name_of(Path::new(rel)), text))
            }
            None => None,
        };
        self.transform_undo = None;
        self.focus_line = None;
        match program {
            Some((name, text)) => {
                self.session.load(&text);
                self.bookmarks.clear();
                self.refresh_path();
                self.path = path.display().to_string();
                self.load_marks_for(path);
                self.suite.program_name = Some(name);
            }
            None => {
                self.clear_program();
                self.path.clear();
                self.suite.program_name = None;
            }
        }
        Ok(())
    }

    /// The name the program of the window has in a project.
    fn suite_program_name(&self) -> String {
        let path = Path::new(&self.path);
        let name = if self.path.is_empty() {
            None
        } else if suitecore::is_project(path) {
            self.suite.program_name.clone()
        } else {
            Some(name_of(path))
        };
        match name {
            Some(name) if eh3dfile::is_program(&name) => name,
            Some(name) => format!("{name}.nc"),
            None => "program.nc".to_owned(),
        }
    }

    /// A file of the machine that the operator asked to put into the project.
    fn suite_machine_file(&self, put: bool, path: &Path) -> Result<Option<Held>, String> {
        if !put {
            return Ok(None);
        }
        let bytes = eh3d_project::appbranch::read_bounded(path, crate::motioncfg::MAX_FILE_BYTES)
            .map_err(|e| e.to_string())?;
        Ok(Some(Held {
            name: path.display().to_string(),
            bytes,
        }))
    }

    /// What the window would put into the branch, and what is to be said about what it leaves out.
    pub(super) fn suite_contents(&self) -> Result<(Contents, Vec<Said>), String> {
        let mut notes = Vec::new();
        let program = (!self.session.program.is_empty()).then(|| Held {
            name: self.suite_program_name(),
            bytes: program_bytes(&self.session.program),
        });
        let tools = self.suite_machine_file(
            self.suite.put_tools,
            &crate::tools::table_path(&self.settings_path),
        )?;
        let mut motion =
            self.suite_machine_file(self.suite.put_motion, &motion_path(&self.settings_path))?;
        // the project is a file that is passed on: the credential of a link never goes into it
        let has_credential = motion.as_ref().is_some_and(|held| {
            String::from_utf8_lossy(&held.bytes)
                .lines()
                .any(|l| l.trim_start().starts_with(CREDENTIAL_KEY))
        });
        if has_credential {
            motion = None;
            notes.push(Said::plain(S::MotionWithCredential));
        }
        let contents = Contents {
            program,
            drawing: None,
            tools,
            motion,
        };
        Ok((contents, notes))
    }

    /// The name offered for a project that is saved for the first time or under another name.
    pub(super) fn suite_suggested_name(&self) -> String {
        let from = self
            .suite
            .file
            .clone()
            .or_else(|| (!self.path.is_empty()).then(|| PathBuf::from(&self.path)));
        let stem = from
            .as_deref()
            .and_then(Path::file_stem)
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "job".to_owned());
        format!("{stem}.eh3d")
    }

    /// The name of the folder offered to extract a branch into.
    pub(super) fn suite_extract_name(&self) -> String {
        let stem = self
            .suite
            .file
            .as_deref()
            .and_then(Path::file_stem)
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "job".to_owned());
        format!("{stem}-{}", self.suite.extract_branch.replace('/', "-"))
    }

    /// Saves into the project of the window, or says that it has none.
    pub(super) fn suite_save(&mut self) -> Step {
        match self.suite.file.clone() {
            Some(file) => self.suite_run(Action::Save(file), false),
            None => Step::NeedsName,
        }
    }

    /// Saves under another name. A new file gets every branch of the project that is open; a file that is there keeps its own
    /// branches and only `cnc` is written, once the operator agreed.
    pub(super) fn suite_save_as(&mut self, target: &Path) -> Step {
        self.suite_run(Action::Save(suitecore::with_extension(target)), false)
    }

    /// Keeps the original of a program or a drawing of another program in the project.
    pub(super) fn suite_import(&mut self, source: &Path) -> Step {
        self.suite_run(Action::Import(source.to_path_buf()), false)
    }

    /// Takes a file of another branch into the project, with the reference that tells later when it is out of date.
    pub(super) fn suite_take(&mut self, branch: &str, rel: &str) -> Step {
        self.suite_run(Action::Take(branch.to_owned(), rel.to_owned()), false)
    }

    /// The operator agreed to the write that was waiting.
    pub(super) fn suite_confirm(&mut self) -> Step {
        match self.suite.pending.take() {
            Some(pending) => self.suite_run(pending.action, true),
            None => Step::Refused,
        }
    }

    /// The operator did not agree: nothing was written and nothing is.
    pub(super) fn suite_cancel(&mut self) {
        self.suite.pending = None;
    }

    /// Writes the files of a branch into a folder; a file that is there is never written over.
    pub(super) fn suite_extract(&mut self, branch: &str, dir: &Path) -> bool {
        let Some(file) = self.suite.file.clone() else {
            self.suite_refuse(S::NeedProject);
            return false;
        };
        match eh3dfile::extract(&file, branch, dir) {
            Ok(count) => {
                let said = Said(
                    S::Extracted,
                    vec![count.to_string(), dir.display().to_string()],
                );
                self.suite_tell(&said);
                true
            }
            Err(e) => {
                self.suite_fail(e);
                false
            }
        }
    }

    /// Asks for the agreement of the operator when there is something to agree to.
    fn suite_ask(&mut self, target: &Path, action: &Action) -> Option<Step> {
        let found = suitecore::warnings(
            target,
            self.suite.file.as_deref(),
            self.suite.seen.as_deref(),
            BRANCH,
        );
        match found {
            Ok(warnings) if warnings.is_empty() => None,
            Ok(warnings) => {
                self.suite.pending = Some(Pending {
                    action: action.clone(),
                    warnings,
                });
                Some(Step::Asked)
            }
            Err(e) => Some(self.suite_fail(e)),
        }
    }

    fn suite_run(&mut self, action: Action, agreed: bool) -> Step {
        self.suite.pending = None;
        match &action {
            Action::Save(target) => self.suite_write(target, &action, agreed),
            Action::Import(source) => self.suite_bring(&action, agreed, |app, file| {
                let (report, said) =
                    eh3dfile::import_file(file, source, &Identity::default(), &app.motion)?;
                let held = eh3dfile::held(file)?;
                app.suite_show_program(file, &held)?;
                Ok((
                    said,
                    suitecore::report_lines(&report),
                    Said(S::Imported, vec![report.source.name]),
                ))
            }),
            Action::Take(branch, name) => self.suite_bring(&action, agreed, |app, file| {
                let (target, said) = eh3dfile::take(file, branch, name, &app.motion)?;
                let held = eh3dfile::held(file)?;
                if held.program.as_deref() == Some(target.as_str()) {
                    app.suite_show_program(file, &held)?;
                }
                Ok((
                    said,
                    Vec::new(),
                    Said(S::Taken, vec![branch.clone(), name.clone()]),
                ))
            }),
        }
    }

    /// What an import and a take share: a machine at rest, a project, the agreement of the operator when there is something to agree
    /// to, and a look at the project after the write.
    fn suite_bring(
        &mut self,
        action: &Action,
        agreed: bool,
        work: impl FnOnce(&mut App, &Path) -> Result<Brought, String>,
    ) -> Step {
        if !self.machine_idle() {
            return self.suite_refuse(S::Busy);
        }
        let Some(file) = self.suite.file.clone() else {
            return self.suite_refuse(S::NeedProject);
        };
        if !agreed {
            if let Some(step) = self.suite_ask(&file, action) {
                return step;
            }
        }
        match work(self, &file) {
            Ok((said, notes, done)) => {
                self.suite_written(&file, &said, notes);
                self.suite_tell(&done);
                Step::Done
            }
            Err(e) => self.suite_fail(e),
        }
    }

    fn suite_write(&mut self, target: &Path, action: &Action, agreed: bool) -> Step {
        if !suitecore::is_project(target) {
            return self.suite_refuse(S::NeedExtension);
        }
        let (mut contents, notes) = match self.suite_contents() {
            Ok(found) => found,
            Err(e) => return self.suite_fail(e),
        };
        let nothing_held = contents.is_empty();
        let current = self.suite.file.clone();
        let same = current
            .as_deref()
            .is_some_and(|c| suitecore::same_file(c, target));
        let copy_from = current.filter(|c| !target.exists() && c.is_file());
        // what the branch has already, in the file that will be written or in the one that is copied to it
        let base = if target.exists() {
            Some(target.to_path_buf())
        } else {
            copy_from.clone()
        };
        if let Some(base) = &base {
            let found = match suitecore::view(base, BRANCH) {
                Ok(found) => found,
                Err(e) => return self.suite_fail(e),
            };
            if found.branch(BRANCH).is_some() {
                match eh3dfile::held(base) {
                    Ok(held) => drop_unchanged(&mut contents, &held, &self.session.program),
                    Err(e) => return self.suite_fail(e),
                }
            }
        }
        if contents.is_empty() && copy_from.is_none() {
            if !same {
                return self.suite_refuse(S::NothingToSave);
            }
            self.suite.notes = notes;
            self.suite_tell(&Said::plain(if nothing_held {
                S::NothingToSave
            } else {
                S::NothingNew
            }));
            return Step::Done;
        }
        if !contents.is_empty() && !agreed {
            if let Some(step) = self.suite_ask(target, action) {
                return step;
            }
        }
        if let Some(from) = &copy_from {
            if let Err(e) = suitecore::copy_project(from, target) {
                return self.suite_fail(e);
            }
        }
        let name = contents.program.as_ref().map(|held| held.name.clone());
        let said = if contents.is_empty() {
            Vec::new()
        } else {
            match eh3dfile::save_contents(target, &contents, &Identity::default(), &self.motion) {
                Ok(said) => said,
                Err(e) => {
                    // a copy that could not be written to is not left behind
                    if copy_from.is_some() {
                        let _ = std::fs::remove_file(target);
                    }
                    return self.suite_fail(e);
                }
            }
        };
        if name.is_some() {
            self.suite.program_name = name;
        }
        // a program that came from the project before follows it to the new name
        if suitecore::is_project(Path::new(&self.path)) {
            self.path = target.display().to_string();
        }
        self.suite_written(target, &said, notes);
        self.remember(target);
        self.suite_tell(&Said(S::Saved, vec![BRANCH.to_owned(), name_of(target)]));
        Step::Done
    }

    /// After a write: the project is the one of the window, its branch is the one that was seen and the notices are told.
    fn suite_written(&mut self, file: &Path, said: &[String], mut notes: Vec<Said>) {
        let (words, other) = suitecore::notices(said);
        for word in &words {
            self.suite_tell(word);
        }
        notes.extend(words);
        self.suite.file = Some(file.to_path_buf());
        self.suite.view = suitecore::view(file, BRANCH).ok();
        self.suite.seen = self.suite.view.as_ref().and_then(|v| v.own_hash.clone());
        self.suite.notes = notes;
        self.suite.extra = other;
    }

    /// The window of files for an entry of the project.
    pub(super) fn suite_browse(&mut self, mode: Mode, purpose: Purpose) {
        let browser = self.browser_for(mode, purpose);
        self.win = Some(if mode == Mode::Open {
            Win::Open(browser)
        } else {
            Win::Save(browser)
        });
    }

    /// The entry Save: into the project of the window, or a name is asked for.
    pub(super) fn suite_save_entry(&mut self) {
        if self.suite_save() == Step::NeedsName {
            self.suite_browse(Mode::Save, Purpose::SuiteSaveAs);
        }
    }

    /// Shows a list of the project, read again.
    pub(super) fn suite_show_list(&mut self, list: SuiteList) {
        self.suite_refresh();
        self.suite.list = Some(list);
    }

    /// The project stays in sight under the top bar: its name, what is out of date, what a write would unsign and what was last done.
    /// It is read again when the window gets the keyboard back, so what another program changed is told.
    pub(super) fn suite_bar(&mut self, ui: &mut egui::Ui) {
        let focused = ui.ctx().input(|i| i.focused);
        if focused && !self.suite.was_focused {
            self.suite_refresh();
        }
        self.suite.was_focused = focused;
        let lines = self.suite_banner();
        if lines.is_empty() {
            return;
        }
        let code = self.suite_code();
        let (danger, dim) = (col(self.theme.danger), col(self.theme.dim));
        egui::Panel::top("suite").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (text, attention) in lines {
                    let text = RichText::new(text);
                    ui.label(if attention {
                        text.strong().color(danger)
                    } else {
                        text.color(dim)
                    });
                    ui.separator();
                }
                ui.checkbox(&mut self.suite.put_tools, S::PutTools.tr(code));
                ui.checkbox(&mut self.suite.put_motion, S::PutMotion.tr(code));
            });
        });
    }

    /// The lists of the project and the question before a write.
    pub(super) fn suite_windows(&mut self, ctx: &egui::Context) {
        self.suite_list_window(ctx);
        self.suite_confirm_window(ctx);
    }

    fn suite_list_window(&mut self, ctx: &egui::Context) {
        let Some(list) = self.suite.list else {
            return;
        };
        let Some(view) = self.suite.view.clone() else {
            self.suite.list = None;
            return;
        };
        let code = self.suite_code();
        let (accent, danger) = (col(self.theme.accent), col(self.theme.danger));
        let title = match list {
            SuiteList::Branches => S::MenuBranches,
            SuiteList::Take => S::MenuTake,
            SuiteList::Extract => S::MenuExtract,
        };
        let mut take = None;
        let mut extract = None;
        let mut again = false;
        let mut close = false;
        self.window(title.tr(code))
            .id(egui::Id::unique("suite-list"))
            .default_size([620.0, 360.0])
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .show(ui, |ui| match list {
                        SuiteList::Branches => {
                            egui::Grid::new("suite-branches")
                                .num_columns(5)
                                .striped(true)
                                .show(ui, |ui| {
                                    for word in [
                                        S::ColBranch,
                                        S::ColApp,
                                        S::ColFormat,
                                        S::ColFiles,
                                        S::ColState,
                                    ] {
                                        ui.label(RichText::new(word.tr(code)).color(accent));
                                    }
                                    ui.end_row();
                                    for b in &view.branches {
                                        ui.label(b.id.as_str());
                                        ui.label(b.app.as_str());
                                        ui.label(b.format.as_str());
                                        ui.label(b.files.to_string());
                                        let state = RichText::new(suitecore::state_text(b, code));
                                        ui.label(if b.stale.is_empty() {
                                            state
                                        } else {
                                            state.color(danger)
                                        });
                                        ui.end_row();
                                    }
                                });
                        }
                        SuiteList::Take => {
                            if view.takeable.is_empty() {
                                ui.label(S::NothingToTake.tr(code));
                            } else {
                                ui.label(S::ChooseFile.tr(code));
                            }
                            for (branch, name) in &view.takeable {
                                if ui
                                    .selectable_label(false, format!("{branch}/{name}"))
                                    .clicked()
                                {
                                    take = Some((branch.clone(), name.clone()));
                                }
                            }
                        }
                        SuiteList::Extract => {
                            ui.label(S::ChooseBranch.tr(code));
                            for b in &view.branches {
                                if ui.selectable_label(false, b.id.as_str()).clicked() {
                                    extract = Some(b.id.clone());
                                }
                            }
                        }
                    });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(S::ReadAgain.tr(code)).clicked() {
                        again = true;
                    }
                    if ui.button(S::Close.tr(code)).clicked() {
                        close = true;
                    }
                });
            });
        if again {
            self.suite_refresh();
        }
        if close {
            self.suite.list = None;
        }
        if let Some((branch, name)) = take {
            self.suite.list = None;
            self.suite_take(&branch, &name);
        }
        if let Some(branch) = extract {
            self.suite.list = None;
            self.suite.extract_branch = branch;
            self.suite_browse(Mode::Save, Purpose::SuiteExtract);
        }
    }

    fn suite_confirm_window(&mut self, ctx: &egui::Context) {
        let Some(pending) = &self.suite.pending else {
            return;
        };
        let code = self.suite_code();
        let lines: Vec<String> = pending.warnings.iter().map(|w| w.text(code)).collect();
        let cancel = self.t(Text::McCancel);
        let danger = col(self.theme.danger);
        let mut answer = None;
        self.window(S::ConfirmTitle.tr(code))
            .id(egui::Id::unique("suite-confirm"))
            .resizable(false)
            .show(ctx, |ui| {
                for line in &lines {
                    ui.label(RichText::new(line.as_str()).color(danger));
                }
                ui.horizontal(|ui| {
                    if ui.button(S::Write.tr(code)).clicked() {
                        answer = Some(true);
                    }
                    if ui.button(cancel).clicked() {
                        answer = Some(false);
                    }
                });
            });
        match answer {
            Some(true) => {
                self.suite_confirm();
            }
            Some(false) => self.suite_cancel(),
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eh3d_container::AtomicOptions;
    use eh3d_project::{
        BranchUpdate, Ed25519Signer, NewProject, OpenOptions, Project, SaveOptions, Writer,
    };
    use std::collections::BTreeMap;
    use std::fs;

    const PROGRAM: &str = "G21\nG90\nG0 X0 Y0\nG1 Z-1 F100\nG1 X10 Y0\nM30\n";
    const OTHER: &str = "G21\nG90\nG1 X5 Y5 F200\nM30\n";

    fn folder(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("eh3d_suitewin_{}_{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn file(dir: &Path, name: &str, text: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, text).unwrap();
        path
    }

    fn write_branch(path: &Path, branch: &str, entries: &[(&str, &[u8])]) {
        let project = Project::open(path, &OpenOptions::default()).unwrap();
        let mut update = BranchUpdate::new(branch, "1.0", "0.1.0");
        for (rel, bytes) in entries {
            update = update.with_entry(rel, bytes);
        }
        let writer = Writer::new(branch, &[branch]);
        project
            .save_branch(&writer, update, &SaveOptions::default())
            .unwrap();
    }

    /// A made-up project with the branches of other programs and none of this one.
    fn foreign(path: &Path) {
        let new = NewProject {
            id: "00000000-0000-4000-8000-000000000003".into(),
            name: "Made up".into(),
            units: "millimetres".into(),
            created: None,
        };
        Project::create(
            path,
            new,
            &OpenOptions::default(),
            &AtomicOptions::default(),
        )
        .unwrap();
        write_branch(path, "cad", &[("face.nc", OTHER.as_bytes())]);
        write_branch(path, "x-vendor", &[("blob", b"anything")]);
    }

    /// Every file of every branch but `cnc`.
    fn others(path: &Path) -> BTreeMap<String, Vec<u8>> {
        let mut project = Project::open(path, &OpenOptions::default()).unwrap();
        let names: Vec<(String, String)> = project
            .branches()
            .filter(|b| b.id != BRANCH)
            .flat_map(|b| b.files.keys().map(|f| (b.id.clone(), f.clone())))
            .collect();
        names
            .into_iter()
            .map(|(b, f)| (format!("{b}/{f}"), project.read_entry(&b, &f).unwrap()))
            .collect()
    }

    fn warned(a: &App) -> Vec<S> {
        let pending = a.suite.pending.as_ref().expect("a write waits");
        pending.warnings.iter().map(|n| n.0).collect()
    }

    fn logged(a: &App, word: S) -> bool {
        let start = word.tr("en").split('{').next().unwrap_or("").to_owned();
        a.session.log.iter().any(|l| l.contains(&start))
    }

    fn window() -> App {
        let mut a = App::isolated();
        a.lang = Lang::En;
        a
    }

    #[test]
    fn a_program_is_saved_into_a_new_project_and_opened_again() {
        let d = folder("new");
        let mut a = window();
        a.open(&file(&d, "part.nc", PROGRAM));
        assert_eq!(a.suite_save(), Step::NeedsName);
        assert_eq!(a.suite_suggested_name(), "part.eh3d");
        assert!(a.suite_banner().is_empty());
        assert_eq!(a.suite_save_as(&d.join("job.zip")), Step::Refused);
        assert_eq!(a.suite_save_as(&d.join("job")), Step::Done);
        let project = d.join("job.eh3d");
        assert_eq!(a.suite.file.as_deref(), Some(project.as_path()));
        let held = eh3dfile::held(&project).unwrap();
        assert_eq!(held.program.as_deref(), Some("programs/part.nc"));
        assert_eq!(held.files["programs/part.nc"], PROGRAM.as_bytes());
        assert!(held.tools.is_none() && held.motion.is_none());
        // nothing changed: nothing is written
        let before = fs::read(&project).unwrap();
        assert_eq!(a.suite_save(), Step::Done);
        assert!(logged(&a, S::NothingNew));
        assert_eq!(fs::read(&project).unwrap(), before);

        let mut b = window();
        b.open(&project);
        assert_eq!(b.session.program, a.session.program);
        assert_eq!(b.path, project.display().to_string());
        assert!(b.suite_banner()[0].0.contains("job.eh3d"));
        // the program is never written over the project as text
        assert!(b.save_program(&project).is_err());
        assert_eq!(fs::read(&project).unwrap(), before);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn saving_under_another_name_keeps_every_branch_and_another_project_is_asked_about() {
        let d = folder("saveas");
        let first = d.join("first.eh3d");
        foreign(&first);
        let mut a = window();
        a.open(&file(&d, "part.nc", PROGRAM));
        assert!(a.suite_open(&first));
        // what was in the window before does not go into the project that is opened
        assert!(a.session.program.is_empty());
        let bytes = fs::read(&first).unwrap();
        assert_eq!(a.suite_save(), Step::Done);
        assert_eq!(fs::read(&first).unwrap(), bytes);
        a.open(&d.join("part.nc"));
        let kept = others(&first);
        assert_eq!(a.suite_save(), Step::Done);
        assert_eq!(others(&first), kept);
        let second = d.join("second.eh3d");
        assert_eq!(a.suite_save_as(&second), Step::Done);
        assert_eq!(fs::read(&second).unwrap(), fs::read(&first).unwrap());

        let theirs = d.join("theirs.eh3d");
        foreign(&theirs);
        let bytes = fs::read(&theirs).unwrap();
        assert_eq!(a.suite_save_as(&theirs), Step::Asked);
        assert_eq!(warned(&a), [S::WarnForeign]);
        assert_eq!(fs::read(&theirs).unwrap(), bytes);
        a.suite_cancel();
        assert_eq!(a.suite_confirm(), Step::Refused);
        assert_eq!(fs::read(&theirs).unwrap(), bytes);
        let kept = others(&theirs);
        assert_eq!(a.suite_save_as(&theirs), Step::Asked);
        assert_eq!(a.suite_confirm(), Step::Done);
        assert_eq!(others(&theirs), kept);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_signed_branch_is_asked_about_and_the_removed_signature_is_told() {
        let d = folder("signed");
        let project = d.join("job.eh3d");
        let mut a = window();
        a.open(&file(&d, "part.nc", PROGRAM));
        assert_eq!(a.suite_save_as(&project), Step::Done);
        let key = Ed25519Signer::from_seed(&[5; 32]);
        Project::open(&project, &OpenOptions::default())
            .unwrap()
            .sign_branch(BRANCH, &key, &SaveOptions::default())
            .unwrap();
        a.suite_refresh();
        assert!(a
            .suite_banner()
            .iter()
            .any(|(line, attention)| *attention && line.contains("is signed")));
        a.session.load(OTHER);
        let bytes = fs::read(&project).unwrap();
        assert_eq!(a.suite_save(), Step::Asked);
        assert!(warned(&a).contains(&S::SignedBranch));
        assert_eq!(fs::read(&project).unwrap(), bytes);
        assert_eq!(a.suite_confirm(), Step::Done);
        assert!(logged(&a, S::NoticeBranchSignature));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_program_taken_from_cad_is_told_as_out_of_date_and_a_save_keeps_its_reference() {
        let d = folder("take");
        let project = d.join("job.eh3d");
        foreign(&project);
        let mut a = window();
        assert_eq!(a.suite_take("cad", "face.nc"), Step::Refused);
        assert!(a.suite_open(&project));
        assert_eq!(a.suite_take("cad", "face.nc"), Step::Done);
        assert_eq!(a.session.program.len(), 4);
        write_branch(&project, "cad", &[("face.nc", PROGRAM.as_bytes())]);
        a.suite_refresh();
        assert!(a.suite_banner().iter().any(|(line, attention)| *attention
            && line.starts_with("OUT OF DATE: cnc was derived from cad")));
        assert_eq!(a.suite_save(), Step::Done);
        assert_eq!(
            eh3dfile::held(&project).unwrap().program.as_deref(),
            Some("taken/cad/face.nc")
        );
        assert_eq!(a.suite_take("cad", "face.nc"), Step::Done);
        assert_eq!(a.session.program.len(), 6);
        assert!(a.suite.view.as_ref().unwrap().stale.is_empty());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn an_import_keeps_the_original_and_a_branch_is_extracted_without_writing_over() {
        let d = folder("import");
        let source = file(&d, "other.nc", OTHER);
        let mut a = window();
        assert_eq!(a.suite_import(&source), Step::Refused);
        let project = d.join("job.eh3d");
        a.open(&file(&d, "part.nc", PROGRAM));
        assert_eq!(a.suite_save_as(&project), Step::Done);
        assert_eq!(a.suite_import(&source), Step::Done, "{:?}", a.session.log);
        assert_eq!(a.session.program.len(), 4);
        assert!(a.suite_banner().iter().any(|(l, _)| l.contains("other.nc")));
        let held = eh3dfile::held(&project).unwrap();
        assert_eq!(held.files["origin/other.nc"], OTHER.as_bytes());

        a.suite.extract_branch = BRANCH.to_owned();
        assert_eq!(a.suite_extract_name(), "job-cnc");
        let out = d.join("job-cnc");
        assert!(a.suite_extract(BRANCH, &out));
        assert_eq!(
            fs::read(out.join("origin/other.nc")).unwrap(),
            OTHER.as_bytes()
        );
        assert!(!a.suite_extract(BRANCH, &out));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn the_files_of_the_machine_go_only_when_asked_and_never_with_a_credential() {
        let d = folder("machine");
        let project = d.join("job.eh3d");
        let mut a = window();
        a.settings_path = d.join("phoenix-cnc.cfg");
        let tools = crate::tools::table_path(&a.settings_path);
        fs::write(&tools, "tool = 1 | diameter=3\n").unwrap();
        let motion = motion_path(&a.settings_path);
        let sample = ["abc", "123"].concat();
        fs::write(&motion, format!("{CREDENTIAL_KEY} = {sample}\n")).unwrap();
        a.open(&file(&d, "part.nc", PROGRAM));
        a.suite.put_tools = true;
        a.suite.put_motion = true;
        assert_eq!(a.suite_save_as(&project), Step::Done, "{:?}", a.session.log);
        let held = eh3dfile::held(&project).unwrap();
        assert!(held.tools.is_some());
        assert!(held.motion.is_none());
        assert!(logged(&a, S::Saved));
        // opening a project does not apply the files of the machine it holds
        let mut b = window();
        let before = b.motion.import.max_bytes;
        assert!(b.suite_open(&project));
        assert_eq!(b.motion.import.max_bytes, before);
        assert!(b
            .suite_banner()
            .iter()
            .any(|(l, _)| l.contains("not applied")));
        assert!(!b.suite.put_tools && !b.suite.put_motion);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_damaged_project_changes_nothing_and_a_running_program_is_not_replaced() {
        let d = folder("damaged");
        let junk = file(&d, "junk.eh3d", "not a project");
        let mut a = window();
        a.open(&file(&d, "part.nc", PROGRAM));
        let lines = a.session.program.clone();
        assert!(!a.suite_open(&junk));
        assert_eq!(a.session.program, lines);
        assert!(a.suite.file.is_none());
        assert_eq!(a.suite_save_as(&junk), Step::Refused);
        assert_eq!(fs::read(&junk).unwrap(), b"not a project");
        let project = d.join("job.eh3d");
        foreign(&project);
        a.session.start();
        assert!(a.session.running);
        assert!(!a.suite_open(&project));
        assert_eq!(a.session.program, lines);
        assert!(logged(&a, S::Busy));
        a.open(&project);
        assert_eq!(a.session.program, lines);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn the_lists_and_the_question_are_drawn() {
        let d = folder("draw");
        let project = d.join("job.eh3d");
        foreign(&project);
        let mut a = window();
        assert!(a.suite_open(&project));
        let ctx = egui::Context::default();
        for list in [SuiteList::Branches, SuiteList::Take, SuiteList::Extract] {
            a.suite_show_list(list);
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                a.suite_bar(ui);
                a.suite_windows(ui.ctx());
            });
            out.textures_delta.clear();
        }
        a.open(&file(&d, "part.nc", PROGRAM));
        let theirs = d.join("theirs.eh3d");
        foreign(&theirs);
        assert_eq!(a.suite_save_as(&theirs), Step::Asked);
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            a.suite_windows(ui.ctx());
        });
        out.textures_delta.clear();
        assert!(a.suite.pending.is_some());
        let _ = fs::remove_dir_all(&d);
    }
}
