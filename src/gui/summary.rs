//! The summary before a program starts (the dry run of its moves, the time, the reach of the axes and what looks wrong) and the list of the
//! keys. The measure is made by `plan`; this only asks for it, shows it and starts the program when the operator says so.

use super::*;
use crate::plan::{analyse, Checks, Report};
use eh3d_summary::summary::Limits;

/// A start that waits for the operator's yes.
pub(super) struct PendingStart {
    pub report: Report,
    /// The program goes to the EH3D server (true) or to the controller of this window (false).
    pub server: bool,
}

impl App {
    /// What the check needs, from the settings and from where the work zero is now.
    pub(super) fn checks(&self) -> Checks {
        Checks {
            travel: self.settings.travel,
            offset: self.work_offset(),
            default_feed: self.settings.feed_mm_min,
            rapid_mm_min: self.settings.rapid_mm_min,
            limits: Limits {
                tiny_mm: self.settings.summary_tiny_mm,
                far_mm: self.settings.summary_far_mm,
                margin_mm: self.settings.summary_margin_mm,
                reject: self.settings.reject_out_of_travel,
            },
        }
    }

    /// The measure of the program that is loaded.
    pub(super) fn report(&self) -> Report {
        analyse(&self.session.program, &self.checks())
    }

    /// Start asked for: a program that is not running yet waits for the summary (unless the settings turn it off); one that is paused goes on.
    pub(super) fn request_start(&mut self) {
        let fresh = !self.session.running && !self.server_busy();
        if fresh && self.settings.confirm_start && !self.session.program.is_empty() {
            self.win = Some(Win::Summary(Box::new(PendingStart {
                report: self.report(),
                server: false,
            })));
        } else {
            self.program_start();
        }
    }

    /// Send to the server asked for: the same check comes first.
    pub(super) fn request_send(&mut self) {
        if self.settings.confirm_start && !self.session.program.is_empty() {
            self.win = Some(Win::Summary(Box::new(PendingStart {
                report: self.report(),
                server: true,
            })));
        } else {
            self.send_to_server(0);
        }
    }

    pub(super) fn summary_window(
        &mut self,
        ctx: &egui::Context,
        p: Box<PendingStart>,
    ) -> Option<Win> {
        let mut go = false;
        let mut cancel = false;
        let refusal = p.report.refusal(self.lang);
        self.window(self.t(Text::SummaryTitle))
            .default_width(520.0)
            .show(ctx, |ui| {
                if p.report.is_empty() {
                    ui.label(self.t(Text::NoFile));
                }
                let lines = p.report.lines(self.lang);
                let facts = lines.len() - p.report.warnings(self.lang).len();
                for (n, l) in lines.iter().enumerate() {
                    if n < facts {
                        ui.label(RichText::new(l).monospace());
                    } else {
                        ui.colored_label(col(self.theme.warn), l);
                    }
                }
                if let Some(why) = &refusal {
                    ui.add_space(6.0);
                    ui.colored_label(col(self.theme.danger), why);
                }
                ui.separator();
                ui.horizontal(|ui| {
                    let label = if p.report.has_warnings() {
                        self.t(Text::McSumStartAnyway)
                    } else {
                        self.t(Text::Start)
                    };
                    let can = refusal.is_none() && !p.report.is_empty();
                    if ui.add_enabled(can, egui::Button::new(label)).clicked() {
                        go = true;
                    }
                    if ui.button(self.t(Text::McCancel)).clicked() {
                        cancel = true;
                    }
                });
            });
        if go {
            if p.server {
                self.send_to_server(0);
            } else {
                self.program_start();
            }
            return None;
        }
        if cancel {
            return None;
        }
        Some(Win::Summary(p))
    }

    /// The list of the keys.
    pub(super) fn shortcuts_window(&mut self, ctx: &egui::Context) -> Option<Win> {
        let mut close = false;
        self.window(self.t(Text::McShortcuts)).show(ctx, |ui| {
            egui::Grid::new("keys")
                .num_columns(2)
                .spacing([16.0, 4.0])
                .show(ui, |ui| {
                    for b in &keys::BINDINGS {
                        ui.label(
                            RichText::new(b.label)
                                .monospace()
                                .color(col(self.theme.accent)),
                        );
                        ui.label(self.t(b.text));
                        ui.end_row();
                    }
                });
            ui.separator();
            if ui.button(self.t(Text::Close)).clicked() {
                close = true;
            }
        });
        (!close).then_some(Win::Shortcuts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        App::new(&Options::default())
    }

    #[test]
    fn a_fresh_start_waits_for_the_summary_and_a_paused_program_goes_on_without_it() {
        let mut a = app();
        a.session.load("G1 X10 F100\nG1 X20");
        a.request_start();
        let Some(Win::Summary(p)) = &a.win else {
            panic!("no summary")
        };
        assert!(!p.server && p.report.within_travel());
        assert!(!a.session.running);
        // a paused program is not asked again
        a.win = None;
        a.program_start();
        a.program_pause();
        a.request_start();
        assert!(a.win.is_none());
    }

    #[test]
    fn with_the_summary_turned_off_the_program_starts_at_once() {
        let mut a = app();
        a.settings.confirm_start = false;
        a.session.load("G1 X10 F100");
        a.request_start();
        assert!(a.win.is_none() && a.session.running);
    }

    #[test]
    fn the_summary_reads_the_travel_and_the_work_zero_of_the_settings() {
        let mut a = app();
        a.session.load("G1 X450 F100");
        // the sample travel of X ends at 500, so the program fits; narrowing the travel from the settings makes it leave
        assert!(a.report().within_travel());
        a.settings.travel[0] = (0.0, 300.0);
        let r = a.report();
        assert!(!r.within_travel());
        assert!(r.refusal(a.lang).is_some());
        a.settings.reject_out_of_travel = false;
        assert!(a.report().refusal(a.lang).is_none());
    }

    #[test]
    fn the_server_send_goes_through_the_same_summary() {
        let mut a = app();
        a.session.load("G1 X10 F100");
        a.request_send();
        let Some(Win::Summary(p)) = &a.win else {
            panic!("no summary")
        };
        assert!(p.server);
    }
}
