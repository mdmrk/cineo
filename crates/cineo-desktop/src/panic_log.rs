use std::path::Path;

pub fn record_panics(data_dir: &Path) {
    let default_hook = std::panic::take_hook();
    fastframe_log::log_panics(
        data_dir.join("panic.log"),
        "cineo",
        env!("CARGO_PKG_VERSION"),
    );
    let record = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        record(info);
        default_hook(info);
    }));
}
