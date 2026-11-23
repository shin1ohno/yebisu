//! The Settings sheet's Hosts and keys page reads the iPad's SSH public key
//! when it opens, as Add host does, without the Read button: selecting the
//! page and opening the sheet on it both start one read on the device-key
//! worker, and a failed read shows the page's usual failure line.
use crate::{
    app::Smoke,
    settings::{
        SettingsHost,
        key_ops::{Failure, Owner, Provider},
    },
    settings_pages::Page,
};
use gpui::{Entity, TestAppContext, VisualTestContext, px, size};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc::{Receiver, SyncSender, sync_channel},
    },
    time::{Duration, Instant},
};

const RAW: &str = "restrict ssh-ed25519 S0VZX09OX09QRU5fU0VOVElORUw= herdr-ipad-injected";
/// The line the page shows for RAW (`crate::keys::authorized_line` renames the comment).
const LINE: &str = "restrict ssh-ed25519 S0VZX09OX09QRU5fU0VOVElORUw= yebisu-injected";

/// An injected provider that counts reads, can hold the first one until the
/// test releases it, and can fail.
struct Held {
    reads: AtomicUsize,
    fail: bool,
    hold: Mutex<Option<Receiver<()>>>,
}

impl Provider for Held {
    fn injected(&self) -> bool {
        true
    }
    fn supported(&self) -> bool {
        true
    }
    fn read_or_initialize(&self) -> Result<String, Failure> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        let hold = self.hold.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(wait) = hold {
            wait.recv_timeout(Duration::from_secs(30))
                .map_err(|_| Failure::Worker)?;
        }
        if self.fail {
            Err(Failure::Provider)
        } else {
            Ok(RAW.into())
        }
    }
    fn replace(&self) -> Result<String, Failure> {
        Err(Failure::Provider)
    }
}

fn directory(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/key-on-open-{name}-{}",
        std::process::id()
    ))
}

/// A window with Settings loaded and `provider` as its key owner; the
/// observation sink admits the key provider preflight, as in production QA.
fn window<'a>(
    cx: &'a mut TestAppContext,
    root: &Path,
    provider: Arc<Held>,
) -> (
    Entity<Smoke>,
    &'a mut VisualTestContext,
    Receiver<serde_json::Value>,
) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1100.), px(900.)));
    let (logs, records): (SyncSender<serde_json::Value>, _) = sync_channel(256);
    view.update(cx, |this, _| {
        let mut host = SettingsHost::open(root.to_path_buf()).unwrap_or_else(|e| panic!("{e}"));
        host.keys = Owner::new(provider);
        this.settings = Some(host);
        this.observation = crate::observation::Observation::injected(logs);
    });
    let until = Instant::now() + Duration::from_secs(5);
    while !view.update(cx, |this, cx| {
        this.poll_settings(cx);
        this.settings.as_ref().is_some_and(SettingsHost::ready)
    }) {
        assert!(Instant::now() < until, "settings did not load");
        std::thread::sleep(Duration::from_millis(2));
    }
    cx.run_until_parked();
    (view, cx, records)
}

fn press(cx: &mut VisualTestContext, selector: &'static str) {
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("missing control {selector}"));
    cx.simulate_click(bounds.center(), Default::default());
    cx.run_until_parked();
}

fn busy(view: &Entity<Smoke>, cx: &mut VisualTestContext) -> bool {
    view.update(cx, |this, cx| {
        this.poll_settings(cx);
        this.settings.as_ref().is_some_and(|host| host.keys.busy())
    })
}

#[track_caller]
fn until_idle(view: &Entity<Smoke>, cx: &mut VisualTestContext) {
    let until = Instant::now() + Duration::from_secs(5);
    while busy(view, cx) {
        assert!(Instant::now() < until, "device key worker did not settle");
        std::thread::sleep(Duration::from_millis(2));
    }
    cx.run_until_parked();
}

fn key_line(view: &Entity<Smoke>, cx: &mut VisualTestContext) -> Option<String> {
    view.read_with(cx, |this, _| {
        this.settings
            .as_ref()
            .and_then(|host| host.keys.line().map(str::to_owned))
    })
}

#[gpui::test]
fn selecting_the_key_page_reads_the_key_off_the_ui_thread_without_a_press(cx: &mut TestAppContext) {
    let root = directory("select");
    let (release, hold) = sync_channel(1);
    let provider = Arc::new(Held {
        reads: AtomicUsize::new(0),
        fail: false,
        hold: Mutex::new(Some(hold)),
    });
    let (view, cx, records) = window(cx, &root, provider.clone());
    press(cx, "open-settings");
    assert_eq!(
        provider.reads.load(Ordering::SeqCst),
        0,
        "Display opens first"
    );
    press(cx, "settings-page-ipad-key");
    // The click returned while the provider is still held: the read runs on
    // the device-key worker, not on the UI thread.
    assert!(busy(&view, cx), "selecting the page must start a read");
    assert_eq!(key_line(&view, cx), None);
    release.send(()).unwrap_or_else(|e| panic!("{e}"));
    until_idle(&view, cx);
    assert_eq!(provider.reads.load(Ordering::SeqCst), 1);
    assert_eq!(key_line(&view, cx).as_deref(), Some(LINE));
    let receipts = records.try_iter().collect::<Vec<_>>();
    assert!(
        receipts
            .iter()
            .any(|r| r["kind"] == "key-provider-preflight" && r["phase"] == "read"),
        "the read went through the key provider preflight: {receipts:?}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[gpui::test]
fn opening_the_sheet_on_the_key_page_reads_the_key_again(cx: &mut TestAppContext) {
    let root = directory("reopen");
    let provider = Arc::new(Held {
        reads: AtomicUsize::new(0),
        fail: false,
        hold: Mutex::new(None),
    });
    let (view, cx, _records) = window(cx, &root, provider.clone());
    press(cx, "open-settings");
    press(cx, "settings-page-ipad-key");
    until_idle(&view, cx);
    assert_eq!(provider.reads.load(Ordering::SeqCst), 1);
    press(cx, "settings-close");
    assert!(!view.read_with(cx, |this, _| this.settings_modal));
    assert_eq!(
        provider.reads.load(Ordering::SeqCst),
        1,
        "closing reads nothing"
    );
    // The sheet remembers the page; opening it there reads once more.
    press(cx, "open-settings");
    assert_eq!(
        view.read_with(cx, |this, _| this.settings_page),
        Page::DeviceKey
    );
    until_idle(&view, cx);
    assert_eq!(provider.reads.load(Ordering::SeqCst), 2);
    assert_eq!(key_line(&view, cx).as_deref(), Some(LINE));
    // Another page reads nothing.
    press(cx, "settings-page-fonts");
    until_idle(&view, cx);
    assert_eq!(provider.reads.load(Ordering::SeqCst), 2);
    let _ = std::fs::remove_dir_all(root);
}

#[gpui::test]
fn a_failed_read_on_open_shows_the_same_failure_as_the_read_button(cx: &mut TestAppContext) {
    let root = directory("failure");
    let provider = Arc::new(Held {
        reads: AtomicUsize::new(0),
        fail: true,
        hold: Mutex::new(None),
    });
    let (view, cx, _records) = window(cx, &root, provider.clone());
    press(cx, "open-settings");
    press(cx, "settings-page-ipad-key");
    until_idle(&view, cx);
    let on_open = view.read_with(cx, |this, _| {
        let keys = &this
            .settings
            .as_ref()
            .unwrap_or_else(|| panic!("host"))
            .keys;
        (keys.failure, keys.label())
    });
    assert_eq!(on_open.0, Some(Failure::Provider));
    assert_eq!(key_line(&view, cx), None);
    // The Read button stays as the retry and lands on the same display.
    press(cx, "settings-key-read");
    until_idle(&view, cx);
    let pressed = view.read_with(cx, |this, _| {
        let keys = &this
            .settings
            .as_ref()
            .unwrap_or_else(|| panic!("host"))
            .keys;
        (keys.failure, keys.label())
    });
    assert_eq!(provider.reads.load(Ordering::SeqCst), 2);
    assert_eq!(pressed, on_open);
    let _ = std::fs::remove_dir_all(root);
}
