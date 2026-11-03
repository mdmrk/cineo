use std::ffi::c_void;
use std::sync::mpsc::{self, Receiver};

use eframe::egui;
use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, PlatformConfig};
use tracing::warn;

use crate::media_keys::Remote;

pub(crate) struct Smtc {
    controls: Option<MediaControls>,
    remote: Receiver<Remote>,
    last: Option<(String, bool)>,
}

impl Smtc {
    pub(crate) fn new(ctx: egui::Context, hwnd: Option<isize>) -> Self {
        let (tx, remote) = mpsc::channel();
        let controls = hwnd.and_then(|hwnd| {
            let config = PlatformConfig {
                display_name: "Cineo",
                dbus_name: "cineo",
                hwnd: Some(hwnd as *mut c_void),
            };
            let attached = MediaControls::new(config).and_then(|mut controls| {
                controls.attach(move |event| {
                    if let Some(remote) = remote_of(&event) {
                        let _ = tx.send(remote);
                        ctx.request_repaint();
                    }
                })?;
                controls.set_playback(MediaPlayback::Stopped)?;
                Ok(controls)
            });
            attached
                .inspect_err(
                    |err| warn!(%err, "media keys and the system media controls may not work"),
                )
                .ok()
        });
        Self {
            controls,
            remote,
            last: None,
        }
    }

    pub(crate) fn set(&mut self, playing: Option<(&str, bool)>) {
        let same = match (&self.last, playing) {
            (None, None) => true,
            (Some((last_title, last_paused)), Some((title, paused))) => {
                last_title == title && *last_paused == paused
            }
            _ => false,
        };
        if same {
            return;
        }
        let title_changed = self.last.as_ref().map(|(t, _)| t.as_str()) != playing.map(|(t, _)| t);
        self.last = playing.map(|(title, paused)| (title.to_owned(), paused));
        let Some(controls) = &mut self.controls else {
            return;
        };
        let result = match playing {
            None => controls.set_playback(MediaPlayback::Stopped),
            Some((title, paused)) => {
                let metadata = if title_changed {
                    controls.set_metadata(MediaMetadata {
                        title: Some(title),
                        ..MediaMetadata::default()
                    })
                } else {
                    Ok(())
                };
                metadata.and_then(|()| controls.set_playback(playback(paused)))
            }
        };
        if let Err(err) = result {
            warn!(%err, "could not update the system media controls");
        }
    }

    pub(crate) fn take(&self) -> Vec<Remote> {
        self.remote.try_iter().collect()
    }
}

fn playback(paused: bool) -> MediaPlayback {
    if paused {
        MediaPlayback::Paused { progress: None }
    } else {
        MediaPlayback::Playing { progress: None }
    }
}

fn remote_of(event: &MediaControlEvent) -> Option<Remote> {
    match event {
        MediaControlEvent::Play => Some(Remote::Play),
        MediaControlEvent::Pause => Some(Remote::Pause),
        MediaControlEvent::Toggle => Some(Remote::PlayPause),
        MediaControlEvent::Stop => Some(Remote::Stop),
        MediaControlEvent::Raise => Some(Remote::Raise),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_buttons_map_to_the_same_remotes_as_mpris() {
        assert_eq!(
            remote_of(&MediaControlEvent::Toggle),
            Some(Remote::PlayPause)
        );
        assert_eq!(remote_of(&MediaControlEvent::Play), Some(Remote::Play));
        assert_eq!(remote_of(&MediaControlEvent::Pause), Some(Remote::Pause));
        assert_eq!(remote_of(&MediaControlEvent::Stop), Some(Remote::Stop));
        assert_eq!(remote_of(&MediaControlEvent::Raise), Some(Remote::Raise));
        assert_eq!(remote_of(&MediaControlEvent::Next), None);
    }
}
