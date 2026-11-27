use std::collections::{BTreeMap, BTreeSet};

use super::clock::{Clock, DeviceId, Stamp};
use super::record::{Record, RecordName, Value};
use crate::addon::TransportUrl;
use crate::app::{Effect, LibraryItem, Setting, Settings};

pub const PUSH_AFTER_QUIET_MS: u64 = 60_000;
pub const PUSH_AT_MOST_MS: u64 = 300_000;
pub const PULL_EVERY_MS: u64 = 600_000;
pub const VERIFY_EVERY_MS: u64 = 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub record: Record,
    pub seq: u64,
    pub dirty: bool,
    pub seeded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changes {
    pub clock: Stamp,
    pub since: u64,
    pub born: u64,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outgoing {
    pub record: Record,
    pub base_seq: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Merged {
    Taken,
    Kept,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Replica {
    clock: Clock,
    defaults: Settings,
    entries: BTreeMap<RecordName, Entry>,
    since: u64,
    born: u64,
    dirty_since: Option<u64>,
    last_change: Option<u64>,
    flush: bool,
    last_pull: Option<u64>,
    last_verify: Option<u64>,
    changed: BTreeSet<RecordName>,
    touched: bool,
}

impl Replica {
    pub fn new(device: DeviceId, defaults: Settings) -> Self {
        Self {
            clock: Clock::new(device),
            defaults,
            entries: BTreeMap::new(),
            since: 0,
            born: 0,
            dirty_since: None,
            last_change: None,
            flush: false,
            last_pull: None,
            last_verify: None,
            changed: BTreeSet::new(),
            touched: false,
        }
    }

    pub fn restore(
        defaults: Settings,
        clock: Stamp,
        since: u64,
        born: u64,
        entries: Vec<Entry>,
    ) -> Self {
        let mut replica = Self::new(clock.device, defaults);
        replica.clock = Clock::resume(clock);
        replica.since = since;
        replica.born = born;
        for entry in entries {
            replica.clock.observe(entry.record.stamp);
            replica.entries.insert(entry.record.name.clone(), entry);
        }
        if replica.entries.values().any(|entry| entry.dirty) {
            replica.mark_change(0);
            replica.flush = true;
        }
        replica
    }

    pub fn take_changes(&mut self) -> Option<Changes> {
        if !std::mem::take(&mut self.touched) {
            return None;
        }
        let entries = std::mem::take(&mut self.changed)
            .into_iter()
            .filter_map(|name| self.entries.get(&name).cloned())
            .collect();
        Some(Changes {
            clock: self.clock.latest(),
            since: self.since,
            born: self.born,
            entries,
        })
    }

    fn touch(&mut self, name: &RecordName) {
        self.changed.insert(name.clone());
        self.touched = true;
    }

    pub fn seed(
        &mut self,
        library: &[LibraryItem],
        addons: &[TransportUrl],
        settings: &Settings,
        now_ms: u64,
    ) {
        let device = self.clock.device();
        let seed = |physical_ms| Stamp {
            physical_ms,
            counter: 0,
            device,
        };
        for item in library {
            self.seed_entry(
                RecordName::Item(item.id.clone()),
                seed(item.updated_ms),
                Value::Item(Box::new(item.clone())),
            );
        }
        self.seed_entry(RecordName::Addons, seed(0), Value::Addons(addons.to_vec()));
        for (setting, default) in settings.all().into_iter().zip(self.defaults.all()) {
            if !setting.is_device_local() && setting != default {
                self.seed_entry(
                    RecordName::Setting(setting.key().to_owned()),
                    seed(0),
                    Value::Setting(setting.value()),
                );
            }
        }
        self.mark_change(now_ms);
        self.flush = true;
    }

    fn seed_entry(&mut self, name: RecordName, stamp: Stamp, value: Value) {
        self.clock.observe(stamp);
        self.touch(&name);
        self.entries.insert(
            name.clone(),
            Entry {
                record: Record { name, stamp, value },
                seq: 0,
                dirty: true,
                seeded: true,
            },
        );
    }

    pub fn record_effect(&mut self, effect: &Effect, now_ms: u64) {
        match effect {
            Effect::SaveLibraryItem(item) => self.set(
                RecordName::Item(item.id.clone()),
                Value::Item(Box::new(item.clone())),
                now_ms,
            ),
            Effect::DeleteLibraryItem(id) => {
                self.set(RecordName::Item(id.clone()), Value::Deleted, now_ms);
            }
            Effect::ClearLibrary => self.set(RecordName::LibraryCleared, Value::Cleared, now_ms),
            Effect::SaveAddons(addons) => {
                self.set(RecordName::Addons, Value::Addons(addons.clone()), now_ms);
            }
            Effect::SaveSettings(settings) => {
                let current = self.settings_over(self.defaults);
                for (setting, before) in settings.all().into_iter().zip(current.all()) {
                    if !setting.is_device_local() && setting != before {
                        self.set(
                            RecordName::Setting(setting.key().to_owned()),
                            Value::Setting(setting.value()),
                            now_ms,
                        );
                    }
                }
            }
            _ => {}
        }
    }

    fn set(&mut self, name: RecordName, value: Value, now_ms: u64) {
        if self
            .entries
            .get(&name)
            .is_some_and(|entry| entry.record.value == value)
        {
            return;
        }
        let stamp = self.clock.tick(now_ms);
        let seq = self.entries.get(&name).map_or(0, |entry| entry.seq);
        self.touch(&name);
        self.entries.insert(
            name.clone(),
            Entry {
                record: Record { name, stamp, value },
                seq,
                dirty: true,
                seeded: false,
            },
        );
        self.mark_change(now_ms);
    }

    fn mark_change(&mut self, now_ms: u64) {
        self.dirty_since.get_or_insert(now_ms);
        self.last_change = Some(now_ms);
    }

    pub fn merge(&mut self, remote: Record, seq: u64, now_ms: u64) -> Merged {
        self.clock.observe(remote.stamp);
        self.since = self.since.max(seq);
        self.touch(&remote.name);
        if let Some(local) = self.entries.get_mut(&remote.name)
            && local.record.stamp >= remote.stamp
        {
            local.seq = local.seq.max(seq);
            local.dirty = local.record.stamp > remote.stamp;
            if local.dirty {
                self.mark_change(now_ms);
            } else {
                self.settle();
            }
            return Merged::Kept;
        }
        let joined = match self.entries.get(&remote.name) {
            Some(local) if local.seeded => match (&local.record.value, &remote.value) {
                (Value::Addons(mine), Value::Addons(theirs)) => {
                    let mut union = theirs.clone();
                    union.extend(mine.iter().filter(|a| !theirs.contains(a)).cloned());
                    (union != *theirs).then_some(union)
                }
                _ => None,
            },
            _ => None,
        };
        self.entries.insert(
            remote.name.clone(),
            Entry {
                record: remote,
                seq,
                dirty: false,
                seeded: false,
            },
        );
        if let Some(union) = joined {
            self.set(RecordName::Addons, Value::Addons(union), now_ms);
        }
        self.settle();
        Merged::Taken
    }

    pub fn outgoing(&self) -> Vec<Outgoing> {
        self.entries
            .values()
            .filter(|entry| entry.dirty)
            .map(|entry| Outgoing {
                record: entry.record.clone(),
                base_seq: entry.seq,
            })
            .collect()
    }

    pub fn pushed(&mut self, name: &RecordName, stamp: Stamp, seq: u64) {
        self.touch(name);
        if let Some(entry) = self.entries.get_mut(name) {
            entry.seq = entry.seq.max(seq);
            if entry.record.stamp == stamp {
                entry.dirty = false;
                entry.seeded = false;
            }
        }
        self.settle();
    }

    fn settle(&mut self) {
        if !self.entries.values().any(|entry| entry.dirty) {
            self.dirty_since = None;
            self.last_change = None;
            self.flush = false;
        }
    }

    pub fn observe_space(&mut self, born: u64, space_seq: u64) -> bool {
        let replaced = self.born != 0 && born != self.born;
        let went_back = self.highest_seq() > space_seq;
        if born != self.born {
            self.born = born;
            self.touched = true;
        }
        if replaced || went_back {
            self.start_over();
            return true;
        }
        false
    }

    pub fn conflicted(&mut self, server: &[(RecordName, u64)]) -> bool {
        let missed = server.iter().any(|(name, server_seq)| {
            let known = self.entries.get(name).map_or(0, |entry| entry.seq);
            *server_seq < known || *server_seq != known && *server_seq <= self.since
        });
        if missed {
            self.start_over();
        }
        missed
    }

    fn highest_seq(&self) -> u64 {
        self.entries
            .values()
            .map(|entry| entry.seq)
            .fold(self.since, u64::max)
    }

    pub fn verify_due(&self, now_ms: u64) -> bool {
        self.last_verify
            .is_none_or(|last| now_ms >= last.saturating_add(VERIFY_EVERY_MS))
    }

    pub fn verify(&mut self, now_ms: u64) {
        self.last_verify = Some(now_ms);
        self.start_over();
    }

    fn start_over(&mut self) {
        self.since = 0;
        let names: Vec<RecordName> = self.entries.keys().cloned().collect();
        for name in &names {
            if let Some(entry) = self.entries.get_mut(name) {
                entry.seq = 0;
                entry.dirty = true;
            }
            self.touch(name);
        }
        self.touched = true;
        if !names.is_empty() {
            self.dirty_since.get_or_insert(0);
            self.last_change.get_or_insert(0);
            self.flush = true;
        }
    }

    pub const fn born(&self) -> u64 {
        self.born
    }

    pub fn flush(&mut self) {
        self.flush = true;
    }

    pub fn push_due(&self, now_ms: u64) -> bool {
        let (Some(dirty_since), Some(last_change)) = (self.dirty_since, self.last_change) else {
            return false;
        };
        self.flush
            || now_ms >= last_change.saturating_add(PUSH_AFTER_QUIET_MS)
            || now_ms >= dirty_since.saturating_add(PUSH_AT_MOST_MS)
    }

    pub fn pull_due(&self, now_ms: u64) -> bool {
        self.last_pull
            .is_none_or(|last| now_ms >= last.saturating_add(PULL_EVERY_MS))
    }

    pub fn pulled(&mut self, now_ms: u64) {
        self.last_pull = Some(now_ms);
    }

    pub const fn since(&self) -> u64 {
        self.since
    }

    pub const fn clock(&self) -> Clock {
        self.clock
    }

    pub fn get(&self, name: &RecordName) -> Option<&Record> {
        self.entries.get(name).map(|entry| &entry.record)
    }

    pub fn library(&self) -> Vec<LibraryItem> {
        let cleared = self.get(&RecordName::LibraryCleared).map(|r| r.stamp);
        self.entries
            .values()
            .filter(|entry| cleared.is_none_or(|stamp| entry.record.stamp > stamp))
            .filter_map(|entry| match &entry.record.value {
                Value::Item(item) => Some(LibraryItem::clone(item)),
                _ => None,
            })
            .collect()
    }

    pub fn addons(&self) -> Option<&[TransportUrl]> {
        match &self.get(&RecordName::Addons)?.value {
            Value::Addons(addons) => Some(addons),
            _ => None,
        }
    }

    pub fn settings_over(&self, mut local: Settings) -> Settings {
        for entry in self.entries.values() {
            if let (RecordName::Setting(key), Value::Setting(value)) =
                (&entry.record.name, &entry.record.value)
                && let Ok(setting) = Setting::parse(key, value)
                && !setting.is_device_local()
            {
                local.set(setting);
            }
        }
        local
    }
}
