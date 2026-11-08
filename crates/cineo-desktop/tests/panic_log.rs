#[test]
fn a_panic_is_recorded_without_its_message() {
    let dir = tempfile::tempdir().unwrap();
    cineo_desktop::panic_log::record_panics(dir.path());
    let _ = std::thread::spawn(|| panic!("private stream url")).join();
    let log = std::fs::read_to_string(dir.path().join("panic.log")).unwrap();
    assert!(log.contains("panic at"));
    assert!(!log.contains("private stream url"));
}
