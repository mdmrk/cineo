use std::collections::BTreeMap;

use super::library::LibraryItem;
use super::state::{Effect, ManifestOrigin, State};
use crate::addon::TransportUrl;
use crate::sync::{
    DeviceId, Merged, Outgoing, Pairing, Record, RecordName, Replica, Stamp, SyncKey,
};

pub const PUSH_ATTEMPTS: u32 = 3;
pub const RETRY_AFTER_FAILURE_MS: u64 = 60_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncFailure {
    Conflict(Vec<(RecordName, u64)>),
    SpaceGone,
    Unavailable { retry_after_s: Option<u64> },
    Rejected(u16),
    Insecure,
    Malformed,
    Network(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncProblem {
    SpaceGone,
    Busy,
    Failed(SyncFailure),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PulledPage {
    pub records: Vec<(Record, u64)>,
    pub more: bool,
    pub space_seq: u64,
    pub born: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PushedBatch {
    pub pushed: Vec<(RecordName, Stamp, u64)>,
    pub oversized: Vec<(RecordName, Stamp)>,
    pub failure: Option<SyncFailure>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Round {
    Idle,
    Pulling { attempt: u32, started_over: bool },
    Pushing { attempt: u32 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct SyncState {
    pub pairing: Pairing,
    pub replica: Replica,
    pub round: Round,
    pub last_ok_ms: Option<u64>,
    pub problem: Option<SyncProblem>,
    pub not_before_ms: u64,
    pub retire: Option<Pairing>,
}

impl SyncState {
    fn new(pairing: Pairing, replica: Replica) -> Self {
        Self {
            pairing,
            replica,
            round: Round::Idle,
            last_ok_ms: None,
            problem: None,
            not_before_ms: 0,
            retire: None,
        }
    }

    pub fn is_running(&self) -> bool {
        self.round != Round::Idle
    }

    pub fn can_sync_now(&self) -> bool {
        !self.is_running() && !self.paused()
    }

    fn paused(&self) -> bool {
        self.problem == Some(SyncProblem::SpaceGone)
    }
}

pub(super) fn restore(state: &mut State, pairing: Pairing, replica: Replica) -> Vec<Effect> {
    state.sync = Some(SyncState::new(pairing, replica));
    Vec::new()
}

pub(super) fn enable(state: &mut State, pairing: Pairing, device: DeviceId) -> Vec<Effect> {
    let mut replica = Replica::new(device, state.default_settings);
    replica.seed(
        &state.library,
        &state.installed,
        &state.settings,
        state.now_ms,
    );
    state.sync = Some(SyncState::new(pairing.clone(), replica));
    vec![Effect::SavePairing { pairing, device }]
}

pub(super) fn rotate(state: &mut State, key: SyncKey) -> Vec<Effect> {
    let Some(sync) = state.sync.as_mut() else {
        return Vec::new();
    };
    let old = sync.pairing.clone();
    sync.pairing.key = key;
    sync.replica.move_to_new_space();
    sync.retire = Some(old);
    sync.problem = None;
    sync.round = Round::Idle;
    sync.not_before_ms = 0;
    vec![Effect::SavePairing {
        pairing: sync.pairing.clone(),
        device: sync.replica.clock().device(),
    }]
}

pub(super) fn leave(state: &mut State, delete_server_data: bool) -> Vec<Effect> {
    let Some(sync) = state.sync.take() else {
        return Vec::new();
    };
    let mut effects = vec![Effect::ForgetSync];
    if delete_server_data {
        effects.push(Effect::DeleteSyncSpace(sync.pairing));
    }
    effects
}

pub(super) fn tick(state: &mut State, now_ms: u64) -> Vec<Effect> {
    state.now_ms = state.now_ms.max(now_ms);
    Vec::new()
}

pub(super) fn sync_now(state: &mut State) -> Vec<Effect> {
    let Some(sync) = state.sync.as_mut().filter(|s| !s.paused()) else {
        return Vec::new();
    };
    sync.not_before_ms = 0;
    sync.replica.flush();
    start_round(sync, state.now_ms, true).into_iter().collect()
}

fn start_round(sync: &mut SyncState, now_ms: u64, forced: bool) -> Option<Effect> {
    if sync.is_running() || sync.paused() || now_ms < sync.not_before_ms {
        return None;
    }
    let verify = sync.replica.verify_due(now_ms);
    if !(forced || verify || sync.replica.pull_due(now_ms) || sync.replica.push_due(now_ms)) {
        return None;
    }
    if verify {
        sync.replica.verify(now_ms);
    }
    sync.round = Round::Pulling {
        attempt: 1,
        started_over: false,
    };
    Some(pull(sync))
}

fn pull(sync: &SyncState) -> Effect {
    Effect::SyncPull {
        pairing: sync.pairing.clone(),
        since: sync.replica.since(),
    }
}

fn push(sync: &SyncState, outgoing: Vec<Outgoing>) -> Effect {
    Effect::SyncPush {
        pairing: sync.pairing.clone(),
        outgoing,
    }
}

fn fail(sync: &mut SyncState, failure: SyncFailure, now_ms: u64) {
    sync.round = Round::Idle;
    match failure {
        SyncFailure::SpaceGone => sync.problem = Some(SyncProblem::SpaceGone),
        SyncFailure::Unavailable { retry_after_s } => {
            sync.problem = Some(SyncProblem::Busy);
            sync.not_before_ms = now_ms.saturating_add(
                retry_after_s.map_or(RETRY_AFTER_FAILURE_MS, |s| s.saturating_mul(1000)),
            );
        }
        failure => {
            sync.problem = Some(SyncProblem::Failed(failure));
            sync.not_before_ms = now_ms.saturating_add(RETRY_AFTER_FAILURE_MS);
        }
    }
}

fn succeed(sync: &mut SyncState, now_ms: u64) -> Vec<Effect> {
    sync.round = Round::Idle;
    sync.problem = None;
    sync.last_ok_ms = Some(now_ms);
    sync.replica.pulled(now_ms);
    sync.retire
        .take()
        .map(Effect::DeleteSyncSpace)
        .into_iter()
        .collect()
}

pub(super) fn pulled(
    state: &mut State,
    pairing: &Pairing,
    result: Result<PulledPage, SyncFailure>,
) -> Vec<Effect> {
    let now_ms = state.now_ms;
    let Some(sync) = state.sync.as_mut().filter(|s| s.pairing == *pairing) else {
        return Vec::new();
    };
    let Round::Pulling {
        attempt,
        started_over,
    } = sync.round
    else {
        return Vec::new();
    };
    let page = match result {
        Ok(page) => page,
        Err(failure) => {
            fail(sync, failure, now_ms);
            return Vec::new();
        }
    };
    if sync.replica.observe_space(page.born, page.space_seq) {
        if started_over {
            fail(sync, SyncFailure::Malformed, now_ms);
            return Vec::new();
        }
        sync.round = Round::Pulling {
            attempt,
            started_over: true,
        };
        return vec![pull(sync)];
    }
    let mut taken = false;
    for (record, seq) in page.records {
        taken |= sync.replica.merge(record, seq, now_ms) == Merged::Taken;
    }
    let mut effects = if taken {
        apply_remote(state)
    } else {
        Vec::new()
    };
    let Some(sync) = state.sync.as_mut() else {
        return effects;
    };
    if page.more {
        effects.push(pull(sync));
        return effects;
    }
    let outgoing = sync.replica.outgoing();
    if outgoing.is_empty() {
        effects.extend(succeed(sync, now_ms));
    } else {
        sync.round = Round::Pushing { attempt };
        effects.push(push(sync, outgoing));
    }
    effects
}

pub(super) fn pushed(state: &mut State, pairing: &Pairing, batch: PushedBatch) -> Vec<Effect> {
    let now_ms = state.now_ms;
    let Some(sync) = state.sync.as_mut().filter(|s| s.pairing == *pairing) else {
        return Vec::new();
    };
    let Round::Pushing { attempt } = sync.round else {
        return Vec::new();
    };
    for (name, stamp, seq) in &batch.pushed {
        sync.replica.pushed(name, *stamp, *seq);
    }
    for (name, stamp) in &batch.oversized {
        let seq = sync.replica.seq(name);
        sync.replica.pushed(name, *stamp, seq);
    }
    match batch.failure {
        None => succeed(sync, now_ms),
        Some(SyncFailure::Conflict(server)) if attempt < PUSH_ATTEMPTS => {
            let started_over = sync.replica.conflicted(&server);
            sync.round = Round::Pulling {
                attempt: attempt + 1,
                started_over,
            };
            vec![pull(sync)]
        }
        Some(failure) => {
            fail(sync, failure, now_ms);
            Vec::new()
        }
    }
}

fn apply_remote(state: &mut State) -> Vec<Effect> {
    let Some(sync) = state.sync.as_ref() else {
        return Vec::new();
    };
    let library = sync.replica.library();
    let settings = sync.replica.settings_over(state.settings);
    let addons = sync.replica.addons().map(<[TransportUrl]>::to_vec);
    let mut effects = apply_library(state, library);
    if settings != state.settings {
        state.settings = settings;
        effects.push(Effect::SaveSettings(settings));
    }
    if let Some(addons) = addons.filter(|a| *a != state.installed) {
        effects.extend(apply_addons(state, addons));
    }
    effects
}

fn apply_library(state: &mut State, library: Vec<LibraryItem>) -> Vec<Effect> {
    let mut remote: BTreeMap<String, LibraryItem> = library
        .into_iter()
        .map(|item| (item.id.clone(), item))
        .collect();
    let mut effects = Vec::new();
    state
        .library
        .retain_mut(|item| match remote.remove(&item.id) {
            None => {
                effects.push(Effect::DeleteLibraryItem(item.id.clone()));
                false
            }
            Some(new) => {
                if *item != new {
                    *item = new.clone();
                    effects.push(Effect::SaveLibraryItem(new));
                }
                true
            }
        });
    for item in remote.into_values() {
        state.library.push(item.clone());
        effects.push(Effect::SaveLibraryItem(item));
    }
    effects
}

fn apply_addons(state: &mut State, addons: Vec<TransportUrl>) -> Vec<Effect> {
    let mut effects = Vec::new();
    for transport in &addons {
        if !state.installed.contains(transport) {
            state.addons_loading.push(transport.clone());
            effects.push(Effect::FetchManifest {
                transport: transport.clone(),
                origin: ManifestOrigin::Restore,
            });
        }
    }
    state.addons.retain(|a| addons.contains(&a.transport));
    state.addons_loading.retain(|t| addons.contains(t));
    state
        .addons
        .sort_by_key(|a| addons.iter().position(|t| *t == a.transport));
    state.installed = addons;
    effects.extend(super::refresh::addons_changed(state, true));
    effects
}

pub(super) fn after(state: &mut State, effects: &[Effect], flush: bool) -> Vec<Effect> {
    let now_ms = state.now_ms;
    let announced = state.remote.announced();
    let Some(sync) = state.sync.as_mut() else {
        state.remote.synced.clear();
        return Vec::new();
    };
    for effect in effects {
        sync.replica.record_effect(effect, now_ms);
    }
    sync.replica.announce(announced, now_ms);
    let computers = sync.replica.computers();
    if computers != state.remote.synced {
        state.remote.synced = computers;
    }
    if flush {
        sync.replica.flush();
    }
    let round = start_round(sync, now_ms, false);
    let changes = sync.replica.take_changes().map(Effect::SaveSync);
    round.into_iter().chain(changes).collect()
}
