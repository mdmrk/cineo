use super::*;

#[test]
fn the_first_torrent_play_asks_for_p2p_consent_and_accepting_starts_the_engine() {
    let mut state = detail_with_streams();
    assert!(update(&mut state, PLAY_TORRENT).is_empty());
    assert_eq!(state.p2p_prompt, Some((0, 1)));
    assert!(state.torrent.is_none(), "nothing starts before consent");

    let effects = update(&mut state, Action::AcceptP2p);
    let [
        Effect::SaveSettings(settings),
        Effect::SaveLibraryItem(item),
        Effect::StartTorrent(request),
        ..,
    ] = effects.as_slice()
    else {
        panic!("{effects:?}");
    };
    assert!(settings.p2p_acknowledged);
    assert_eq!(item.id, "tt0000001");
    assert_eq!(
        request,
        &TorrentRequest {
            info_hash: FIXTURE_HASH.into(),
            file_idx: Some(2),
            filename: None,
            trackers: vec![url::Url::parse("udp://tracker.example:1337").unwrap()],
        }
    );
    assert_eq!(state.p2p_prompt, None);
    let torrent = state.torrent.as_ref().unwrap();
    assert_eq!(torrent.status, TorrentStatus::Starting);
    assert!(
        torrent.connecting().is_some(),
        "the shell can show the player while the engine prepares the file"
    );
}

#[test]
fn declining_p2p_consent_plays_nothing_and_asks_again_next_time() {
    let mut state = detail_with_streams();
    update(&mut state, PLAY_TORRENT);
    assert!(update(&mut state, Action::DeclineP2p).is_empty());
    assert_eq!(state.p2p_prompt, None);
    assert!(!state.settings.p2p_acknowledged);
    update(&mut state, PLAY_TORRENT);
    assert_eq!(state.p2p_prompt, Some((0, 1)));
}

#[test]
fn a_served_torrent_plays_from_the_loopback_url_and_stopping_playback_stops_the_engine() {
    let mut state = detail_with_p2p_accepted();
    update(&mut state, PLAY_TORRENT);
    let served = url::Url::parse("http://127.0.0.1:40000/token/0").unwrap();

    let stale = update(
        &mut state,
        Action::TorrentReady {
            info_hash: "f".repeat(40),
            url: served.clone(),
        },
    );
    assert!(stale.is_empty());

    let effects = update(
        &mut state,
        Action::TorrentReady {
            info_hash: FIXTURE_HASH.into(),
            url: served.clone(),
        },
    );
    let [Effect::Play(play)] = effects.as_slice() else {
        panic!("{effects:?}");
    };
    assert_eq!(play.url, served);
    assert!(play.headers.is_empty());
    assert_eq!(play.meta_id, "tt0000001");
    assert_eq!(
        state.torrent.as_ref().unwrap().connecting(),
        None,
        "handed to the player"
    );

    let status = TorrentStatus::Streaming {
        peers: 3,
        download_bytes_per_sec: 1000,
        downloaded: 10,
        size: 100,
    };
    update(
        &mut state,
        Action::TorrentStatus {
            info_hash: FIXTURE_HASH.into(),
            status: status.clone(),
        },
    );
    assert_eq!(state.torrent.as_ref().unwrap().status, status);

    assert_eq!(
        update(&mut state, Action::PlaybackStopped),
        vec![
            Effect::StopTorrent,
            Effect::DeleteLibraryItem("tt0000001".into())
        ],
        "it never played, so it is not kept"
    );
    assert!(state.torrent.is_none());
    assert!(update(&mut state, Action::PlaybackStopped).is_empty());
}

fn seconds(seconds: u64) -> Buffered {
    Buffered {
        seconds,
        bytes_per_second: None,
    }
}

fn holding(seconds: u64, bytes_per_second: u64) -> Buffered {
    Buffered {
        seconds,
        bytes_per_second: Some(bytes_per_second),
    }
}

fn ahead(fill: &mut Lookahead, buffered: Buffered, size: u64, duration_ms: u64) -> Option<u64> {
    fill.update(buffered, size, duration_ms)
        .map(|refill| refill.lookahead)
}

#[test]
fn the_torrent_fills_up_to_two_minutes_and_resumes_at_100_seconds() {
    let mut fill = Lookahead::default();
    let (size, duration_ms) = (1_440_000_000, 7_200_000);
    assert_eq!(
        ahead(&mut fill, seconds(0), 0, duration_ms),
        None,
        "no size yet"
    );
    assert_eq!(
        ahead(&mut fill, seconds(0), size, 0),
        None,
        "no duration yet"
    );
    assert_eq!(
        ahead(&mut fill, seconds(0), size, duration_ms),
        Some(24_000_000),
        "120 s at 200 kB/s while nothing is buffered"
    );
    assert_eq!(
        ahead(&mut fill, seconds(0), size, duration_ms),
        None,
        "unchanged"
    );
    assert_eq!(
        ahead(&mut fill, seconds(10), size, duration_ms),
        Some(22_000_000)
    );
    assert_eq!(
        ahead(&mut fill, seconds(119), size, duration_ms),
        Some(200_000)
    );
    assert_eq!(
        ahead(&mut fill, seconds(120), size, duration_ms),
        Some(0),
        "full"
    );
    assert_eq!(
        ahead(&mut fill, seconds(117), size, duration_ms),
        None,
        "stays stopped"
    );
    assert_eq!(ahead(&mut fill, seconds(101), size, duration_ms), None);
    assert_eq!(
        ahead(&mut fill, seconds(100), size, duration_ms),
        Some(4_000_000),
        "at 100 s it fills up to 120 s again"
    );
    assert_eq!(
        ahead(&mut fill, seconds(101), size, duration_ms),
        Some(3_800_000)
    );
    assert_eq!(
        ahead(&mut fill, seconds(0), size, duration_ms),
        Some(24_000_000),
        "a seek"
    );
}

#[test]
fn a_measured_bitrate_above_the_average_sizes_the_refill() {
    let mut fill = Lookahead::default();
    let (size, duration_ms) = (1_440_000_000, 7_200_000);
    assert_eq!(
        ahead(&mut fill, holding(20, 1_000_000), size, duration_ms),
        Some(100_000_000),
        "100 s at the 1 MB/s mpv holds, not at the 200 kB/s average"
    );
    assert_eq!(
        ahead(&mut fill, holding(20, 50_000), size, duration_ms),
        Some(20_000_000),
        "a measure under the average does not shrink the refill"
    );
    assert_eq!(
        ahead(&mut fill, holding(20, 0), size, duration_ms),
        None,
        "nothing measured, the same as the average"
    );
    assert_eq!(
        ahead(&mut fill, holding(120, 1_000_000), size, duration_ms),
        Some(0),
        "full"
    );
}

#[test]
fn without_a_duration_the_measured_bitrate_sizes_the_refill() {
    let mut fill = Lookahead::default();
    let size = 1_440_000_000;
    assert_eq!(
        ahead(&mut fill, seconds(5), size, 0),
        None,
        "no bitrate yet"
    );
    assert_eq!(
        ahead(&mut fill, holding(60, 500_000), size, 0),
        Some(30_000_000),
        "60 s at 500 kB/s"
    );
    assert_eq!(
        ahead(&mut fill, holding(120, 500_000), 0, 0),
        Some(0),
        "full"
    );
    assert_eq!(ahead(&mut fill, holding(110, 500_000), 0, 0), None);
    assert_eq!(
        ahead(&mut fill, holding(100, 500_000), 0, 0),
        Some(10_000_000)
    );
}

#[test]
fn a_refill_is_paced_at_twice_the_bitrate_once_30_seconds_are_held() {
    let mut fill = Lookahead::default();
    let (size, duration_ms) = (7_200_000_000, 7_200_000);
    let refill = |lookahead, bytes_per_second| {
        Some(Refill {
            lookahead,
            bytes_per_second,
        })
    };
    assert_eq!(
        fill.update(seconds(0), size, duration_ms),
        refill(120_000_000, None),
        "a start or a seek downloads as fast as it can"
    );
    assert_eq!(
        fill.update(seconds(29), size, duration_ms),
        refill(91_000_000, None)
    );
    assert_eq!(
        fill.update(seconds(30), size, duration_ms),
        refill(90_000_000, Some(2_000_000)),
        "twice the 1 MB/s average"
    );
    assert_eq!(
        fill.update(holding(31, 1_100_000), size, duration_ms),
        refill(97_900_000, Some(2_000_000)),
        "a bitrate within an eighth keeps the pace"
    );
    assert_eq!(
        fill.update(holding(32, 3_000_000), size, duration_ms),
        refill(264_000_000, Some(6_000_000)),
        "a high-bitrate scene raises it"
    );
    assert_eq!(
        fill.update(holding(120, 3_000_000), size, duration_ms),
        refill(0, Some(6_000_000)),
        "full"
    );
    assert_eq!(
        fill.update(seconds(5), size, duration_ms),
        refill(115_000_000, None),
        "a seek lifts the pace"
    );
    assert_eq!(
        fill.update(holding(40, 50_000), 0, 0),
        refill(4_000_000, Some(262_144)),
        "never slower than 256 KiB/s"
    );
}

#[test]
fn an_engine_url_that_is_not_loopback_http_is_refused() {
    let mut state = detail_with_p2p_accepted();
    update(&mut state, PLAY_TORRENT);
    let effects = update(
        &mut state,
        Action::TorrentReady {
            info_hash: FIXTURE_HASH.into(),
            url: url::Url::parse("http://192.168.1.10:8080/x").unwrap(),
        },
    );
    assert_eq!(effects, vec![Effect::StopTorrent]);
    assert!(state.torrent.is_none());
    assert!(state.notice.is_some());
}

#[test]
fn an_engine_failure_is_shown_and_stops_the_torrent() {
    let mut state = detail_with_p2p_accepted();
    update(&mut state, PLAY_TORRENT);
    let effects = update(
        &mut state,
        Action::TorrentFailed {
            info_hash: FIXTURE_HASH.into(),
            reason: "no peers found".into(),
        },
    );
    assert_eq!(
        effects,
        vec![
            Effect::StopTorrent,
            Effect::DeleteLibraryItem("tt0000001".into())
        ]
    );
    assert_eq!(
        state.notice,
        Some(Notice::PlaybackFailed {
            failure: PlaybackFailure::Torrent("no peers found".into()),
            pick_another: false,
        })
    );
}

#[test]
fn turning_p2p_off_blocks_torrents_and_stops_a_running_one() {
    let mut state = detail_with_p2p_accepted();
    update(&mut state, PLAY_TORRENT);
    let effects = update(
        &mut state,
        Action::ChangeSetting(Setting::P2pEnabled(false)),
    );
    let [Effect::SaveSettings(settings), Effect::StopTorrent] = effects.as_slice() else {
        panic!("{effects:?}");
    };
    assert!(!settings.p2p_enabled);
    assert!(state.torrent.is_none());

    assert!(update(&mut state, PLAY_TORRENT).is_empty());
    assert_eq!(state.notice, Some(Notice::TorrentsOff));
}

#[test]
fn playing_an_http_stream_stops_a_running_torrent() {
    let mut state = detail_with_p2p_accepted();
    update(&mut state, PLAY_TORRENT);
    let effects = update(
        &mut state,
        Action::Play {
            group: 0,
            stream: 0,
        },
    );
    assert!(
        matches!(
            effects.as_slice(),
            [
                Effect::SaveLibraryItem(_),
                Effect::StopTorrent,
                Effect::Play(_),
                ..
            ]
        ),
        "{effects:?}"
    );
}
