//! The machine on the EH3D server: where it is, a jog by hand, the emergency stop and the release of its latch. The rules are `cncroutes` (tested without a
//! window): a jog is held to the limits of the settings, one longer than `server_jog_confirm_mm` is sent only after the person said yes, nothing is sent
//! against a latched machine or a job that runs, the emergency stop goes at once with no question, and the latch is released only after a question and only
//! with the login of an administrator. The requests run on a thread of their own, so the window never waits for the network.

use std::time::Duration;

use super::*;
use crate::cncroutes::{self, Axis, Cmd, Jog, Limits, Watch};
use crate::words::Msg;

/// What the window waits for an answer to.
enum Ask {
    Jog(Jog),
    Release,
}

/// The window as it is: the thread that talks to the server (none while the address or the machine is not set), the jog being written, the question
/// being asked and the last thing the window refused by itself.
pub(super) struct MachineState {
    watch: Option<Watch>,
    axis: Axis,
    distance: f64,
    feed: f64,
    ask: Option<Ask>,
    refused: Option<String>,
}

impl App {
    /// The limits of a jog, from the settings.
    pub(super) fn jog_limits(&self) -> Limits {
        Limits {
            jog_max_mm: self.settings.server_jog_max_mm,
            jog_max_feed: self.settings.server_jog_max_feed,
            confirm_above_mm: self.settings.server_jog_confirm_mm,
        }
    }

    /// The window for the settings as they are now: the thread starts when the address and the machine are set.
    pub(super) fn machine_state(&self) -> MachineState {
        let limits = self.jog_limits();
        let conn = crate::serverlink::connect(&self.settings, &self.server_login);
        let set =
            conn.settings().is_configured() && !self.settings.server_machine.trim().is_empty();
        MachineState {
            watch: set.then(|| {
                Watch::start(
                    conn,
                    self.lang.code(),
                    Duration::from_millis(cncroutes::POLL_MS),
                )
            }),
            axis: Axis::X,
            distance: self
                .settings
                .step_mm
                .clamp(0.01, limits.jog_max_mm.max(0.01)),
            feed: self
                .settings
                .feed_mm_min
                .clamp(1.0, limits.jog_max_feed.max(1.0)),
            ask: None,
            refused: None,
        }
    }

    pub(super) fn machine_window(
        &mut self,
        ctx: &egui::Context,
        mut st: Box<MachineState>,
    ) -> Option<Win> {
        let lang = self.lang.code();
        let limits = self.jog_limits();
        let view = st.watch.as_ref().map(Watch::view).unwrap_or_default();
        let admin = self
            .server_pane
            .as_ref()
            .and_then(|p| p.panel.who())
            .is_some_and(|w| w.role == "admin");
        let machine = self.settings.server_machine.clone();
        let mut open = true;
        let mut order: Option<Cmd> = None;
        self.window(self.w(Msg::CncTitle))
            .open(&mut open)
            .resizable(false)
            .default_width(440.0)
            .show(ctx, |ui| {
                ui.label(RichText::new(self.w(Msg::CncSimNote)).color(col(self.theme.dim)));
                ui.add_space(6.0);
                if st.watch.is_none() {
                    ui.colored_label(col(self.theme.warn), self.w(Msg::CncNotSet));
                    return;
                }
                // where the machine is
                match (&view.position, &view.error) {
                    (Some(p), _) => {
                        for line in p.lines(lang) {
                            ui.label(RichText::new(line).monospace());
                        }
                    }
                    (None, Some(why)) => {
                        ui.colored_label(col(self.theme.danger), why.as_str());
                    }
                    (None, None) => {
                        ui.label(RichText::new("...").color(col(self.theme.dim)));
                    }
                }
                if view.position.is_some() {
                    if let Some(why) = &view.error {
                        ui.colored_label(col(self.theme.warn), why.as_str());
                    }
                }
                let latched = view.position.as_ref().is_some_and(|p| p.latched);
                let busy = view.position.as_ref().is_some_and(|p| p.busy());
                if latched {
                    ui.colored_label(col(self.theme.danger), self.w(Msg::CncLatched));
                }
                ui.add_space(8.0);

                // the emergency stop: always there, one click, no question
                let stop = egui::Button::new(
                    RichText::new(self.w(Msg::CncStopBtn))
                        .strong()
                        .color(Color32::WHITE),
                )
                .fill(col(self.theme.danger))
                .min_size(Vec2::new(220.0, 40.0));
                if ui.add(stop).clicked() {
                    order = Some(Cmd::Stop);
                }
                ui.add_space(8.0);
                ui.separator();

                // the jog
                ui.label(RichText::new(self.w(Msg::CncJogTitle)).color(col(self.theme.dim)));
                ui.horizontal(|ui| {
                    for a in Axis::ALL {
                        ui.selectable_value(&mut st.axis, a, a.letter().to_string());
                    }
                });
                numfield::field(
                    ui,
                    "mach-dist",
                    &mut st.distance,
                    (-limits.jog_max_mm, limits.jog_max_mm),
                    0.1,
                    "mm",
                );
                numfield::field(
                    ui,
                    "mach-feed",
                    &mut st.feed,
                    (1.0, limits.jog_max_feed),
                    10.0,
                    "mm/min",
                );
                if ui
                    .add_enabled(
                        !latched && !busy && st.ask.is_none(),
                        egui::Button::new(self.w(Msg::CncJogBtn)),
                    )
                    .clicked()
                {
                    st.refused = None;
                    match Jog::new(st.axis, st.distance, st.feed, &limits) {
                        Ok(j) if j.needs_confirmation(&limits) => st.ask = Some(Ask::Jog(j)),
                        Ok(j) => order = Some(Cmd::Jog(j)),
                        Err(r) => st.refused = Some(r.text(lang)),
                    }
                }
                if busy {
                    ui.label(RichText::new(self.w(Msg::CncRunning)).color(col(self.theme.warn)));
                }

                // the release of the latch
                if latched {
                    ui.add_space(6.0);
                    if ui
                        .add_enabled(
                            admin && st.ask.is_none(),
                            egui::Button::new(self.w(Msg::CncReleaseBtn)),
                        )
                        .clicked()
                    {
                        st.ask = Some(Ask::Release);
                    }
                    if !admin {
                        ui.label(
                            RichText::new(self.w(Msg::CncNeedAdmin)).color(col(self.theme.warn)),
                        );
                    }
                }

                // the question
                let mut answer: Option<bool> = None;
                if let Some(ask) = &st.ask {
                    ui.add_space(8.0);
                    ui.group(|ui| {
                        let question = match ask {
                            Ask::Jog(j) => j.question(lang),
                            Ask::Release => cncroutes::release_question(&machine, lang),
                        };
                        ui.label(question);
                        ui.horizontal(|ui| {
                            if ui.button(self.t(Text::Ok)).clicked() {
                                answer = Some(true);
                            }
                            if ui.button(self.t(Text::McCancel)).clicked() {
                                answer = Some(false);
                            }
                        });
                    });
                }
                match (answer, st.ask.take()) {
                    (Some(true), Some(Ask::Jog(j))) => order = Some(Cmd::Jog(j)),
                    (Some(true), Some(Ask::Release)) => order = Some(Cmd::Release { admin }),
                    (None, ask) => st.ask = ask,
                    _ => {}
                }

                // what the window refused by itself, and what the server answered to the last order
                if let Some(text) = &st.refused {
                    ui.colored_label(col(self.theme.warn), text.as_str());
                }
                if let Some((_, text, ok)) = &view.said {
                    let c = if *ok {
                        self.theme.ok
                    } else {
                        self.theme.danger
                    };
                    ui.colored_label(col(c), text.as_str());
                }
            });
        if let (Some(w), Some(cmd)) = (&st.watch, order) {
            w.send(cmd);
        }
        ctx.request_repaint_after(Duration::from_millis(cncroutes::POLL_MS));
        open.then_some(Win::Machine(st))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_opens_and_waits_for_nothing_when_the_server_is_not_set() {
        let mut a = App::isolated();
        a.open_window("machine");
        match a.win.as_ref() {
            Some(Win::Machine(s)) => assert!(s.watch.is_none()),
            _ => panic!("the machine window did not open"),
        }
    }

    #[test]
    fn the_limits_of_a_jog_come_from_the_settings() {
        let mut a = App::isolated();
        a.settings.server_jog_max_mm = 7.0;
        a.settings.server_jog_max_feed = 300.0;
        a.settings.server_jog_confirm_mm = 1.5;
        let l = a.jog_limits();
        assert_eq!(
            (l.jog_max_mm, l.jog_max_feed, l.confirm_above_mm),
            (7.0, 300.0, 1.5)
        );
        // the first jog the window proposes is inside them
        a.settings.step_mm = 100.0;
        a.settings.feed_mm_min = 9000.0;
        let s = a.machine_state();
        assert_eq!((s.distance, s.feed), (7.0, 300.0));
    }
}
