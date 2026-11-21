//! Klipper, driven the way `--klipper HOST[:PORT]` drives it: the program opens the HTTP link of the shared controller crate, with the
//! `klipper.*` keys of the motion file, in the guard. The other end is a fake Moonraker in the test itself: a thread with a `TcpListener` on
//! `127.0.0.1` (a port the system picks) that answers with the simulated Klipper of the shared crate. Every test has a fixed seed or none
//! at all, and none touches a real network or a real printer; nothing here has run against a real Klipper.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use eh3d_controller::klippersim::{KlipperDevice, RpcDevice};
use eh3d_controller::{Command, Family, State};
use eh3d_phoenix_cnc::cli::Options;
use eh3d_phoenix_cnc::ctltext::{explain_error, LANGS};
use eh3d_phoenix_cnc::link;
use eh3d_phoenix_cnc::motioncfg::MotionCfg;

/// The credential the tests give; the point of them is that it goes in one header and nowhere else.
const CRED_SAMPLE: &str = "Zq3-sample-credential-8841";

struct FakeMoonraker {
    port: u16,
    heads: Arc<Mutex<Vec<String>>>,
    device: Arc<Mutex<KlipperDevice>>,
}

impl FakeMoonraker {
    fn start() -> FakeMoonraker {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let heads = Arc::new(Mutex::new(Vec::new()));
        let device = Arc::new(Mutex::new(KlipperDevice::new()));
        let (h, d) = (heads.clone(), device.clone());
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { return };
                let (h, d) = (h.clone(), d.clone());
                thread::spawn(move || serve(stream, &h, &d));
            }
        });
        FakeMoonraker {
            port,
            heads,
            device,
        }
    }

    fn address(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }

    fn heads(&self) -> Vec<String> {
        self.heads.lock().unwrap().clone()
    }
}

/// One request: the head, the body, and the answer of the simulated Klipper as the body of the response.
fn serve(mut s: TcpStream, heads: &Mutex<Vec<String>>, device: &Mutex<KlipperDevice>) {
    let _ = s.set_read_timeout(Some(Duration::from_secs(10)));
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    let (head_end, length) = loop {
        let Ok(n) = s.read(&mut chunk) else { return };
        if n == 0 {
            return;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&buf[..i]).to_string();
            let length = head
                .lines()
                .find_map(|l| {
                    let (k, v) = l.split_once(':')?;
                    k.eq_ignore_ascii_case("content-length")
                        .then(|| v.trim().parse::<usize>().ok())?
                })
                .unwrap_or(0);
            break (i + 4, length);
        }
    };
    while buf.len() < head_end + length {
        let Ok(n) = s.read(&mut chunk) else { return };
        if n == 0 {
            return;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    heads
        .lock()
        .unwrap()
        .push(String::from_utf8_lossy(&buf[..head_end]).to_string());
    let body = String::from_utf8_lossy(&buf[head_end..head_end + length]).to_string();
    let answers = device.lock().unwrap().handle(&body);
    let answer = answers
        .into_iter()
        .find(|a| a.contains("\"id\""))
        .unwrap_or_else(|| "{}".to_string());
    let reply = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answer}",
        answer.len()
    );
    let _ = s.write_all(reply.as_bytes());
    let _ = s.flush();
}

fn options(args: &[&str]) -> Options {
    Options::parse(args.iter().map(|a| a.to_string())).unwrap()
}

#[test]
fn klipper_is_driven_over_http_from_the_option_in_the_guard_and_the_credential_goes_in_one_header()
{
    let moonraker = FakeMoonraker::start();
    let (motion, problems) = MotionCfg::parse(&format!("klipper.api_key = {CRED_SAMPLE}\n"));
    assert!(problems.is_empty(), "{problems:?}");
    let o = options(&["--klipper", &moonraker.address()]);
    assert!(o.is_real());
    let mut l = link::guarded_for_motion(&o, Duration::from_secs(5), &motion).unwrap();
    assert!(l.guard.is_some(), "the guard wraps it");
    assert_eq!(l.controller.family(), Family::Klipper);
    assert!(matches!(
        l.controller.status().unwrap().state,
        State::Idle | State::Alarm
    ));
    l.controller.command(Command::Home(vec![])).unwrap();
    l.controller.send_line("G90").unwrap();
    l.controller.send_line("G1 X10 Y5 F600").unwrap();
    let st = l.controller.status().unwrap();
    assert_eq!((st.machine_mm[0], st.machine_mm[1]), (10.0, 5.0));
    assert!(moonraker.device.lock().unwrap().is_homed()[0]);
    let heads = moonraker.heads();
    assert!(heads.len() >= 4, "{}", heads.len());
    for h in &heads {
        assert!(h.starts_with("POST /server/jsonrpc HTTP/1.1"), "{h}");
        assert!(
            h.to_ascii_lowercase()
                .contains(&format!("x-api-key: {}", CRED_SAMPLE.to_ascii_lowercase())),
            "{h}"
        );
    }
}

#[test]
fn a_refusal_of_klipper_through_the_link_is_told_in_each_language_and_the_credential_is_not_in_it()
{
    let moonraker = FakeMoonraker::start();
    let (motion, _) = MotionCfg::parse(&format!("klipper.api_key = {CRED_SAMPLE}\n"));
    let o = options(&["--klipper", &moonraker.address()]);
    let mut c = link::controller_for_motion(&o, Duration::from_secs(5), &motion).unwrap();
    // the machine is not homed: Klipper refuses the move in its own words
    let e = c.send_line("G1 X5 F600").unwrap_err();
    assert_eq!(e.code(), Some(eh3d_controller::klipper::E_HOME_FIRST));
    let mut told = Vec::new();
    for l in LANGS {
        let said = explain_error(Family::Klipper, &e, l);
        assert!(!said.contains(CRED_SAMPLE) && !said.to_lowercase().contains("must home"));
        told.push(said);
    }
    told.sort_unstable();
    told.dedup();
    assert_eq!(told.len(), 8);
    assert!(!format!("{e:?}").contains(CRED_SAMPLE));
}

#[test]
fn nothing_listening_is_a_failure_of_the_link_with_a_number_and_words_in_each_language() {
    // a port that was free a moment ago and has nobody on it
    let port = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let (motion, problems) = MotionCfg::parse("klipper.connect_timeout_ms = 200\n");
    assert!(problems.is_empty());
    let o = options(&["--klipper", &format!("127.0.0.1:{port}")]);
    let mut c = link::controller_for_motion(&o, Duration::from_millis(500), &motion).unwrap();
    let e = c.status().unwrap_err();
    let code = e.code().expect("a failure of the HTTP link has a number");
    assert!((209..=219).contains(&code), "{code}: {e}");
    let mut told = Vec::new();
    for l in LANGS {
        let said = explain_error(Family::Klipper, &e, l);
        assert_ne!(said, e.to_string(), "{l}");
        told.push(said);
    }
    told.sort_unstable();
    told.dedup();
    assert_eq!(told.len(), 8);
}

#[test]
fn an_address_outside_the_local_network_needs_the_switch_and_a_name_needs_it_too() {
    let motion = MotionCfg::default();
    for addr in ["8.8.8.8:7125", "printer.example", "0.0.0.0", "239.255.0.1"] {
        let o = options(&["--klipper", addr]);
        let e = link::controller_for_motion(&o, Duration::from_secs(1), &motion)
            .err()
            .unwrap_or_else(|| panic!("{addr} was taken"));
        assert!(e.starts_with("klipper: "), "{addr}: {e}");
    }
    // with the switch on the public address is taken (nothing is connected until a question is asked); the unspecified one never
    let (open, problems) = MotionCfg::parse("klipper.allow_non_local = on\n");
    assert!(problems.is_empty());
    let o = options(&["--klipper", "8.8.8.8:7125"]);
    assert!(link::controller_for_motion(&o, Duration::from_secs(1), &open).is_ok());
    let o = options(&["--klipper", "0.0.0.0"]);
    assert!(link::controller_for_motion(&o, Duration::from_secs(1), &open).is_err());
}

#[test]
fn the_check_of_klipper_passes_against_its_simulator_and_says_so_in_the_language() {
    let r = eh3d_phoenix_cnc::cli_tools::dialect_check("klipper", None, "es");
    assert_eq!(r.code, 0, "{}{}", r.out, r.err);
    assert!(r.out.contains("Prueba del dialecto superada"), "{}", r.out);
    assert!(r.out.contains("Klipper"), "{}", r.out);
}
