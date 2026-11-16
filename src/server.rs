//! The way to the EH3D server: a G-code program is sent to it as a job, the server runs it on its machine and this program follows it, holds it, lets it go
//! on and stops it. The server's address, the token of this program and the id of the machine are in the settings file, not in the code.
//!
//! The requests are plain HTTP/1.1, one per connection, on a thread of their own, so the window never waits for the network. Closing the program does not stop
//! the job on the server (the machine is the server's); the job is stopped with [`Follow::cancel`].

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde_json::{json, Value};

/// How often the job is asked how it goes, in milliseconds.
pub const POLL_MS: u64 = 300;
/// How many answers in a row may fail before the job is given up as lost (the job may still be running on the server).
pub const LOST_AFTER: u32 = 40;

/// Where the server is and who this program is on it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Client {
    pub address: String,
    pub token: String,
    pub machine: String,
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

/// What the server answers when it refuses, as one line.
pub fn error_text(status: u16, body: &Value) -> String {
    match (body["error"].as_str(), body["detail"].as_str()) {
        (Some(code), Some(detail)) if !detail.is_empty() => format!("{code}: {detail}"),
        (Some(code), _) => code.to_string(),
        _ => format!("the server answered {status}"),
    }
}

/// The most the answer of the server may have: a reply above it is an error, not a reason to keep growing in memory.
const MAX_ANSWER_BYTES: u64 = 1 << 20;

/// Whether a machine or a job id can go into a path as it is: letters, digits, `_` and `-`, up to 64.
fn safe_segment(s: &str) -> bool {
    (1..=64).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn checked_id(id: &str) -> Result<&str, String> {
    if safe_segment(id) {
        Ok(id)
    } else {
        Err(String::from(
            "the id of the job has characters that cannot go into a request",
        ))
    }
}

impl Client {
    /// The client for these settings; `None` while the address or the machine is missing.
    pub fn new(address: &str, token: &str, machine: &str) -> Option<Client> {
        let (a, m) = (address.trim(), machine.trim());
        if a.is_empty() || m.is_empty() || !safe_segment(m) {
            return None;
        }
        Some(Client {
            address: a.to_string(),
            token: token.trim().to_string(),
            machine: m.to_string(),
        })
    }

    /// One request: the status and the JSON body (`null` when it is not JSON).
    pub fn call(
        &self,
        method: &str,
        path: &str,
        body: Option<&Value>,
    ) -> Result<(u16, Value), String> {
        let target = self
            .address
            .to_socket_addrs()
            .map_err(|e| format!("{}: {e}", self.address))?
            .next()
            .ok_or_else(|| format!("{}: no address", self.address))?;
        let mut stream = TcpStream::connect_timeout(&target, Duration::from_secs(5))
            .map_err(|e| format!("{}: {e}", self.address))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(15)))
            .and_then(|_| stream.set_write_timeout(Some(Duration::from_secs(15))))
            .map_err(|e| e.to_string())?;
        let payload = body.map(Value::to_string).unwrap_or_default();
        let mut request = format!(
            "{method} {path} HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\nConnection: close\r\nAccept: application/json\r\n",
            self.address, self.token
        );
        if body.is_some() {
            request.push_str(&format!(
                "Content-Type: application/json\r\nContent-Length: {}\r\n",
                payload.len()
            ));
        } else if method == "POST" || method == "PUT" {
            request.push_str("Content-Length: 0\r\n");
        }
        request.push_str("\r\n");
        request.push_str(&payload);
        stream
            .write_all(request.as_bytes())
            .map_err(|e| e.to_string())?;
        let mut raw = Vec::new();
        (&stream)
            .take(MAX_ANSWER_BYTES + 1)
            .read_to_end(&mut raw)
            .map_err(|e| e.to_string())?;
        if raw.len() as u64 > MAX_ANSWER_BYTES {
            return Err(String::from("the answer of the server is too big"));
        }
        parse_response(&raw)
    }

    fn ok(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Value, String> {
        let (status, answer) = self.call(method, path, body)?;
        if status == 200 {
            Ok(answer)
        } else {
            Err(error_text(status, &answer))
        }
    }

    /// Makes the job on the machine and starts it; the answer is the job. A job the server kept but could not start is not left lying ready on it.
    pub fn send(&self, request: &Value) -> Result<JobInfo, String> {
        let made = self.ok(
            "POST",
            &format!("/api/v1/machines/{}/jobs", self.machine),
            Some(request),
        )?;
        let id = made["id"]
            .as_str()
            .ok_or("the server did not give the job an id")?;
        let id = checked_id(id)?.to_string();
        match self.ok("POST", &format!("/api/v1/jobs/{id}/start"), None) {
            Ok(v) => Ok(JobInfo::from_json(&v)),
            Err(why) => {
                let _ = self.call("POST", &format!("/api/v1/jobs/{id}/cancel"), None);
                Err(why)
            }
        }
    }

    pub fn job(&self, id: &str) -> Result<JobInfo, String> {
        let id = checked_id(id)?;
        Ok(JobInfo::from_json(&self.ok(
            "GET",
            &format!("/api/v1/jobs/{id}"),
            None,
        )?))
    }

    pub fn order(&self, id: &str, what: Order) -> Result<(), String> {
        if what == Order::Cancel {
            // the stop button reaches the machine at once, without waiting for the line the server is sending; a server that does not have it cancels the job
            let stop = format!("/api/v1/machines/{}/stop", self.machine);
            if self.ok("POST", &stop, None).is_ok() {
                return Ok(());
            }
        }
        let id = checked_id(id)?;
        self.ok("POST", &format!("/api/v1/jobs/{id}/{}", what.word()), None)
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

/// The status and the JSON body of an HTTP response (a body sent in chunks is put together).
pub fn parse_response(raw: &[u8]) -> Result<(u16, Value), String> {
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or("the answer of the server is incomplete")?;
    let head = String::from_utf8_lossy(&raw[..split]).to_string();
    let status: u16 = head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .ok_or("the answer of the server has no status")?;
    let body = &raw[split + 4..];
    let chunked = head.lines().any(|l| {
        let l = l.to_ascii_lowercase();
        l.starts_with("transfer-encoding:") && l.contains("chunked")
    });
    let body = if chunked {
        unchunk(body)?
    } else {
        body.to_vec()
    };
    Ok((status, serde_json::from_slice(&body).unwrap_or(Value::Null)))
}

fn unchunk(mut rest: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    loop {
        let eol = rest
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or("a chunk of the answer is cut")?;
        let size = usize::from_str_radix(String::from_utf8_lossy(&rest[..eol]).trim(), 16)
            .map_err(|_| "a chunk of the answer has a bad size")?;
        rest = &rest[eol + 2..];
        if size == 0 {
            return Ok(out);
        }
        if rest.len() < size + 2 {
            return Err("a chunk of the answer is cut".into());
        }
        out.extend_from_slice(&rest[..size]);
        rest = &rest[size + 2..];
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
    /// is made from `again`, which carries the line it starts from.
    Attach { id: String, again: Value },
}

impl Follow {
    pub fn start(client: Client, how: Start) -> Follow {
        let shared = Arc::new(Mutex::new(Snapshot::default()));
        let (orders, rx) = channel();
        let detach = Arc::new(AtomicBool::new(false));
        let (s, d) = (shared.clone(), detach.clone());
        thread::spawn(move || follow(&client, how, &s, &rx, &d));
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
    client: &Client,
    how: Start,
    shared: &Mutex<Snapshot>,
    orders: &Receiver<Order>,
    detach: &AtomicBool,
) {
    let first = match how {
        Start::Send(request) => client.send(&request),
        Start::Attach { id, again } => match client.job(&id) {
            Ok(info) if info.active() || info.state == "ready" => Ok(info),
            // it ended, or the server lost it: the program goes on from the line the server reached (or from the line it knew)
            _ => client.send(&again),
        },
    };
    let mut info = match first {
        Ok(i) => i,
        Err(why) => {
            set(shared, |s| {
                s.error = Some(why);
                s.over = true;
            });
            return;
        }
    };
    let id = info.id.clone();
    set(shared, |s| s.info = Some(info.clone()));
    let mut lost = 0u32;
    loop {
        if detach.load(Ordering::SeqCst) {
            return;
        }
        while let Ok(order) = orders.try_recv() {
            if let Err(why) = client.order(&id, order) {
                set(shared, |s| s.error = Some(why));
            }
        }
        match client.job(&id) {
            Ok(i) => {
                lost = 0;
                info = i;
                let over = info.finished();
                set(shared, |s| {
                    s.info = Some(info.clone());
                    s.over = over;
                });
                if over {
                    return;
                }
            }
            Err(why) => {
                lost += 1;
                if lost >= LOST_AFTER {
                    set(shared, |s| {
                        s.error = Some(format!(
                            "lost the server ({why}); the job may still be running on it"
                        ));
                        s.over = true;
                    });
                    return;
                }
            }
        }
        thread::sleep(Duration::from_millis(POLL_MS));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn a_machine_or_job_id_that_would_change_the_request_is_refused() {
        assert!(Client::new("10.0.0.2:18200", "t", "../admin").is_none());
        assert!(Client::new("10.0.0.2:18200", "t", "a b").is_none());
        assert!(Client::new("10.0.0.2:18200", "t", &"m".repeat(65)).is_none());
        assert!(Client::new("10.0.0.2:18200", "t", "router_2-b").is_some());
        let c = Client::new("10.0.0.2:18200", "t", "router").unwrap();
        for bad in ["", "a/b", "a b", "x?y"] {
            assert!(c.job(bad).is_err(), "{bad:?}");
            assert!(c.order(bad, Order::Pause).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn a_client_needs_an_address_and_a_machine() {
        assert!(Client::new("", "t", "m").is_none());
        assert!(Client::new("10.0.0.2:18200", "t", " ").is_none());
        let c = Client::new(" 10.0.0.2:18200 ", " tok ", " router ").unwrap();
        assert_eq!(
            (c.address.as_str(), c.token.as_str(), c.machine.as_str()),
            ("10.0.0.2:18200", "tok", "router")
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
    fn a_response_is_split_a_chunked_body_is_put_together_and_a_refusal_is_one_line() {
        let raw = b"HTTP/1.1 200 OK\r\ncontent-length: 14\r\n\r\n{\"id\":\"j1234\"}";
        let (status, body) = parse_response(raw).unwrap();
        assert_eq!((status, body["id"].as_str()), (200, Some("j1234")));
        let raw = b"HTTP/1.1 422 X\r\ntransfer-encoding: chunked\r\n\r\n6\r\n{\"erro\r\n7\r\nr\":\"x\"}\r\n0\r\n\r\n";
        let (status, body) = parse_response(raw).unwrap();
        assert_eq!((status, body["error"].as_str()), (422, Some("x")));
        assert!(parse_response(b"HTTP/1.1 200 OK\r\ncontent-le").is_err());
        assert_eq!(
            error_text(422, &json!({"error":"bad_gcode","detail":"line 3: no"})),
            "bad_gcode: line 3: no"
        );
        assert_eq!(error_text(500, &Value::Null), "the server answered 500");
    }

    /// A stand-in server: answers each request by its path, keeps the requests it saw.
    fn stand_in(script: Vec<(&'static str, String)>) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut s) = stream else { continue };
                let mut buf = vec![0u8; 65536];
                let n = s.read(&mut buf).unwrap_or(0);
                let text = String::from_utf8_lossy(&buf[..n]).to_string();
                let line = text.lines().next().unwrap_or("").to_string();
                assert!(text.contains("Authorization: Bearer secret"), "{text}");
                log.lock().unwrap().push(format!(
                    "{line} | {}",
                    text.split("\r\n\r\n").nth(1).unwrap_or("")
                ));
                let mut reply = (404, "{}".to_string());
                for (method_path, body) in &script {
                    if line.starts_with(method_path) {
                        reply = (200, body.clone());
                    }
                }
                let out = format!(
                    "HTTP/1.1 {} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                    reply.0,
                    reply.1.len(),
                    reply.1
                );
                let _ = s.write_all(out.as_bytes());
            }
        });
        (address, seen)
    }

    fn client(address: String) -> Client {
        Client {
            address,
            token: "secret".into(),
            machine: "router".into(),
        }
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
    fn a_program_is_sent_started_and_followed_to_its_end() {
        let (address, seen) = stand_in(vec![
            (
                "POST /api/v1/machines/router/jobs",
                r#"{"id":"j1","state":"ready"}"#.into(),
            ),
            (
                "POST /api/v1/jobs/j1/start",
                r#"{"id":"j1","state":"connecting"}"#.into(),
            ),
            (
                "GET /api/v1/jobs/j1",
                r#"{"id":"j1","state":"done","items_done":4,"items_total":4,"line":5}"#.into(),
            ),
        ]);
        let f = Follow::start(
            client(address),
            Start::Send(job_request("frame", "G0 X1\nG0 X2", 0, false)),
        );
        let s = wait_over(&f);
        let info = s.info.unwrap();
        assert_eq!((info.state.as_str(), info.line, s.error), ("done", 5, None));
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
        let (address, _) = stand_in(vec![]);
        let f = Follow::start(
            client(address),
            Start::Send(job_request("x", "G0 X1", 0, false)),
        );
        let s = wait_over(&f);
        assert!(s.error.is_some() && s.info.is_none());
    }

    #[test]
    fn an_order_reaches_the_job_and_a_job_that_ended_in_failure_is_made_again_from_its_line() {
        let (address, seen) = stand_in(vec![
            (
                "POST /api/v1/machines/router/jobs",
                r#"{"id":"j2","state":"ready"}"#.into(),
            ),
            (
                "POST /api/v1/jobs/j2/start",
                r#"{"id":"j2","state":"connecting"}"#.into(),
            ),
            (
                "GET /api/v1/jobs/j2",
                r#"{"id":"j2","state":"done","items_done":1,"items_total":1,"line":3}"#.into(),
            ),
            (
                "GET /api/v1/jobs/j1",
                r#"{"id":"j1","state":"failed","message":"line 3","line":2}"#.into(),
            ),
            ("POST /api/v1/jobs/j2/pause", r#"{"ok":true}"#.into()),
        ]);
        // j1 failed at line 2: the program goes on from there as a new job
        let again = job_request("x", "G0 X1\nG0 X2\nG0 X3", 2, false);
        let f = Follow::start(
            client(address),
            Start::Attach {
                id: "j1".into(),
                again,
            },
        );
        let s = wait_over(&f);
        assert_eq!(s.info.unwrap().id, "j2");
        let seen = seen.lock().unwrap().clone();
        assert!(seen.iter().any(|l| l.contains("\"skip\":2")), "{seen:?}");
        // an order goes to /api/v1/jobs/<id>/<word>
        let c = Client {
            address: stand_in(vec![(
                "POST /api/v1/jobs/j2/pause",
                r#"{"ok":true}"#.into(),
            )])
            .0,
            token: "secret".into(),
            machine: "router".into(),
        };
        assert!(c.order("j2", Order::Pause).is_ok());
        assert!(c.order("j2", Order::Cancel).is_err());
        // the stop goes to the machine, not to the job
        let c = Client {
            address: stand_in(vec![(
                "POST /api/v1/machines/router/stop",
                r#"{"ok":true,"stopped":true}"#.into(),
            )])
            .0,
            token: "secret".into(),
            machine: "router".into(),
        };
        assert!(c.order("j2", Order::Cancel).is_ok());
    }
}
