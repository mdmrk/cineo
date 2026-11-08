#[test]
fn a_panic_is_recorded_without_its_message() {
    let dir = std::env::temp_dir().join(format!("cineo-panic-log-{}", std::process::id()));
    cineo_desktop::panic_log::record_panics(&dir);
    let _ = std::thread::spawn(|| panic!("private stream url")).join();
    let log = std::fs::read_to_string(dir.join("panic.log")).unwrap();
    assert!(log.contains("panic at"));
    assert!(!log.contains("private stream url"));
    let _ = std::fs::remove_dir_all(&dir);
}
