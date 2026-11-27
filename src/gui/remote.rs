//! The program sent to the EH3D server: the window sends the text of the program as a job, follows it and passes on the pause, the go-on and the stop. A job
//! that stopped is taken up again from the line the server reached, and the server asks the operator before the first move (the job waits, and Start lets it go).

use super::*;
use crate::logwords::Lg;
use crate::server::{job_request, Client, Follow, Start};

impl App {
    pub(super) fn server_client(&self) -> Option<Client> {
        Client::from_settings(&self.settings).map(|c| c.with_login(self.server_login.clone()))
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

    /// The emergency stop of the window: the controller of this computer at once and, while a job was sent to the server and it is not known to be over,
    /// the machine of the server too.
    pub(super) fn emergency_stop(&mut self) {
        self.session.estop();
        let may_run = self.server.is_some()
            && !self
                .server_snap
                .info
                .as_ref()
                .is_some_and(crate::server::JobInfo::finished);
        if !may_run {
            return;
        }
        let Some(client) = self.server_client() else {
            let lang = self.server_lang();
            let said = crate::server::estop_lost_notice(lang);
            self.log_tell(Lg::SrvSays, &[said.to_string()]);
            return;
        };
        self.server_estops.push(crate::server::emergency_now(
            client,
            self.lang.code().to_string(),
        ));
        self.log_note(Lg::SrvEstopSent);
    }

    /// A message of the log with no holes, in the language of the window.
    pub(super) fn log_note(&mut self, message: Lg) {
        let line = crate::logwords::text(self.lang.code(), message);
        self.session.log.push(line.to_string());
    }

    /// A message of the log in the language of the window.
    pub(super) fn log_tell(&mut self, message: Lg, args: &[String]) {
        let line = crate::logwords::say(self.lang.code(), message, args);
        self.session.log.push(line);
    }

    fn server_lang(&self) -> eh3d_serverclient::Lang {
        eh3d_serverclient::Lang::from_tag(self.lang.code()).unwrap_or(eh3d_serverclient::Lang::En)
    }

    /// Writes in the log what the server answered to the emergency stops that were sent.
    fn estop_tick(&mut self, ctx: &egui::Context) {
        let code = self.lang.code();
        let log = &mut self.session.log;
        self.server_estops.retain(|answer| match answer.try_recv() {
            Ok(Ok(said) | Err(said)) => {
                log.push(crate::logwords::say(code, Lg::SrvSays, &[&said]));
                false
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => true,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => false,
        });
        if !self.server_estops.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(crate::server::POLL_MS));
        }
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
        !(self.session.running && !self.session.paused) || self.session.held_at_door()
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
            self.log_note(Lg::SrvNoSettings);
            return;
        };
        if self.session.program.is_empty() {
            return;
        }
        let request = self.program_request(skip);
        self.start_following(Follow::start(client, Start::Send(request)));
        let name = self.program_name();
        let count = self.session.program.len().to_string();
        self.log_tell(Lg::SrvSending, &[name, count]);
    }

    /// Takes up a job the server already has (the program was closed while it ran); if it ended without finishing, the program goes on from `skip`.
    pub(super) fn attach_to_server(&mut self, id: String, skip: usize) {
        // a job that still runs is followed; a new one is made only where the program can really go on from
        let refusal = self
            .session
            .recovery_refusal(skip, self.settings.resume_backoff_lines as usize);
        let Some(client) = self.server_client() else {
            self.log_note(Lg::SrvNoSettings);
            return;
        };
        let again = match refusal {
            Some(why) => Err(why),
            None => Ok(self.program_request(skip)),
        };
        self.start_following(Follow::start(client, Start::Attach { id, again }));
        self.log_tell(Lg::SrvFollowing, &[(skip + 1).to_string()]);
    }

    fn start_following(&mut self, f: Follow) {
        self.server = Some(f);
        self.server_snap = crate::server::Snapshot::default();
        self.server_key = None;
        self.server_err = None;
    }

    /// Reads how the job on the server goes: logs what changes, keeps the file the program is taken up from, and asks for another frame while it lives.
    pub(super) fn server_tick(&mut self, ctx: &egui::Context) {
        self.estop_tick(ctx);
        let Some(f) = &self.server else { return };
        let snap = f.snapshot();
        let key = snap
            .info
            .as_ref()
            .map(|i| (i.state.clone(), i.pause.clone()));
        let total = self.session.program.len();
        if key != self.server_key {
            if let Some(i) = &snap.info {
                let code = self.lang.code();
                let mut state = crate::logwords::job_state(code, &i.state);
                if !i.pause.is_empty() {
                    let pause = crate::logwords::job_state(code, &i.pause);
                    state = format!("{state} ({pause})");
                }
                self.log_tell(
                    Lg::SrvState,
                    &[state, i.percent().to_string(), (i.line + 1).to_string()],
                );
                if i.pause == "resume" {
                    let lang = eh3d_serverclient::Lang::from_tag(self.settings.lang.code())
                        .unwrap_or(eh3d_serverclient::Lang::En);
                    let notice = crate::server::resume_notice(lang);
                    self.log_tell(Lg::SrvSays, &[notice.to_string()]);
                }
                if !i.message.is_empty() {
                    let line = crate::logwords::say(self.lang.code(), Lg::SrvSays, &[&i.message]);
                    self.session.log.push(line);
                }
            }
            self.server_key = key;
        }
        if snap.error != self.server_err || snap.failed_orders > self.server_snap.failed_orders {
            if let Some(e) = &snap.error {
                let line = crate::logwords::say(self.lang.code(), Lg::SrvSays, &[e]);
                self.session.log.push(line);
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
                    // like on the controller of this computer: the last moves it took may not have been made
                    next: recovery::backed_off(
                        &self.session.program,
                        i.line,
                        self.settings.resume_backoff_lines as usize,
                    ),
                    total,
                    job: i.id.clone(),
                    sha256: recovery::program_hash(&self.session.program),
                    shifted: false,
                };
                if r.save(&path).is_ok() {
                    self.recovery_saved = Some(i.line);
                }
            }
        }
        if snap.link != self.server_snap.link {
            if let Some(link) = snap.link {
                let word =
                    crate::logwords::job_state(self.lang.code(), crate::server::link_word(link));
                self.log_tell(Lg::SrvLink, &[word]);
            }
        }
        let over = snap.over;
        self.server_snap = snap;
        if !over {
            ctx.request_repaint_after(std::time::Duration::from_millis(crate::server::POLL_MS));
        }
    }
}
