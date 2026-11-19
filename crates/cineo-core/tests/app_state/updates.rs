use super::*;
use cineo_core::app::{UpdateFailure, UpdateProgress, UpdateRequest};
use semver::Version;
use url::Url;

fn offered(self_update: bool) -> State {
    let mut state = State {
        self_update,
        ..State::default()
    };
    update(&mut state, Action::RestoreSettings(Settings::default()));
    update(
        &mut state,
        Action::LatestReleaseLoaded(Ok(Version::new(999, 1, 0))),
    );
    state
}

fn asset(name: &str) -> Url {
    Url::parse(&format!(
        "https://github.com/mdmrk/cineo/releases/download/v999.1.0/{name}"
    ))
    .unwrap()
}

#[test]
fn installing_an_update_downloads_the_offered_release() {
    let mut state = offered(true);
    let effects = update(&mut state, Action::InstallUpdate);
    assert_eq!(
        effects,
        vec![Effect::InstallUpdate(Box::new(UpdateRequest {
            version: Version::new(999, 1, 0),
            sums: asset("SHA256SUMS"),
            signature: asset("SHA256SUMS.minisig"),
            executable: asset("cineo-x86_64-windows.exe"),
            licenses: asset("cineo-licenses.zip"),
        }))]
    );
    assert_eq!(state.update_progress, UpdateProgress::Downloading);
    assert!(update(&mut state, Action::InstallUpdate).is_empty());
}

#[test]
fn a_copy_that_cannot_update_itself_downloads_nothing() {
    let mut state = offered(false);
    assert!(update(&mut state, Action::InstallUpdate).is_empty());
    assert_eq!(state.update_progress, UpdateProgress::Idle);
}

#[test]
fn nothing_downloads_without_a_newer_release() {
    let mut state = State {
        self_update: true,
        ..State::default()
    };
    update(&mut state, Action::RestoreSettings(Settings::default()));
    assert!(update(&mut state, Action::InstallUpdate).is_empty());
}

#[test]
fn a_finished_update_waits_for_a_restart() {
    let mut state = offered(true);
    update(&mut state, Action::InstallUpdate);
    assert!(update(&mut state, Action::RestartToUpdate).is_empty());
    update(&mut state, Action::UpdateFinished(Ok(())));
    assert_eq!(state.update_progress, UpdateProgress::Ready);
    assert!(update(&mut state, Action::InstallUpdate).is_empty());
    assert_eq!(
        update(&mut state, Action::RestartToUpdate),
        vec![Effect::RestartToUpdate]
    );
}

#[test]
fn a_failed_update_says_why_and_can_be_tried_again() {
    let mut state = offered(true);
    update(&mut state, Action::InstallUpdate);
    update(
        &mut state,
        Action::UpdateFinished(Err(UpdateFailure::Signature)),
    );
    assert_eq!(
        state.update_progress,
        UpdateProgress::Failed(UpdateFailure::Signature)
    );
    assert_eq!(update(&mut state, Action::InstallUpdate).len(), 1);
    assert_eq!(state.update_progress, UpdateProgress::Downloading);
}

#[test]
fn a_late_result_without_a_download_changes_nothing() {
    let mut state = offered(true);
    update(&mut state, Action::UpdateFinished(Ok(())));
    assert_eq!(state.update_progress, UpdateProgress::Idle);
}
