use crate::{
    app::Smoke,
    core::Command,
    settings::SettingsHost,
    shell::{
        HerdrKind,
        herdr2_frame::sidebar::{CoreRow, herdr2_hosts},
        sidebar::model::HostLink,
    },
};
use gpui::{AppContext, AppLifecyclePhase, Entity, TestAppContext};
use std::{
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};

const HOST: &str = "paired";
const SESSION: &str = "review-session";

fn wait_for(
    cx: &mut TestAppContext,
    message: &str,
    mut ready: impl FnMut(&mut TestAppContext) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !ready(cx) {
        assert!(Instant::now() < deadline, "{message}");
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn keep_core(cx: &mut TestAppContext, peer: &crate::core_mock::Host) -> Entity<Smoke> {
    let view = cx.new(Smoke::new);
    let (backend, _wire) = peer
        .recorded(false)
        .unwrap_or_else(|error| panic!("{error}"));
    view.update(cx, |app, cx| {
        app.terminal.core_test_backend = Some(backend);
        app.connect_named_core(HOST.into(), SESSION.into(), None, cx)
            .unwrap_or_else(|error| panic!("{error}"));
    });
    wait_for(cx, "initial core never connected", |cx| {
        view.update(cx, |app, cx| {
            app.poll_terminal(cx);
            app.terminal.live.connected
        })
    });
    view.update(cx, |app, _| app.select_host_kind(HOST, HerdrKind::Herdr1));
    view
}

fn background_and_resume(
    view: &Entity<Smoke>,
    cx: &mut TestAppContext,
    peer: &crate::core_mock::Host,
    reconnect_shell: bool,
) {
    let (backend, _wire) = peer
        .recorded(false)
        .unwrap_or_else(|error| panic!("{error}"));
    view.update(cx, |app, cx| {
        app.terminal.reconnect.automatic = reconnect_shell;
        if reconnect_shell {
            app.terminal.test_client =
                Some(crate::mock_peer::connect_board().unwrap_or_else(|error| panic!("{error}")));
        }
        app.lifecycle(AppLifecyclePhase::Background, cx);
        let hidden = app
            .board
            .hidden_core
            .get_mut(HOST)
            .unwrap_or_else(|| panic!("kept core"));
        assert!(hidden.core.is_none());
        assert!(!hidden.live.connected);
        assert!(hidden.reconnect.suspended);
        hidden.core_test_backend = Some(backend);
        app.lifecycle(AppLifecyclePhase::Foreground, cx);
        app.lifecycle(AppLifecyclePhase::Active, cx);
    });
}

#[gpui::test]
fn background_round_trip_restores_paired_core_rows_without_changing_selection(
    cx: &mut TestAppContext,
) {
    let peer = crate::core_mock::Host::default();
    let view = keep_core(cx, &peer);
    let (old_generation, selection) = view.update(cx, |app, _| {
        (
            app.board.hidden_core[HOST].link.generation,
            app.board.selection,
        )
    });
    background_and_resume(&view, cx, &peer, true);
    wait_for(
        cx,
        "kept core's connected session row did not return",
        |cx| {
            view.update(cx, |app, cx| {
                app.poll_terminal(cx);
                assert_eq!(app.board.selection, selection);
                assert_eq!(app.terminal.reconnect.target, HOST);
                assert_eq!(app.terminal.herdr_kind(), HerdrKind::Herdr1);
                let hosts = herdr2_hosts(&app.shell.sidebar, &app.terminal, &app.board.hidden_core);
                app.terminal.live.connected
                    && hosts.iter().any(|host| {
                        host.host == HOST
                            && host.link == HostLink::Connected
                            && host.generation != old_generation
                            && host.spaces.iter().any(|space| space.id == SESSION)
                    })
            })
        },
    );
    view.update(cx, |app, cx| {
        let hidden = &app.board.hidden_core[HOST];
        let boot = crate::board::snapshot(&hidden.live)
            .unwrap_or_else(|| panic!("core snapshot"))
            .boot_id
            .clone();
        let generation = hidden.link.generation;
        app.lifecycle(AppLifecyclePhase::Active, cx);
        assert_eq!(app.board.hidden_core[HOST].link.generation, generation);
        app.open_herdr2_row(HOST, &boot, generation, CoreRow::Space(SESSION.into()), cx)
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(app.terminal.herdr_kind(), HerdrKind::Herdr2);
        assert_eq!(app.terminal.link.generation, generation);
    });
}

#[gpui::test]
fn poll_hidden_updates_a_kept_core_after_return(cx: &mut TestAppContext) {
    let peer = crate::core_mock::Host::default();
    let view = keep_core(cx, &peer);
    background_and_resume(&view, cx, &peer, false);
    wait_for(
        cx,
        "poll_hidden did not publish the resumed core frame",
        |cx| {
            view.update(cx, |app, cx| {
                app.poll_hidden(cx);
                app.board.hidden_core[HOST].live.connected
            })
        },
    );
    let revision = view.update(cx, |app, _| {
        let kept = &app.board.hidden_core[HOST];
        let core = kept.core.as_ref().unwrap_or_else(|| panic!("resumed core"));
        let revision = crate::board::snapshot(&kept.live)
            .unwrap_or_else(|| panic!("resumed snapshot"))
            .revision;
        core.send(Command::NewTab)
            .unwrap_or_else(|error| panic!("{error}"));
        revision
    });
    wait_for(cx, "poll_hidden did not publish the new core tab", |cx| {
        view.update(cx, |app, cx| {
            app.poll_hidden(cx);
            assert_eq!(app.terminal.herdr_kind(), HerdrKind::Herdr1);
            assert!(crate::board::snapshot(&app.terminal.live).is_none());
            let kept = &app.board.hidden_core[HOST];
            crate::board::snapshot(&kept.live).is_some_and(|snapshot| snapshot.revision > revision)
                && kept.core.as_ref().is_some_and(|core| {
                    core.sessions(kept.live.input_epoch)
                        .iter()
                        .any(|session| session.name == SESSION && session.tabs.len() == 2)
                })
        })
    });
}

#[gpui::test]
fn leaving_destination_retires_resumed_core_and_link(cx: &mut TestAppContext) {
    let peer = crate::core_mock::Host::default();
    let view = keep_core(cx, &peer);
    background_and_resume(&view, cx, &peer, false);
    let finished = view.update(cx, |app, _| {
        let kept = &app.board.hidden_core[HOST];
        let worker = kept
            .core
            .as_ref()
            .unwrap_or_else(|| panic!("no resumed worker to retire"));
        let finished = worker.finished.clone();
        let inbox = kept.link.inbox.clone();
        let active = kept.link.notification_active.clone();
        let generation = kept.link.generation;
        app.select_host_kind("elsewhere", HerdrKind::Herdr1);
        app.terminal.reconnect.automatic = false;
        let retired = &app.board.hidden_core[HOST];
        assert!(retired.core.is_none());
        assert!(!retired.live.connected);
        assert!(!Arc::ptr_eq(&inbox, &retired.link.inbox));
        assert!(!active.load(Ordering::Acquire));
        assert_ne!(retired.link.generation, generation);
        finished
    });
    wait_for(cx, "retired core worker is still running", |_| {
        finished.load(Ordering::Acquire)
    });
    view.update(cx, |app, cx| {
        app.select_host_kind(HOST, HerdrKind::Herdr1);
        app.lifecycle(AppLifecyclePhase::Background, cx);
        let retired_generation = app.board.hidden_core[HOST].link.generation;
        app.lifecycle(AppLifecyclePhase::Active, cx);
        app.poll_hidden(cx);
        assert_eq!(
            app.board.hidden_core[HOST].link.generation,
            retired_generation
        );
        assert!(app.board.hidden_core[HOST].core.is_none());
    });
}

#[gpui::test]
fn connected_hidden_core_keeps_the_display_awake(cx: &mut TestAppContext) {
    let view = cx.new(Smoke::new);
    view.update(cx, |app, cx| {
        app.select_host_kind(HOST, HerdrKind::Herdr2);
        app.terminal.live.connected = true;
        app.select_host_kind(HOST, HerdrKind::Herdr1);
        assert!(!app.terminal.live.connected);
        app.sync_keep_awake(cx);
    });
    cx.run_until_parked();
    assert_eq!(cx.active_idle_sleep_preventions(), 1);
    view.update(cx, |app, cx| {
        app.board
            .hidden_core
            .get_mut(HOST)
            .unwrap_or_else(|| panic!("kept core"))
            .live
            .connected = false;
        app.sync_keep_awake(cx);
    });
    cx.run_until_parked();
    assert_eq!(cx.active_idle_sleep_preventions(), 0);
}

#[gpui::test]
fn hidden_core_receives_saved_input_settings(cx: &mut TestAppContext) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/hidden-core-settings-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap_or_else(|error| panic!("{error}"));
    let settings = serde_json::json!({
        "version": 1,
        "language": "en-US",
        "compact": false,
        "terminal_priority": false,
        "board_hidden": false,
        "option_key": "alt",
        "command_key": "ctrl",
        "japanese_candidates": "caret",
        "return_policy": "tap",
    });
    std::fs::write(root.join("yebisu-settings.json"), settings.to_string())
        .unwrap_or_else(|error| panic!("{error}"));
    let view = cx.new(Smoke::new);
    view.update(cx, |app, _| {
        app.select_host_kind(HOST, HerdrKind::Herdr2);
        app.select_host_kind(HOST, HerdrKind::Herdr1);
        app.settings =
            Some(SettingsHost::open(root.clone()).unwrap_or_else(|error| panic!("{error}")));
    });
    wait_for(cx, "saved settings were not loaded", |cx| {
        view.update(cx, |app, cx| {
            app.poll_settings(cx);
            app.settings.as_ref().is_some_and(SettingsHost::ready)
        })
    });
    view.update(cx, |app, _| {
        app.sync_input_settings();
        assert!(
            app.terminal.input.options.option_alt,
            "selected view loaded the file"
        );
        let hidden = &app.board.hidden_core[HOST];
        assert!(hidden.input.options.option_alt);
        assert!(hidden.input.options.command_control);
        assert!(hidden.input.options.caret_anchor);
        assert!(!hidden.reconnect.automatic);
    });
    std::fs::remove_dir_all(root).unwrap_or_else(|error| panic!("{error}"));
}
