//! UIApplicationMain consumer probe. Selector injection is not queued OS input.
//! Frame scheduling drives this test only; production recovery never uses it.
use crate::app::Smoke;
use gpui::{App, AsyncWindowContext, ClipboardItem, Entity, Window};
use herdr_uikit::retirement_probe::{self as native, NativeClient};
use sheprd_client::protocol::{ClientMessage, ClientPaneInputEvent};
use std::sync::mpsc::Receiver;

struct Probe {
    context: AsyncWindowContext,
    entity: Entity<Smoke>,
    receipts: Receiver<ClientMessage>,
    events: Vec<ClientPaneInputEvent>,
}
impl Probe {
    fn update<R>(
        &mut self,
        f: impl FnOnce(&mut Smoke, &mut Window, &mut gpui::Context<Smoke>) -> R,
    ) -> R {
        let entity = self.entity.clone();
        self.context
            .update(|window, cx| entity.update(cx, |this, cx| f(this, window, cx)))
            .unwrap_or_else(|e| panic!("consumer native probe update: {e}"))
    }
    fn current(&self) -> NativeClient {
        native::capture(self.context.window_handle())
            .unwrap_or_else(|| panic!("native client absent"))
    }
}

pub(crate) fn start(entity: &Entity<Smoke>, window: &mut Window, cx: &mut App) {
    let (client, receipts) = crate::mock_peer::connect_recorded(1)
        .unwrap_or_else(|e| panic!("consumer native peer: {e}"));
    entity.update(cx, |this, cx| {
        this.terminal
            .reconnect
            .begin("retirement-private-peer".into());
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
        this.terminal.open = true;
        this.terminal.needs_focus = true;
        this.bind_text_retirement(window, cx);
        cx.notify();
    });
    let probe = Probe {
        context: window.to_async(cx),
        entity: entity.clone(),
        receipts,
        events: Vec::new(),
    };
    let mut context = probe.context.clone();
    context.on_next_frame(move |_, _| native::next_native_turn(move || ready(probe, 0)));
}

fn ready(mut probe: Probe, tries: usize) {
    assert!(
        tries < 300,
        "consumer native fixture/focus did not become ready"
    );
    let fixture_ready = probe.update(|this, window, cx| {
        this.poll_terminal(cx);
        this.terminal.size = None;
        this.refresh_terminal_input();
        this.terminal_focus.focus(window, cx);
        window.request_virtual_keyboard();
        this.terminal.input.ready
    });
    if !fixture_ready || !probe.current().first_responder() {
        let mut context = probe.context.clone();
        context
            .on_next_frame(move |_, _| native::next_native_turn(move || ready(probe, tries + 1)));
        return;
    }
    let old = probe.current();
    probe.update(|this, window, cx| {
        this.publish_native_edits(window);
        cx.write_to_clipboard(ClipboardItem::new_string("native-paste".into()));
    });
    old.mark("same😀");
    probe.update(|this, _, _| assert_eq!(this.terminal.input.composition.text, "same😀"));
    // One real native Edit selector; no synthetic hardware key admission.
    let context = probe.context.clone();
    context
        .spawn(async move |_| {
            assert!(native::edit(
                probe.context.window_handle(),
                herdr_uikit::NativeEditAction::Paste,
                false
            ));
            probe.update(|this, _, _| {
                assert!(
                    this.terminal.input.requires_gesture(),
                    "queued is not acknowledgement"
                )
            });
            native::next_native_turn(move || replacement(probe, old, 0));
        })
        .detach();
}

fn replacement(mut probe: Probe, old: NativeClient, tries: usize) {
    assert!(tries < 60, "replacement did not acquire native responder");
    let state = herdr_uikit::text_input_session_state(probe.context.window_handle())
        .unwrap_or_else(|| panic!("native state absent"));
    if state.replacement_pending || state.client == old.identity() || !state.first_responder {
        let mut context = probe.context.clone();
        context.on_next_frame(move |_, _| {
            native::next_native_turn(move || replacement(probe, old, tries + 1))
        });
        return;
    }
    assert!(!old.attached());
    assert!(!old.first_responder());
    assert_eq!(old.availability(false), Some(([false; 4], [false; 4])));
    old.mark("retired-old-mark");
    old.insert("retired-old-commit");
    old.unmark();
    old.delete();
    old.edit_actions();
    probe.update(|this, _, _| {
        this.terminal.size = None;
        this.refresh_terminal_input();
        assert!(
            !this.terminal.input.requires_gesture(),
            "scoped callback did not reopen owner"
        );
        assert!(
            this.terminal.input.composition.text.is_empty(),
            "old selector leaked"
        );
    });
    let current = probe.current();
    // Identical old/new text prevents a text-equality filter from passing.
    current.mark("same😀");
    probe.update(|this, _, _| assert_eq!(this.terminal.input.composition.text, "same😀"));
    current.insert("same😀");
    native_unmark(&mut probe, &current);
    wait_paste(probe, current, 0);
}

fn native_unmark(probe: &mut Probe, current: &NativeClient) {
    let revision = probe.update(|this, window, _| {
        assert!(this.terminal_focus.is_focused(window), "UNMARK_SETUP focus");
        assert!(this.terminal.input.ready, "UNMARK_SETUP ready");
        assert!(!this.handoff.blocks_input(), "UNMARK_SETUP handoff");
        assert!(
            !this.terminal.input.requires_gesture(),
            "UNMARK_SETUP fence"
        );
        this.terminal.input.discard_revision()
    });
    current.mark("b1-unmark");
    probe.update(|this, _, _| {
        assert_eq!(
            this.terminal.input.composition.text, "b1-unmark",
            "UNMARK_SETUP mark"
        );
    });
    // Native unmark commits through replace_text_in_range. It does not call
    // Smoke::unmark_text (the separate consumer cancellation callback).
    current.unmark();
    current.unmark(); // No remaining marked range: must not commit twice.
    probe.update(|this, _, _| {
        assert!(
            !this.terminal.input.requires_gesture(),
            "UNMARK_COMMIT invented fence"
        );
        assert!(
            this.terminal.input.composition.text.is_empty(),
            "UNMARK_COMMIT retained text"
        );
        assert_eq!(
            this.terminal.input.discard_revision(),
            revision,
            "UNMARK_COMMIT discard revision"
        );
        assert_eq!(this.retirement.reason, None, "UNMARK_COMMIT refusal");
    });
    assert_eq!(
        probe.current().identity(),
        current.identity(),
        "UNMARK_COMMIT client changed"
    );
    eprintln!("CONSUMER_UNMARK_COMMIT_LOCAL peer_assertion_pending=true");
}

fn wait_paste(mut probe: Probe, current: NativeClient, tries: usize) {
    assert!(tries < 300, "first paste worker did not finish");
    let busy = probe.update(|this, _, _| this.handoff.workers != 0);
    if busy {
        let mut context = probe.context.clone();
        context.on_next_frame(move |_, _| {
            native::next_native_turn(move || wait_paste(probe, current, tries + 1))
        });
        return;
    }
    native_paste(probe, current);
}

fn native_paste(mut probe: Probe, current: NativeClient) {
    probe.update(|_, _, cx| {
        cx.write_to_clipboard(ClipboardItem::new_string("hardware-paste".into()))
    });
    probe.update(|this, window, _| {
        this.terminal.size = None;
        this.refresh_terminal_input();
        this.sync_handoff();
        assert!(this.terminal_focus.is_focused(window), "PASTE_SETUP focus");
        assert!(this.terminal.input.ready, "PASTE_SETUP ready");
        assert!(this.handoff.stamp.is_some(), "PASTE_SETUP destination");
        assert_eq!(this.handoff.workers, 0, "PASTE_SETUP workers");
        assert!(!this.handoff.blocks_input(), "PASTE_SETUP handoff");
        assert!(!this.terminal.input.requires_gesture(), "PASTE_SETUP fence");
        assert!(
            this.handoff
                .cancellation
                .as_ref()
                .is_none_or(|c| c.is_finished()),
            "PASTE_SETUP cancellation"
        );
    });
    current.mark("b1-paste");
    probe.update(|this, _, _| {
        assert_eq!(
            this.terminal.input.composition.text, "b1-paste",
            "PASTE_SETUP mark"
        );
    });
    // SDK calls real copy:/cut:/paste:/selectAll: selectors. With no selection,
    // only Paste changes the receiving terminal; native origin reaches the app.
    assert!(native::empty_hardware_entry(probe.context.window_handle()));
    assert!(native::edit(
        probe.context.window_handle(),
        herdr_uikit::NativeEditAction::Paste,
        false
    ));
    probe.update(|this, _, _| {
        assert!(this.terminal.input.requires_gesture());
        assert_eq!(
            this.retirement.reason,
            Some("Text input paused · hardware recovery unsupported by SDK")
        );
    });
    current.insert("b1-old-paste");
    current.mark("b1-paste-new-composition");
    probe.update(|this, _, _| assert!(this.terminal.input.composition.text.is_empty()));
    assert_eq!(probe.current().identity(), current.identity());
    eprintln!(
        "CONSUMER_HARDWARE_REFUSED entry=empty-hardware-entry->nativePaste->start_paste new_composition_rejected=true"
    );
    drain(probe, false, 0);
}

fn drain(mut probe: Probe, mut sentinel_sent: bool, tries: usize) {
    assert!(
        tries < 300,
        "consumer receiving peer did not reach sentinel"
    );
    let preparing = probe.update(|this, _, _| {
        this.terminal.size = None;
        this.refresh_terminal_input();
        this.flush_terminal();
        assert!(
            this.terminal.input.requires_gesture(),
            "frame/poll unexpectedly reopened B1"
        );
        this.handoff.preparing
    });
    if !preparing && !sentinel_sent {
        probe.update(|this, _, _| {
            this.terminal
                .link
                .handle
                .as_ref()
                .unwrap_or_else(|| panic!("peer handle"))
                .send_input(
                    "boot-v1",
                    "w1:p1",
                    [ClientPaneInputEvent::Paste("retirement-end".into())],
                )
                .unwrap_or_else(|e| panic!("sentinel {e}"));
        });
        sentinel_sent = true;
    }
    while let Ok(message) = probe.receipts.try_recv() {
        if let ClientMessage::ClientShellPaneInput { pane_id, events } = message {
            assert_eq!(pane_id, "w1:p1");
            for event in events {
                if event == ClientPaneInputEvent::Paste("retirement-end".into()) {
                    assert_eq!(
                        probe.events,
                        [
                            ClientPaneInputEvent::Paste("native-paste".into()),
                            ClientPaneInputEvent::TextCommit("same😀".into()),
                            ClientPaneInputEvent::TextCommit("b1-unmark".into()),
                            ClientPaneInputEvent::Paste("hardware-paste".into()),
                        ],
                        "PEER_SEQUENCE: native paste, fresh commit, unmark commit, hardware-context paste exactly once"
                    );
                    eprintln!(
                        "CONSUMER_NATIVE_PARTIAL_PASS events={:?} LIMITED_EDIT_PASS HARDWARE_UNSUPPORTED B2_UNVERIFIED",
                        probe.events
                    );
                    probe.update(|this, _, _| {
                        this.observation.record(serde_json::json!({
                            "kind":"consumer-native-retirement-partial", "new_text_once":true,
                            "old_text_absent":true, "native_unmark_once":true, "native_paste_once":true,
                            "B1":"limited-edit-subset", "hardware":"unsupported", "B2":"unverified", "Z1":"not-passed"
                        }))
                    });
                    return;
                }
                probe.events.push(event);
            }
        }
    }
    let mut context = probe.context.clone();
    context.on_next_frame(move |_, _| {
        native::next_native_turn(move || drain(probe, sentinel_sent, tries + 1))
    });
}
