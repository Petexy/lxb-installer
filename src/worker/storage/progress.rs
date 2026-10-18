//! The storage half's events: one `Begin` per step, measured `Progress`
//! inside it, and technical `Log` lines for Details.
use crate::model::protocol::{Event, Step, Warning};

/// Reports the step in progress through the worker's event callback.
pub struct Steps<'a> {
    report: &'a mut dyn FnMut(Event),
    current: Option<(Step, u64, u64)>,
}

impl<'a> Steps<'a> {
    pub fn new(report: &'a mut dyn FnMut(Event)) -> Self {
        Self {
            report,
            current: None,
        }
    }

    /// Starts `step`, which takes `total` units of work (at least one, as
    /// the protocol never sends a total of 0).
    pub fn begin(&mut self, step: Step, total: u64) {
        let total = total.max(1);
        (self.report)(Event::Begin { step });
        (self.report)(Event::Progress {
            step,
            done: 0,
            total,
        });
        self.current = Some((step, 0, total));
    }

    /// One more unit of the current step is done.
    pub fn advance(&mut self) {
        if let Some((step, done, total)) = self.current {
            self.set(step, done + 1, total);
        }
    }

    /// Measured progress in the current step's own units (bytes written).
    pub fn measure(&mut self, done: u64, total: u64) {
        if let Some((step, _, _)) = self.current {
            self.set(step, done, total.max(1));
        }
    }

    fn set(&mut self, step: Step, done: u64, total: u64) {
        let done = done.min(total);
        self.current = Some((step, done, total));
        (self.report)(Event::Progress { step, done, total });
    }

    /// A technical line for Details and the log.
    pub fn log(&mut self, line: impl Into<String>) {
        (self.report)(Event::Log { line: line.into() });
    }

    /// Something Done says once the installation has finished: the
    /// installed system is sound, but the person should know it (a Windows
    /// partition shrunk, which Windows checks the next time it starts).
    pub fn warn(&mut self, warning: Warning) {
        (self.report)(Event::Warning { warning });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_never_passes_its_total_and_never_has_a_zero_total() {
        let mut events = Vec::new();
        let mut report = |event: Event| events.push(event);
        let mut steps = Steps::new(&mut report);
        steps.advance();
        steps.begin(Step::Swap, 0);
        steps.advance();
        steps.advance();
        steps.measure(5, 0);
        assert_eq!(
            events,
            [
                Event::Begin { step: Step::Swap },
                Event::Progress {
                    step: Step::Swap,
                    done: 0,
                    total: 1
                },
                Event::Progress {
                    step: Step::Swap,
                    done: 1,
                    total: 1
                },
                Event::Progress {
                    step: Step::Swap,
                    done: 1,
                    total: 1
                },
                Event::Progress {
                    step: Step::Swap,
                    done: 1,
                    total: 1
                },
            ]
        );
    }
}
