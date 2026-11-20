//! The settings of the link to the EH3D server as the window shows them: the panel of the suite (`eh3d-view`: the address, the token of the program, the
//! machine, the limits of the link, the state of the link, the login and the logout) over the settings of this program. The panel and its words are the
//! ones every program of the suite shows; what is this program's own is here: how its settings become the settings of the link and back, and the place where
//! a login is kept so that the job that is sent to the server and the panel use the same one. Nothing here has a window, so it is tested without one.

use std::sync::{Arc, Mutex};

use eh3d_serverclient as sc;
use eh3d_view::server_panel::{Action, ServerPanel};

use crate::server::APP;
use crate::settings::Settings;

/// Where the token of a login is kept while the program runs: in memory, shared by every clone, so the panel that logs in and the connection that follows a
/// job hold the same login. It is never written anywhere and it never shows in a `Debug`.
#[derive(Clone, Default)]
pub struct LoginStore(Arc<Mutex<Option<sc::Secret>>>);

impl LoginStore {
    pub fn new() -> LoginStore {
        LoginStore::default()
    }

    /// Whether a login is kept.
    pub fn is_logged(&self) -> bool {
        self.0.lock().map(|g| g.is_some()).unwrap_or(false)
    }
}

impl std::fmt::Debug for LoginStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LoginStore(logged: {})", self.is_logged())
    }
}

/// Two stores are equal when they are the same place.
impl PartialEq for LoginStore {
    fn eq(&self, other: &LoginStore) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for LoginStore {}

impl sc::TokenStore for LoginStore {
    fn load(&self) -> Option<sc::Secret> {
        self.0.lock().ok().and_then(|g| g.clone())
    }

    fn save(&self, token: &sc::Secret) -> Result<(), String> {
        let mut g = self.0.lock().map_err(|_| "the login store is broken")?;
        *g = Some(token.clone());
        Ok(())
    }

    fn clear(&self) {
        if let Ok(mut g) = self.0.lock() {
            *g = None;
        }
    }
}

/// The settings of the link that the settings of this program describe. A value the link does not accept (a token with a space in it) is left at its
/// default, as the link reads its own file.
pub fn link_settings(s: &Settings) -> sc::Settings {
    let mut link = sc::Settings::default();
    let on = |b: bool| if b { "on" } else { "off" };
    for (key, value) in [
        ("server_address", s.server_address.trim().to_string()),
        ("server_token", s.server_token.trim().to_string()),
        ("server_machine", s.server_machine.trim().to_string()),
        ("server_timeout_s", s.server_timeout_s.to_string()),
        ("server_retries", s.server_retries.to_string()),
        ("server_mdns", on(s.server_mdns).to_string()),
        ("server_max_answer_mib", s.server_max_answer_mib.to_string()),
    ] {
        let _ = link.set(key, &value);
    }
    link
}

/// Writes the settings of the link into the settings of this program (the seven keys; nothing else is touched).
pub fn apply_link(into: &mut Settings, from: &sc::Settings) {
    into.server_address = from.server_address.clone();
    into.server_token = from.server_token.expose().to_string();
    into.server_machine = from.server_machine.clone();
    into.server_timeout_s = from.server_timeout_s;
    into.server_retries = from.server_retries;
    into.server_mdns = from.server_mdns;
    into.server_max_answer_mib = from.server_max_answer_mib;
}

/// A client over plain HTTP for these settings that keeps its login in `store`.
pub fn connect(s: &Settings, store: &LoginStore) -> sc::Client {
    sc::Client::connect(link_settings(s)).with_store(Box::new(store.clone()))
}

/// The panel of the settings of the server and the client it works with.
pub struct ServerPane {
    pub panel: ServerPanel,
    conn: sc::Client,
    store: LoginStore,
}

impl ServerPane {
    /// The panel for these settings, over a plain HTTP connection.
    pub fn new(s: &Settings, store: LoginStore) -> ServerPane {
        let conn = connect(s, &store);
        ServerPane::over(s, conn, store)
    }

    /// As `new`, over a client already made (a server in memory for the tests).
    pub fn over(s: &Settings, conn: sc::Client, store: LoginStore) -> ServerPane {
        ServerPane {
            panel: ServerPanel::new(&link_settings(s), APP),
            conn,
            store,
        }
    }

    /// The boxes go back to the settings of the program and the connection is made again with them (the window opens, or the changes were not kept).
    pub fn reset(&mut self, s: &Settings) {
        self.panel.form = eh3d_view::server_panel::ServerForm::from_settings(&link_settings(s));
        self.conn = connect(s, &self.store);
        self.panel.login.wipe_password();
        self.panel.login.open = false;
    }

    /// Reads the state of the link again (a window calls it every frame; it only reads).
    pub fn refresh(&mut self) {
        self.panel.refresh(&self.conn);
    }

    /// Does what the window asked. Gives `true` when the person applied new settings: they were written into `s` and the connection was made again with
    /// them (the login that was kept stays).
    pub fn handle(&mut self, action: Action, s: &mut Settings) -> bool {
        match self.panel.handle(action, &mut self.conn) {
            Some(applied) => {
                apply_link(s, &applied);
                self.conn = connect(s, &self.store);
                self.panel.refresh(&self.conn);
                true
            }
            None => false,
        }
    }

    /// Whether a login is kept (the job that goes to the server uses it).
    pub fn is_logged(&self) -> bool {
        self.store.is_logged()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eh3d_view::server_panel::LinkState;
    use sc::sim::SimServer;

    fn settings() -> Settings {
        Settings {
            server_address: "192.168.0.180:18200".into(),
            server_token: String::new(),
            server_machine: "router".into(),
            server_timeout_s: 30,
            server_retries: 4,
            server_mdns: true,
            server_max_answer_mib: 64,
            ..Settings::default()
        }
    }

    fn pane(sim: &SimServer, s: &Settings) -> (ServerPane, LoginStore) {
        let store = LoginStore::new();
        let conn = sc::Client::new(link_settings(s), Box::new(sim.clone()))
            .with_sleeper(Box::new(sim.sleeper()))
            .with_clock(Box::new(sim.clock()))
            .with_store(Box::new(store.clone()));
        (ServerPane::over(s, conn, store.clone()), store)
    }

    #[test]
    fn the_settings_of_the_program_become_the_settings_of_the_link_and_come_back() {
        let s = settings();
        let link = link_settings(&s);
        assert_eq!(link.server_address, "192.168.0.180:18200");
        assert_eq!(link.server_machine, "router");
        assert_eq!(
            (link.server_timeout_s, link.server_retries, link.server_mdns),
            (30, 4, true)
        );
        assert_eq!(link.server_max_answer_mib, 64);
        let mut back = Settings::default();
        apply_link(&mut back, &link);
        assert_eq!(back.server_address, s.server_address);
        assert_eq!(back.server_machine, s.server_machine);
        assert_eq!(
            (back.server_timeout_s, back.server_retries, back.server_mdns),
            (30, 4, true)
        );
        assert_eq!(back.server_max_answer_mib, 64);
        // nothing else was touched
        assert_eq!(
            back.server_restore_outputs,
            Settings::default().server_restore_outputs
        );
        assert_eq!(back.spindle_rpm, Settings::default().spindle_rpm);
    }

    #[test]
    fn a_token_the_link_cannot_send_is_left_out_and_the_others_are_trimmed() {
        let mut s = settings();
        s.server_token = "has a space".into();
        s.server_address = "  192.168.0.180:18200  ".into();
        let link = link_settings(&s);
        assert!(link.server_token.is_empty());
        assert_eq!(link.server_address, "192.168.0.180:18200");
        s.server_token = " good-token ".into();
        assert_eq!(link_settings(&s).server_token.expose(), "good-token");
    }

    #[test]
    fn a_value_out_of_its_range_keeps_the_default_of_the_link() {
        let mut s = settings();
        s.server_timeout_s = 9999;
        s.server_retries = 77;
        s.server_max_answer_mib = 0;
        let link = link_settings(&s);
        let d = sc::Settings::default();
        assert_eq!(link.server_timeout_s, d.server_timeout_s);
        assert_eq!(link.server_retries, d.server_retries);
        assert_eq!(link.server_max_answer_mib, d.server_max_answer_mib);
    }

    #[test]
    fn the_store_is_shared_by_its_clones_and_never_shows_the_token() {
        use sc::TokenStore;
        let a = LoginStore::new();
        let b = a.clone();
        assert!(!a.is_logged() && a == b && a != LoginStore::new());
        a.save(&sc::Secret::new("very-secret-token")).unwrap();
        assert!(b.is_logged());
        assert_eq!(b.load().unwrap().expose(), "very-secret-token");
        let shown = format!("{a:?}");
        assert!(!shown.contains("very-secret"), "{shown}");
        assert!(shown.contains("true"), "{shown}");
        b.clear();
        assert!(!a.is_logged() && a.load().is_none());
    }

    #[test]
    fn a_login_in_the_panel_is_the_login_of_the_job_and_the_logout_ends_it() {
        let sim = SimServer::new();
        let s = settings();
        let (mut p, store) = pane(&sim, &s);
        let mut work = s.clone();
        p.refresh();
        assert!(!p.is_logged());
        assert!(p.panel.who().is_none());
        // the person opens the login window, types and sends
        assert!(!p.handle(Action::OpenLogin, &mut work));
        assert!(p.panel.login.open);
        p.panel.login.name = "admin".into();
        p.panel.login.password = "secret-password".into();
        assert!(!p.handle(Action::SubmitLogin, &mut work));
        assert!(!p.panel.login.open);
        assert_eq!(p.panel.who().map(|w| w.name.as_str()), Some("admin"));
        assert_eq!(p.panel.state(), LinkState::Connected);
        assert!(p.is_logged() && store.is_logged());
        // the connection that follows a job takes the same login, and the program token stays empty
        let job = crate::server::Client::from_settings(&work)
            .unwrap()
            .with_login(store.clone())
            .connect();
        assert!(job.has_login());
        assert!(work.server_token.is_empty());
        // the logout ends it for both
        assert!(!p.handle(Action::Logout, &mut work));
        assert!(!p.is_logged() && p.panel.who().is_none());
        assert_eq!(p.panel.state(), LinkState::NoSession);
    }

    #[test]
    fn a_wrong_password_keeps_the_window_open_and_logs_nothing_in() {
        let sim = SimServer::new();
        let s = settings();
        let (mut p, store) = pane(&sim, &s);
        let mut work = s.clone();
        p.handle(Action::OpenLogin, &mut work);
        p.panel.login.name = "admin".into();
        p.panel.login.password = "wrong".into();
        p.handle(Action::SubmitLogin, &mut work);
        assert!(p.panel.login.open);
        assert!(p.panel.login.error().is_some());
        assert!(!store.is_logged() && p.panel.who().is_none());
        // the error is told in the language of the person, and the password is not in it
        let es = p
            .panel
            .login
            .error_text(eh3d_view::texts::Lang::Es)
            .unwrap();
        assert!(!es.contains("wrong"), "{es}");
        p.handle(Action::CancelLogin, &mut work);
        assert!(!p.panel.login.open && p.panel.login.password.is_empty());
    }

    #[test]
    fn applying_the_boxes_writes_the_settings_and_a_new_token_or_its_removal_is_kept() {
        let sim = SimServer::new();
        let mut s = settings();
        s.server_token = "old-token".into();
        let (mut p, _) = pane(&sim, &s);
        // a new address and a new token typed in the boxes
        p.panel.form.address = "10.0.0.5:18200".into();
        p.panel.form.token_input = "new-token".into();
        p.panel.form.machine = "mill".into();
        assert!(p.handle(Action::Apply, &mut s));
        assert_eq!(s.server_address, "10.0.0.5:18200");
        assert_eq!(s.server_token, "new-token");
        assert_eq!(s.server_machine, "mill");
        // what the boxes do not show stays
        assert_eq!((s.server_timeout_s, s.server_max_answer_mib), (30, 64));
        // the token is removed only when the person asks for it
        p.panel.form.remove_token();
        assert!(p.handle(Action::Apply, &mut s));
        assert!(s.server_token.is_empty());
        // a box that is wrong is not applied
        p.panel.form.address = "no port here".into();
        assert!(!p.handle(Action::Apply, &mut s));
        assert_eq!(s.server_address, "10.0.0.5:18200");
    }

    #[test]
    fn reset_brings_the_boxes_back_to_the_settings_and_closes_the_login() {
        let sim = SimServer::new();
        let s = settings();
        let (mut p, _) = pane(&sim, &s);
        p.panel.form.address = "elsewhere:1".into();
        p.panel.login.open = true;
        p.panel.login.password = "typed".into();
        p.reset(&s);
        assert_eq!(p.panel.form.address, s.server_address);
        assert!(!p.panel.login.open && p.panel.login.password.is_empty());
        assert!(!p.panel.form.is_dirty());
    }

    #[test]
    fn the_test_of_the_connection_tells_whether_the_server_answered() {
        let sim = SimServer::new();
        let s = settings();
        let (mut p, _) = pane(&sim, &s);
        let mut work = s.clone();
        p.handle(Action::TestConnection, &mut work);
        assert!(
            matches!(p.panel.notice, eh3d_view::server_panel::Notice::Answered(_)),
            "{:?}",
            p.panel.notice
        );
        assert_ne!(p.panel.state(), LinkState::Disconnected);
    }
}
