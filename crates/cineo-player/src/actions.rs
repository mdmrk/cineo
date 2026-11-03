use cineo_core::app::Action;

use crate::PlayerEvent;

#[derive(Debug, Clone, PartialEq)]
pub struct EventActions {
    pub actions: Vec<Action>,
    pub last: bool,
}

pub fn event_actions(
    event: PlayerEvent,
    meta_id: &str,
    video_id: &str,
    now_ms: u64,
) -> EventActions {
    let (time_ms, duration_ms, last) = match event {
        PlayerEvent::Progress {
            time_ms,
            duration_ms,
        } => (time_ms, duration_ms, None),
        PlayerEvent::Ended {
            time_ms,
            duration_ms,
        } => (time_ms, duration_ms, Some(Action::PlaybackEnded)),
        PlayerEvent::Closed {
            time_ms,
            duration_ms,
        } => (time_ms, duration_ms, Some(Action::PlaybackStopped)),
        PlayerEvent::Failed(reason) => {
            return EventActions {
                actions: vec![Action::PlaybackFailed(reason)],
                last: true,
            };
        }
        PlayerEvent::Buffered { .. } => {
            return EventActions {
                actions: Vec::new(),
                last: false,
            };
        }
    };
    let mut actions = Vec::new();
    if time_ms > 0 {
        actions.push(Action::PlaybackProgress {
            meta_id: meta_id.to_owned(),
            video_id: video_id.to_owned(),
            time_ms,
            duration_ms,
            now_ms,
        });
    }
    let done = last.is_some();
    actions.extend(last);
    EventActions {
        actions,
        last: done,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn progress(time_ms: u64) -> Action {
        Action::PlaybackProgress {
            meta_id: "tt1".into(),
            video_id: "tt1:1:2".into(),
            time_ms,
            duration_ms: 9_000,
            now_ms: 5,
        }
    }

    #[test]
    fn player_events_become_progress_and_a_final_action() {
        let run = |event| event_actions(event, "tt1", "tt1:1:2", 5);
        assert_eq!(
            run(PlayerEvent::Progress {
                time_ms: 1_000,
                duration_ms: 9_000
            }),
            EventActions {
                actions: vec![progress(1_000)],
                last: false
            }
        );
        assert_eq!(
            run(PlayerEvent::Progress {
                time_ms: 0,
                duration_ms: 9_000
            }),
            EventActions {
                actions: vec![],
                last: false
            }
        );
        assert_eq!(
            run(PlayerEvent::Ended {
                time_ms: 9_000,
                duration_ms: 9_000
            }),
            EventActions {
                actions: vec![progress(9_000), Action::PlaybackEnded],
                last: true
            }
        );
        assert_eq!(
            run(PlayerEvent::Closed {
                time_ms: 0,
                duration_ms: 9_000
            }),
            EventActions {
                actions: vec![Action::PlaybackStopped],
                last: true
            }
        );
        assert_eq!(
            run(PlayerEvent::Failed("gone".into())),
            EventActions {
                actions: vec![Action::PlaybackFailed("gone".into())],
                last: true
            }
        );
        assert_eq!(
            run(PlayerEvent::Buffered { seconds: 3 }),
            EventActions {
                actions: vec![],
                last: false
            }
        );
    }
}
