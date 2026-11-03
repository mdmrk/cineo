use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;

use cineo_core::app::{Action, Effect, Notice, State, update};
use cineo_net::{AddonClient, MAX_IMAGE_BYTES, NetPolicy, fetch_task};
use cineo_store::Store;
use tokio::runtime::{Handle, Runtime};
use tracing::{error, warn};
use url::Url;

use crate::view::{AppView, view};

#[uniffi::export(foreign)]
pub trait StateListener: Send + Sync + std::fmt::Debug {
    fn changed(&self, view: AppView);
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum StartError {
    #[error("cannot start the async runtime: {reason}")]
    Runtime { reason: String },
    #[error("cannot open the database: {reason}")]
    Store { reason: String },
    #[error("cannot create the HTTP client: {reason}")]
    Http { reason: String },
    #[error("cannot start a thread: {reason}")]
    Thread { reason: String },
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum ImageError {
    #[error("invalid image URL: {reason}")]
    InvalidUrl { reason: String },
    #[error("image request failed: {reason}")]
    Fetch { reason: String },
}

#[derive(Debug)]
enum Msg {
    Action(Action),
    Notice(Notice),
    Shutdown,
}

#[derive(Debug, uniffi::Object)]
pub struct Cineo {
    tx: Sender<Msg>,
    client: AddonClient,
    shell: Option<JoinHandle<()>>,
    runtime: Option<Runtime>,
}

#[uniffi::export]
impl Cineo {
    #[uniffi::constructor]
    pub fn new(data_dir: &str, listener: Arc<dyn StateListener>) -> Result<Arc<Self>, StartError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|err| StartError::Runtime {
                reason: err.to_string(),
            })?;
        let store_error = |err: cineo_store::StoreError| StartError::Store {
            reason: err.to_string(),
        };
        let store = Store::open_in(Path::new(data_dir)).map_err(store_error)?;
        let restore = Action::Restore {
            addons: store.addons().map_err(store_error)?,
            library: store.library().map_err(store_error)?,
        };
        let settings = store.settings().map_err(store_error)?;
        let client = AddonClient::new(NetPolicy {
            allow_private_networks: settings.allow_private_network,
            ..NetPolicy::default()
        })
        .map_err(|err| StartError::Http {
            reason: err.to_string(),
        })?;
        let thread_error = |err: std::io::Error| StartError::Thread {
            reason: err.to_string(),
        };
        let (tx, rx) = channel();
        let notices = tx.clone();
        let (store_tx, writer) = cineo_store::spawn_writer(store, move |err| {
            let _ = notices.send(Msg::Notice(Notice::SaveFailed(err.to_string())));
        })
        .map_err(thread_error)?;
        let shell = Shell {
            state: State::default(),
            client: client.clone(),
            runtime: runtime.handle().clone(),
            tx: tx.clone(),
            store_tx,
            listener,
            shown: None,
        };
        let restore = [restore, Action::RestoreSettings(settings)];
        let shell = std::thread::Builder::new()
            .name("shell".into())
            .spawn(move || {
                shell.run(&rx, restore);
                if writer.join().is_err() {
                    error!("the store thread panicked");
                }
            })
            .map_err(thread_error)?;
        Ok(Arc::new(Self {
            tx,
            client,
            shell: Some(shell),
            runtime: Some(runtime),
        }))
    }

    pub fn install_addon(&self, url: String) {
        self.send(Action::InstallAddon(url));
    }

    pub fn dismiss_notice(&self) {
        self.send(Action::DismissNotice);
    }

    pub async fn fetch_image(&self, url: String) -> Result<Vec<u8>, ImageError> {
        let url = Url::parse(&url).map_err(|err| ImageError::InvalidUrl {
            reason: err.to_string(),
        })?;
        let runtime = self.runtime.as_ref().ok_or_else(|| ImageError::Fetch {
            reason: "the runtime has stopped".into(),
        })?;
        let client = self.client.clone();
        let task = runtime
            .handle()
            .spawn(async move { client.fetch_image(&url, MAX_IMAGE_BYTES).await });
        match task.await {
            Ok(result) => result.map_err(|err| ImageError::Fetch {
                reason: err.to_string(),
            }),
            Err(err) => Err(ImageError::Fetch {
                reason: err.to_string(),
            }),
        }
    }
}

impl Cineo {
    fn send(&self, action: Action) {
        if self.tx.send(Msg::Action(action)).is_err() {
            warn!("the shell thread has stopped");
        }
    }
}

impl Drop for Cineo {
    fn drop(&mut self) {
        let _ = self.tx.send(Msg::Shutdown);
        if let Some(shell) = self.shell.take()
            && shell.join().is_err()
        {
            error!("the shell thread panicked");
        }
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

#[derive(Debug)]
struct Shell {
    state: State,
    client: AddonClient,
    runtime: Handle,
    tx: Sender<Msg>,
    store_tx: Sender<Effect>,
    listener: Arc<dyn StateListener>,
    shown: Option<AppView>,
}

impl Shell {
    fn run(mut self, rx: &Receiver<Msg>, restore: [Action; 2]) {
        for action in restore {
            self.dispatch(action);
        }
        self.publish();
        while let Ok(first) = rx.recv() {
            let mut running = self.handle(first);
            while running && let Ok(msg) = rx.try_recv() {
                running = self.handle(msg);
            }
            if !running {
                return;
            }
            self.publish();
        }
    }

    fn handle(&mut self, msg: Msg) -> bool {
        match msg {
            Msg::Action(action) => self.dispatch(action),
            Msg::Notice(notice) => self.state.notice = Some(notice),
            Msg::Shutdown => return false,
        }
        true
    }

    fn dispatch(&mut self, action: Action) {
        for effect in update(&mut self.state, action) {
            self.run_effect(effect);
        }
    }

    fn run_effect(&mut self, effect: Effect) {
        match effect {
            effect @ (Effect::FetchManifest { .. }
            | Effect::FetchCatalog { .. }
            | Effect::FetchAddonList { .. }
            | Effect::FetchMeta { .. }
            | Effect::FetchStreams { .. }
            | Effect::FetchSubtitles { .. }
            | Effect::CheckForUpdate(_)) => {
                if let Some(task) = fetch_task(&self.client, effect) {
                    let tx = self.tx.clone();
                    self.runtime.spawn(async move {
                        let _ = tx.send(Msg::Action(task.await));
                    });
                }
            }
            effect @ (Effect::SaveAddons(_)
            | Effect::SaveLibraryItem(_)
            | Effect::DeleteLibraryItem(_)
            | Effect::ClearLibrary
            | Effect::SaveSettings(_)) => {
                if self.store_tx.send(effect).is_err() {
                    self.state.notice = Some(Notice::StoreClosed);
                }
            }
            Effect::Play(_) | Effect::StartTorrent(_) | Effect::StopTorrent => {
                warn!(?effect, "playback is not available on Android yet");
            }
        }
    }

    fn publish(&mut self) {
        let next = view(&self.state);
        if self.shown.as_ref() != Some(&next) {
            self.listener.changed(next.clone());
            self.shown = Some(next);
        }
    }
}
