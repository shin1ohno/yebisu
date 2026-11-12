use crate::{
    app::Smoke,
    settings::{
        SettingsHost,
        key_ops::{Failure, Owner, Provider},
    },
};
use gpui::{AppContext as _, TestAppContext};
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
struct Counted {
    injected: bool,
    reads: AtomicUsize,
    replacements: AtomicUsize,
}
impl Provider for Counted {
    fn injected(&self) -> bool {
        self.injected
    }
    fn supported(&self) -> bool {
        true
    }
    fn read_or_initialize(&self) -> Result<String, Failure> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok("restrict ssh-ed25519 RkFLRV9QMl9RQQ== herdr-ipad-injected-qa".into())
    }
    fn replace(&self) -> Result<String, Failure> {
        self.replacements.fetch_add(1, Ordering::SeqCst);
        Err(Failure::Provider)
    }
}
#[gpui::test]
fn actual_provider_preflight_precedes_read_and_refuses_missing_full_disconnected_sink(
    cx: &mut TestAppContext,
) {
    for mode in [
        "allowed",
        "not-injected",
        "full",
        "disconnected",
        "disabled",
    ] {
        let fake = Arc::new(Counted {
            injected: mode != "not-injected",
            reads: AtomicUsize::new(0),
            replacements: AtomicUsize::new(0),
        });
        let (send, receive) = mpsc::sync_channel(1);
        let mut receive = Some(receive);
        if mode == "full" {
            send.send(serde_json::json!({"kind":"positive-control"}))
                .unwrap_or_else(|e| panic!("{e}"));
        }
        if mode == "disconnected" {
            receive.take();
        }
        let view = cx.new(Smoke::new);
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/preflight-{}-{mode}",
            std::process::id()
        ));
        view.update(cx, |this, cx| {
            let mut host = SettingsHost::open(root.clone()).unwrap_or_else(|e| panic!("{e}"));
            host.keys = Owner::new(fake.clone());
            this.settings = Some(host);
            if mode != "disabled" {
                this.observation = crate::observation::Observation::injected(send);
            }
            this.read_device_key(cx);
        });
        if mode == "allowed" {
            let receipt = receive
                .as_ref()
                .unwrap_or_else(|| panic!("sink"))
                .recv_timeout(Duration::from_secs(1))
                .unwrap_or_else(|e| panic!("{e}"));
            assert_eq!(receipt["kind"], "key-provider-preflight");
            assert_eq!(receipt["phase"], "read");
            assert_eq!(receipt["injected"], true);
            let until = Instant::now() + Duration::from_secs(3);
            while view.update(cx, |this, cx| {
                this.poll_settings(cx);
                this.settings.as_ref().is_some_and(|h| h.keys.busy())
            }) {
                assert!(Instant::now() < until);
                std::thread::sleep(Duration::from_millis(2));
            }
            assert_eq!(fake.reads.load(Ordering::SeqCst), 1);
            // A previously successful preflight must not authorize another owner.
            let other = Arc::new(Counted {
                injected: false,
                reads: AtomicUsize::new(0),
                replacements: AtomicUsize::new(0),
            });
            view.update(cx, |this, cx| {
                this.settings
                    .as_mut()
                    .unwrap_or_else(|| panic!("host"))
                    .keys = Owner::new(other.clone());
                this.read_device_key(cx);
            });
            assert_eq!(other.reads.load(Ordering::SeqCst), 0);
        } else {
            assert_eq!(fake.reads.load(Ordering::SeqCst), 0, "{mode}");
            assert_eq!(fake.replacements.load(Ordering::SeqCst), 0);
            assert!(!view.read_with(cx, |this, _| {
                this.settings.as_ref().is_some_and(|h| h.keys.busy())
            }));
        }
    }
}

#[gpui::test]
fn copy_and_confirm_require_current_owner_and_admitted_preflight(cx: &mut TestAppContext) {
    for admitted in [false, true] {
        let fake = Arc::new(Counted {
            injected: true,
            reads: AtomicUsize::new(0),
            replacements: AtomicUsize::new(0),
        });
        let mut owner = Owner::new(fake.clone());
        // Seed only the explicit fake, to test copy/confirm rather than merely
        // passing because no key/ticket exists. This is not native preflight proof.
        assert!(owner.read_or_initialize());
        let until = Instant::now() + Duration::from_secs(3);
        while owner.busy() {
            owner.poll(Instant::now());
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(1));
        }
        let ticket = owner.confirmation().unwrap_or_else(|| panic!("ticket"));
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/copy-preflight-{}-{admitted}",
            std::process::id()
        ));
        let view = cx.new(Smoke::new);
        let (send, receive) = mpsc::sync_channel(4);
        if !admitted {
            for _ in 0..4 {
                send.send(serde_json::json!({"kind":"fill"}))
                    .unwrap_or_else(|e| panic!("{e}"));
            }
        }
        cx.write_to_clipboard(gpui::ClipboardItem::new_string("clipboard-control".into()));
        view.update(cx, |this, cx| {
            let mut host = SettingsHost::open(root).unwrap_or_else(|e| panic!("{e}"));
            host.keys = owner;
            this.settings = Some(host);
            this.observation = crate::observation::Observation::injected(send);
            assert_eq!(this.copy_device_key(cx), admitted);
            this.confirm_device_key(ticket, true);
            assert_eq!(
                this.settings.as_ref().is_some_and(|h| h.keys.busy()),
                admitted
            );
        });
        if admitted {
            let receipts = receive.try_iter().collect::<Vec<_>>();
            assert_eq!(
                receipts
                    .iter()
                    .map(|r| r["phase"].as_str().unwrap_or(""))
                    .collect::<Vec<_>>(),
                ["copy", "replace"]
            );
            while view.update(cx, |this, cx| {
                this.poll_settings(cx);
                this.settings.as_ref().is_some_and(|h| h.keys.busy())
            }) {
                assert!(Instant::now() < until);
                std::thread::sleep(Duration::from_millis(1));
            }
            assert_eq!(fake.replacements.load(Ordering::SeqCst), 1);
        } else {
            assert_eq!(fake.replacements.load(Ordering::SeqCst), 0);
            assert_eq!(
                cx.read_from_clipboard().and_then(|v| v.text()).as_deref(),
                Some("clipboard-control")
            );
        }
    }
}
