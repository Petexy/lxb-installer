//! An installation as the interface follows it: the worker's events, folded
//! into what the Installing screen shows.
//!
//! Pure: [`Install::take`] is handed one event at a time, from the worker
//! started through polkit or from the demo's rehearsal, and nothing here knows
//! which. The bar is weighted by how long each step usually takes rather than
//! counting steps as equals: copying the system is most of an installation,
//! and a bar that spent a twelfth of its length on it would crawl there and
//! then race through eleven steps that take seconds.

use crate::model::protocol::{Event, Failure, Step, Warning};

/// How the installation ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ending {
    Finished,
    Failed {
        step: Option<Step>,
        failure: Failure,
        /// Technical: for Details and the log only.
        detail: String,
    },
}

/// Everything the interface knows about an installation in progress.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Install {
    /// The steps this run takes, from the worker's plan.
    pub steps: Vec<Step>,
    /// The step under way.
    pub current: Option<Step>,
    /// Measured progress inside it, when the worker measures it.
    pub done: u64,
    pub total: u64,
    /// The technical lines, for Details.
    pub log: Vec<String>,
    pub warnings: Vec<Warning>,
    pub ending: Option<Ending>,
}

/// The most log lines kept for Details. The worker's own log file has every
/// one; a window holding a copy of a long `pacman -Syu` would be a window
/// holding megabytes for a view that shows the last screenful.
pub const LOG_KEPT: usize = 2000;

/// How long a step usually takes, as a share of a whole installation. Only
/// the proportions matter: the shares of the steps a plan holds are scaled to
/// fill the bar.
pub fn weight(step: Step) -> f32 {
    match step {
        Step::Copy => 40.0,
        Step::Updates => 18.0,
        Step::Flatpaks => 10.0,
        Step::Desktop => 8.0,
        Step::Initramfs => 5.0,
        Step::Packages | Step::Resize => 4.0,
        Step::Bootloader => 3.0,
        Step::Format | Step::Drivers | Step::Snapshots | Step::System => 2.0,
        Step::Check
        | Step::Partition
        | Step::Swap
        | Step::Region
        | Step::Accounts
        | Step::Finish => 1.0,
    }
}

impl Install {
    /// Fold one event in. An installation ends once: whatever ending comes
    /// first stands, and another after it is not taken.
    pub fn take(&mut self, event: Event) {
        if self.ending.is_some() && matches!(event, Event::Finished | Event::Failed { .. }) {
            return;
        }
        match event {
            Event::Plan { steps } => self.steps = steps,
            Event::Begin { step } => {
                self.current = Some(step);
                self.done = 0;
                self.total = 0;
            }
            Event::Progress { step, done, total } => {
                if self.current != Some(step) {
                    self.current = Some(step);
                }
                self.done = done.min(total);
                self.total = total;
            }
            Event::Log { line } => {
                self.log.push(line);
                if self.log.len() > LOG_KEPT {
                    let extra = self.log.len() - LOG_KEPT;
                    self.log.drain(..extra);
                }
            }
            Event::Warning { warning } => {
                if !self.warnings.contains(&warning) {
                    self.warnings.push(warning);
                }
            }
            Event::Finished => {
                self.current = None;
                self.ending = Some(Ending::Finished);
            }
            Event::Failed {
                step,
                failure,
                detail,
            } => {
                self.ending = Some(Ending::Failed {
                    step: step.or(self.current),
                    failure,
                    detail,
                });
            }
        }
    }

    /// Which step of how many this is, counting from one: "Step 4 of 12".
    pub fn place(&self) -> (usize, usize) {
        let of = self.steps.len();
        let at = self
            .current
            .and_then(|step| self.steps.iter().position(|one| *one == step))
            .map_or(0, |index| index + 1);
        let at = if self.ending == Some(Ending::Finished) {
            of
        } else {
            at
        };
        (at.min(of), of)
    }

    /// How far through the whole installation it is, nought to one.
    pub fn through(&self) -> f32 {
        if self.ending == Some(Ending::Finished) {
            return 1.0;
        }
        let total: f32 = self.steps.iter().map(|step| weight(*step)).sum();
        if total <= 0.0 {
            return 0.0;
        }
        let Some(index) = self
            .current
            .and_then(|step| self.steps.iter().position(|one| *one == step))
        else {
            return 0.0;
        };
        let before: f32 = self.steps[..index].iter().map(|step| weight(*step)).sum();
        let inside = if self.total > 0 {
            self.done as f32 / self.total as f32
        } else {
            0.0
        };
        ((before + weight(self.steps[index]) * inside) / total).clamp(0.0, 1.0)
    }

    pub fn finished(&self) -> bool {
        self.ending == Some(Ending::Finished)
    }

    pub fn failed(&self) -> bool {
        matches!(self.ending, Some(Ending::Failed { .. }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn planned() -> Install {
        let mut install = Install::default();
        install.take(Event::Plan {
            steps: vec![
                Step::Check,
                Step::Partition,
                Step::Copy,
                Step::Bootloader,
                Step::Finish,
            ],
        });
        install
    }

    #[test]
    fn the_bar_is_weighted_by_how_long_steps_take() {
        let mut install = planned();
        assert_eq!(install.through(), 0.0);
        install.take(Event::Begin { step: Step::Copy });
        let at_copy = install.through();
        install.take(Event::Progress {
            step: Step::Copy,
            done: 1,
            total: 2,
        });
        let halfway = install.through();
        assert!(halfway > at_copy, "progress inside a step moved nothing");
        let total = 1.0 + 1.0 + 40.0 + 3.0 + 1.0;
        assert!((at_copy - 2.0 / total).abs() < 1e-6);
        assert!((halfway - 22.0 / total).abs() < 1e-6);
        assert_eq!(install.place(), (3, 5));
    }

    #[test]
    fn the_end_is_the_whole_bar_and_the_last_step() {
        let mut install = planned();
        install.take(Event::Begin { step: Step::Finish });
        install.take(Event::Finished);
        assert!(install.finished());
        assert_eq!(install.through(), 1.0);
        assert_eq!(install.place(), (5, 5));
    }

    #[test]
    fn a_failure_remembers_where_it_happened() {
        let mut install = planned();
        install.take(Event::Begin {
            step: Step::Partition,
        });
        install.take(Event::Failed {
            step: None,
            failure: Failure::DiskBusy,
            detail: "busy".into(),
        });
        assert!(install.failed());
        assert_eq!(
            install.ending,
            Some(Ending::Failed {
                step: Some(Step::Partition),
                failure: Failure::DiskBusy,
                detail: "busy".into()
            })
        );
    }

    #[test]
    fn the_first_ending_stands() {
        let mut install = planned();
        install.take(Event::Finished);
        install.take(Event::Failed {
            step: None,
            failure: Failure::Internal,
            detail: "said after the end".into(),
        });
        assert!(
            install.finished(),
            "a finished installation became a failed one"
        );
        let mut install = planned();
        install.take(Event::Failed {
            step: Some(Step::Copy),
            failure: Failure::DiskBusy,
            detail: "busy".into(),
        });
        install.take(Event::Finished);
        assert!(
            install.failed(),
            "a failed installation became a finished one"
        );
    }

    #[test]
    fn the_log_kept_for_details_does_not_grow_without_end() {
        let mut install = Install::default();
        for line in 0..LOG_KEPT + 50 {
            install.take(Event::Log {
                line: line.to_string(),
            });
        }
        assert_eq!(install.log.len(), LOG_KEPT);
        assert_eq!(install.log[0], "50");
        install.take(Event::Warning {
            warning: Warning::UpdatesFailed,
        });
        install.take(Event::Warning {
            warning: Warning::UpdatesFailed,
        });
        assert_eq!(
            install.warnings.len(),
            1,
            "a warning said twice is one warning"
        );
    }
}
