//! Shared helpers of the L1 settings tests: a fresh store directory and a
//! bounded wait on the real settings worker.
use crate::settings::SettingsHost;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

pub(super) fn directory(name: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/settings-tests")
        .join(format!(
            "yebisu-l1-{}-{name}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ))
}

#[track_caller]
pub(super) fn wait(host: &mut SettingsHost, condition: impl Fn(&SettingsHost) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        host.poll();
        if condition(host) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "settings condition timed out: {:?}",
            host.error
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Opens the store at `root` as a fresh process does, after its load.
#[track_caller]
pub(super) fn open(root: &std::path::Path) -> SettingsHost {
    let mut host = SettingsHost::open(root.to_path_buf()).unwrap_or_else(|e| panic!("open: {e}"));
    wait(&mut host, |h| h.ready() || h.error.is_some());
    host
}

/// Writes `document` as the settings file, as an older or newer build did.
pub(super) fn write_file(root: &std::path::Path, document: &serde_json::Value) {
    std::fs::create_dir_all(root).unwrap_or_else(|e| panic!("mkdir: {e}"));
    std::fs::write(root.join("yebisu-settings.json"), document.to_string())
        .unwrap_or_else(|e| panic!("write: {e}"));
}
