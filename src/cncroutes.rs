//! The routes of the machine of the plugin `cnc` of the EH3D server, used from the window in a way that cannot do harm by mistake: where the machine is
//! (`position`), a jog (`jog`), the emergency stop (`emergency-stop`) and the release of its latch (`release`). The server keeps the last word (its limits,
//! its roles, the latch); this module refuses first what it already knows is wrong, so a wrong order never leaves the program:
//!
//! * a jog is held to the limits of the settings (the same keys and ranges as the machine of the server: `server_jog_max_mm`, `server_jog_max_feed`) before
//!   it is sent, and a jog longer than `server_jog_confirm_mm` is sent only after the person confirmed it;
//! * a jog is not sent while the last position said that the machine is latched or that a job runs;
//! * the emergency stop is sent at once, with no question, and goes before any other order that waits;
//! * the release of the latch is sent only when the person confirmed it and the login is an administrator's (the server asks for the role as well).
//!
//! The requests run on a thread of their own ([`Watch`]), as the following of a job does, so the window never waits for the network. Nothing here has a
//! window, so all of it is tested with a stand-in for the server. The machine behind these routes is a simulator so far: nothing has moved a real one.

use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use eh3d_serverclient as sc;
use sc::Json;

use crate::server::{error_message, Failure};
use crate::words::{say, Msg};

/// The limits of a jog the machine of the server has by default (`jog_max_mm` and `jog_max_feed` of the plugin).
pub const DEFAULT_JOG_MAX_MM: f64 = 50.0;
pub const DEFAULT_JOG_MAX_FEED: f64 = 1000.0;
/// A jog longer than this many mm is sent only after the person confirmed it.
pub const DEFAULT_CONFIRM_MM: f64 = 10.0;
/// The ranges the settings hold these three numbers to (those of the machine of the server for the first two).
pub const JOG_MAX_MM_RANGE: (f64, f64) = (0.01, 500.0);
pub const JOG_MAX_FEED_RANGE: (f64, f64) = (1.0, 30_000.0);
pub const CONFIRM_MM_RANGE: (f64, f64) = (0.0, 500.0);
/// How often the position is asked, in milliseconds.
pub const POLL_MS: u64 = 500;

/// An axis of a jog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    pub const ALL: [Axis; 3] = [Axis::X, Axis::Y, Axis::Z];

    pub fn letter(self) -> char {
        match self {
            Axis::X => 'X',
            Axis::Y => 'Y',
            Axis::Z => 'Z',
        }
    }

    /// `x`, `Y`... one letter, whatever its case; anything else is not an axis.
    pub fn from_text(text: &str) -> Option<Axis> {
        let mut chars = text.trim().chars();
        let (first, rest) = (chars.next()?, chars.next());
        if rest.is_some() {
            return None;
        }
        Axis::ALL
            .into_iter()
            .find(|a| a.letter().eq_ignore_ascii_case(&first))
    }
}

/// The limits a jog is held to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Limits {
    pub jog_max_mm: f64,
    pub jog_max_feed: f64,
    pub confirm_above_mm: f64,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits {
            jog_max_mm: DEFAULT_JOG_MAX_MM,
            jog_max_feed: DEFAULT_JOG_MAX_FEED,
            confirm_above_mm: DEFAULT_CONFIRM_MM,
        }
    }
}

/// Why an order is not sent. The text comes in the eight languages.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Refusal {
    BadAxis,
    /// The distance is 0, not a number or longer than this many mm.
    Distance(f64),
    /// The feed is 0, not a number or faster than this many mm/min.
    Feed(f64),
    /// The address or the machine of the server is not set.
    NotSet,
    /// A job runs on the machine.
    Running,
    /// The emergency stop is latched.
    Latched,
    NeedAdmin,
    NeedConfirm,
}

fn shown(v: f64) -> String {
    format!("{v}")
}

impl Refusal {
    pub fn text(self, lang: &str) -> String {
        match self {
            Refusal::BadAxis => say::<&str>(lang, Msg::CncBadAxis, &[]),
            Refusal::Distance(max) => say(lang, Msg::CncBadDistance, &[shown(max)]),
            Refusal::Feed(max) => say(lang, Msg::CncBadFeed, &[shown(max)]),
            Refusal::NotSet => say::<&str>(lang, Msg::CncNotSet, &[]),
            Refusal::Running => say::<&str>(lang, Msg::CncRunning, &[]),
            Refusal::Latched => say::<&str>(lang, Msg::CncLatched, &[]),
            Refusal::NeedAdmin => say::<&str>(lang, Msg::CncNeedAdmin, &[]),
            Refusal::NeedConfirm => say::<&str>(lang, Msg::CncNeedConfirm, &[]),
        }
    }

    fn failure(self, lang: &str) -> Failure {
        Failure {
            text: self.text(lang),
            needs_login: false,
        }
    }
}

/// Where the machine is and how it is, as the server says it.
#[derive(Debug, Clone, PartialEq)]
pub struct Position {
    pub machine: [f64; 3],
    pub work: [f64; 3],
    /// `Idle`, `Run` or `Alarm`.
    pub state: String,
    pub spindle_rpm: f64,
    pub latched: bool,
}

fn number(j: &Json) -> Option<f64> {
    match j {
        Json::Num(n) if n.is_finite() => Some(*n),
        _ => None,
    }
}

fn xyz(j: &Json) -> Option<[f64; 3]> {
    Some([
        number(j.get("x")?)?,
        number(j.get("y")?)?,
        number(j.get("z")?)?,
    ])
}

impl Position {
    /// Reads the answer of `GET .../position`; `None` when a part is missing or is not what it should be.
    pub fn parse(body: &str) -> Option<Position> {
        let j = Json::parse(body).ok()?;
        Some(Position {
            machine: xyz(j.get("machine_mm")?)?,
            work: xyz(j.get("work_mm")?)?,
            state: j.get("state")?.as_str()?.to_string(),
            spindle_rpm: number(j.get("spindle_rpm")?)?,
            latched: j.get("latched")?.as_bool()?,
        })
    }

    /// A job runs on the machine.
    pub fn busy(&self) -> bool {
        self.state == "Run"
    }

    /// The text of the position and of the state, for the window.
    pub fn lines(&self, lang: &str) -> [String; 2] {
        let f = |v: f64| format!("{v:.3}");
        [
            say(
                lang,
                Msg::CncPosition,
                &[
                    f(self.work[0]),
                    f(self.work[1]),
                    f(self.work[2]),
                    f(self.machine[0]),
                    f(self.machine[1]),
                    f(self.machine[2]),
                ],
            ),
            say(
                lang,
                Msg::CncState,
                &[self.state.clone(), format!("{}", self.spindle_rpm)],
            ),
        ]
    }
}

/// One jog that passed the limits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Jog {
    pub axis: Axis,
    pub distance_mm: f64,
    pub feed: f64,
}

impl Jog {
    /// A jog inside the limits: a distance that is a number, is not 0 and is at most `jog_max_mm`, and a feed above 0 and at most `jog_max_feed`.
    pub fn new(axis: Axis, distance_mm: f64, feed: f64, l: &Limits) -> Result<Jog, Refusal> {
        if !distance_mm.is_finite() || distance_mm == 0.0 || distance_mm.abs() > l.jog_max_mm {
            return Err(Refusal::Distance(l.jog_max_mm));
        }
        if !feed.is_finite() || feed <= 0.0 || feed > l.jog_max_feed {
            return Err(Refusal::Feed(l.jog_max_feed));
        }
        Ok(Jog {
            axis,
            distance_mm,
            feed,
        })
    }

    /// The body of the request.
    pub fn body(&self) -> String {
        format!(
            "{{\"axis\":\"{}\",\"distance_mm\":{},\"feed\":{}}}",
            self.axis.letter().to_ascii_lowercase(),
            self.distance_mm,
            self.feed
        )
    }

    /// The window asks before it sends a jog this long.
    pub fn needs_confirmation(&self, l: &Limits) -> bool {
        self.distance_mm.abs() > l.confirm_above_mm
    }

    /// What the last position already says against a jog: latched, or a job that runs.
    pub fn check_against(&self, pos: &Position) -> Result<(), Refusal> {
        if pos.latched || pos.state == "Alarm" {
            Err(Refusal::Latched)
        } else if pos.busy() {
            Err(Refusal::Running)
        } else {
            Ok(())
        }
    }

    /// The question before it is sent.
    pub fn question(&self, lang: &str) -> String {
        say(
            lang,
            Msg::CncJogAsk,
            &[
                self.axis.letter().to_string(),
                shown(self.distance_mm),
                shown(self.feed),
            ],
        )
    }
}

fn path(c: &sc::Client, op: &str) -> Result<String, Refusal> {
    let machine = c.settings().server_machine.trim();
    if machine.is_empty() || !c.settings().is_configured() {
        return Err(Refusal::NotSet);
    }
    Ok(format!(
        "/api/v1/work/machines/{}/{op}",
        sc::path_segment(machine)
    ))
}

fn fail(e: &sc::ClientError, lang: &str) -> Failure {
    Failure {
        text: error_message(e, sc::Lang::from_tag(lang).unwrap_or(sc::Lang::En)),
        needs_login: e.needs_login(),
    }
}

/// Asks where the machine is.
pub fn position(c: &mut sc::Client, lang: &str) -> Result<Position, Failure> {
    let p = path(c, "position").map_err(|r| r.failure(lang))?;
    let reply = c.get(&p).map_err(|e| fail(&e, lang))?;
    Position::parse(&reply.body)
        .ok_or_else(|| fail(&sc::ClientError::BadReply("the position".to_string()), lang))
}

/// Sends a jog. `last` is the position the window knows; a jog against it (latched, a job that runs) is not sent. Gives the sentence to show.
pub fn jog(
    c: &mut sc::Client,
    jog: &Jog,
    last: Option<&Position>,
    lang: &str,
) -> Result<String, Failure> {
    if let Some(pos) = last {
        jog.check_against(pos).map_err(|r| r.failure(lang))?;
    }
    let p = path(c, "jog").map_err(|r| r.failure(lang))?;
    c.post(&p, Some(&jog.body())).map_err(|e| fail(&e, lang))?;
    Ok(say(
        lang,
        Msg::CncJogDone,
        &[jog.axis.letter().to_string(), shown(jog.distance_mm)],
    ))
}

/// Sends the emergency stop: at once, with no question. Gives the sentence to show (the machine is latched or it is not).
pub fn emergency_stop(c: &mut sc::Client, lang: &str) -> Result<String, Failure> {
    let p = path(c, "emergency-stop").map_err(|r| r.failure(lang))?;
    let reply = c.post(&p, None).map_err(|e| fail(&e, lang))?;
    let latched = Json::parse(&reply.body)
        .ok()
        .and_then(|j| j.get("latched").and_then(Json::as_bool))
        .unwrap_or(true);
    let msg = if latched {
        Msg::CncStopLatched
    } else {
        Msg::CncStopFree
    };
    Ok(say::<&str>(lang, msg, &[]))
}

/// Releases the latch of the emergency stop. Nothing is sent unless the person confirmed it and the login is an administrator's.
pub fn release(
    c: &mut sc::Client,
    admin: bool,
    confirmed: bool,
    lang: &str,
) -> Result<String, Failure> {
    if !admin {
        return Err(Refusal::NeedAdmin.failure(lang));
    }
    if !confirmed {
        return Err(Refusal::NeedConfirm.failure(lang));
    }
    let p = path(c, "release").map_err(|r| r.failure(lang))?;
    let reply = c.post(&p, None).map_err(|e| fail(&e, lang))?;
    let was = Json::parse(&reply.body)
        .ok()
        .and_then(|j| j.get("released").and_then(Json::as_bool))
        .unwrap_or(true);
    let msg = if was {
        Msg::CncReleased
    } else {
        Msg::CncNotLatched
    };
    Ok(say::<&str>(lang, msg, &[]))
}

/// The question before the latch is released.
pub fn release_question(machine: &str, lang: &str) -> String {
    say(lang, Msg::CncReleaseAsk, &[machine.to_string()])
}

/// An order for the thread that talks to the server.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Cmd {
    Jog(Jog),
    Stop,
    /// `admin` is whether the login is an administrator's; the person has confirmed the release already (the window asks).
    Release {
        admin: bool,
    },
}

/// The orders that wait, with the emergency stop first (the rest keep their order).
pub fn stop_first(mut cmds: Vec<Cmd>) -> Vec<Cmd> {
    cmds.sort_by_key(|c| !matches!(c, Cmd::Stop));
    cmds
}

/// What the window shows of the machine.
#[derive(Debug, Clone, Default)]
pub struct View {
    pub position: Option<Position>,
    /// The last thing the server answered to an order: its number (it grows), the sentence and whether it went well.
    pub said: Option<(u64, String, bool)>,
    /// The state of the link after the last call.
    pub link: Option<sc::Link>,
    /// Why the position cannot be read now.
    pub error: Option<String>,
    /// The cure is a new login.
    pub needs_login: bool,
}

/// The thread that asks for the position and sends the orders. It ends when the `Watch` is dropped.
pub struct Watch {
    shared: Arc<Mutex<View>>,
    orders: Sender<Cmd>,
}

impl Watch {
    /// Starts the thread over a client; the position is asked every `interval`.
    pub fn start(client: sc::Client, lang: &str, interval: Duration) -> Watch {
        let shared = Arc::new(Mutex::new(View::default()));
        let (orders, rx) = channel::<Cmd>();
        let (s, lang) = (shared.clone(), lang.to_string());
        thread::spawn(move || {
            let mut c = client;
            let update = |f: &dyn Fn(&mut View)| {
                if let Ok(mut g) = s.lock() {
                    f(&mut g);
                }
            };
            let mut seq = 0u64;
            loop {
                let mut batch = Vec::new();
                match rx.recv_timeout(interval) {
                    Ok(first) => {
                        batch.push(first);
                        while let Ok(more) = rx.try_recv() {
                            batch.push(more);
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => return,
                }
                for cmd in stop_first(batch) {
                    let known = s.lock().ok().and_then(|g| g.position.clone());
                    let done = match cmd {
                        Cmd::Jog(j) => jog(&mut c, &j, known.as_ref(), &lang),
                        Cmd::Stop => emergency_stop(&mut c, &lang),
                        Cmd::Release { admin } => release(&mut c, admin, true, &lang),
                    };
                    seq += 1;
                    let (text, ok, login) = match done {
                        Ok(t) => (t, true, false),
                        Err(f) => (f.text, false, f.needs_login),
                    };
                    let link = c.link();
                    update(&|v| {
                        v.said = Some((seq, text.clone(), ok));
                        v.link = Some(link);
                        v.needs_login = login;
                    });
                }
                match position(&mut c, &lang) {
                    Ok(p) => {
                        let link = c.link();
                        update(&|v| {
                            v.position = Some(p.clone());
                            v.error = None;
                            v.needs_login = false;
                            v.link = Some(link);
                        });
                    }
                    Err(f) => {
                        let link = c.link();
                        update(&|v| {
                            v.error = Some(f.text.clone());
                            v.needs_login = f.needs_login;
                            v.link = Some(link);
                        });
                    }
                }
            }
        });
        Watch { shared, orders }
    }

    pub fn view(&self) -> View {
        self.shared.lock().map(|g| g.clone()).unwrap_or_default()
    }

    pub fn send(&self, cmd: Cmd) {
        let _ = self.orders.send(cmd);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A stand-in for the server: answers by method and path, keeps what it saw.
    #[derive(Clone)]
    struct Stand {
        script: Vec<(&'static str, u16, String)>,
        seen: Arc<Mutex<Vec<String>>>,
        hits: Arc<AtomicUsize>,
    }

    impl sc::Transport for Stand {
        fn send(&self, r: &sc::Request) -> Result<sc::Reply, sc::TransportError> {
            self.hits.fetch_add(1, Ordering::SeqCst);
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

    const POSITION: &str = r#"{"machine_mm":{"x":1.5,"y":2,"z":-3},"work_mm":{"x":1.5,"y":2,"z":-3},"state":"Idle","spindle_rpm":0,"overrides":{"feed_pct":100,"rapid_pct":100,"spindle_pct":100},"latched":false}"#;

    fn client(machine: &str, script: Vec<(&'static str, u16, String)>) -> (sc::Client, Stand) {
        let stand = Stand {
            script,
            seen: Arc::new(Mutex::new(Vec::new())),
            hits: Arc::new(AtomicUsize::new(0)),
        };
        let mut s = sc::Settings::default();
        s.set("server_address", "192.168.0.180:18200").unwrap();
        s.set("server_machine", machine).unwrap();
        s.set("server_token", "secret").unwrap();
        (
            sc::Client::new(s, Box::new(stand.clone())).with_auto_refresh(false),
            stand,
        )
    }

    fn seen(s: &Stand) -> Vec<String> {
        s.seen.lock().unwrap().clone()
    }

    fn limits() -> Limits {
        Limits::default()
    }

    #[test]
    fn a_jog_inside_the_limits_is_made_and_its_body_is_the_one_the_server_reads() {
        let j = Jog::new(Axis::X, -5.5, 300.0, &limits()).unwrap();
        assert_eq!(j.body(), r#"{"axis":"x","distance_mm":-5.5,"feed":300}"#);
        let parsed = Json::parse(&j.body()).unwrap();
        assert_eq!(parsed.get("axis").and_then(Json::as_str), Some("x"));
        assert_eq!(parsed.get("distance_mm"), Some(&Json::Num(-5.5)));
        // the limits are included
        assert!(Jog::new(Axis::Z, 50.0, 1000.0, &limits()).is_ok());
        assert!(Jog::new(Axis::Z, -50.0, 0.001, &limits()).is_ok());
    }

    #[test]
    fn a_jog_outside_the_limits_or_not_a_number_is_refused_with_the_limit() {
        let l = limits();
        for d in [
            0.0,
            -0.0,
            50.01,
            -50.01,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ] {
            assert_eq!(
                Jog::new(Axis::X, d, 100.0, &l),
                Err(Refusal::Distance(50.0)),
                "{d}"
            );
        }
        for f in [0.0, -1.0, 1000.01, f64::NAN, f64::INFINITY] {
            assert_eq!(
                Jog::new(Axis::X, 1.0, f, &l),
                Err(Refusal::Feed(1000.0)),
                "{f}"
            );
        }
        // the limits of the settings are the ones that hold
        let tight = Limits {
            jog_max_mm: 2.0,
            jog_max_feed: 100.0,
            confirm_above_mm: 0.5,
        };
        assert!(Jog::new(Axis::X, 2.0, 100.0, &tight).is_ok());
        assert_eq!(
            Jog::new(Axis::X, 2.5, 100.0, &tight),
            Err(Refusal::Distance(2.0))
        );
        assert_eq!(
            Jog::new(Axis::X, 1.0, 150.0, &tight),
            Err(Refusal::Feed(100.0))
        );
        let es = Refusal::Distance(2.0).text("es");
        assert!(es.contains("2 mm"), "{es}");
    }

    #[test]
    fn a_long_jog_needs_a_confirmation_and_a_short_one_does_not() {
        let l = limits();
        let long = Jog::new(Axis::Y, 10.5, 100.0, &l).unwrap();
        let edge = Jog::new(Axis::Y, -10.0, 100.0, &l).unwrap();
        let short = Jog::new(Axis::Y, 0.1, 100.0, &l).unwrap();
        assert!(long.needs_confirmation(&l));
        assert!(!edge.needs_confirmation(&l) && !short.needs_confirmation(&l));
        // 0 asks for every jog
        let always = Limits {
            confirm_above_mm: 0.0,
            ..l
        };
        assert!(short.needs_confirmation(&always));
        let q = long.question("de");
        assert!(
            q.contains("Y") && q.contains("10.5") && q.contains("100"),
            "{q}"
        );
    }

    #[test]
    fn the_axes_are_one_letter_whatever_the_case() {
        assert_eq!(Axis::from_text("x"), Some(Axis::X));
        assert_eq!(Axis::from_text(" Z "), Some(Axis::Z));
        for bad in ["", "xy", "a", "xx", "1", "XYZ"] {
            assert_eq!(Axis::from_text(bad), None, "{bad}");
        }
    }

    #[test]
    fn the_position_is_read_and_a_wrong_answer_is_not_a_position() {
        let p = Position::parse(POSITION).unwrap();
        assert_eq!(p.machine, [1.5, 2.0, -3.0]);
        assert_eq!(
            (p.state.as_str(), p.latched, p.busy()),
            ("Idle", false, false)
        );
        for bad in [
            "",
            "{}",
            "[]",
            r#"{"machine_mm":{"x":1,"y":2},"work_mm":{"x":1,"y":2,"z":3},"state":"Idle","spindle_rpm":0,"latched":false}"#,
            r#"{"machine_mm":{"x":1,"y":2,"z":"a"},"work_mm":{"x":1,"y":2,"z":3},"state":"Idle","spindle_rpm":0,"latched":false}"#,
            r#"{"machine_mm":{"x":1,"y":2,"z":3},"work_mm":{"x":1,"y":2,"z":3},"state":5,"spindle_rpm":0,"latched":false}"#,
            r#"{"machine_mm":{"x":1,"y":2,"z":3},"work_mm":{"x":1,"y":2,"z":3},"state":"Idle","spindle_rpm":0}"#,
        ] {
            assert!(Position::parse(bad).is_none(), "{bad}");
        }
        let [pos, state] = p.lines("en");
        assert_eq!(
            pos,
            "Position (mm): work X1.500 Y2.000 Z-3.000, machine X1.500 Y2.000 Z-3.000"
        );
        assert_eq!(state, "State: Idle; spindle 0 rpm");
    }

    #[test]
    fn a_jog_is_not_sent_against_a_latched_machine_or_a_job_that_runs() {
        let (mut c, stand) = client("router", vec![]);
        let j = Jog::new(Axis::X, 1.0, 100.0, &limits()).unwrap();
        let mut p = Position::parse(POSITION).unwrap();
        p.latched = true;
        let r = jog(&mut c, &j, Some(&p), "en").unwrap_err();
        assert!(r.text.contains("latched"), "{}", r.text);
        p.latched = false;
        p.state = "Run".into();
        let r = jog(&mut c, &j, Some(&p), "es").unwrap_err();
        assert!(r.text.contains("trabajo en marcha"), "{}", r.text);
        p.state = "Alarm".into();
        assert!(jog(&mut c, &j, Some(&p), "en").is_err());
        // nothing left the program
        assert!(seen(&stand).is_empty());
    }

    #[test]
    fn the_routes_go_to_the_machine_of_the_settings() {
        let (mut c, stand2) = client(
            "router-1",
            vec![
                (
                    "GET /api/v1/work/machines/router-1/position",
                    200,
                    POSITION.to_string(),
                ),
                (
                    "POST /api/v1/work/machines/router-1/jog",
                    200,
                    "{}".to_string(),
                ),
            ],
        );
        let p = position(&mut c, "en").unwrap();
        assert_eq!(p.work, [1.5, 2.0, -3.0]);
        let j = Jog::new(Axis::Z, 2.0, 200.0, &limits()).unwrap();
        let said = jog(&mut c, &j, Some(&p), "en").unwrap();
        assert_eq!(said, "Jog sent: Z 2 mm");
        assert_eq!(
            seen(&stand2),
            vec![
                "GET /api/v1/work/machines/router-1/position | ".to_string(),
                "POST /api/v1/work/machines/router-1/jog | {\"axis\":\"z\",\"distance_mm\":2,\"feed\":200}".to_string(),
            ]
        );
    }

    #[test]
    fn nothing_is_sent_when_the_machine_or_the_address_is_not_set() {
        let mut s = sc::Settings::default();
        s.set("server_address", "192.168.0.180:18200").unwrap();
        let stand = Stand {
            script: vec![],
            seen: Arc::new(Mutex::new(Vec::new())),
            hits: Arc::new(AtomicUsize::new(0)),
        };
        let mut c = sc::Client::new(s, Box::new(stand.clone()));
        let e = position(&mut c, "fr").unwrap_err();
        assert!(e.text.contains("pas définies"), "{}", e.text);
        assert!(emergency_stop(&mut c, "en").is_err());
        assert!(seen(&stand).is_empty());
    }

    #[test]
    fn a_refusal_of_the_server_is_told_by_its_code_in_the_language_of_the_person() {
        let (mut c, _) = client(
            "router",
            vec![(
                "POST /api/v1/work/machines/router/jog",
                409,
                r#"{"error":"machine_alarm","detail":"latched"}"#.to_string(),
            )],
        );
        let j = Jog::new(Axis::X, 1.0, 100.0, &limits()).unwrap();
        let e = jog(&mut c, &j, None, "es").unwrap_err();
        assert!(!e.text.is_empty() && !e.needs_login, "{}", e.text);
        assert!(e.text.contains("latched"), "{}", e.text);
    }

    #[test]
    fn the_emergency_stop_is_sent_with_no_body_and_says_whether_the_machine_is_latched() {
        let (mut c, stand) = client(
            "router",
            vec![(
                "POST /api/v1/work/machines/router/emergency-stop",
                200,
                r#"{"stopped":true,"latched":true}"#.to_string(),
            )],
        );
        let said = emergency_stop(&mut c, "en").unwrap();
        assert!(said.contains("latched until an administrator"), "{said}");
        assert_eq!(
            seen(&stand),
            vec!["POST /api/v1/work/machines/router/emergency-stop | ".to_string()]
        );
        let (mut c, _) = client(
            "router",
            vec![(
                "POST /api/v1/work/machines/router/emergency-stop",
                200,
                r#"{"stopped":false,"latched":false}"#.to_string(),
            )],
        );
        assert!(emergency_stop(&mut c, "en")
            .unwrap()
            .contains("not latched"));
    }

    #[test]
    fn the_release_needs_an_administrator_and_a_confirmation_and_sends_nothing_without_them() {
        let (mut c, stand) = client(
            "router",
            vec![(
                "POST /api/v1/work/machines/router/release",
                200,
                r#"{"released":true,"latched":false}"#.to_string(),
            )],
        );
        let no_admin = release(&mut c, false, true, "en").unwrap_err();
        assert!(no_admin.text.contains("administrator"), "{}", no_admin.text);
        let unsure = release(&mut c, true, false, "en").unwrap_err();
        assert!(unsure.text.contains("confirmation"), "{}", unsure.text);
        assert!(seen(&stand).is_empty());
        let said = release(&mut c, true, true, "en").unwrap();
        assert_eq!(said, "The latch of the emergency stop was released");
        assert_eq!(
            seen(&stand),
            vec!["POST /api/v1/work/machines/router/release | ".to_string()]
        );
        // a machine that was not latched says so
        let (mut c, _) = client(
            "router",
            vec![(
                "POST /api/v1/work/machines/router/release",
                200,
                r#"{"released":false,"latched":false}"#.to_string(),
            )],
        );
        assert_eq!(
            release(&mut c, true, true, "en").unwrap(),
            "The machine was not latched"
        );
        assert!(release_question("router", "en").contains("router"));
    }

    #[test]
    fn the_emergency_stop_goes_before_the_orders_that_wait() {
        let j = |d| Cmd::Jog(Jog::new(Axis::X, d, 100.0, &limits()).unwrap());
        let got = stop_first(vec![
            j(1.0),
            Cmd::Release { admin: true },
            Cmd::Stop,
            j(2.0),
        ]);
        assert_eq!(got[0], Cmd::Stop);
        assert_eq!(got[1], j(1.0));
        assert_eq!(got[3], j(2.0));
        assert_eq!(stop_first(vec![]), vec![]);
        assert_eq!(stop_first(vec![j(1.0)]), vec![j(1.0)]);
    }

    fn wait_for(w: &Watch, ready: impl Fn(&View) -> bool) -> View {
        for _ in 0..400 {
            let v = w.view();
            if ready(&v) {
                return v;
            }
            thread::sleep(Duration::from_millis(5));
        }
        panic!("the thread did not get there: {:?}", w.view());
    }

    #[test]
    fn the_thread_reads_the_position_sends_the_orders_and_ends_when_it_is_dropped() {
        let (c, stand) = client(
            "router",
            vec![
                (
                    "GET /api/v1/work/machines/router/position",
                    200,
                    POSITION.to_string(),
                ),
                (
                    "POST /api/v1/work/machines/router/emergency-stop",
                    200,
                    r#"{"stopped":true,"latched":true}"#.to_string(),
                ),
                (
                    "POST /api/v1/work/machines/router/jog",
                    200,
                    "{}".to_string(),
                ),
            ],
        );
        let w = Watch::start(c, "en", Duration::from_millis(5));
        let v = wait_for(&w, |v| v.position.is_some());
        assert_eq!(v.position.unwrap().work, [1.5, 2.0, -3.0]);
        assert_eq!(v.link, Some(sc::Link::Connected));
        w.send(Cmd::Jog(Jog::new(Axis::X, 1.0, 100.0, &limits()).unwrap()));
        let v = wait_for(&w, |v| v.said.is_some());
        assert_eq!(v.said.as_ref().map(|s| (s.0, s.2)), Some((1, true)));
        w.send(Cmd::Stop);
        let v = wait_for(&w, |v| v.said.as_ref().is_some_and(|s| s.0 == 2));
        assert!(v.said.unwrap().1.contains("latched"));
        // the order was sent
        assert!(seen(&stand)
            .iter()
            .any(|l| l.starts_with("POST /api/v1/work/machines/router/emergency-stop")));
        // the thread ends with the watch
        drop(w);
        thread::sleep(Duration::from_millis(100));
        let n = stand.hits.load(Ordering::SeqCst);
        thread::sleep(Duration::from_millis(100));
        assert_eq!(stand.hits.load(Ordering::SeqCst), n);
    }

    #[test]
    fn a_server_that_does_not_answer_is_said_and_a_lost_login_asks_for_a_new_one() {
        let (c, _) = client(
            "router",
            vec![(
                "GET /api/v1/work/machines/router/position",
                401,
                r#"{"error":"unauthorized","detail":""}"#.to_string(),
            )],
        );
        let w = Watch::start(c, "en", Duration::from_millis(5));
        let v = wait_for(&w, |v| v.error.is_some());
        assert!(v.needs_login && v.position.is_none(), "{v:?}");
    }

    /// A small generator with a fixed seed, so the properties below are the same on every machine.
    struct Lcg(u64);

    impl Lcg {
        fn next(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((self.0 >> 11) as f64) / ((1u64 << 53) as f64)
        }
    }

    #[test]
    fn for_values_from_a_fixed_seed_a_jog_is_made_exactly_when_it_is_inside_the_limits() {
        let l = limits();
        let mut g = Lcg(12);
        let (mut made, mut refused) = (0, 0);
        for _ in 0..2000 {
            let d = (g.next() - 0.5) * 140.0;
            let f = (g.next() - 0.1) * 1300.0;
            let axis = Axis::ALL[(g.next() * 3.0) as usize % 3];
            let inside = d != 0.0 && d.abs() <= l.jog_max_mm && f > 0.0 && f <= l.jog_max_feed;
            match Jog::new(axis, d, f, &l) {
                Ok(j) => {
                    assert!(inside, "{d} {f}");
                    made += 1;
                    // the body gives back the numbers it was made from
                    let b = Json::parse(&j.body()).unwrap();
                    assert_eq!(b.get("distance_mm"), Some(&Json::Num(d)));
                    assert_eq!(b.get("feed"), Some(&Json::Num(f)));
                    assert_eq!(
                        b.get("axis")
                            .and_then(Json::as_str)
                            .and_then(Axis::from_text),
                        Some(axis)
                    );
                }
                Err(_) => {
                    assert!(!inside, "{d} {f}");
                    refused += 1;
                }
            }
        }
        assert!(made > 500 && refused > 500, "{made} {refused}");
    }

    #[test]
    fn every_refusal_is_told_in_the_eight_languages() {
        for r in [
            Refusal::BadAxis,
            Refusal::Distance(50.0),
            Refusal::Feed(1000.0),
            Refusal::NotSet,
            Refusal::Running,
            Refusal::Latched,
            Refusal::NeedAdmin,
            Refusal::NeedConfirm,
        ] {
            for lang in crate::words::LANGS {
                let t = r.text(lang);
                assert!(!t.is_empty() && !t.contains('{'), "{r:?} {lang}: {t}");
            }
        }
    }
}
