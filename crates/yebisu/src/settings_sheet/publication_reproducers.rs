//! Ignored, expected-failing F19 acceptance reproducers; never a leak-as-PASS test.
use crate::{
    app::Smoke,
    core::{
        Command, Session,
        publication_probe::{AfterCompare, Control, Journal, Key, Peer, Transcript, WAIT},
    },
};
use gpui::{Entity, EntityInputHandler, TestAppContext, VisualTestContext};
use serde_json::json;
use sheprd_backend::ClientPaneInputEvent;
use sheprd_core_client::frame::Kind;
use std::{
    sync::{atomic::Ordering, mpsc},
    time::{Duration, Instant},
};
const POSITIVE: &str = "F19_SAME_CONNECTION_POSITIVE";
const FORBIDDEN: &str = "F19_UNPUBLISHED_BEFORE_MODAL";
const TAIL: &str = "F19_QUEUED_TAIL";

#[gpui::test]
#[ignore = "requires explicit lease; expected FAIL on current product at final zero-leak assertion"]
fn f19_a_after_compare_reproducer(cx: &mut TestAppContext) {
    reproduce(cx, false);
}

#[gpui::test]
#[ignore = "requires explicit lease; expected FAIL on current product at final zero-leak assertion"]
fn f19_b0_after_dequeue_reproducer(cx: &mut TestAppContext) {
    reproduce(cx, true);
}

fn send_text(view: &Entity<Smoke>, cx: &mut VisualTestContext, text: &str) {
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            assert!(!this.settings_modal);
            this.terminal.size = None;
            this.terminal_focus.focus(window, cx);
            this.refresh_terminal_input();
            this.terminal.input.fresh_gesture();
            this.replace_text_in_range(None, text, window, cx);
        })
    });
}
fn reproduce(cx: &mut TestAppContext, dequeue: bool) {
    let source_head = option_env!("YEBISU_PROBE_SOURCE_HEAD")
        .unwrap_or_else(|| panic!("setup requires compile-time YEBISU_PROBE_SOURCE_HEAD and external clean-source/build receipt"));
    assert!(
        source_head.len() == 40 && source_head.bytes().all(|b| b.is_ascii_hexdigit()),
        "setup requires an exact commit, not a branch or short hash"
    );
    let journal = Journal::new(if dequeue { "P11-B0" } else { "P11-A" });
    let control = Control::default();
    let (backend, peer) = Peer::connect(journal.clone(), control.clone())
        .unwrap_or_else(|e| panic!("peer setup: {e}"));
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    view.update(cx, |this, _| {
        // Direct disposable backend installation avoids native/auth/persistent host paths.
        this.select_host("f19-disposable-probe");
        this.terminal.core_mode = true;
        let generation = this.terminal.link.reset_for_core();
        this.terminal.core = Some(
            Session::start(
                "f19-disposable-probe".into(),
                generation,
                this.terminal.link.inbox.clone(),
                move || Ok(backend),
                None,
            )
            .unwrap_or_else(|e| panic!("session: {e}")),
        );
        this.terminal.open = true;
        this.terminal.size = None;
        this.settings_fonts = vec!["SauceCodePro Nerd Font Mono".into()];
    });
    let deadline = Instant::now() + WAIT;
    loop {
        if view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            this.terminal.live.connected && this.terminal.live.surface.is_some()
        }) {
            break;
        }
        assert!(Instant::now() < deadline, "fixture never became coherent");
        std::thread::sleep(Duration::from_millis(2));
    }
    send_text(&view, cx, POSITIVE);
    let positive = peer
        .received
        .recv_timeout(WAIT)
        .unwrap_or_else(|e| panic!("same-connection positive absent: {e}"));
    assert_eq!(
        (positive.channel, positive.payload.as_slice()),
        (1, POSITIVE.as_bytes())
    );
    let key = view.update(cx, |this, _| {
        let destination =
            crate::terminal_input::destination(&this.terminal.live, this.terminal.link.generation)
                .unwrap_or_else(|| panic!("destination"));
        let epoch = this
            .terminal
            .core
            .as_ref()
            .unwrap_or_else(|| panic!("core"))
            .probe_input_epoch();
        journal.bind(destination, epoch)
    });
    journal
        .record("positive-confirmed", key.json())
        .unwrap_or_else(|e| panic!("{e}"));
    let (arrived, release) = if dequeue {
        control.arm(key.clone(), FORBIDDEN.as_bytes())
    } else {
        let (entered, arrived) = mpsc::sync_channel(1);
        let (release, resume) = mpsc::sync_channel(1);
        view.update(cx, |this, _| {
            this.terminal
                .core
                .as_ref()
                .unwrap_or_else(|| panic!("core"))
                .send(Command::ArmPublicationProbe(Box::new(AfterCompare {
                    key: key.clone(),
                    event: ClientPaneInputEvent::TextCommit(FORBIDDEN.into()),
                    arrived: entered,
                    resume,
                    journal: journal.clone(),
                })))
                .unwrap_or_else(|e| panic!("arm: {e}"))
        });
        (arrived, release)
    };
    send_text(&view, cx, FORBIDDEN);
    assert_eq!(
        arrived
            .recv_timeout(WAIT)
            .unwrap_or_else(|e| panic!("exact boundary absent: {e}")),
        key
    );
    // More semantic input while the selected write is held. A's tail is still
    // before its worker compare; B0's can already reside in the SDK queue.
    send_text(&view, cx, TAIL);
    let modal_order = cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.replace_and_mark_text_in_range(None, "old composition", Some(0..1), window, cx);
            this.begin_settings_modal(window, cx);
            assert!(this.settings_modal);
            assert!(this.terminal.input.composition.text.is_empty());
            let epoch = this
                .terminal
                .core
                .as_ref()
                .unwrap_or_else(|| panic!("core"))
                .probe_input_epoch();
            assert!(epoch > key.input_epoch);
            assert_eq!(this.handoff.clipboard_reads, 0);
            journal
                .record(
                    "modal-observed-open",
                    json!({"key":key.json(),"observed_input_epoch":epoch}),
                )
                .unwrap_or_else(|e| panic!("{e}"))
        })
    });
    release
        .send(key.clone())
        .unwrap_or_else(|e| panic!("release: {e}"));
    // All UI producers remain fenced. The ordered consumer command is behind
    // both semantic inputs. Its reply is only one part of final settlement.
    let (finished, settled) = mpsc::sync_channel(1);
    let producer = view.update(cx, |this, _| {
        assert!(this.settings_modal);
        let core = this
            .terminal
            .core
            .as_ref()
            .unwrap_or_else(|| panic!("core"));
        core.send(Command::FinishPublicationProbe(key.clone(), finished))
            .unwrap_or_else(|e| panic!("finish: {e}"));
        core.finished.clone()
    });
    assert_eq!(
        settled
            .recv_timeout(WAIT)
            .unwrap_or_else(|e| panic!("ordered producer/RPC absent: {e}")),
        key
    );
    let deadline = Instant::now() + WAIT;
    while !producer.load(Ordering::Acquire) {
        assert!(
            Instant::now() < deadline,
            "consumer producer did not finish"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    journal
        .record("consumer-producer-finished", key.json())
        .unwrap_or_else(|e| panic!("{e}"));
    let transcript: Transcript = peer.finish();
    assert!(transcript.receiver_eof && !transcript.marker_id.is_null());
    assert!(view.read_with(cx, |this, _| this.settings_modal));
    let rows = journal.rows();
    assert!(
        rows.iter()
            .any(|r| r["kind"] == "transport-dropped" && r["data"]["complete"] == true)
    );
    let received: Vec<_> = transcript
        .frames
        .iter()
        .filter(|f| f.channel == key.channel && f.kind == Kind::Data)
        .map(|f| f.payload.clone())
        .collect();
    let frames: Vec<_> = transcript
        .frames
        .iter()
        .enumerate()
        .map(|(index, frame)| {
            json!({"receiver_sequence":index+1,"channel":frame.channel,
               "kind":u8::from(frame.kind),"payload":frame.payload})
        })
        .collect();
    print_receipt(
        source_head,
        &key,
        modal_order,
        &rows,
        &received,
        transcript.marker_id,
        &frames,
    );
    // This is the ORIGINAL zero-leak requirement. Current production is expected
    // to FAIL here, with FORBIDDEN after the modal observation. Never invert it.
    assert_eq!(
        received,
        [POSITIVE.as_bytes().to_vec()],
        "F19 PRODUCT FAILURE: unpublished input reached peer while modal stayed open; inspect keyed full transcript above"
    );
}
fn print_receipt(
    source_head: &str,
    key: &Key,
    modal: u64,
    rows: &[serde_json::Value],
    received: &[Vec<u8>],
    marker: serde_json::Value,
    frames: &[serde_json::Value],
) {
    println!(
        "{}",
        json!({"kind":"F19-test-only-reproducer","key":key.json(),
        "source_head":source_head,
        "source_binding":"requires external exact-head clean-source/build receipt",
        "sdk_commit":"398c132a06723a0cc1494842a34666d173ec0200",
        "modal_event_order":modal,"publication_event_order":"writer-accepted/frame-complete rows",
        "writer_sequence":"per-frame rows","accepted_offsets":"writer-accepted rows",
        "receiver_sequence":"receiver-frame rows","ordered_list_id":marker,
        "final_settlement_status":"producer finished + transport dropped complete + receiver EOF/join",
        "events":rows,"receiver_frames":frames,"actual_terminal_frames":received,"product_acceptance":false})
    );
}
