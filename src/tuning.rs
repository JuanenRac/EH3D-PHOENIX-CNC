//! The values of the program tools (the checker, the keep-out boxes, the repeat and pause rules of the queue, the session log, the macros and
//! the statistics) that the operator may tune in the settings file. The lines are the same `key = value` lines of the settings file, so
//! they can live in that file: what is not known is left alone and what is not valid keeps its default. The example text is the defaults.

use crate::boxes::KeepOut;

#[derive(Debug, Clone, PartialEq)]
pub struct Tuning {
    /// The second checker of the program (`progcheck`): on by default. The limits of feed and speed and the floor of the rapid moves
    /// are those of `lint` in the settings (`lint_max_feed`, `lint_max_rpm`, `lint_rapid_floor`).
    pub progcheck: bool,
    /// The boxes the tool must not enter and how much bigger they are tested (`keepout`, `keepout_margin`).
    pub keepouts: Vec<KeepOut>,
    pub keepout_margin: f64,
    /// A program with a move that enters a box cannot be started (`keepout_reject`); off: it is only said.
    pub keepout_reject: bool,
    /// The most a program may be repeated in the queue, whether the operator confirms between two runs, and the wait in seconds when
    /// they do not (`queue_repeat_max`, `queue_confirm_between`, `queue_pause`).
    pub repeat_max: u32,
    pub confirm_between: bool,
    pub pause_s: f64,
    /// The queue of programs is kept in a file between runs (`queue_remember`); what it holds never starts by itself.
    pub queue_remember: bool,
    /// The speed of the simulation of a program, as a multiple of real time (`sim_speed`).
    pub sim_speed: f64,
    /// The most entries the session log keeps (`log_max_entries`).
    pub log_max: usize,
    /// Every macro asks before it runs (`macros_confirm`) and the most lines of one (`macros_max_lines`).
    pub macros_confirm: bool,
    pub macro_max_lines: usize,
    /// The time a tool change adds to the estimate, in seconds (`tool_change_time`).
    pub tool_change_s: f64,
    /// Starting a program from one of its lines asks for a tick that says the tool, the spindle and the zero were checked
    /// (`startline_confirm`).
    pub startline_confirm: bool,
}

impl Default for Tuning {
    fn default() -> Self {
        Tuning {
            progcheck: true,
            keepouts: Vec::new(),
            keepout_margin: 1.0,
            keepout_reject: true,
            repeat_max: 100,
            confirm_between: true,
            pause_s: 0.0,
            queue_remember: true,
            sim_speed: 10.0,
            log_max: 5_000,
            macros_confirm: true,
            macro_max_lines: 200,
            tool_change_s: 30.0,
            startline_confirm: true,
        }
    }
}

fn on(v: &str) -> bool {
    matches!(
        v.to_ascii_lowercase().as_str(),
        "yes" | "on" | "true" | "1" | "y"
    )
}

fn yes_no(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}

impl Tuning {
    /// Reads the `key = value` lines; a line that starts with `#` or `;`, a key that is not known and a value that is not valid are
    /// left out and the default stays.
    pub fn parse(text: &str) -> Tuning {
        let mut t = Tuning::default();
        for line in text.trim_start_matches('\u{feff}').lines() {
            let line = line.trim();
            if line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let (k, v) = (k.trim().to_ascii_lowercase(), v.trim());
            let num = |v: &str| {
                v.replace(',', ".")
                    .parse::<f64>()
                    .ok()
                    .filter(|x| x.is_finite())
            };
            match k.as_str() {
                "progcheck" => t.progcheck = on(v),
                "keepout" => {
                    if let Some(b) = KeepOut::parse(v) {
                        // a name that comes again replaces the earlier box; there are never more than 64
                        t.keepouts.retain(|k| k.name != b.name);
                        if t.keepouts.len() < 64 {
                            t.keepouts.push(b);
                        }
                    }
                }
                "keepout_margin" => {
                    t.keepout_margin = num(v)
                        .filter(|x| (0.0..=100.0).contains(x))
                        .unwrap_or(t.keepout_margin)
                }
                "keepout_reject" => t.keepout_reject = on(v),
                "queue_remember" => t.queue_remember = on(v),
                "sim_speed" => {
                    t.sim_speed = num(v)
                        .filter(|x| (0.1..=10_000.0).contains(x))
                        .unwrap_or(t.sim_speed)
                }
                "queue_repeat_max" => {
                    t.repeat_max = v
                        .parse::<u32>()
                        .ok()
                        .filter(|x| (1..=10_000).contains(x))
                        .unwrap_or(t.repeat_max)
                }
                "queue_confirm_between" => t.confirm_between = on(v),
                "queue_pause" => {
                    t.pause_s = num(v)
                        .filter(|x| (0.0..=86_400.0).contains(x))
                        .unwrap_or(t.pause_s)
                }
                "log_max_entries" => {
                    t.log_max = v
                        .parse::<usize>()
                        .ok()
                        .filter(|x| (10..=1_000_000).contains(x))
                        .unwrap_or(t.log_max)
                }
                "macros_confirm" => t.macros_confirm = on(v),
                "macros_max_lines" => {
                    t.macro_max_lines = v
                        .parse::<usize>()
                        .ok()
                        .filter(|x| (1..=10_000).contains(x))
                        .unwrap_or(t.macro_max_lines)
                }
                "startline_confirm" => t.startline_confirm = on(v),
                "tool_change_time" => {
                    t.tool_change_s = num(v)
                        .filter(|x| (0.0..=3600.0).contains(x))
                        .unwrap_or(t.tool_change_s)
                }
                _ => {}
            }
        }
        t
    }

    /// The lines of the settings file for these values. The text of `Tuning::default()` is the example shipped with the program.
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        let mut put = |k: &str, v: String| s.push_str(&format!("{k} = {v}\n"));
        put("progcheck", yes_no(self.progcheck).to_string());
        put("keepout_margin", self.keepout_margin.to_string());
        put("keepout_reject", yes_no(self.keepout_reject).to_string());
        put("queue_remember", yes_no(self.queue_remember).to_string());
        put("sim_speed", self.sim_speed.to_string());
        put("queue_repeat_max", self.repeat_max.to_string());
        put(
            "queue_confirm_between",
            yes_no(self.confirm_between).to_string(),
        );
        put("queue_pause", self.pause_s.to_string());
        put("log_max_entries", self.log_max.to_string());
        put("macros_confirm", yes_no(self.macros_confirm).to_string());
        put("macros_max_lines", self.macro_max_lines.to_string());
        put("tool_change_time", self.tool_change_s.to_string());
        put(
            "startline_confirm",
            yes_no(self.startline_confirm).to_string(),
        );
        for b in &self.keepouts {
            s.push_str(&b.to_setting());
            s.push('\n');
        }
        s
    }

    /// The example of the settings file: the defaults, with a keep-out box shown as a comment.
    pub fn example() -> String {
        let mut s = Tuning::default().to_text();
        s.push_str("# keepout = clamp 0 0 -5 20 20 40\n");
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_example_is_the_defaults() {
        assert_eq!(Tuning::parse(&Tuning::example()), Tuning::default());
        assert_eq!(Tuning::parse(""), Tuning::default());
    }

    #[test]
    fn the_defaults_are_the_cautious_ones() {
        let d = Tuning::default();
        assert!(d.progcheck && d.confirm_between && d.macros_confirm);
        // starting from a line asks for the tick
        assert!(d.startline_confirm);
        assert!(!Tuning::parse("startline_confirm = no").startline_confirm);
        // a program that enters a box does not start, and the queue kept in the file waits for the operator
        assert!(d.keepout_reject && d.queue_remember);
        assert_eq!(d.sim_speed, 10.0);
        assert_eq!(d.repeat_max, 100);
        assert_eq!(d.pause_s, 0.0);
        assert!(d.keepouts.is_empty());
    }

    #[test]
    fn values_are_read_and_a_wrong_one_keeps_the_default() {
        let t = Tuning::parse(
            "queue_repeat_max = 0\n\
             queue_pause = 90\nlog_max_entries = 5\nmacros_max_lines = 50\ntool_change_time = 1e9\nPROGCHECK = no\n",
        );
        assert_eq!(t.repeat_max, 100);
        assert_eq!(t.pause_s, 90.0);
        assert_eq!(t.log_max, 5_000);
        assert_eq!(t.macro_max_lines, 50);
        assert_eq!(t.tool_change_s, 30.0);
        assert!(!t.progcheck);
    }

    #[test]
    fn the_refusal_the_memory_of_the_queue_and_the_speed_of_the_simulation_are_read() {
        let t = Tuning::parse("keepout_reject = no\nqueue_remember = off\nsim_speed = 250\n");
        assert!(!t.keepout_reject && !t.queue_remember);
        assert_eq!(t.sim_speed, 250.0);
        // out of the range keeps the default
        assert_eq!(Tuning::parse("sim_speed = 0\n").sim_speed, 10.0);
        assert_eq!(Tuning::parse("sim_speed = 1e9\n").sim_speed, 10.0);
        assert_eq!(Tuning::parse(&t.to_text()), t);
    }

    #[test]
    fn nan_and_infinite_values_are_refused() {
        let t = Tuning::parse("queue_pause = inf\nkeepout_margin = -1\n");
        assert_eq!(t, Tuning::default());
    }

    #[test]
    fn comments_and_unknown_keys_are_ignored_and_a_bom_is_skipped() {
        let t = Tuning::parse(
            "\u{feff}# progcheck = no\n; queue_pause = 1\nwhatever = 3\nqueue_pause = 9000\n",
        );
        assert_eq!(t.pause_s, 9000.0);
        assert!(t.progcheck);
    }

    #[test]
    fn boxes_are_read_replaced_and_written_back() {
        let t = Tuning::parse("keepout = a 0 0 0 1 1 1\nkeepout = b 5 5 5 6 6 6\nkeepout = a 0 0 0 2 2 2\nkeepout = bad 1 1\n");
        assert_eq!(t.keepouts.len(), 2);
        assert_eq!(t.keepouts[1].name, "a");
        assert_eq!(t.keepouts[1].max, [2.0, 2.0, 2.0]);
        assert_eq!(Tuning::parse(&t.to_text()), t);
    }

    #[test]
    fn there_are_never_more_than_sixty_four_boxes() {
        let mut text = String::new();
        for i in 0..80 {
            text.push_str(&format!("keepout = b{i} 0 0 0 1 1 1\n"));
        }
        assert_eq!(Tuning::parse(&text).keepouts.len(), 64);
    }

    #[test]
    fn switches_accept_the_usual_words() {
        for (w, expect) in [
            ("yes", true),
            ("On", true),
            ("TRUE", true),
            ("1", true),
            ("no", false),
            ("off", false),
            ("0", false),
            ("", false),
        ] {
            assert_eq!(
                Tuning::parse(&format!("queue_confirm_between = {w}")).confirm_between,
                expect,
                "{w}"
            );
        }
    }
}
