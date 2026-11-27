//! The way to the EH3D server: a G-code program is sent to it as a job, the server runs it on its machine and this program follows it, holds it, lets it go
//! on and stops it. The server's address, the token of this program and the id of the machine are in the settings file, not in the code.
//!
//! The link itself is the one client of the suite (`eh3d-serverclient` of EH3D-COMMON): the same keys with the same ranges, a login for the application
//! `phoenix-cnc` or the token of a program, a renewal of the login, retries with growing pauses (a `POST` is never sent twice), a state of the link, and
//! the refusals of the server by their code in the language of the window. What is this program's own is here: the job, its follower and its words.
//!
//! The requests are plain HTTP/1.1, one per connection, on a thread of their own, so the window never waits for the network. Closing the program does not stop
//! the job on the server (the machine is the server's); the job is stopped with [`Follow::cancel`].

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use eh3d_serverclient as sc;
use serde_json::{json, Value};

use crate::serverlink::LoginStore;
use crate::settings::Settings;

pub use sc::Link;

/// How often the job is asked how it goes, in milliseconds.
pub const POLL_MS: u64 = 300;
/// How many answers in a row may fail before the job is given up as lost (the job may still be running on the server).
pub const LOST_AFTER: u32 = 40;
/// The name this program gives the server when it logs in.
pub const APP: &str = "phoenix-cnc";

/// Where the server is and who this program is on it: the settings of the link, ready to make a connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Client {
    link: sc::Settings,
    lang: sc::Lang,
    /// Where the login of the person is kept (the panel of the settings logs in there); none: only the token of the program is used.
    login: Option<LoginStore>,
}

/// What the server says of a job.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct JobInfo {
    pub id: String,
    /// `ready`, `connecting`, `running`, `waiting_operator`, `done`, `cancelled` or `failed`.
    pub state: String,
    pub items_done: usize,
    pub items_total: usize,
    /// The line of the program text the job has got to: where a job that goes on after a stop starts again.
    pub line: usize,
    pub message: String,
    /// `hold` (the operator paused it) or `resume` (it waits for the operator before its first move) while it waits.
    pub pause: String,
}

impl JobInfo {
    pub fn from_json(v: &Value) -> JobInfo {
        JobInfo {
            id: v["id"].as_str().unwrap_or("").to_string(),
            state: v["state"].as_str().unwrap_or("").to_string(),
            items_done: v["items_done"].as_u64().unwrap_or(0) as usize,
            items_total: v["items_total"].as_u64().unwrap_or(0) as usize,
            line: v["line"].as_u64().unwrap_or(0) as usize,
            message: v["message"].as_str().unwrap_or("").to_string(),
            pause: v["pause"]["kind"].as_str().unwrap_or("").to_string(),
        }
    }

    pub fn finished(&self) -> bool {
        matches!(self.state.as_str(), "done" | "cancelled" | "failed")
    }

    /// The job runs or waits for a person (it is not over and not only ready).
    pub fn active(&self) -> bool {
        matches!(
            self.state.as_str(),
            "connecting" | "running" | "waiting_operator"
        )
    }

    pub fn held(&self) -> bool {
        self.state == "waiting_operator"
    }

    pub fn percent(&self) -> usize {
        (self.items_done * 100)
            .checked_div(self.items_total)
            .map_or(0, |p| p.min(100))
    }
}

/// The request that makes a G-code job on the machine. `skip` is how many lines of the text are already done (a job that goes on after a stop).
pub fn job_request(name: &str, gcode: &str, skip: usize, restore_outputs: bool) -> Value {
    json!({
        "kind": "gcode", "name": name, "gcode": gcode,
        "skip": skip, "restore_outputs": restore_outputs,
    })
}

/// What a refusal or a failure says to the person, in their language: the text of the code of the server, followed by the detail the server gave when the
/// text does not already hold it (`bad_gcode` and `line 3: no`).
pub fn error_message(e: &sc::ClientError, lang: sc::Lang) -> String {
    let text = e.text(lang);
    match e {
        sc::ClientError::Server { detail, .. } if !detail.is_empty() && !text.contains(detail) => {
            format!("{text}: {detail}")
        }
        _ => text,
    }
}

/// A failure of a call: what to tell the person, and whether the cure is a new login (the token is gone or was refused).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub text: String,
    pub needs_login: bool,
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text)
    }
}

impl Failure {
    fn plain(text: &str) -> Failure {
        Failure {
            text: text.to_string(),
            needs_login: false,
        }
    }
}

/// The state of the link in a word for the log of the session.
pub fn link_word(link: Link) -> &'static str {
    match link {
        Link::Unknown => "not tried yet",
        Link::Connected => "connected",
        Link::Disconnected => "disconnected",
        Link::LoggedOut => "logged out",
    }
}

/// Whether a machine or a job id can go into a path as it is: letters, digits, `_` and `-`, up to 64.
fn safe_segment(s: &str) -> bool {
    (1..=64).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn checked_id(id: &str) -> Result<&str, Failure> {
    if safe_segment(id) {
        Ok(id)
    } else {
        Err(Failure::plain(
            "the id of the job has characters that cannot go into a request",
        ))
    }
}

impl Client {
    /// The client for these settings; `None` while the address or the machine is missing, or a value cannot be sent.
    pub fn new(address: &str, token: &str, machine: &str) -> Option<Client> {
        let (a, m) = (address.trim(), machine.trim());
        if a.is_empty() || m.is_empty() || !safe_segment(m) {
            return None;
        }
        let mut link = sc::Settings::default();
        link.set("server_address", a).ok()?;
        link.set("server_token", token.trim()).ok()?;
        link.set("server_machine", m).ok()?;
        Some(Client {
            link,
            lang: sc::Lang::En,
            login: None,
        })
    }

    /// The client the settings of the program describe: the three keys of the server and the four limits of the link, and the language of the window for
    /// the texts of the refusals.
    pub fn from_settings(s: &Settings) -> Option<Client> {
        let mut c = Client::new(&s.server_address, &s.server_token, &s.server_machine)?;
        c.link
            .set("server_timeout_s", &s.server_timeout_s.to_string())
            .ok()?;
        c.link
            .set("server_retries", &s.server_retries.to_string())
            .ok()?;
        c.link
            .set("server_mdns", if s.server_mdns { "on" } else { "off" })
            .ok()?;
        c.link
            .set(
                "server_max_answer_mib",
                &s.server_max_answer_mib.to_string(),
            )
            .ok()?;
        c.lang = sc::Lang::from_tag(s.lang.code()).unwrap_or(sc::Lang::En);
        Some(c)
    }

    /// The settings of the link, with the token that never shows in a `Debug`.
    pub fn link_settings(&self) -> &sc::Settings {
        &self.link
    }

    /// The same client keeping its login in this store: a person who logged in through the panel of the settings sends the job with that login.
    pub fn with_login(mut self, store: LoginStore) -> Client {
        self.login = Some(store);
        self
    }

    /// A connection over plain HTTP to the address of the settings.
    pub fn connect(&self) -> Conn {
        let mut inner = sc::Client::connect(self.link.clone());
        if let Some(store) = &self.login {
            inner = inner.with_store(Box::new(store.clone()));
        }
        Conn::over(inner, self.lang)
    }
}

/// One connection to the server: the client of the suite and the language the refusals are told in.
pub struct Conn {
    inner: sc::Client,
    lang: sc::Lang,
}

impl Conn {
    /// A connection over a client already made (a server in memory for the tests, a tunnel).
    pub fn over(inner: sc::Client, lang: sc::Lang) -> Conn {
        Conn { inner, lang }
    }

    /// Whether a login or a token of a program is there to send.
    pub fn has_login(&self) -> bool {
        self.inner.has_token()
    }

    /// The state of the link for the corner of the window.
    pub fn link(&self) -> Link {
        self.inner.link()
    }

    fn fail(&self, e: &sc::ClientError) -> Failure {
        Failure {
            text: error_message(e, self.lang),
            needs_login: e.needs_login(),
        }
    }

    /// `GET /health` (no login needed): whether the server is there.
    pub fn health(&mut self) -> Result<sc::Health, Failure> {
        self.inner.health().map_err(|e| self.fail(&e))
    }

    /// Logs in for this program when there is no token yet (a program token in `server_token` is used as it is); `None` when no login was needed.
    pub fn login_if_needed(
        &mut self,
        name: &str,
        password: &str,
    ) -> Result<Option<sc::Session>, Failure> {
        if self.inner.has_token() {
            return Ok(None);
        }
        self.inner
            .login(name, password, APP)
            .map(Some)
            .map_err(|e| self.fail(&e))
    }

    /// One request to a route that needs the login: the JSON of the answer (`null` when it is not JSON).
    fn json(&mut self, method: &str, path: &str, body: Option<&Value>) -> Result<Value, Failure> {
        let text = body.map(Value::to_string);
        match self.inner.request(method, path, text.as_deref()) {
            Ok(reply) => Ok(serde_json::from_str(&reply.body).unwrap_or(Value::Null)),
            Err(e) => Err(self.fail(&e)),
        }
    }

    fn machine_path(&self, suffix: &str) -> Result<String, Failure> {
        self.inner.machine_path(suffix).map_err(|e| self.fail(&e))
    }

    /// Makes the job on the machine and starts it; the answer is the job. A job the server kept but could not start is not left lying ready on it.
    pub fn send(&mut self, request: &Value) -> Result<JobInfo, Failure> {
        let path = self.machine_path("/jobs")?;
        let made = self.json("POST", &path, Some(request))?;
        let id = made["id"]
            .as_str()
            .ok_or_else(|| Failure::plain("the server did not give the job an id"))?;
        let id = checked_id(id)?.to_string();
        match self.json("POST", &format!("/api/v1/jobs/{id}/start"), None) {
            Ok(v) => Ok(JobInfo::from_json(&v)),
            Err(why) => {
                let _ = self.json("POST", &format!("/api/v1/jobs/{id}/cancel"), None);
                Err(why)
            }
        }
    }

    pub fn job(&mut self, id: &str) -> Result<JobInfo, Failure> {
        let id = checked_id(id)?;
        Ok(JobInfo::from_json(&self.json(
            "GET",
            &format!("/api/v1/jobs/{id}"),
            None,
        )?))
    }

    pub fn order(&mut self, id: &str, what: Order) -> Result<(), Failure> {
        if what == Order::Cancel {
            // the stop button reaches the machine at once, without waiting for the line the server is sending; a server that does not have it cancels the job
            let stop = self.machine_path("/stop")?;
            if self.json("POST", &stop, None).is_ok() {
                return Ok(());
            }
        }
        let id = checked_id(id)?;
        self.json("POST", &format!("/api/v1/jobs/{id}/{}", what.word()), None)
            .map(|_| ())
    }
}

/// What the operator can tell a job on the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    Pause,
    Resume,
    Cancel,
}

impl Order {
    fn word(self) -> &'static str {
        match self {
            Order::Pause => "pause",
            Order::Resume => "resume",
            Order::Cancel => "cancel",
        }
    }
}

/// What the window shows of a job on the server.
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub info: Option<JobInfo>,
    /// Why the job is not followed any more when it is not because it ended (a refusal, a lost server).
    pub error: Option<String>,
    /// The following is over (the job ended, was refused, or the server was lost).
    pub over: bool,
    /// The state of the link after the last call: nothing until the first one.
    pub link: Option<Link>,
}

/// A job on the server and the thread that follows it.
pub struct Follow {
    shared: Arc<Mutex<Snapshot>>,
    orders: Sender<Order>,
    detach: Arc<AtomicBool>,
}

/// How the following starts.
pub enum Start {
    /// Send this program as a new job.
    Send(Value),
    /// Follow a job the server already has (the program was closed while it ran). If the server no longer holds it, or it ended without finishing, a new job
    /// is made from `again`, which carries the line it starts from; an `Err` there is why the program cannot go on from that line. No new job is made of a
    /// job that finished, nor from the first line (the server would start it without waiting for the operator).
    Attach {
        id: String,
        again: Result<Value, String>,
    },
}

impl Follow {
    pub fn start(client: Client, how: Start) -> Follow {
        Follow::start_over(client.connect(), how)
    }

    /// As `start`, over a connection already made.
    pub fn start_over(conn: Conn, how: Start) -> Follow {
        let shared = Arc::new(Mutex::new(Snapshot::default()));
        let (orders, rx) = channel();
        let detach = Arc::new(AtomicBool::new(false));
        let (s, d) = (shared.clone(), detach.clone());
        thread::spawn(move || follow(conn, how, &s, &rx, &d));
        Follow {
            shared,
            orders,
            detach,
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        self.shared.lock().map(|g| g.clone()).unwrap_or_default()
    }

    pub fn pause(&self) {
        let _ = self.orders.send(Order::Pause);
    }

    pub fn resume(&self) {
        let _ = self.orders.send(Order::Resume);
    }

    /// Stops the job on the server.
    pub fn cancel(&self) {
        let _ = self.orders.send(Order::Cancel);
    }
}

impl Drop for Follow {
    fn drop(&mut self) {
        // the job stays where it is on the server: only the following ends
        self.detach.store(true, Ordering::SeqCst);
    }
}

fn set(shared: &Mutex<Snapshot>, f: impl FnOnce(&mut Snapshot)) {
    if let Ok(mut g) = shared.lock() {
        f(&mut g);
    }
}

fn follow(
    mut conn: Conn,
    how: Start,
    shared: &Mutex<Snapshot>,
    orders: &Receiver<Order>,
    detach: &AtomicBool,
) {
    let first = match how {
        Start::Send(request) => conn.send(&request),
        Start::Attach { id, again } => match conn.job(&id) {
            Ok(info) if info.active() || info.state == "ready" => Ok(info),
            Ok(info) if info.state == "done" => Err(Failure::plain(
                "the job on the server had finished: nothing is sent again",
            )),
            // it ended, or the server lost it: the program goes on from the line the server reached (or from the line it knew)
            _ => match again {
                Ok(again) if again["skip"].as_u64().is_some_and(|n| n > 0) => conn.send(&again),
                Ok(_) => Err(Failure::plain(
                    "the job is not on the server any more and no line of it is known to be done: send the program again yourself",
                )),
                Err(why) => Err(Failure::plain(&why)),
            },
        },
    };
    let mut info = match first {
        Ok(i) => i,
        Err(why) => {
            set(shared, |s| {
                s.error = Some(why.text);
                s.link = Some(conn.link());
                s.over = true;
            });
            return;
        }
    };
    let id = info.id.clone();
    set(shared, |s| {
        s.info = Some(info.clone());
        s.link = Some(conn.link());
    });
    let mut lost = 0u32;
    loop {
        if detach.load(Ordering::SeqCst) {
            return;
        }
        while let Ok(order) = orders.try_recv() {
            if let Err(why) = conn.order(&id, order) {
                set(shared, |s| s.error = Some(why.text));
            }
        }
        match conn.job(&id) {
            Ok(i) => {
                lost = 0;
                info = i;
                let over = info.finished();
                set(shared, |s| {
                    s.info = Some(info.clone());
                    s.link = Some(conn.link());
                    s.over = over;
                });
                if over {
                    return;
                }
            }
            Err(why) => {
                lost += 1;
                // a login that is gone will not come back by waiting: the person has to log in again
                if why.needs_login || lost >= LOST_AFTER {
                    set(shared, |s| {
                        s.error = Some(if why.needs_login {
                            why.text.clone()
                        } else {
                            format!(
                                "lost the server ({}); the job may still be running on it",
                                why.text
                            )
                        });
                        s.link = Some(conn.link());
                        s.over = true;
                    });
                    return;
                }
                set(shared, |s| s.link = Some(conn.link()));
            }
        }
        thread::sleep(Duration::from_millis(POLL_MS));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eh3d_serverclient::sim::{Fault, SimServer};

    /// A stand-in server in memory: answers each request by its method and path, keeps the requests it saw and checks the token that came with them.
    #[derive(Clone)]
    struct Stand {
        script: Vec<(&'static str, u16, String)>,
        seen: Arc<Mutex<Vec<String>>>,
    }

    impl sc::Transport for Stand {
        fn send(&self, r: &sc::Request) -> Result<sc::Reply, sc::TransportError> {
            assert_eq!(r.bearer(), Some("secret"));
            let line = format!("{} {}", r.method, r.path);
            self.seen
                .lock()
                .unwrap()
                .push(format!("{line} | {}", r.body.clone().unwrap_or_default()));
            let mut reply = (404, "{}".to_string());
            for (route, status, body) in &self.script {
                if line.starts_with(route) {
                    reply = (*status, body.clone());
                }
            }
            Ok(sc::Reply {
                status: reply.0,
                body: reply.1,
            })
        }
    }

    fn stand_in(script: Vec<(&'static str, u16, String)>) -> (Stand, Arc<Mutex<Vec<String>>>) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        (
            Stand {
                script,
                seen: seen.clone(),
            },
            seen,
        )
    }

    fn ok(route: &'static str, body: &str) -> (&'static str, u16, String) {
        (route, 200, body.to_string())
    }

    fn link_settings() -> sc::Settings {
        let mut s = sc::Settings::default();
        s.set("server_address", "10.0.0.2:18200").unwrap();
        s.set("server_token", "secret").unwrap();
        s.set("server_machine", "router").unwrap();
        s.set("server_retries", "0").unwrap();
        s
    }

    fn conn(stand: Stand) -> Conn {
        Conn::over(
            sc::Client::new(link_settings(), Box::new(stand)),
            sc::Lang::En,
        )
    }

    fn wait_over(f: &Follow) -> Snapshot {
        for _ in 0..400 {
            let s = f.snapshot();
            if s.over {
                return s;
            }
            thread::sleep(Duration::from_millis(25));
        }
        panic!("the following did not end: {:?}", f.snapshot());
    }

    #[test]
    fn a_machine_or_job_id_that_would_change_the_request_is_refused() {
        assert!(Client::new("10.0.0.2:18200", "t", "../admin").is_none());
        assert!(Client::new("10.0.0.2:18200", "t", "a b").is_none());
        assert!(Client::new("10.0.0.2:18200", "t", &"m".repeat(65)).is_none());
        assert!(Client::new("10.0.0.2:18200", "t", "router_2-b").is_some());
        let (stand, seen) = stand_in(vec![]);
        let mut c = conn(stand);
        for bad in ["", "a/b", "a b", "x?y"] {
            assert!(c.job(bad).is_err(), "{bad:?}");
            assert!(c.order(bad, Order::Pause).is_err(), "{bad:?}");
        }
        // nothing of it went out
        assert!(seen.lock().unwrap().is_empty());
    }

    #[test]
    fn a_client_needs_an_address_and_a_machine() {
        assert!(Client::new("", "t", "m").is_none());
        assert!(Client::new("10.0.0.2:18200", "t", " ").is_none());
        let c = Client::new(" 10.0.0.2:18200 ", " tok ", " router ").unwrap();
        let l = c.link_settings();
        assert_eq!(
            (
                l.server_address.as_str(),
                l.server_token.expose(),
                l.server_machine.as_str()
            ),
            ("10.0.0.2:18200", "tok", "router")
        );
        // a token that holds a space cannot be sent as a bearer: there is no client
        assert!(Client::new("10.0.0.2:18200", "to ken", "router").is_none());
    }

    #[test]
    fn the_client_takes_the_four_limits_and_the_language_from_the_settings_and_never_shows_the_token(
    ) {
        let s = Settings {
            server_address: "10.0.0.2:18200".into(),
            server_token: "very-secret-token".into(),
            server_machine: "router".into(),
            server_timeout_s: 30,
            server_retries: 4,
            server_mdns: true,
            server_max_answer_mib: 16,
            lang: eh3d_view::texts::Lang::Pt,
            ..Settings::default()
        };
        let c = Client::from_settings(&s).unwrap();
        let l = c.link_settings();
        assert_eq!(
            (
                l.server_timeout_s,
                l.server_retries,
                l.server_mdns,
                l.server_max_answer_mib
            ),
            (30, 4, true, 16)
        );
        assert_eq!(c.lang, sc::Lang::Pt);
        assert!(!format!("{c:?}").contains("very-secret-token"));
        // the default limits are the ones of the suite
        let d = Client::from_settings(&Settings {
            server_address: "h:1".into(),
            server_machine: "m".into(),
            ..Settings::default()
        })
        .unwrap();
        let l = d.link_settings();
        assert_eq!(
            (
                l.server_timeout_s,
                l.server_retries,
                l.server_mdns,
                l.server_max_answer_mib
            ),
            (15, 2, false, 8)
        );
        assert_eq!(d.lang, sc::Lang::En);
        assert!(Client::from_settings(&Settings::default()).is_none());
    }

    #[test]
    fn every_state_of_the_link_has_its_word() {
        let words: Vec<&str> = [
            Link::Unknown,
            Link::Connected,
            Link::Disconnected,
            Link::LoggedOut,
        ]
        .into_iter()
        .map(link_word)
        .collect();
        assert_eq!(
            words,
            ["not tried yet", "connected", "disconnected", "logged out"]
        );
    }

    #[test]
    fn the_request_names_the_program_the_line_and_the_outputs() {
        let r = job_request("frame", "G0 X1", 12, false);
        assert_eq!(r["kind"], "gcode");
        assert_eq!(
            (r["skip"].as_u64(), r["restore_outputs"].as_bool()),
            (Some(12), Some(false))
        );
        assert_eq!(r["gcode"], "G0 X1");
    }

    #[test]
    fn a_job_is_read_and_its_state_is_told_apart() {
        let v = json!({"id":"j1","state":"waiting_operator","items_done":3,"items_total":12,"line":9,"message":"","pause":{"kind":"resume","tool":null}});
        let i = JobInfo::from_json(&v);
        assert_eq!((i.id.as_str(), i.percent(), i.line), ("j1", 25, 9));
        assert!(i.active() && i.held() && !i.finished());
        assert_eq!(i.pause, "resume");
        let done = JobInfo::from_json(&json!({"state":"done","items_done":5,"items_total":5}));
        assert!(done.finished() && !done.active());
        assert_eq!(done.percent(), 100);
        assert_eq!(JobInfo::default().percent(), 0);
        assert!(JobInfo::from_json(&json!({"state":"ready"})).state == "ready");
    }

    #[test]
    fn a_refusal_is_told_by_its_code_in_each_of_the_eight_languages_with_the_detail_of_the_server()
    {
        let e = sc::ClientError::Server {
            status: 422,
            code: Some("bad_gcode".into()),
            detail: "line 3: no".into(),
        };
        let mut texts = Vec::new();
        for lang in sc::Lang::ALL {
            let t = error_message(&e, lang);
            assert!(t.ends_with(": line 3: no"), "{t}");
            texts.push(t);
        }
        assert_eq!(texts.len(), 8);
        texts.sort();
        texts.dedup();
        assert_eq!(texts.len(), 8, "every language has its own words");
        // a code the table does not know is shown as itself, with its detail
        let unknown = sc::ClientError::Server {
            status: 500,
            code: Some("new_code_of_a_newer_server".into()),
            detail: "why".into(),
        };
        assert_eq!(
            error_message(&unknown, sc::Lang::En),
            "new_code_of_a_newer_server: why"
        );
        // no detail: only the text
        let plain = sc::ClientError::Server {
            status: 422,
            code: Some("bad_gcode".into()),
            detail: String::new(),
        };
        assert_eq!(
            error_message(&plain, sc::Lang::En),
            plain.text(sc::Lang::En)
        );
    }

    #[test]
    fn a_program_is_sent_started_and_followed_to_its_end() {
        let (stand, seen) = stand_in(vec![
            ok(
                "POST /api/v1/machines/router/jobs",
                r#"{"id":"j1","state":"ready"}"#,
            ),
            ok(
                "POST /api/v1/jobs/j1/start",
                r#"{"id":"j1","state":"connecting"}"#,
            ),
            ok(
                "GET /api/v1/jobs/j1",
                r#"{"id":"j1","state":"done","items_done":4,"items_total":4,"line":5}"#,
            ),
        ]);
        let f = Follow::start_over(
            conn(stand),
            Start::Send(job_request("frame", "G0 X1\nG0 X2", 0, false)),
        );
        let s = wait_over(&f);
        let info = s.info.unwrap();
        assert_eq!((info.state.as_str(), info.line, s.error), ("done", 5, None));
        assert_eq!(s.link, Some(Link::Connected));
        let seen = seen.lock().unwrap().clone();
        assert!(
            seen[0].starts_with("POST /api/v1/machines/router/jobs"),
            "{seen:?}"
        );
        assert!(seen[0].contains("\"kind\":\"gcode\""));
        assert!(seen[1].starts_with("POST /api/v1/jobs/j1/start"));
    }

    #[test]
    fn a_refusal_ends_the_following_with_its_reason() {
        let (stand, _) = stand_in(vec![]);
        let f = Follow::start_over(
            conn(stand),
            Start::Send(job_request("x", "G0 X1", 0, false)),
        );
        let s = wait_over(&f);
        assert!(s.error.is_some() && s.info.is_none());
        // the server answered, so the link is up
        assert_eq!(s.link, Some(Link::Connected));
    }

    #[test]
    fn a_job_that_ended_in_failure_is_made_again_from_its_line_and_an_order_reaches_the_job() {
        let (stand, seen) = stand_in(vec![
            ok(
                "POST /api/v1/machines/router/jobs",
                r#"{"id":"j2","state":"ready"}"#,
            ),
            ok(
                "POST /api/v1/jobs/j2/start",
                r#"{"id":"j2","state":"connecting"}"#,
            ),
            ok(
                "GET /api/v1/jobs/j2",
                r#"{"id":"j2","state":"done","items_done":1,"items_total":1,"line":3}"#,
            ),
            ok(
                "GET /api/v1/jobs/j1",
                r#"{"id":"j1","state":"failed","message":"line 3","line":2}"#,
            ),
            ok("POST /api/v1/jobs/j2/pause", r#"{"ok":true}"#),
        ]);
        // j1 failed at line 2: the program goes on from there as a new job
        let again = job_request("x", "G0 X1\nG0 X2\nG0 X3", 2, false);
        let f = Follow::start_over(
            conn(stand.clone()),
            Start::Attach {
                id: "j1".into(),
                again: Ok(again),
            },
        );
        let s = wait_over(&f);
        assert_eq!(s.info.unwrap().id, "j2");
        let seen = seen.lock().unwrap().clone();
        assert!(seen.iter().any(|l| l.contains("\"skip\":2")), "{seen:?}");
        // an order goes to /api/v1/jobs/<id>/<word>
        let mut c = conn(stand);
        assert!(c.order("j2", Order::Pause).is_ok());
        // the stop has no route here and the job has none either: refused
        assert!(c.order("j2", Order::Cancel).is_err());
        // the stop goes to the machine, not to the job
        let (stand, seen) = stand_in(vec![ok(
            "POST /api/v1/machines/router/stop",
            r#"{"ok":true,"stopped":true}"#,
        )]);
        assert!(conn(stand).order("j2", Order::Cancel).is_ok());
        assert_eq!(seen.lock().unwrap().len(), 1);
    }

    #[test]
    fn no_new_job_is_made_where_going_on_is_not_safe() {
        let failed = r#"{"id":"j1","state":"failed","line":2}"#;
        let done = r#"{"id":"j1","state":"done","line":3}"#;
        let from = |skip| Ok(job_request("x", "G0 X1\nG0 X2\nG0 X3", skip, false));
        for (job, again, want) in [
            (
                failed,
                Err("resume: not here".to_string()),
                "resume: not here",
            ),
            (failed, from(0), "send the program again yourself"),
            (done, from(2), "had finished"),
        ] {
            let (stand, seen) = stand_in(vec![ok("GET /api/v1/jobs/j1", job)]);
            let f = Follow::start_over(
                conn(stand),
                Start::Attach {
                    id: "j1".into(),
                    again,
                },
            );
            let s = wait_over(&f);
            let why = s.error.unwrap();
            assert!(why.contains(want), "{why}");
            // the job was asked about and nothing else went out
            let seen = seen.lock().unwrap().clone();
            assert_eq!(seen, ["GET /api/v1/jobs/j1 | "]);
        }
    }

    #[test]
    fn a_login_that_is_gone_ends_the_following_at_once_and_says_to_log_in() {
        let (stand, _) = stand_in(vec![
            ok(
                "POST /api/v1/machines/router/jobs",
                r#"{"id":"j3","state":"ready"}"#,
            ),
            ok(
                "POST /api/v1/jobs/j3/start",
                r#"{"id":"j3","state":"running"}"#,
            ),
            (
                "GET /api/v1/jobs/j3",
                401,
                r#"{"error":"unauthorized","detail":""}"#.to_string(),
            ),
        ]);
        let started = std::time::Instant::now();
        let f = Follow::start_over(
            conn(stand),
            Start::Send(job_request("x", "G0 X1", 0, false)),
        );
        let s = wait_over(&f);
        // not the forty tries of a lost server (twelve seconds): the first refusal ends it
        assert!(started.elapsed() < Duration::from_secs(5));
        let why = s.error.unwrap();
        assert!(!why.contains("lost the server"), "{why}");
        assert_eq!(s.link, Some(Link::LoggedOut));
        assert!(s.over);
    }

    fn sim_conn(sim: &SimServer, token: &str) -> Conn {
        let mut s = link_settings();
        s.set("server_token", token).unwrap();
        s.set("server_retries", "2").unwrap();
        let inner = sc::Client::new(s, Box::new(sim.clone()))
            .with_sleeper(Box::new(sim.sleeper()))
            .with_clock(Box::new(sim.clock()));
        Conn::over(inner, sc::Lang::Es)
    }

    #[test]
    fn the_program_logs_in_as_phoenix_cnc_when_it_has_no_token_and_not_when_it_has_one() {
        let sim = SimServer::new();
        let mut c = sim_conn(&sim, "");
        assert_eq!(c.link(), Link::Unknown);
        let session = c
            .login_if_needed("admin", "secret-password")
            .unwrap()
            .unwrap();
        assert_eq!(
            (session.app.as_str(), session.role.as_str()),
            (APP, "admin")
        );
        assert_eq!(c.link(), Link::Connected);
        assert_eq!(sim.open_logins(), 1);
        // there is a token now: no second login
        assert!(c
            .login_if_needed("admin", "secret-password")
            .unwrap()
            .is_none());
        assert_eq!(sim.open_logins(), 1);
        // a program token needs none
        let mut p = sim_conn(&sim, "program-token");
        assert!(p.login_if_needed("x", "y").unwrap().is_none());
        assert_eq!(sim.open_logins(), 1);
    }

    #[test]
    fn a_wrong_password_is_told_in_the_language_of_the_window_and_is_not_kept() {
        let sim = SimServer::new();
        let mut c = sim_conn(&sim, "");
        let why = c.login_if_needed("admin", "wrong-password").unwrap_err();
        assert!(why.needs_login);
        assert!(why.text.contains("contraseña"), "{}", why.text);
        assert!(!why.text.contains("wrong-password"));
        assert_eq!(sim.open_logins(), 0);
    }

    #[test]
    fn a_server_that_is_not_there_is_tried_again_with_growing_pauses_and_the_link_says_so() {
        let sim = SimServer::new();
        let mut c = sim_conn(&sim, "program-token");
        sim.script(vec![Fault::NoConnection, Fault::NoConnection]);
        let h = c.health().unwrap();
        assert_eq!(h.status, "ok");
        assert_eq!(
            sim.pauses(),
            [Duration::from_millis(200), Duration::from_millis(400)]
        );
        // more failures than tries: the link is down and the text says it cannot be reached
        sim.script(vec![Fault::NoConnection; 3]);
        let why = c.health().unwrap_err();
        assert_eq!(c.link(), Link::Disconnected);
        assert!(!why.needs_login);
        assert!(
            why.text.starts_with("No se puede llegar al servidor"),
            "{}",
            why.text
        );
    }

    #[test]
    fn a_login_the_server_forgot_is_a_new_login_and_the_token_is_dropped() {
        let sim = SimServer::new();
        let mut c = sim_conn(&sim, "");
        c.login_if_needed("admin", "secret-password").unwrap();
        sim.end_all_logins();
        let why = c.json("GET", "/api/v1/echo", None).unwrap_err();
        assert!(why.needs_login);
        assert_eq!(c.link(), Link::LoggedOut);
        // the token is gone, so the next login is made again
        let again = c.login_if_needed("admin", "secret-password").unwrap();
        assert!(again.is_some());
    }
}
