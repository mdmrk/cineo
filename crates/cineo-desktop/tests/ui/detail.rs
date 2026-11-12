use super::*;

#[test]
fn detail_lists_streams_and_only_playable_ones_can_be_played() {
    let mut harness = harness(detail_state(), ViewState::default());
    harness.get_by_label("First Example Film");
    harness.get_by_label("Example HTTP stream");
    harness.get_by_label("Basic Fixture");
    let play: Vec<_> = harness.get_all_by_label("Play").collect();
    assert_eq!(play.len(), 6, "one button per stream");
    let enabled: Vec<bool> = play
        .iter()
        .map(|b| !b.accesskit_node().is_disabled())
        .collect();
    assert_eq!(enabled, vec![true, true, false, false, false, false]);

    play[0].click();
    harness.run();
    assert_eq!(
        harness.state().2,
        vec![Action::Play {
            group: 0,
            stream: 0
        }]
    );
}

#[test]
fn stream_search_and_quality_filter_narrow_the_list_and_keep_stream_indexes() {
    let mut harness = harness(detail_state(), ViewState::default());
    assert_eq!(harness.get_all_by_label("Play").count(), 6);

    harness.get_by_role(Role::TextInput).focus();
    harness
        .get_by_role(Role::TextInput)
        .type_text("DEPRECATED field");
    harness.run();
    assert_eq!(harness.get_all_by_label("Play").count(), 1);
    harness.get_by_label("Deprecated title field");

    harness.get_by_role(Role::TextInput).focus();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.key_press(Key::Backspace);
    harness.run();
    harness.get_by_role_and_label(Role::Button, "1080p").click();
    harness.run();
    let play: Vec<_> = harness.get_all_by_label("Play").collect();
    assert_eq!(play.len(), 1);
    play[0].click();
    harness.run();
    assert_eq!(
        harness.state().2,
        vec![Action::Play {
            group: 0,
            stream: 0
        }]
    );
}

#[test]
fn sorting_by_seeders_reorders_the_list_and_plays_the_right_stream() {
    let body = r#"{"streams":[
        {"name":"Example\n720p","description":"Few seeds\n👤 3 💾 700 MB","url":"https://media.example/a.mp4"},
        {"name":"Example\n1080p","description":"Many seeds\n👤 90 💾 1.5 GB","url":"https://media.example/b.mp4"}
    ]}"#;
    let mut harness = harness(detail_with_streams(body.as_bytes()), ViewState::default());
    let top = |h: &Harness<'_, Ui>, label: &str| h.get_by_label_contains(label).rect().top();
    assert!(top(&harness, "Few seeds") < top(&harness, "Many seeds"));
    harness
        .get_by_role_and_label(Role::Button, "Seeders")
        .click();
    harness.run();
    assert!(top(&harness, "Many seeds") < top(&harness, "Few seeds"));
    harness.get_all_by_label("Play").next().unwrap().click();
    harness.run();
    assert_eq!(
        harness.state().2,
        vec![Action::Play {
            group: 0,
            stream: 1
        }]
    );
}

#[test]
fn turning_p2p_off_hides_torrent_streams() {
    let mut state = detail_state();
    update(
        &mut state,
        Action::ChangeSetting(Setting::P2pEnabled(false)),
    );
    let harness = harness(state, ViewState::default());
    assert_eq!(harness.get_all_by_label("Play").count(), 5);
    harness.get_by_label("Torrent streams hidden: 1 (peer-to-peer is off in Settings)");
}

#[test]
fn the_first_torrent_play_shows_the_p2p_notice() {
    let mut state = detail_state();
    update(
        &mut state,
        Action::Play {
            group: 0,
            stream: 1,
        },
    );
    assert!(state.p2p_prompt.is_some());

    let mut harness = harness(state.clone(), ViewState::default());
    harness.get_by_label("Peer-to-peer streaming");
    harness.get_by_label_contains("uploads the parts it has already downloaded");
    harness.get_by_label("Accept and play").click();
    harness.run();
    assert_eq!(harness.state().2, vec![Action::AcceptP2p]);

    let mut harness = self::harness(state, ViewState::default());
    harness.get_by_label("Cancel").click();
    harness.run();
    assert_eq!(harness.state().2, vec![Action::DeclineP2p]);
}

#[test]
fn long_stream_lists_lay_out_only_visible_cards_and_still_scroll_to_the_end() {
    let streams: Vec<String> = (0..300)
        .map(|i| {
            format!(r#"{{"name":"1080p","description":"Release {i}","url":"https://media.example/{i}.mp4"}}"#)
        })
        .collect();
    let body = format!(r#"{{"streams":[{}]}}"#, streams.join(","));
    let mut harness = harness(detail_with_streams(body.as_bytes()), ViewState::default());
    harness.run();
    assert!(harness.query_all_by_label("Play").count() < 50);
    assert!(harness.query_by_label("Release 299").is_none());

    harness.hover_at(pos2(640.0, 450.0));
    harness.event(Event::MouseWheel {
        unit: MouseWheelUnit::Point,
        delta: vec2(0.0, -1.0e6),
        phase: TouchPhase::Move,
        modifiers: Modifiers::NONE,
    });
    harness.run_steps(60);
    harness.run();
    harness.get_by_label("Release 299");
}
