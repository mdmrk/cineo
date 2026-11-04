use std::sync::mpsc::{Sender, channel};
use std::thread::JoinHandle;

use cineo_core::app::Effect;
use tracing::error;

use crate::{Store, StoreError};

pub fn spawn_writer(
    mut store: Store,
    on_error: impl Fn(StoreError) + Send + 'static,
) -> std::io::Result<(Sender<Effect>, JoinHandle<()>)> {
    let (tx, rx) = channel::<Effect>();
    let thread = std::thread::Builder::new()
        .name("store".into())
        .spawn(move || {
            while let Ok(first) = rx.recv() {
                for effect in coalesce(std::iter::once(first).chain(rx.try_iter()).collect()) {
                    if let Err(err) = store.apply(&effect) {
                        error!(error = %err, "saving failed");
                        on_error(err);
                    }
                }
            }
        })?;
    Ok((tx, thread))
}

fn coalesce(batch: Vec<Effect>) -> impl Iterator<Item = Effect> {
    let keep: Vec<bool> = (0..batch.len())
        .map(|i| {
            !batch[i + 1..]
                .iter()
                .any(|later| supersedes(later, &batch[i]))
        })
        .collect();
    batch
        .into_iter()
        .zip(keep)
        .filter_map(|(effect, keep)| keep.then_some(effect))
}

fn supersedes(later: &Effect, earlier: &Effect) -> bool {
    match (later, earlier) {
        (Effect::SaveSettings(_), Effect::SaveSettings(_))
        | (Effect::SaveAddons(_), Effect::SaveAddons(_)) => true,
        (Effect::SaveLibraryItem(later), Effect::SaveLibraryItem(earlier)) => {
            later.id == earlier.id
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use cineo_core::addon::ContentType;
    use cineo_core::app::{LibraryItem, Percent, Settings};

    use super::*;

    #[test]
    fn queued_saves_keep_only_the_last_write_of_each_key() {
        let item = |id: &str, time_offset_ms| {
            Effect::SaveLibraryItem(LibraryItem {
                id: id.into(),
                content_type: ContentType::new("movie").unwrap(),
                name: id.into(),
                poster: None,
                video_id: id.into(),
                time_offset_ms,
                duration_ms: 0,
                updated_ms: 1,
                favorited: None,
                stream: None,
            })
        };
        let quiet = Settings {
            volume: Percent::new(10),
            ..Settings::default()
        };
        let batch = vec![
            Effect::SaveSettings(Settings::default()),
            item("a", 1),
            item("b", 1),
            Effect::DeleteLibraryItem("a".into()),
            item("a", 2),
            Effect::SaveSettings(quiet),
            Effect::ClearLibrary,
        ];
        let kept: Vec<Effect> = coalesce(batch).collect();
        assert_eq!(
            kept,
            [
                item("b", 1),
                Effect::DeleteLibraryItem("a".into()),
                item("a", 2),
                Effect::SaveSettings(quiet),
                Effect::ClearLibrary,
            ]
        );
    }
}
