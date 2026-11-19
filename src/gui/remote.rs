//! The program sent to the EH3D server: the window sends the text of the program as a job, follows it and passes on the pause, the go-on and the stop. A job
//! that stopped is taken up again from the line the server reached, and the server asks the operator before the first move (the job waits, and Start lets it go).

use super::*;
use crate::server::{job_request, Client, Follow, Start};

impl App {
    pub(super) fn server_client(&self) -> Option<Client> {
        Client::from_settings(&self.settings)
    }

    /// A job on the server is being followed (it runs or waits).
    pub(super) fn server_busy(&self) -> bool {
        self.server.is_some() && !self.server_snap.over
    }

    fn server_state(&self) -> Option<&str> {
        self.server_snap
            .info
            .as_ref()
            .filter(|_| self.server_busy())
            .map(|i| i.state.as_str())
    }

    pub(super) fn server_held(&self) -> bool {
        self.server_state() == Some("waiting_operator")
    }

    fn server_going(&self) -> bool {
        matches!(self.server_state(), Some("connecting" | "running"))
    }

    /// The program can be sent: the server is set, there is a program and nothing runs.
    pub(super) fn can_send_to_server(&self) -> bool {
        self.server_client().is_some()
            && !self.session.program.is_empty()
            && !self.session.running
            && !self.server_busy()
    }

    /// The line the program has got to, from the controller or from the server.
    pub(super) fn program_line(&self) -> usize {
        match (&self.server_snap.info, self.server_busy()) {
            (Some(i), true) => i.line.min(self.session.program.len()),
            _ => self.session.next.min(self.session.program.len()),
        }
    }

    pub(super) fn can_start(&self) -> bool {
        if self.server_busy() {
            return self.server_held();
        }
        !(self.session.running && !self.session.paused)
    }

    pub(super) fn can_pause(&self) -> bool {
        if self.server_busy() {
            return self.server_going();
        }
        self.session.running && !self.session.paused
    }

    pub(super) fn can_stop(&self) -> bool {
        self.server_busy() || self.session.running || self.session.paused
    }

    pub(super) fn program_start(&mut self) {
        match (&self.server, self.server_busy()) {
            (Some(f), true) => f.resume(),
            _ => self.session.start(),
        }
    }

    pub(super) fn program_pause(&mut self) {
        match (&self.server, self.server_busy()) {
            (Some(f), true) => f.pause(),
            _ => self.session.pause(),
        }
    }

    pub(super) fn program_stop(&mut self) {
        match (&self.server, self.server_busy()) {
            (Some(f), true) => f.cancel(),
            _ => self.session.stop(),
        }
        self.forget_recovery();
    }

    fn program_name(&self) -> String {
        Path::new(&self.path).file_name().map_or_else(
            || "program".to_string(),
            |n| n.to_string_lossy().to_string(),
        )
    }

    fn program_request(&self, skip: usize) -> serde_json::Value {
        job_request(
            &self.program_name(),
            &self.session.program.join("\n"),
            skip,
            self.settings.server_restore_outputs,
        )
    }

    /// Sends the program to the server and starts it there. `skip` is how many lines are already done.
    pub(super) fn send_to_server(&mut self, skip: usize) {
        let Some(client) = self.server_client() else {
            self.session
                .log
                .push("server: the address and the machine are not set in the settings".into());
            return;
        };
        if self.session.program.is_empty() {
            return;
        }
        let request = self.program_request(skip);
        self.start_following(Follow::start(client, Start::Send(request)));
        self.session.log.push(format!(
            "server: sending {} ({} lines)",
            self.program_name(),
            self.session.program.len()
        ));
    }

    /// Takes up a job the server already has (the program was closed while it ran); if it ended without finishing, the program goes on from `skip`.
    pub(super) fn attach_to_server(&mut self, id: String, skip: usize) {
        let Some(client) = self.server_client() else {
            self.session
                .log
                .push("server: the address and the machine are not set in the settings".into());
            return;
        };
        let again = self.program_request(skip);
        self.start_following(Follow::start(client, Start::Attach { id, again }));
        self.session
            .log
            .push(format!("server: going on from line {}", skip + 1));
    }

    fn start_following(&mut self, f: Follow) {
        self.server = Some(f);
        self.server_snap = crate::server::Snapshot::default();
        self.server_key = None;
        self.server_err = None;
    }

    /// Reads how the job on the server goes: logs what changes, keeps the file the program is taken up from, and asks for another frame while it lives.
    pub(super) fn server_tick(&mut self, ctx: &egui::Context) {
        let Some(f) = &self.server else { return };
        let snap = f.snapshot();
        let key = snap
            .info
            .as_ref()
            .map(|i| (i.state.clone(), i.pause.clone()));
        let total = self.session.program.len();
        if key != self.server_key {
            if let Some(i) = &snap.info {
                let pause = if i.pause.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", i.pause)
                };
                self.session.log.push(format!(
                    "server: {}{} - {}%, line {}",
                    i.state,
                    pause,
                    i.percent(),
                    i.line + 1
                ));
                if !i.message.is_empty() {
                    self.session.log.push(format!("server: {}", i.message));
                }
            }
            self.server_key = key;
        }
        if snap.error != self.server_err {
            if let Some(e) = &snap.error {
                self.session.log.push(format!("server: {e}"));
            }
            self.server_err = snap.error.clone();
        }
        if let Some(i) = &snap.info {
            let path = recovery::recovery_path(&self.settings_path);
            if i.state == "done" {
                Recovery::clear(&path);
                self.recovery_saved = None;
            } else if (i.active() || i.state == "failed")
                && !self.path.is_empty()
                && i.line < total
                && !cfg!(test)
                && (i.state == "failed" || recovery::due(self.recovery_saved, i.line, 20))
            {
                let r = Recovery {
                    file: self.path.clone(),
                    next: i.line,
                    total,
                    job: i.id.clone(),
                    sha256: recovery::program_hash(&self.session.program),
                };
                if r.save(&path).is_ok() {
                    self.recovery_saved = Some(i.line);
                }
            }
        }
        if snap.link != self.server_snap.link {
            if let Some(link) = snap.link {
                self.session
                    .log
                    .push(format!("server: link {}", crate::server::link_word(link)));
            }
        }
        let over = snap.over;
        self.server_snap = snap;
        if !over {
            ctx.request_repaint_after(std::time::Duration::from_millis(crate::server::POLL_MS));
        }
    }
}
