//! A question put to the machine on a thread of its own.
//!
//! Some of what the interface needs to know takes seconds to find out —
//! whether the internet can be reached, which software the image carries —
//! and a frame that waited for it would be a window that froze. So the
//! question is asked on a thread, the frame carries on, and the answer is
//! folded in on whichever frame it arrives by. Until then the last answer
//! (if there was one) stands, and the screen says it is still finding out.

use std::sync::mpsc::{self, Receiver, TryRecvError};

/// One question and what is known of its answer.
#[derive(Debug)]
pub struct Asked<T> {
    /// The last answer, kept while the question is asked again.
    known: Option<T>,
    asking: Option<Receiver<T>>,
}

impl<T> Default for Asked<T> {
    fn default() -> Self {
        Self {
            known: None,
            asking: None,
        }
    }
}

impl<T: Send + 'static> Asked<T> {
    /// Ask on a thread of its own, unless the question is already out.
    pub fn ask(&mut self, work: impl FnOnce() -> T + Send + 'static) {
        if self.asking.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("lxb-installer-asking".into())
            .spawn(move || {
                let _ = tx.send(work());
            });
        if spawned.is_ok() {
            self.asking = Some(rx);
        }
    }

    /// Take the answer as known, with no thread at all: the made-up
    /// machine's, which has nothing to find out.
    pub fn tell(&mut self, answer: T) {
        self.known = Some(answer);
        self.asking = None;
    }

    /// Fold the answer in if it has arrived. Answers whether it did, which
    /// is when a screen applies what follows from it.
    pub fn poll(&mut self) -> bool {
        let Some(asking) = &self.asking else {
            return false;
        };
        match asking.try_recv() {
            Ok(answer) => {
                self.known = Some(answer);
                self.asking = None;
                true
            }
            Err(TryRecvError::Empty) => false,
            // The thread went without an answer: the question is closed and
            // the last answer, if any, stands.
            Err(TryRecvError::Disconnected) => {
                self.asking = None;
                false
            }
        }
    }
}

impl<T> Asked<T> {
    /// The answer, if one is known.
    pub fn known(&self) -> Option<&T> {
        self.known.as_ref()
    }

    /// Whether the question is out now.
    pub fn asking(&self) -> bool {
        self.asking.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_answer_told_is_known_at_once_and_nothing_is_left_asking() {
        let mut online = Asked::default();
        assert!(online.known().is_none() && !online.asking());
        online.tell(true);
        assert_eq!(online.known(), Some(&true));
        assert!(!online.poll(), "a told answer arrived again");
    }

    #[test]
    fn an_answer_from_a_thread_arrives_once() {
        let mut sum = Asked::default();
        sum.ask(|| 2 + 2);
        assert!(sum.asking());
        let mut arrived = 0;
        for _ in 0..2000 {
            if sum.poll() {
                arrived += 1;
            }
            if !sum.asking() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(arrived, 1);
        assert_eq!(sum.known(), Some(&4));
    }
}
