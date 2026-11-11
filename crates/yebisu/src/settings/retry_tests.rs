use super::tests::{directory, wait};
use super::*;
use gpui::AppLifecyclePhase;
use std::{fs, time::Duration};

// Substitute only the OS token; acquisition, retention and drop decisions are
// the same SettingsHost lifecycle/poll branches compiled for iOS.
#[cfg(not(target_os = "ios"))]
pub(super) struct BackgroundTask(std::sync::Arc<std::sync::atomic::AtomicBool>);
#[cfg(not(target_os = "ios"))]
impl BackgroundTask {
    pub(super) fn begin(_: &str) -> Option<Self> {
        Some(Self(std::sync::Arc::new(
            std::sync::atomic::AtomicBool::new(true),
        )))
    }
}
#[cfg(not(target_os = "ios"))]
impl Drop for BackgroundTask {
    fn drop(&mut self) {
        self.0.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

#[test]
#[cfg(not(target_os = "ios"))]
fn retry_holds_background_task_until_durable_ack() {
    let root = directory("retry-lease");
    store::save(&root, &Values::default()).unwrap_or_else(|e| panic!("{e}"));
    let mut host = SettingsHost::open(root.clone()).unwrap_or_else(|e| panic!("{e}"));
    wait(&mut host, |h| h.loaded);
    fs::create_dir(root.join("yebisu-settings.pending")).unwrap_or_else(|e| panic!("{e}"));
    host.edit(Values {
        compact: true,
        ..Values::default()
    });
    host.lifecycle(AppLifecyclePhase::Background);
    wait(&mut host, |h| h.error.is_some());
    assert!(host.background.is_none(), "failed idle writer retains task");
    fs::remove_dir(root.join("yebisu-settings.pending")).unwrap_or_else(|e| panic!("{e}"));
    let (entered, paused) = mpsc::sync_channel(1);
    let (resume, gate) = mpsc::sync_channel(1);
    host.sender
        .send(Request::Pause(entered, gate))
        .unwrap_or_else(|e| panic!("{e}"));
    paused
        .recv_timeout(Duration::from_secs(3))
        .unwrap_or_else(|e| panic!("{e}"));
    host.edit(Values {
        compact: true,
        board_hidden: true,
        ..Values::default()
    });
    host.lifecycle(AppLifecyclePhase::Background);
    let alive = host
        .background
        .as_ref()
        .unwrap_or_else(|| panic!("missing background task"))
        .0
        .clone();
    host.poll();
    assert_eq!(host.in_flight, Some(2));
    assert!(
        host.error.is_some(),
        "past error remains visible until success"
    );
    assert!(
        alive.load(std::sync::atomic::Ordering::SeqCst),
        "retry task dropped before acknowledgement"
    );
    assert_eq!(
        store::load(&root).unwrap_or_else(|e| panic!("{e}")),
        Values::default()
    );
    resume.send(()).unwrap_or_else(|e| panic!("{e}"));
    wait(&mut host, SettingsHost::ready);
    assert!(!alive.load(std::sync::atomic::Ordering::SeqCst));
    assert!(host.background.is_none());
    assert_eq!(
        store::load(&root).unwrap_or_else(|e| panic!("{e}")),
        host.draft
    );
    fs::remove_dir_all(root).unwrap_or_else(|e| panic!("{e}"));
}
