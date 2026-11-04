use std::time::{SystemTime, UNIX_EPOCH};

use cineo_core::app::Action;
use tokio::sync::mpsc::UnboundedReceiver;

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

pub async fn forward_events(
    mut events: UnboundedReceiver<PlayerEvent>,
    meta_id: String,
    video_id: String,
    observe: impl Fn(&PlayerEvent),
    send: impl Fn(Action),
) {
    while let Some(event) = events.recv().await {
        observe(&event);
        let EventActions { actions, last } = event_actions(event, &meta_id, &video_id, now_ms());
        for action in actions {
            send(action);
        }
        if last {
            break;
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

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

    #[tokio::test]
    async fn forwarding_stops_after_the_last_event() {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        for event in [
            PlayerEvent::Buffered { seconds: 4 },
            PlayerEvent::Closed {
                time_ms: 0,
                duration_ms: 9_000,
            },
            PlayerEvent::Buffered { seconds: 5 },
        ] {
            tx.send(event).unwrap();
        }
        let seen = Mutex::new(Vec::new());
        let sent = Mutex::new(Vec::new());
        forward_events(
            rx,
            "tt1".into(),
            "tt1:1:2".into(),
            |event| seen.lock().unwrap().push(event.clone()),
            |action| sent.lock().unwrap().push(action),
        )
        .await;
        assert_eq!(seen.into_inner().unwrap().len(), 2);
        assert_eq!(sent.into_inner().unwrap(), [Action::PlaybackStopped]);
    }
}
