use super::*;
use gpui::{AppContext as _, AppLifecyclePhase, TestAppContext};
use std::{
    fs,
    time::{Duration, Instant},
};

fn directory(name: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/settings-tests")
        .join(format!(
            "yebisu-settings-{}-{name}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ))
}
fn wait(host: &mut SettingsHost, condition: impl Fn(&SettingsHost) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        host.poll();
        if condition(host) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "settings completion timed out: {:?}",
            host.error
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[gpui::test]
fn background_interrupt_flushes_through_real_adapter(cx: &mut TestAppContext) {
    let root = directory("background");
    let mut host = SettingsHost::open(root.clone()).unwrap_or_else(|e| panic!("{e}"));
    wait(&mut host, |h| h.loaded);
    let view = cx.new(crate::app::Smoke::new);
    view.update(cx, |this, cx| {
        this.settings = Some(host);
        this.terminal.compact = true;
        // Do not call the regular poll: UIKit interrupts between edit and timer.
        this.lifecycle(AppLifecyclePhase::Inactive, cx);
        this.actions.terminal_priority = true;
        this.lifecycle(AppLifecyclePhase::Background, cx);
        let Some(host) = &mut this.settings else {
            panic!("missing host")
        };
        wait(host, SettingsHost::ready);
    });
    assert_eq!(
        store::load(&root).unwrap_or_else(|e| panic!("{e}")),
        Values {
            compact: true,
            terminal_priority: true,
            board_hidden: false
        }
    );
    fs::remove_dir_all(root).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn corrupt_disk_is_preserved_and_not_overwritten() {
    let root = directory("corrupt");
    fs::create_dir_all(&root).unwrap_or_else(|e| panic!("{e}"));
    for bytes in [
        b"{broken".as_slice(),
        br#"{"version":2}"#,
        br#"{"version":1,"language":"en-US","compact":"yes"}"#,
    ] {
        fs::write(root.join("yebisu-settings.json"), bytes).unwrap_or_else(|e| panic!("{e}"));
        let mut host = SettingsHost::open(root.clone()).unwrap_or_else(|e| panic!("{e}"));
        wait(&mut host, |h| h.loaded);
        host.edit(Values {
            compact: true,
            ..Values::default()
        });
        host.lifecycle(AppLifecyclePhase::Background);
        host.poll();
        // FIFO barrier: the worker has finished every preceding Save, even
        // when the corrupt-file guard is deliberately disabled in mutation QA.
        let (ack, handled) = mpsc::sync_channel(1);
        host.sender
            .send(Request::Barrier(ack))
            .unwrap_or_else(|e| panic!("{e}"));
        handled
            .recv_timeout(Duration::from_secs(3))
            .unwrap_or_else(|e| panic!("worker barrier: {e}"));
        assert_eq!(
            fs::read(root.join("yebisu-settings.json")).unwrap_or_else(|e| panic!("{e}")),
            bytes
        );
        host.poll();
        assert!(host.error.is_some());
        assert!(!host.ready());
        assert_eq!(
            fs::read(root.join("yebisu-settings.json")).unwrap_or_else(|e| panic!("{e}")),
            bytes
        );
    }
    fs::remove_dir_all(root).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn stale_completion_cannot_acknowledge_or_overwrite_new_draft() {
    let root = directory("stale");
    let mut host = SettingsHost::open(root.clone()).unwrap_or_else(|e| panic!("{e}"));
    wait(&mut host, |h| h.loaded);
    host.edit(Values {
        compact: true,
        ..Values::default()
    });
    host.flush();
    host.edit(Values {
        terminal_priority: true,
        ..Values::default()
    });
    wait(&mut host, SettingsHost::ready);
    let newest = host.draft.clone();
    host.complete(1, Ok(()));
    assert!(host.ready());
    assert_eq!(host.draft, newest);
    assert_eq!(host.saved, 2);
    assert_eq!(store::load(&root).unwrap_or_else(|e| panic!("{e}")), newest);
    // Simulated termination before rename leaves the old complete file intact.
    fs::write(root.join("yebisu-settings.pending"), b"partial").unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(store::load(&root).unwrap_or_else(|e| panic!("{e}")), newest);
    fs::remove_dir_all(root).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn confirmation_is_explicit_one_shot_and_invalidated_by_edit() {
    let root = directory("confirmation");
    let mut host = SettingsHost::open(root).unwrap_or_else(|e| panic!("{e}"));
    assert!(!host.take_regeneration_confirmation());
    host.confirm_regeneration();
    assert!(host.take_regeneration_confirmation());
    assert!(!host.take_regeneration_confirmation());
    host.confirm_regeneration();
    host.edit(Values {
        compact: true,
        ..Values::default()
    });
    assert!(!host.take_regeneration_confirmation());
}

#[gpui::test]
fn restored_density_survives_first_connection_and_host_switch(cx: &mut TestAppContext) {
    let root = directory("host-switch");
    store::save(
        &root,
        &Values {
            compact: true,
            ..Values::default()
        },
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let host = SettingsHost::open(root.clone()).unwrap_or_else(|e| panic!("{e}"));
    let view = cx.new(crate::app::Smoke::new);
    view.update(cx, |this, _| {
        this.settings = Some(host);
        let deadline = Instant::now() + Duration::from_secs(3);
        while !this.terminal.compact {
            this.poll_settings();
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        }
        this.select_host("one");
        assert!(this.terminal.compact);
        this.select_host("two");
        this.poll_settings();
        assert!(this.terminal.compact);
    });
    fs::remove_dir_all(root).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn failed_write_retries_after_active_or_edit() {
    for active in [true, false] {
        let root = directory("retry");
        store::save(&root, &Values::default()).unwrap_or_else(|e| panic!("{e}"));
        let mut host = SettingsHost::open(root.clone()).unwrap_or_else(|e| panic!("{e}"));
        wait(&mut host, |h| h.loaded);
        fs::create_dir(root.join("yebisu-settings.pending")).unwrap_or_else(|e| panic!("{e}"));
        host.edit(Values {
            compact: true,
            ..Values::default()
        });
        host.flush();
        wait(&mut host, |h| h.error.is_some());
        assert_eq!(
            store::load(&root).unwrap_or_else(|e| panic!("{e}")),
            Values::default()
        );
        fs::remove_dir(root.join("yebisu-settings.pending")).unwrap_or_else(|e| panic!("{e}"));
        if active {
            host.lifecycle(AppLifecyclePhase::Active);
        } else {
            host.edit(Values {
                compact: true,
                board_hidden: true,
                ..Values::default()
            });
        }
        wait(&mut host, SettingsHost::ready);
        assert_eq!(
            store::load(&root).unwrap_or_else(|e| panic!("{e}")),
            host.draft
        );
        fs::remove_dir_all(root).unwrap_or_else(|e| panic!("{e}"));
    }
}

#[gpui::test]
fn edit_before_first_load_preserves_untouched_disk_fields(cx: &mut TestAppContext) {
    let root = directory("load-race");
    store::save(
        &root,
        &Values {
            compact: false,
            terminal_priority: true,
            board_hidden: true,
        },
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let host = SettingsHost::open(root.clone()).unwrap_or_else(|e| panic!("{e}"));
    assert!(!host.loaded);
    let view = cx.new(crate::app::Smoke::new);
    view.update(cx, |this, _| {
        this.settings = Some(host);
        this.terminal.compact = true;
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            this.poll_settings();
            if this.settings.as_ref().is_some_and(SettingsHost::ready) {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(
            this.settings_values(),
            Values {
                compact: true,
                terminal_priority: true,
                board_hidden: true
            }
        );
    });
    assert_eq!(
        store::load(&root).unwrap_or_else(|e| panic!("{e}")),
        Values {
            compact: true,
            terminal_priority: true,
            board_hidden: true
        }
    );
    fs::remove_dir_all(root).unwrap_or_else(|e| panic!("{e}"));
}
