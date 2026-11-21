use crate::{
    app::Smoke,
    settings::{
        SettingsHost,
        key_ops::{Failure, Owner, Provider, State},
    },
    shell::add_host::SheetMode,
};
use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext, px, size};
use std::{
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

const HOST: &str = "qa@127.0.0.1:1";
const OLD_KEY: &str = "restrict ssh-ed25519 RkFLRV9QMl9RQQ== herdr-ipad-old";
const CURRENT_KEY: &str = "restrict ssh-ed25519 RkFLRV9QMl9RQg== herdr-ipad-current";
const EXPECTED_COPY: &str = "restrict ssh-ed25519 RkFLRV9QMl9RQg== yebisu-current";
const TIMEOUT: Duration = Duration::from_secs(2);

struct InjectedKey {
    key: Mutex<Result<&'static str, Failure>>,
    finish_replace: Mutex<mpsc::Receiver<()>>,
    reads: mpsc::Sender<()>,
}

impl Provider for InjectedKey {
    fn injected(&self) -> bool {
        true
    }

    fn supported(&self) -> bool {
        true
    }

    fn read_or_initialize(&self) -> Result<String, Failure> {
        let key = *self.key.lock().map_err(|_| Failure::Worker)?;
        self.reads.send(()).map_err(|_| Failure::Worker)?;
        key.map(str::to_owned)
    }

    fn replace(&self) -> Result<String, Failure> {
        self.finish_replace
            .lock()
            .map_err(|_| Failure::Worker)?
            .recv_timeout(TIMEOUT)
            .map_err(|_| Failure::Worker)?;
        *self.key.lock().map_err(|_| Failure::Worker)? = Ok(CURRENT_KEY);
        Ok(CURRENT_KEY.to_owned())
    }
}

struct Fixture {
    provider: Arc<InjectedKey>,
    reads: mpsc::Receiver<()>,
    finish_replace: mpsc::Sender<()>,
    observations: mpsc::Receiver<serde_json::Value>,
    root: std::path::PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn window<'a>(
    cx: &'a mut TestAppContext,
    key: Result<&'static str, Failure>,
    name: &str,
) -> (Fixture, Entity<Smoke>, &'a mut VisualTestContext) {
    let (read, reads) = mpsc::channel();
    let (finish_replace, replace_gate) = mpsc::channel();
    let (record, observations) = mpsc::sync_channel(256);
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .join(format!("open-key-{}-{name}", std::process::id()));
    let provider = Arc::new(InjectedKey {
        key: Mutex::new(key),
        finish_replace: Mutex::new(replace_gate),
        reads: read,
    });
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1376.), px(1032.)));
    view.update(cx, |this, _| {
        let mut host = SettingsHost::open(root.clone()).unwrap_or_else(|error| panic!("{error}"));
        host.keys = Owner::new(provider.clone());
        this.settings = Some(host);
        this.observation = crate::observation::Observation::injected(record);
        this.setup.hosts.push(HOST.to_owned());
    });
    (
        Fixture {
            provider,
            reads,
            finish_replace,
            observations,
            root,
        },
        view,
        cx,
    )
}

fn keys(this: &mut Smoke) -> &mut Owner {
    &mut this
        .settings
        .as_mut()
        .unwrap_or_else(|| panic!("settings are available"))
        .keys
}

fn pump(view: &Entity<Smoke>, cx: &mut VisualTestContext, fixture: &Fixture) {
    cx.run_until_parked();
    cx.background_executor
        .advance_clock(Duration::from_millis(33));
    view.update(cx, |this, cx| this.poll_settings(cx));
    cx.run_until_parked();
    fixture.observations.try_iter().for_each(drop);
}

fn await_state(
    view: &Entity<Smoke>,
    cx: &mut VisualTestContext,
    fixture: &Fixture,
    expected: State,
) {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        pump(view, cx, fixture);
        if view.update(cx, |this, _| {
            keys(this).state == expected && !keys(this).busy()
        }) {
            break;
        }
        assert!(Instant::now() < deadline, "key never reached {expected:?}");
        std::thread::yield_now();
    }
}

fn assert_copy(cx: &mut VisualTestContext, expected: &str) {
    let copy = cx
        .debug_bounds("key-copy")
        .unwrap_or_else(|| panic!("step 1 Copy button"));
    cx.simulate_click(copy.center(), Modifiers::default());
    assert_eq!(
        cx.read_from_clipboard()
            .and_then(|item| item.text())
            .as_deref(),
        Some(expected)
    );
}

#[gpui::test]
fn saved_host_reads_existing_device_key(cx: &mut TestAppContext) {
    let (fixture, view, cx) = window(cx, Ok(CURRENT_KEY), "existing");
    view.update(cx, |this, cx| {
        this.hosts_open_add(SheetMode::Open, HOST, cx)
    });
    await_state(&view, cx, &fixture, State::Ready);
    assert_eq!(fixture.reads.try_iter().count(), 1);
    assert_copy(cx, EXPECTED_COPY);
}

#[gpui::test]
fn saved_host_shows_loading_until_sign_in_finishes_then_reads_key(cx: &mut TestAppContext) {
    let (fixture, view, cx) = window(cx, Err(Failure::Missing), "loading");
    view.update(cx, |this, cx| {
        this.hosts_open_add(SheetMode::Open, HOST, cx)
    });
    // Draw synchronously so the background sign-in task cannot deliver its reply yet.
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    assert!(cx.debug_bounds("device-key-sign-in-progress").is_some());
    assert!(fixture.reads.try_recv().is_err(), "read raced sign-in");
    assert!(!view.update(cx, |this, _| keys(this).busy()));
    *fixture
        .provider
        .key
        .lock()
        .unwrap_or_else(|error| panic!("{error}")) = Ok(CURRENT_KEY);
    await_state(&view, cx, &fixture, State::Ready);
    assert!(cx.debug_bounds("device-key-sign-in-progress").is_none());
    assert_eq!(fixture.reads.try_iter().count(), 1);
    assert_copy(cx, EXPECTED_COPY);
}

#[gpui::test]
fn saved_host_shows_guidance_if_device_key_read_fails(cx: &mut TestAppContext) {
    let (fixture, view, cx) = window(cx, Err(Failure::Missing), "missing");
    view.update(cx, |this, cx| {
        this.hosts_open_add(SheetMode::Open, HOST, cx)
    });
    let deadline = Instant::now() + TIMEOUT;
    loop {
        pump(&view, cx, &fixture);
        if cx.debug_bounds("device-key-guidance").is_some() {
            break;
        }
        assert!(Instant::now() < deadline, "step 1 did not show guidance");
        std::thread::yield_now();
    }
    assert_eq!(fixture.reads.try_iter().count(), 1);
    let copy = cx
        .debug_bounds("key-copy")
        .unwrap_or_else(|| panic!("Copy button"));
    cx.simulate_click(copy.center(), Modifiers::default());
    assert!(cx.read_from_clipboard().is_none());
}

fn start_replace(view: &Entity<Smoke>, cx: &mut VisualTestContext, fixture: &Fixture) {
    view.update(cx, |this, cx| this.read_device_key(cx));
    await_state(view, cx, fixture, State::Ready);
    assert_eq!(fixture.reads.try_iter().count(), 1);
    view.update(cx, |this, _| {
        let keys = keys(this);
        let ticket = keys
            .confirmation()
            .unwrap_or_else(|| panic!("replacement ticket"));
        assert!(keys.confirm(ticket, true, false));
        assert_eq!(keys.state, State::Submitted);
    });
}

#[gpui::test]
fn saved_host_reads_current_device_key_after_replace(cx: &mut TestAppContext) {
    let (fixture, view, cx) = window(cx, Ok(OLD_KEY), "replaced");
    start_replace(&view, cx, &fixture);
    fixture
        .finish_replace
        .send(())
        .unwrap_or_else(|error| panic!("{error}"));
    await_state(&view, cx, &fixture, State::Unknown);
    view.update(cx, |this, cx| {
        this.hosts_open_add(SheetMode::Open, HOST, cx)
    });
    await_state(&view, cx, &fixture, State::Ready);
    assert_eq!(fixture.reads.try_iter().count(), 1);
    assert_copy(cx, EXPECTED_COPY);
}

#[gpui::test]
fn saved_host_waits_for_unknown_replacement_to_finish_before_reading(cx: &mut TestAppContext) {
    let (fixture, view, cx) = window(cx, Ok(OLD_KEY), "unknown");
    start_replace(&view, cx, &fixture);
    view.update(cx, |this, cx| {
        let keys = keys(this);
        assert!(keys.poll(Instant::now() + Duration::from_secs(16)));
        assert_eq!(keys.state, State::Unknown);
        this.hosts_open_add(SheetMode::Open, HOST, cx);
    });
    pump(&view, cx, &fixture);
    assert!(fixture.reads.try_recv().is_err(), "read raced replacement");
    assert!(view.update(cx, |this, _| keys(this).busy()));
    fixture
        .finish_replace
        .send(())
        .unwrap_or_else(|error| panic!("{error}"));
    await_state(&view, cx, &fixture, State::Ready);
    assert_eq!(fixture.reads.try_iter().count(), 1);
    assert_copy(cx, EXPECTED_COPY);
}
