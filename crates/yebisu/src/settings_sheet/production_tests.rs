//! Host-shaped production route/owner tests; never instantiate the live key provider.
use crate::{
    app::Smoke,
    settings::{
        SettingsHost,
        key_ops::{Failure, Owner, Provider, State},
    },
};
use gpui::{TestAppContext, px, size};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
const RAW: &str = "restrict ssh-ed25519 RkFLRV9QVUJMSUNfS0VZX1NFTlRJTkVM herdr-ipad-injected";
struct Injected {
    replaces: AtomicUsize,
    reject: AtomicBool,
}
impl Provider for Injected {
    fn supported(&self) -> bool {
        true
    }
    fn read_or_initialize(&self) -> Result<String, Failure> {
        if self.reject.load(Ordering::SeqCst) {
            Ok("FAKE_PRIVATE_SENTINEL".into())
        } else {
            Ok(RAW.into())
        }
    }
    fn replace(&self) -> Result<String, Failure> {
        self.replaces.fetch_add(1, Ordering::SeqCst);
        Ok(RAW.into())
    }
}
fn directory(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../target/p11-{name}-{}", std::process::id()))
}

#[gpui::test]
fn p11_production_font_button_persists_real_consumer(cx: &mut TestAppContext) {
    let root = directory("font-button");
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1100.), px(900.)));
    view.update(cx, |this, _| {
        this.settings = Some(SettingsHost::open(root.clone()).unwrap_or_else(|e| panic!("{e}")));
        this.settings_fonts = vec!["SauceCodePro Nerd Font Mono".into()];
    });
    let until = Instant::now() + Duration::from_secs(5);
    while !view.update(cx, |this, cx| {
        this.poll_settings(cx);
        this.settings.as_ref().is_some_and(SettingsHost::ready)
    }) {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(2));
    }
    cx.run_until_parked();
    let open = cx
        .debug_bounds("open-settings")
        .unwrap_or_else(|| panic!("production entry"));
    cx.simulate_click(open.center(), Default::default());
    cx.run_until_parked();
    let panel = cx
        .debug_bounds("settings-panel")
        .unwrap_or_else(|| panic!("production sheet"));
    assert_eq!(panel.size, size(px(1000.), px(760.)));
    let plus = cx
        .debug_bounds("font-plus")
        .unwrap_or_else(|| panic!("font control"));
    cx.simulate_click(plus.center(), Default::default());
    view.update(cx, |this, cx| {
        assert_eq!(this.terminal.font.size, Some(15.));
        this.settings_lifecycle(gpui::AppLifecyclePhase::Background);
        this.poll_settings(cx);
    });
    while !view.update(cx, |this, cx| {
        this.poll_settings(cx);
        this.settings.as_ref().is_some_and(SettingsHost::ready)
    }) {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(2));
    }
    let json: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join("yebisu-settings.json")).unwrap_or_else(|e| panic!("{e}")),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(json["terminal_font_size"], 15.);
    let close = cx
        .debug_bounds("settings-close")
        .unwrap_or_else(|| panic!("close"));
    cx.simulate_click(close.center(), Default::default());
    assert!(!view.read_with(cx, |this, _| this.settings_modal));
    std::fs::remove_dir_all(root).unwrap_or_else(|e| panic!("{e}"));
}

#[gpui::test]
fn p11_f01_f03_f07_f18_f24_f25_actual_buttons_single_owner_and_nonsecret_receipts(
    cx: &mut TestAppContext,
) {
    let root = directory("keys-buttons");
    let fake = Arc::new(Injected {
        replaces: AtomicUsize::new(0),
        reject: AtomicBool::new(false),
    });
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1100.), px(900.)));
    let (logs, records) = std::sync::mpsc::sync_channel(256);
    view.update(cx, |this, _| {
        let mut host = SettingsHost::open(root.clone()).unwrap_or_else(|e| panic!("{e}"));
        host.keys = Owner::new(fake.clone());
        this.settings = Some(host);
        this.settings_fonts = vec!["SauceCodePro Nerd Font Mono".into()];
        this.observation = crate::observation::Observation::injected(logs);
        this.observation
            .record(serde_json::json!({"kind":"p11-positive-control"}));
    });
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.begin_settings_modal(window, cx);
            this.settings_page = crate::settings_pages::Page::DeviceKey;
        })
    });
    cx.run_until_parked();
    let read = cx
        .debug_bounds("settings-key-read")
        .unwrap_or_else(|| panic!("read"));
    cx.simulate_click(read.center(), Default::default());
    let until = Instant::now() + Duration::from_secs(5);
    while view.update(cx, |this, cx| {
        this.poll_settings(cx);
        this.settings.as_ref().is_some_and(|h| h.keys.busy())
    }) {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(2));
    }
    cx.run_until_parked();
    let replace = cx
        .debug_bounds("settings-key-replace")
        .unwrap_or_else(|| panic!("replace"));
    cx.simulate_click(replace.center(), Default::default());
    cx.run_until_parked();
    let cancel = cx
        .debug_bounds("settings-key-cancel")
        .unwrap_or_else(|| panic!("cancel"));
    cx.simulate_click(cancel.center(), Default::default());
    assert_eq!(fake.replaces.load(Ordering::SeqCst), 0);
    cx.simulate_click(replace.center(), Default::default());
    cx.run_until_parked();
    let confirm = cx
        .debug_bounds("settings-key-confirm")
        .unwrap_or_else(|| panic!("confirm"));
    cx.simulate_click(confirm.center(), Default::default());
    view.update(cx, |this, _| {
        assert!(this.settings.as_ref().and_then(|h| h.keys.line()).is_none());
        this.select_host("injected-other-host");
        assert!(this.settings.as_ref().and_then(|h| h.keys.line()).is_none());
    });
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.end_settings_modal(window, cx);
            this.begin_settings_modal(window, cx);
        })
    });
    while view.update(cx, |this, cx| {
        this.poll_settings(cx);
        this.settings.as_ref().is_some_and(|h| h.keys.busy())
    }) {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(fake.replaces.load(Ordering::SeqCst), 1);
    view.update(cx, |this, _| {
        assert_eq!(
            this.settings.as_ref().map(|h| h.keys.state),
            Some(State::Unknown)
        );
    });
    cx.run_until_parked();
    fake.reject.store(true, Ordering::SeqCst);
    view.update(cx, |this, cx| this.read_device_key(cx));
    while view.update(cx, |this, cx| {
        this.poll_settings(cx);
        this.settings.as_ref().is_some_and(|h| h.keys.busy())
    }) {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(2));
    }
    cx.run_until_parked();
    let output = records
        .try_iter()
        .map(|r| r.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(output.contains("p11-positive-control"));
    assert!(!output.contains("RkFLRV9QVUJMSUNfS0VZX1NFTlRJTkVM"));
    assert!(!output.contains("FAKE_PRIVATE_SENTINEL"));
    assert!(!output.contains("key_line"));
    let leaked = format!("{output}FAKE_PRIVATE_SENTINEL");
    assert!(
        leaked.contains("FAKE_PRIVATE_SENTINEL"),
        "negative detector control"
    );
}
