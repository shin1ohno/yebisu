use super::*;
use crate::{mock_peer, terminal_input::TerminalInput};
use gpui::{AppContext, TestAppContext};
use sheprd_client::ClientEvent;
use std::sync::Arc;

fn live() -> Live {
    let mut live = Live::default();
    live.connected = true;
    live.apply(ClientEvent::Snapshot(Arc::new(
        serde_json::from_str(mock_peer::SNAPSHOT).unwrap_or_else(|e| panic!("fixture {e}")),
    )));
    live.apply(ClientEvent::Surface(Arc::new(mock_peer::surface(7))));
    live
}
#[test]
fn per_host_generation_does_not_reuse_previous_attempt() {
    let mut state = Reconnect::default();
    assert_eq!(state.begin("a@host".into()), 1);
    assert_eq!(state.begin("b@host".into()), 1);
    assert!(!state.current("a@host", 1));
    assert_eq!(state.begin("a@host".into()), 2);
    assert!(!state.current("a@host", 1));
    assert!(!state.current("b@host", 1));
    assert!(state.current("a@host", 2));
}
#[test]
fn reconnect_discards_held_input_marked_text_and_disconnected_typing() {
    let mut input = TerminalInput::default();
    input.sync(&live(), 1, None);
    input.ready = false;
    let event = input
        .commit("queued")
        .unwrap_or_else(|| panic!("ready target"));
    input.queue(event);
    input.mark("旧かな", Some(0..1));
    input.sync(&Live::default(), 2, None);
    assert!(input.composition.text.is_empty());
    assert!(input.commit("typed while disconnected").is_none());
    input.mark("disconnected composition", None);
    input.sync(&live(), 3, None);
    assert!(input.next().is_none());
    assert!(input.composition.text.is_empty());
    assert!(input.commit("late old commit").is_none());
    input.fresh_gesture();
    assert!(input.commit("fresh input").is_some());
}
#[gpui::test]
fn lifecycle_isolates_old_inbox_and_does_not_redial_in_manual_mode(cx: &mut TestAppContext) {
    let view = cx.new(Smoke::new);
    view.update(cx, |this, cx| {
        this.terminal.open = true;
        this.terminal.reconnect.begin("a@host".into());
        this.terminal.reconnect.automatic = false;
        this.terminal.live = live();
        this.terminal.input.sync(&this.terminal.live, 0, None);
        this.terminal.input.mark("古い変換", None);
        let old = this.terminal.link.inbox.clone();
        this.lifecycle(AppLifecyclePhase::Inactive, cx);
        let generation = this.terminal.link.generation;
        this.lifecycle(AppLifecyclePhase::Background, cx);
        assert_eq!(this.terminal.link.generation, generation);
        old.lock()
            .unwrap_or_else(|e| panic!("lock {e}"))
            .apply(ClientEvent::Surface(Arc::new(mock_peer::surface(7))));
        this.lifecycle(AppLifecyclePhase::Active, cx);
        this.poll_terminal(cx);
        assert_eq!(this.terminal.link.generation, generation);
        assert!(this.terminal.live.surface.is_none());
        assert!(this.terminal.input.composition.text.is_empty());
        assert!(this.terminal.input.commit("late").is_none());
    });
}
#[gpui::test]
fn version_mismatch_never_automatically_reconnects(cx: &mut TestAppContext) {
    let view = cx.new(Smoke::new);
    view.update(cx, |this, cx| {
        this.terminal.open = true;
        this.terminal.live.version_mismatch =
            Some(sheprd_client::VersionMismatch::DaemonOutdated {
                server_version: Some("0.8".into()),
            });
        this.lifecycle(AppLifecyclePhase::Background, cx);
        let generation = this.terminal.link.generation;
        this.lifecycle(AppLifecyclePhase::Active, cx);
        assert_eq!(this.terminal.link.generation, generation);
    });
}
