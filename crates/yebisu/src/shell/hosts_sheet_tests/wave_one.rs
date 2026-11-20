use super::*;
use crate::settings::{
    SettingsHost,
    key_ops::{Failure, Owner, Provider},
};
use gpui::{px, size};
use std::{
    sync::{Arc, mpsc},
    time::Duration,
};

#[gpui::test]
fn add_host_header_buttons_are_text_buttons(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    cx.simulate_resize(size(px(1376.), px(1032.)));
    view.update(cx, |this, cx| this.hosts_open_add(SheetMode::Add, "", cx));
    cx.run_until_parked();
    let panel = cx
        .debug_bounds("hosts-panel")
        .unwrap_or_else(|| panic!("Add host panel"));
    let header = cx
        .debug_bounds("add-host-header")
        .unwrap_or_else(|| panic!("Add host header"));
    let cancel = cx
        .debug_bounds("hosts-add-cancel")
        .unwrap_or_else(|| panic!("Cancel"));
    let connect = cx
        .debug_bounds("hosts-add-connect")
        .unwrap_or_else(|| panic!("Connect"));
    assert_eq!(panel.top(), px(96.));
    assert_eq!(header.size.height, px(60.));
    assert_eq!(cancel.size, size(px(88.), px(44.)));
    assert_eq!(connect.size, cancel.size);
    assert_eq!(cancel.center().y, connect.center().y);
    assert!(cx.debug_bounds("host-auth").is_none());
    cx.simulate_resize(size(px(670.), px(560.)));
    cx.run_until_parked();
    let narrow = cx
        .debug_bounds("hosts-panel")
        .unwrap_or_else(|| panic!("resized Add host"));
    assert_eq!(narrow.top(), px(16.));
}

struct InjectedKey(mpsc::Sender<()>);

impl Provider for InjectedKey {
    fn injected(&self) -> bool {
        true
    }

    fn supported(&self) -> bool {
        true
    }

    fn read_or_initialize(&self) -> Result<String, Failure> {
        self.0.send(()).map_err(|_| Failure::Worker)?;
        Ok("restrict ssh-ed25519 RkFLRV9QMl9RQQ== herdr-ipad-injected-qa".into())
    }

    fn replace(&self) -> Result<String, Failure> {
        panic!("opening Add host must never replace a key")
    }
}

#[gpui::test]
fn device_key_is_read_when_the_sheet_opens(cx: &mut TestAppContext) {
    for manual_open in [false, true] {
        let (read, reads) = mpsc::channel();
        let (record, records) = mpsc::sync_channel(128);
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target")
            .join(format!("wave-one-key-{}-{manual_open}", std::process::id()));
        let (view, cx) = window(cx);
        view.update(cx, |this, cx| {
            let mut host =
                SettingsHost::open(root.clone()).unwrap_or_else(|error| panic!("{error}"));
            host.keys = Owner::new(Arc::new(InjectedKey(read)));
            this.settings = Some(host);
            this.observation = crate::observation::Observation::injected(record);
            if manual_open {
                this.hosts_open_add(SheetMode::Add, "", cx);
            } else {
                this.setup.loaded = true;
                cx.notify();
            }
        });
        cx.run_until_parked();
        assert!(
            reads.recv_timeout(Duration::from_secs(2)).is_ok(),
            "Add host did not read on open"
        );
        assert!(
            records
                .try_iter()
                .any(|event| event["phase"] == "read" && event["injected"] == true)
        );
        view.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        assert!(reads.try_recv().is_err(), "a redraw started a second read");
        assert!(cx.debug_bounds("key-create").is_none());
        // Finish the injected operation before leaving the view.
        let until = std::time::Instant::now() + Duration::from_secs(2);
        while view.update(cx, |this, cx| {
            this.poll_settings(cx);
            this.settings.as_ref().is_some_and(|host| host.keys.busy())
        }) {
            assert!(std::time::Instant::now() < until);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(view.read_with(cx, |this, _| {
            this.settings
                .as_ref()
                .and_then(|host| host.keys.line())
                .is_some()
        }));
        let _ = std::fs::remove_dir_all(root);
    }
}
