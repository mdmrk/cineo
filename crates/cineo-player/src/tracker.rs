//! Turns mpv's playback events into [`PlayerEvent`]s. Shared by the IPC and
//! embedded backends so both report progress, ends and failures alike.

use std::time::{Duration, Instant};

use crate::PlayerEvent;

const PROGRESS_INTERVAL: Duration = Duration::from_secs(5);

/// One mpv event, independent of how it arrived.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Input {
    TimePos(f64),
    Duration(f64),
    FileLoaded,
    EndFile(EndReason),
}

/// Why mpv stopped playing a file.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum EndReason {
    Eof,
    Error(String),
    /// `stop`, `quit` or `redirect`: the user closed it or mpv replaced it.
    Other,
}

/// What the backend does after an input.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Step {
    /// Forward this to the shell.
    pub(crate) event: Option<PlayerEvent>,
    /// Seek to this position (seconds, absolute) to resume playback.
    pub(crate) seek_to: Option<f64>,
    /// The session is over; `event` is its last event.
    pub(crate) done: bool,
}

/// The position, duration and lifecycle of one playback.
#[derive(Debug)]
pub(crate) struct Tracker {
    start_ms: u64,
    time_ms: u64,
    duration_ms: u64,
    last_report: Option<Instant>,
    started: bool,
}

impl Tracker {
    /// A tracker that resumes at `start_ms` once the file has loaded.
    pub(crate) fn new(start_ms: u64) -> Self {
        Self {
            start_ms,
            time_ms: 0,
            duration_ms: 0,
            last_report: None,
            started: false,
        }
    }

    /// Applies `input` received at `now`.
    pub(crate) fn on(&mut self, input: Input, now: Instant) -> Step {
        match input {
            Input::TimePos(seconds) => {
                self.time_ms = seconds_to_ms(seconds);
                self.progress(now)
            }
            Input::Duration(seconds) => {
                self.duration_ms = seconds_to_ms(seconds);
                self.progress(now)
            }
            Input::FileLoaded => {
                self.started = true;
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "milliseconds of a video fit in f64"
                )]
                let seek_to = (self.start_ms > 0).then(|| self.start_ms as f64 / 1000.0);
                Step {
                    seek_to,
                    ..Step::default()
                }
            }
            Input::EndFile(EndReason::Eof) => finish(PlayerEvent::Ended {
                time_ms: self.time_ms,
                duration_ms: self.duration_ms,
            }),
            Input::EndFile(EndReason::Error(detail)) => finish(PlayerEvent::Failed(format!(
                "mpv could not play the stream: {detail}"
            ))),
            // Before the file loaded this is mpv replacing its idle state.
            Input::EndFile(EndReason::Other) if self.started => finish(self.closed()),
            Input::EndFile(EndReason::Other) => Step::default(),
        }
    }

    /// The last event when mpv goes away without ending the file.
    pub(crate) fn closed(&self) -> PlayerEvent {
        PlayerEvent::Closed {
            time_ms: self.time_ms,
            duration_ms: self.duration_ms,
        }
    }

    fn progress(&mut self, now: Instant) -> Step {
        let due = self
            .last_report
            .is_none_or(|t| now.duration_since(t) >= PROGRESS_INTERVAL);
        if !due || self.time_ms == 0 {
            return Step::default();
        }
        self.last_report = Some(now);
        Step {
            event: Some(PlayerEvent::Progress {
                time_ms: self.time_ms,
                duration_ms: self.duration_ms,
            }),
            ..Step::default()
        }
    }
}

fn finish(event: PlayerEvent) -> Step {
    Step {
        event: Some(event),
        seek_to: None,
        done: true,
    }
}

fn seconds_to_ms(seconds: f64) -> u64 {
    if seconds.is_finite() && seconds > 0.0 {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "positive, finite, sub-2^53"
        )]
        let ms = (seconds * 1000.0) as u64;
        ms
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn progress(time_ms: u64, duration_ms: u64) -> Option<PlayerEvent> {
        Some(PlayerEvent::Progress {
            time_ms,
            duration_ms,
        })
    }

    #[test]
    fn progress_is_throttled_and_skips_zero() {
        let t0 = Instant::now();
        let mut tracker = Tracker::new(0);
        assert_eq!(
            tracker.on(Input::Duration(600.0), t0).event,
            None,
            "no position yet"
        );
        assert_eq!(
            tracker.on(Input::TimePos(1.5), t0).event,
            progress(1500, 600_000)
        );
        assert_eq!(
            tracker
                .on(Input::TimePos(2.0), t0 + Duration::from_secs(1))
                .event,
            None
        );
        assert_eq!(
            tracker
                .on(Input::TimePos(7.0), t0 + PROGRESS_INTERVAL)
                .event,
            progress(7000, 600_000)
        );
    }

    #[test]
    fn resumes_after_the_file_loads() {
        let mut tracker = Tracker::new(61_500);
        let step = tracker.on(Input::FileLoaded, Instant::now());
        assert_eq!(step.seek_to, Some(61.5));
        assert!(!step.done);
        assert_eq!(
            Tracker::new(0)
                .on(Input::FileLoaded, Instant::now())
                .seek_to,
            None
        );
    }

    #[test]
    fn end_reasons_map_to_events() {
        let now = Instant::now();
        let mut tracker = Tracker::new(0);
        tracker.on(Input::TimePos(5.0), now);
        let ended = tracker.on(Input::EndFile(EndReason::Eof), now);
        assert!(ended.done);
        assert_eq!(
            ended.event,
            Some(PlayerEvent::Ended {
                time_ms: 5000,
                duration_ms: 0
            })
        );

        let failed = Tracker::new(0).on(Input::EndFile(EndReason::Error("x".into())), now);
        assert_eq!(
            failed.event,
            Some(PlayerEvent::Failed(
                "mpv could not play the stream: x".into()
            ))
        );
    }

    #[test]
    fn a_stop_counts_only_after_the_file_loaded() {
        let now = Instant::now();
        let mut tracker = Tracker::new(0);
        assert_eq!(
            tracker.on(Input::EndFile(EndReason::Other), now),
            Step::default()
        );
        tracker.on(Input::FileLoaded, now);
        let step = tracker.on(Input::EndFile(EndReason::Other), now);
        assert!(step.done);
        assert_eq!(
            step.event,
            Some(PlayerEvent::Closed {
                time_ms: 0,
                duration_ms: 0
            })
        );
    }

    #[test]
    fn invalid_seconds_are_zero() {
        assert_eq!(seconds_to_ms(f64::NAN), 0);
        assert_eq!(seconds_to_ms(-1.0), 0);
        assert_eq!(seconds_to_ms(1.25), 1250);
    }
}
