use super::{super::nearby::MAX_DEVICES, *};
use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext, size};
use std::{
    sync::mpsc::{self, Sender},
    thread,
    time::{Duration, Instant},
};

const TIMEOUT: Duration = Duration::from_secs(2);

struct FakeBrowser {
    snapshots: Sender<(Status, Vec<Device>)>,
    stopped: mpsc::Receiver<()>,
    worker: thread::ThreadId,
}

fn open(cx: &mut TestAppContext) -> (FakeBrowser, Entity<Smoke>, &mut VisualTestContext) {
    let (snapshots, incoming) = mpsc::channel::<(Status, Vec<Device>)>();
    let (stop, stopped) = mpsc::channel();
    let (started, worker) = mpsc::channel();
    let (entity, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1376.), px(1032.)));
    entity.update(cx, |this, cx| {
        this.hosts_open_add(SheetMode::Add, "", cx);
        this.start_nearby_with(
            move |mailbox| {
                let _ = started.send(thread::current().id());
                while !mailbox.cancelled() {
                    match incoming.recv_timeout(Duration::from_millis(5)) {
                        Ok((status, devices)) => mailbox.publish(status, devices),
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
                let _ = stop.send(());
            },
            cx,
        );
    });
    let worker = worker
        .recv_timeout(TIMEOUT)
        .unwrap_or_else(|error| panic!("{error}"));
    (
        FakeBrowser {
            snapshots,
            stopped,
            worker,
        },
        entity,
        cx,
    )
}

fn device(name: &str, address: &str) -> Device {
    Device {
        name: name.into(),
        address: address.into(),
    }
}

fn pump(cx: &mut VisualTestContext, mut received: impl FnMut(&mut VisualTestContext) -> bool) {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        cx.run_until_parked();
        cx.background_executor.advance_clock(POLL_INTERVAL);
        cx.run_until_parked();
        if received(cx) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "fake browser update did not reach the sheet"
        );
        thread::sleep(Duration::from_millis(1));
    }
}

#[gpui::test]
fn nearby_rows_appear_and_disappear(cx: &mut TestAppContext) {
    let (fake, _, cx) = open(cx);
    assert_ne!(fake.worker, thread::current().id());
    fake.snapshots
        .send((Status::WaitingForPermission, vec![]))
        .unwrap_or_else(|error| panic!("{error}"));
    fake.snapshots
        .send((Status::Searching, vec![device("Studio", "studio.local")]))
        .unwrap_or_else(|error| panic!("{error}"));
    pump(cx, |cx| cx.debug_bounds("nearby-device-0").is_some());
    assert_eq!(
        cx.debug_bounds("nearby-device-0")
            .map(|row| row.size.height),
        Some(px(56.))
    );
    fake.snapshots
        .send((Status::Unavailable, vec![]))
        .unwrap_or_else(|error| panic!("{error}"));
    pump(cx, |cx| cx.debug_bounds("nearby-device-0").is_none());
}

#[gpui::test]
fn nearby_tap_fills_host_and_name_but_preserves_hand_typed_name(cx: &mut TestAppContext) {
    let (fake, view, cx) = open(cx);
    view.update(cx, |this, cx| {
        this.setup_input.set_text("alex@".into());
        cx.notify();
    });
    fake.snapshots
        .send((
            Status::Finished,
            vec![
                device("Mini", "mini.local"),
                device("Studio", "studio.local:2222"),
            ],
        ))
        .unwrap_or_else(|error| panic!("{error}"));
    pump(cx, |cx| cx.debug_bounds("nearby-device-1").is_some());
    let first = cx
        .debug_bounds("nearby-device-0")
        .unwrap_or_else(|| panic!("first row missing"));
    cx.simulate_click(first.center(), Modifiers::default());
    view.update(cx, |this, cx| {
        assert_eq!(this.setup_input.text, "alex@mini.local");
        assert_eq!(
            this.shell
                .hosts_sheet
                .name
                .as_ref()
                .and_then(|field| field.read(cx).name_to_save("alex@mini.local"))
                .as_deref(),
            Some("Mini")
        );
        assert!(!this.setup.busy);
        assert!(this.setup.pending.is_none() && !this.terminal.open);
    });

    let name = cx
        .debug_bounds("host-name-field")
        .unwrap_or_else(|| panic!("Name field missing"));
    cx.simulate_click(name.center(), Modifiers::default());
    cx.simulate_keystrokes("cmd-a");
    cx.simulate_input("Workshop");
    let second = cx
        .debug_bounds("nearby-device-1")
        .unwrap_or_else(|| panic!("second row missing"));
    cx.simulate_click(second.center(), Modifiers::default());
    view.update(cx, |this, cx| {
        assert_eq!(this.setup_input.text, "alex@studio.local:2222");
        assert_eq!(
            this.shell
                .hosts_sheet
                .name
                .as_ref()
                .and_then(|field| field.read(cx).name_to_save("alex@studio.local:2222"))
                .as_deref(),
            Some("Workshop")
        );
        assert!(!this.setup.busy);
        assert!(this.setup.pending.is_none() && !this.terminal.open);
    });
}

#[gpui::test]
fn closing_add_host_stops_browsing(cx: &mut TestAppContext) {
    let (fake, _, cx) = open(cx);
    fake.snapshots
        .send((Status::Searching, vec![device("Studio", "studio.local")]))
        .unwrap_or_else(|error| panic!("{error}"));
    pump(cx, |cx| cx.debug_bounds("nearby-device-0").is_some());
    let cancel = cx
        .debug_bounds("hosts-add-cancel")
        .unwrap_or_else(|| panic!("Cancel is missing"));
    cx.simulate_click(cancel.center(), Modifiers::default());
    fake.stopped
        .recv_timeout(TIMEOUT)
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(cx.debug_bounds("nearby-device-0").is_none());
}

#[gpui::test]
fn nearby_snapshot_caps_rows_and_excludes_saved_hosts(cx: &mut TestAppContext) {
    let (fake, view, cx) = open(cx);
    view.update(cx, |this, _| {
        this.setup.hosts.push("other@mac-00.local".into())
    });
    let devices = (0..MAX_DEVICES + 8)
        .map(|index| device(&format!("Mac {index:02}"), &format!("mac-{index:02}.local")))
        .collect();
    fake.snapshots
        .send((Status::Finished, devices))
        .unwrap_or_else(|error| panic!("{error}"));
    pump(cx, |cx| cx.debug_bounds("nearby-device-0").is_some());
    view.update(cx, |this, _| {
        let sheet = &this.shell.hosts_sheet.nearby;
        assert_eq!(sheet.snapshot.devices.len(), MAX_DEVICES);
        assert_eq!(
            sheet.visible_devices(&this.setup.hosts).count(),
            MAX_DEVICES - 1
        );
    });
}
