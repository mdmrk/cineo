#![allow(clippy::unwrap_used, reason = "test helpers panic on purpose")]

use std::collections::BTreeSet;

use cineo_core::addon::{ContentType, TransportUrl};
use cineo_core::app::{
    DEVICE_LOCAL_SETTINGS, Effect, LibraryItem, Percent, SeekStep, Setting, Settings,
};
use cineo_core::sync::{
    Clock, DeviceId, Merged, PUSH_AFTER_QUIET_MS, PUSH_AT_MOST_MS, Record, RecordName, Replica,
    Stamp, VERIFY_EVERY_MS, Value,
};

const A: DeviceId = DeviceId(1);
const B: DeviceId = DeviceId(2);
const C: DeviceId = DeviceId(3);

fn stamp(physical_ms: u64, counter: u32, device: DeviceId) -> Stamp {
    Stamp {
        physical_ms,
        counter,
        device,
    }
}

fn item(id: &str, offset_ms: u64) -> LibraryItem {
    LibraryItem {
        id: id.to_owned(),
        content_type: ContentType::new("movie").unwrap(),
        name: format!("Name of {id}"),
        poster: Some("https://img.example/p.jpg".parse().unwrap()),
        video_id: id.to_owned(),
        time_offset_ms: offset_ms,
        duration_ms: 6_000_000,
        updated_ms: 1_000,
        favorited: None,
        stream: None,
    }
}

fn addon(host: &str) -> TransportUrl {
    TransportUrl::parse(&format!("https://{host}/manifest.json")).unwrap()
}

fn record(name: RecordName, stamp: Stamp, value: Value) -> Record {
    Record { name, stamp, value }
}

fn item_record(id: &str, offset_ms: u64, stamp: Stamp) -> Record {
    record(
        RecordName::Item(id.to_owned()),
        stamp,
        Value::Item(Box::new(item(id, offset_ms))),
    )
}

fn replica(device: DeviceId) -> Replica {
    Replica::new(device, Settings::default())
}

fn ids(replica: &Replica) -> Vec<(String, u64)> {
    replica
        .library()
        .into_iter()
        .map(|i| (i.id, i.time_offset_ms))
        .collect()
}

fn exchange(from: &mut Replica, to: &mut Replica, seq: &mut u64, now_ms: u64) {
    for out in from.outgoing() {
        *seq += 1;
        from.pushed(&out.record.name, out.record.stamp, *seq);
        to.merge(out.record, *seq, now_ms);
    }
}

#[test]
fn stamps_order_by_time_then_counter_then_device() {
    assert!(stamp(2, 0, A) > stamp(1, 9, C));
    assert!(stamp(2, 1, A) > stamp(2, 0, C));
    assert!(stamp(2, 1, B) > stamp(2, 1, A));
}

#[test]
fn the_clock_never_goes_back_when_the_wall_clock_does() {
    let mut clock = Clock::new(A);
    let first = clock.tick(10_000);
    let second = clock.tick(4_000);
    let third = clock.tick(4_000);
    assert!(
        second > first && third > second,
        "{first:?} {second:?} {third:?}"
    );
    assert_eq!(clock.tick(20_000), stamp(20_000, 0, A));
}

#[test]
fn an_edit_after_seeing_a_remote_change_wins_with_a_slow_clock() {
    let fast_now = 9_000_000;
    let slow_now = 1_000;
    let mut fast = replica(A);
    let mut slow = replica(B);
    let mut seq = 0;
    fast.record_effect(&Effect::SaveLibraryItem(item("tt1", 100)), fast_now);
    exchange(&mut fast, &mut slow, &mut seq, slow_now);
    slow.record_effect(&Effect::SaveLibraryItem(item("tt1", 200)), slow_now);
    exchange(&mut slow, &mut fast, &mut seq, fast_now);
    assert_eq!(ids(&fast), [("tt1".to_owned(), 200)]);
    assert_eq!(ids(&slow), [("tt1".to_owned(), 200)]);
}

#[test]
fn a_newer_remote_item_replaces_the_local_one() {
    let mut local = replica(A);
    local.record_effect(&Effect::SaveLibraryItem(item("tt1", 100)), 1_000);
    let merged = local.merge(item_record("tt1", 300, stamp(2_000, 0, B)), 7, 1_500);
    assert_eq!(merged, Merged::Taken);
    assert_eq!(ids(&local), [("tt1".to_owned(), 300)]);
    assert!(
        local.outgoing().is_empty(),
        "the older local edit is dropped"
    );
}

#[test]
fn an_older_remote_item_is_ignored_and_the_local_one_stays_dirty() {
    let mut local = replica(A);
    local.record_effect(&Effect::SaveLibraryItem(item("tt1", 100)), 5_000);
    let merged = local.merge(item_record("tt1", 300, stamp(2_000, 0, B)), 7, 5_000);
    assert_eq!(merged, Merged::Kept);
    assert_eq!(ids(&local), [("tt1".to_owned(), 100)]);
    let outgoing = local.outgoing();
    assert_eq!(outgoing.len(), 1);
    assert_eq!(
        outgoing[0].base_seq, 7,
        "the push overwrites the server's version"
    );
}

#[test]
fn a_tombstone_beats_an_older_edit_and_loses_to_a_newer_one() {
    let mut local = replica(A);
    local.record_effect(&Effect::SaveLibraryItem(item("tt1", 100)), 1_000);
    local.merge(
        record(
            RecordName::Item("tt1".into()),
            stamp(2_000, 0, B),
            Value::Deleted,
        ),
        1,
        1_500,
    );
    assert!(local.library().is_empty());
    local.record_effect(&Effect::SaveLibraryItem(item("tt1", 50)), 1_600);
    assert_eq!(ids(&local), [("tt1".to_owned(), 50)]);
}

#[test]
fn clearing_hides_older_items_but_not_newer_ones() {
    let mut local = replica(A);
    local.merge(item_record("old", 1, stamp(1_000, 0, B)), 1, 0);
    local.merge(item_record("new", 2, stamp(3_000, 0, B)), 2, 0);
    local.merge(
        record(
            RecordName::LibraryCleared,
            stamp(2_000, 0, C),
            Value::Cleared,
        ),
        3,
        0,
    );
    assert_eq!(ids(&local), [("new".to_owned(), 2)]);
}

fn permutations<T: Clone>(items: &[T]) -> Vec<Vec<T>> {
    if items.len() <= 1 {
        return vec![items.to_vec()];
    }
    let mut all = Vec::new();
    for i in 0..items.len() {
        let mut rest = items.to_vec();
        let first = rest.remove(i);
        for mut tail in permutations(&rest) {
            tail.insert(0, first.clone());
            all.push(tail);
        }
    }
    all
}

#[test]
fn merging_in_any_order_gives_the_same_state() {
    let setting = |value: &str, stamp| {
        record(
            RecordName::Setting("seek_step".into()),
            stamp,
            Value::Setting(value.into()),
        )
    };
    let records = [
        item_record("tt1", 100, stamp(5_000, 0, A)),
        item_record("tt1", 200, stamp(5_000, 0, B)),
        record(
            RecordName::Item("tt1".into()),
            stamp(4_000, 3, C),
            Value::Deleted,
        ),
        item_record("tt2", 300, stamp(900_000_000, 0, C)),
        record(
            RecordName::LibraryCleared,
            stamp(4_500, 0, A),
            Value::Cleared,
        ),
        setting("30", stamp(1, 0, A)),
        setting("5", stamp(1, 0, C)),
    ];
    let state = |order: &[Record]| {
        let mut replica = replica(DeviceId(9));
        for (seq, r) in (1..).zip(order) {
            replica.merge(r.clone(), seq, 0);
        }
        let first = (ids(&replica), replica.settings_over(Settings::default()));
        for r in order {
            replica.merge(r.clone(), 99, 0);
        }
        assert_eq!(
            (ids(&replica), replica.settings_over(Settings::default())),
            first,
            "merging again changes nothing"
        );
        first
    };
    let expected = state(&records);
    assert_eq!(
        expected.0,
        [("tt1".to_owned(), 200), ("tt2".to_owned(), 300)]
    );
    assert_eq!(expected.1.seek_step, SeekStep::S5);
    for order in permutations(&records) {
        assert_eq!(state(&order), expected, "{order:?}");
    }
}

#[test]
fn two_devices_converge_after_exchanging_changes() {
    let mut a = replica(A);
    let mut b = replica(B);
    let mut seq = 0;
    a.record_effect(&Effect::SaveLibraryItem(item("tt1", 100)), 50_000);
    a.record_effect(&Effect::SaveLibraryItem(item("tt2", 100)), 50_001);
    b.record_effect(&Effect::SaveLibraryItem(item("tt1", 900)), 10_000);
    b.record_effect(&Effect::SaveLibraryItem(item("tt2", 5)), 10_000);
    b.record_effect(&Effect::DeleteLibraryItem("tt2".into()), 60_000);
    let mut settings = Settings::default();
    settings.set(Setting::SeekStep(SeekStep::S30));
    b.record_effect(&Effect::SaveSettings(settings), 10_000);
    exchange(&mut a, &mut b, &mut seq, 70_000);
    exchange(&mut b, &mut a, &mut seq, 70_000);
    exchange(&mut a, &mut b, &mut seq, 70_000);
    assert_eq!(ids(&a), ids(&b));
    assert_eq!(ids(&a), [("tt1".to_owned(), 100)]);
    assert_eq!(
        a.settings_over(Settings::default()).seek_step,
        SeekStep::S30
    );
    assert!(a.outgoing().is_empty() && b.outgoing().is_empty());
}

#[test]
fn device_local_settings_are_never_pushed_or_applied() {
    let mut settings = Settings::default();
    settings.set(Setting::AllowPrivateNetwork(true));
    settings.set(Setting::Volume(Percent::new(10)));
    let mut local = replica(A);
    local.seed(&[], &[], &settings, 0);
    local.record_effect(&Effect::SaveSettings(settings), 1_000);
    assert!(
        local
            .outgoing()
            .iter()
            .all(|o| !matches!(o.record.name, RecordName::Setting(_))),
        "{:?}",
        local.outgoing()
    );
    local.merge(
        record(
            RecordName::Setting("allow_private_network".into()),
            stamp(5_000, 0, B),
            Value::Setting("true".into()),
        ),
        1,
        5_000,
    );
    assert!(
        !local
            .settings_over(Settings::default())
            .allow_private_network
    );
}

#[test]
fn every_setting_is_either_synced_or_device_local() {
    const SYNCED: &[&str] = &[
        "subtitle_language",
        "secondary_subtitle_language",
        "audio_language",
        "secondary_audio_language",
        "seek_step",
        "short_seek_step",
        "hide_controls",
        "subtitle_font",
        "subtitle_bold",
        "subtitle_color",
        "subtitle_outline",
        "subtitle_background",
        "subtitle_opacity",
        "keep_subtitle_styles",
        "ui_language",
        "watched_at",
        "binge_watching",
        "next_video_notice",
    ];
    let all: BTreeSet<&str> = Settings::default().all().iter().map(|s| s.key()).collect();
    let synced: BTreeSet<&str> = SYNCED.iter().copied().collect();
    let local: BTreeSet<&str> = DEVICE_LOCAL_SETTINGS.iter().copied().collect();
    assert!(synced.is_disjoint(&local));
    assert_eq!(
        synced.union(&local).copied().collect::<BTreeSet<_>>(),
        all,
        "a new setting must be listed as synced here or in DEVICE_LOCAL_SETTINGS"
    );
}

#[test]
fn joining_keeps_the_addons_of_both_devices() {
    let mut joining = replica(B);
    joining.seed(
        &[],
        &[addon("x.example"), addon("y.example")],
        &Settings::default(),
        1_000,
    );
    joining.merge(
        record(
            RecordName::Addons,
            stamp(500, 0, A),
            Value::Addons(vec![addon("z.example"), addon("x.example")]),
        ),
        4,
        1_000,
    );
    let union = [addon("z.example"), addon("x.example"), addon("y.example")];
    assert_eq!(joining.addons(), Some(&union[..]));
    let outgoing = joining.outgoing();
    assert!(
        outgoing
            .iter()
            .any(|o| o.record.name == RecordName::Addons && o.base_seq == 4),
        "{outgoing:?}"
    );
}

#[test]
fn a_later_addon_change_replaces_the_list() {
    let mut local = replica(B);
    local.record_effect(&Effect::SaveAddons(vec![addon("x.example")]), 1_000);
    local.merge(
        record(
            RecordName::Addons,
            stamp(2_000, 0, A),
            Value::Addons(vec![addon("z.example")]),
        ),
        1,
        2_000,
    );
    assert_eq!(local.addons(), Some(&[addon("z.example")][..]));
}

#[test]
fn unknown_record_kinds_are_kept_and_ignored() {
    let unknown = r#"{"v":1,"name":"watchlist/tt9","stamp":{"t":5,"c":0,"d":3},"extra":[1,2]}"#;
    let newer = r#"{"v":2,"name":"library/tt1","stamp":{"t":9000,"c":0,"d":3},"item2":{}}"#;
    let mut local = replica(A);
    local.record_effect(&Effect::SaveLibraryItem(item("tt1", 100)), 1_000);
    for (seq, text) in (1..).zip([unknown, newer]) {
        let record = Record::parse(text).unwrap();
        assert!(matches!(record.value, Value::Other(_)), "{record:?}");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&record.to_json()).unwrap(),
            serde_json::from_str::<serde_json::Value>(text).unwrap(),
            "written back unchanged"
        );
        local.merge(record, seq, 1_000);
    }
    assert!(
        local
            .get(&RecordName::Other("watchlist/tt9".into()))
            .is_some()
    );
    assert!(
        local.library().is_empty(),
        "an item in a newer format is not shown"
    );
}

#[test]
fn records_read_back_what_they_write() {
    let mut favorite = item("tt1", 42);
    favorite.favorited = Some(3);
    favorite.poster = None;
    let records = [
        record(
            RecordName::Item("tt1".into()),
            stamp(1, 2, A),
            Value::Item(Box::new(favorite)),
        ),
        item_record("a/b:c", 7, stamp(u64::MAX, u32::MAX, DeviceId(u64::MAX))),
        record(
            RecordName::Item("tt2".into()),
            stamp(3, 0, B),
            Value::Deleted,
        ),
        record(RecordName::LibraryCleared, stamp(4, 0, C), Value::Cleared),
        record(
            RecordName::Addons,
            stamp(5, 0, A),
            Value::Addons(vec![addon("x.example"), addon("y.example")]),
        ),
        record(
            RecordName::Setting("watched_at".into()),
            stamp(6, 0, A),
            Value::Setting("90".into()),
        ),
    ];
    for r in records {
        assert_eq!(Record::parse(&r.to_json()), Some(r));
    }
}

#[test]
fn malformed_records_are_rejected() {
    for text in [
        "not json",
        r#"{"v":1,"name":"library/tt1","stamp":{"t":1,"c":0,"d":1},"item":{"type":""}}"#,
        r#"{"v":1,"name":"library/tt1","item":{}}"#,
        r#"{"v":1,"name":"","stamp":{"t":1,"c":0,"d":1}}"#,
        r#"{"v":1,"name":"setting/x","stamp":{"t":1,"c":4294967296,"d":1},"value":"1"}"#,
    ] {
        assert_eq!(Record::parse(text), None, "{text}");
    }
}

#[test]
fn an_unchanged_value_is_not_pushed_again() {
    let mut local = replica(A);
    local.record_effect(&Effect::SaveLibraryItem(item("tt1", 100)), 1_000);
    let out = local.outgoing().remove(0);
    local.pushed(&out.record.name, out.record.stamp, 1);
    local.record_effect(&Effect::SaveLibraryItem(item("tt1", 100)), 2_000);
    assert!(local.outgoing().is_empty());
    assert!(!local.push_due(10_000_000));
}

#[test]
fn an_edit_is_pushed_once_the_changes_stop_for_a_minute() {
    let mut local = replica(A);
    local.record_effect(&Effect::ClearLibrary, 1_000);
    assert!(!local.push_due(1_000 + PUSH_AFTER_QUIET_MS - 1));
    assert!(local.push_due(1_000 + PUSH_AFTER_QUIET_MS));
}

#[test]
fn progress_pushes_at_most_every_five_minutes_while_playing() {
    let mut local = replica(A);
    let mut pushes = Vec::new();
    for now in (0..=900_000).step_by(5_000) {
        local.record_effect(&Effect::SaveLibraryItem(item("tt1", now)), now);
        if local.push_due(now) {
            pushes.push(now);
            for out in local.outgoing() {
                local.pushed(&out.record.name, out.record.stamp, now);
            }
        }
    }
    assert_eq!(pushes, [PUSH_AT_MOST_MS, PUSH_AT_MOST_MS * 2 + 5_000]);
}

#[test]
fn stopping_playback_pushes_at_once() {
    let mut local = replica(A);
    local.record_effect(&Effect::SaveLibraryItem(item("tt1", 5_000)), 5_000);
    assert!(!local.push_due(5_000));
    local.flush();
    assert!(local.push_due(5_000));
}

#[test]
fn pulls_every_ten_minutes() {
    let mut local = replica(A);
    assert!(local.pull_due(0));
    local.pulled(1_000);
    assert!(!local.pull_due(600_999));
    assert!(local.pull_due(601_000));
}

#[test]
fn seeding_pushes_customized_settings_but_a_space_value_wins() {
    let mut settings = Settings::default();
    settings.set(Setting::SeekStep(SeekStep::S30));
    let mut joining = replica(B);
    joining.seed(&[item("tt1", 5)], &[], &settings, 10_000);
    assert!(joining.push_due(10_000), "seeded data is pushed right away");
    assert!(
        joining
            .outgoing()
            .iter()
            .any(|o| o.record.name == RecordName::Setting("seek_step".into()))
    );
    joining.merge(
        record(
            RecordName::Setting("seek_step".into()),
            stamp(1, 0, A),
            Value::Setting("5".into()),
        ),
        1,
        10_000,
    );
    assert_eq!(joining.settings_over(settings).seek_step, SeekStep::S5);
}

#[test]
fn changes_hold_only_what_changed_since_the_last_take() {
    let mut local = replica(A);
    assert_eq!(local.take_changes(), None);
    local.record_effect(&Effect::SaveLibraryItem(item("tt1", 1)), 1_000);
    local.record_effect(&Effect::SaveLibraryItem(item("tt2", 1)), 1_000);
    let first = local.take_changes().unwrap();
    assert_eq!(first.entries.len(), 2);
    assert_eq!(first.clock, local.clock().latest());
    local.record_effect(&Effect::SaveLibraryItem(item("tt2", 9)), 2_000);
    let second = local.take_changes().unwrap();
    assert_eq!(
        second
            .entries
            .iter()
            .map(|e| e.record.name.clone())
            .collect::<Vec<_>>(),
        [RecordName::Item("tt2".into())]
    );
    assert_eq!(local.take_changes(), None);
}

#[test]
fn a_restored_replica_continues_where_it_left_off() {
    let mut local = replica(A);
    local.merge(item_record("tt1", 5, stamp(9_000, 0, B)), 12, 9_000);
    local.observe_space(55, 12);
    local.record_effect(&Effect::SaveLibraryItem(item("tt2", 7)), 9_000);
    let changes = local.take_changes().unwrap();
    assert_eq!(changes.born, 55);
    let mut restored = Replica::restore(
        Settings::default(),
        changes.clock,
        changes.since,
        changes.born,
        changes.entries,
    );
    assert_eq!(ids(&restored), ids(&local));
    assert_eq!(restored.since(), 12);
    assert_eq!(restored.born(), local.born());
    assert_eq!(restored.outgoing(), local.outgoing());
    assert!(restored.push_due(0), "unpushed edits go out on start");
    restored.record_effect(&Effect::SaveLibraryItem(item("tt3", 1)), 1);
    let tt3 = restored.get(&RecordName::Item("tt3".into())).unwrap();
    assert!(
        tt3.stamp > stamp(9_000, 0, B),
        "the clock resumes past what it saw"
    );
}

#[test]
fn the_sync_key_is_never_printed() {
    let pairing = cineo_core::sync::Pairing {
        key: cineo_core::sync::SyncKey::from_bytes([0xAB; 16]),
        server: "https://sync.example/".parse().unwrap(),
    };
    let printed = format!("{pairing:?}");
    assert!(
        !printed.contains("171") && !printed.to_lowercase().contains("ab, "),
        "{printed}"
    );
    assert!(printed.contains("SyncKey(..)"));
}

fn synced(entries: &[(&str, u64)]) -> Replica {
    let mut local = replica(A);
    for (seq, (id, _)) in (1..).zip(entries) {
        local.merge(item_record(id, seq, stamp(1_000 + seq, 0, B)), seq, 2_000);
    }
    assert!(!local.observe_space(77, entries.len() as u64));
    local
}

#[test]
fn the_first_sight_of_a_space_is_not_a_restart() {
    let mut local = replica(A);
    assert!(!local.observe_space(0, 0), "nobody wrote yet");
    assert!(!local.observe_space(77, 3), "then a device created it");
    assert_eq!(local.born(), 77);
}

#[test]
fn a_server_that_went_back_makes_the_replica_start_over() {
    let mut local = synced(&[("tt1", 1), ("tt2", 2), ("tt3", 3)]);
    assert_eq!(local.since(), 3);
    assert!(local.observe_space(77, 1));
    assert_eq!(local.since(), 0);
    let outgoing = local.outgoing();
    assert_eq!(outgoing.len(), 3);
    assert!(outgoing.iter().all(|o| o.base_seq == 0));
    assert!(local.push_due(0));
}

#[test]
fn a_replaced_space_makes_the_replica_start_over() {
    let mut local = synced(&[("tt1", 1)]);
    assert!(local.observe_space(0, 0), "swept or emptied");
    assert!(
        !local.observe_space(78, 1),
        "recreated by this device's push"
    );
    let mut other = synced(&[("tt1", 1)]);
    assert!(other.observe_space(78, 5), "recreated by another device");
}

#[test]
fn after_starting_over_only_what_the_server_lacks_is_pushed() {
    let mut local = synced(&[("tt1", 1), ("tt2", 2), ("tt3", 3)]);
    local.record_effect(&Effect::SaveLibraryItem(item("tt9", 1)), 5_000);
    assert!(local.observe_space(77, 1));
    let kept = local.get(&RecordName::Item("tt1".into())).unwrap().clone();
    local.merge(kept, 1, 6_000);
    let mut names: Vec<(String, u64)> = local
        .outgoing()
        .into_iter()
        .map(|o| (o.record.name.to_string(), o.base_seq))
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            ("library/tt2".to_owned(), 0),
            ("library/tt3".to_owned(), 0),
            ("library/tt9".to_owned(), 0)
        ]
    );
}

#[test]
fn a_conflict_below_the_known_version_starts_over() {
    let mut local = synced(&[("tt1", 1), ("tt2", 2)]);
    assert!(local.conflicted(&[(RecordName::Item("tt2".into()), 0)]));
    assert_eq!(local.since(), 0);
}

#[test]
fn a_conflict_with_a_version_already_passed_starts_over() {
    let mut local = synced(&[("tt1", 1), ("tt2", 2), ("tt3", 3)]);
    assert!(
        local.conflicted(&[(RecordName::Item("tt1".into()), 2)]),
        "seq 2 is behind the pull position, so this device missed it"
    );
}

#[test]
fn a_conflict_with_a_newer_version_only_needs_a_pull() {
    let mut local = synced(&[("tt1", 1), ("tt2", 2)]);
    assert!(!local.conflicted(&[(RecordName::Item("tt1".into()), 9)]));
    assert!(!local.conflicted(&[]), "a lost race names no records");
    assert_eq!(local.since(), 2);
}

#[test]
fn a_pushed_version_above_the_space_also_means_it_went_back() {
    let mut local = replica(A);
    local.record_effect(&Effect::SaveLibraryItem(item("tt1", 1)), 1_000);
    let out = local.outgoing().remove(0);
    local.pushed(&out.record.name, out.record.stamp, 50);
    assert_eq!(local.since(), 0);
    assert!(local.observe_space(0, 10));
}

#[test]
fn the_daily_check_pushes_what_the_server_lost() {
    let mut local = synced(&[("tt1", 1), ("tt2", 2)]);
    assert!(local.verify_due(0), "on the first sync");
    local.verify(5_000);
    assert!(!local.verify_due(5_000 + VERIFY_EVERY_MS - 1));
    assert!(local.verify_due(5_000 + VERIFY_EVERY_MS));
    assert_eq!(local.since(), 0);
    let kept = local.get(&RecordName::Item("tt1".into())).unwrap().clone();
    local.merge(kept, 1, 6_000);
    let names: Vec<String> = local
        .outgoing()
        .into_iter()
        .map(|o| o.record.name.to_string())
        .collect();
    assert_eq!(names, ["library/tt2"]);
}

#[test]
fn deleting_an_item_that_is_already_gone_records_nothing() {
    let mut local = replica(A);
    local.record_effect(&Effect::DeleteLibraryItem("never".into()), 1_000);
    local.merge(item_record("old", 1, stamp(500, 0, B)), 1, 1_000);
    local.merge(
        record(RecordName::LibraryCleared, stamp(600, 0, B), Value::Cleared),
        2,
        1_000,
    );
    local.record_effect(&Effect::DeleteLibraryItem("old".into()), 1_000);
    assert!(
        local.outgoing().is_empty(),
        "a remote clear does not echo back as tombstones"
    );
}

#[test]
fn values_never_synced_give_way_to_the_space_whatever_the_device_ids() {
    let addon = |host: &str| {
        cineo_core::addon::TransportUrl::parse(&format!("https://{host}/manifest.json")).unwrap()
    };
    let mut settings = Settings::default();
    settings.set(Setting::SeekStep(SeekStep::S30));
    let mut joining = replica(C);
    joining.seed(&[], &[addon("mine.example")], &settings, 1_000);
    let seed = |name, value| record(name, stamp(0, 0, A), value);
    joining.merge(
        seed(
            RecordName::Setting("seek_step".into()),
            Value::Setting("5".into()),
        ),
        1,
        1_000,
    );
    joining.merge(
        seed(
            RecordName::Addons,
            Value::Addons(vec![addon("theirs.example")]),
        ),
        2,
        1_000,
    );
    assert_eq!(joining.settings_over(settings).seek_step, SeekStep::S5);
    assert_eq!(
        joining.addons(),
        Some(&[addon("theirs.example"), addon("mine.example")][..])
    );
}
