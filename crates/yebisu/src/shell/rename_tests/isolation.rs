//! A rename waiting for its result shares nothing with navigation (issue
//! AC12): `live.pending_request` stays as it is, a move and a palette
//! command still run, and the terminal's input is left as it is. (The sheprd client queues a request behind one the
//! host never answers, so the moves here run on the answering peer while
//! the rename's answer is not yet read.)
use super::*;
use crate::shell::search::tests::mark_text;
use crate::{board::Target, mock_peer};

fn silent(cx: &mut gpui::TestAppContext) -> (Entity<Smoke>, Receipts, &mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(gpui::size(gpui::px(1200.), gpui::px(1000.)));
    let (client, receipts) =
        mock_peer::connect_recorded_fault("rename-silent").unwrap_or_else(|e| panic!("peer: {e}"));
    view.update(cx, |this, _| {
        this.select_host("A");
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
        this.terminal.open = true;
    });
    wait(&view, cx, "private peer did not become ready", |this| {
        this.terminal.live.surface.is_some()
            && crate::board::snapshot(&this.terminal.live).is_some()
    });
    (view, receipts, cx)
}

#[gpui::test]
fn pending_request_untouched(cx: &mut gpui::TestAppContext) {
    let (view, receipts, cx) = silent(cx);
    let fake = install_fake(&view, cx);
    opened(&view, &fake, cx, space("w1"), RenameAnchor::SpaceRow);
    commit(&fake, cx, "ship");
    assert_eq!(renames(&view, &receipts, cx).len(), 1, "fixture: sent");
    view.read_with(cx, |this, _| {
        assert!(this.shell.rename.pending.is_some(), "fixture: waiting");
        let inbox = this.terminal.link.inbox.try_lock().map(|live| {
            (
                live.pending_request.clone(),
                live.rename.as_ref().map(|t| t.request.clone()),
            )
        });
        let (pending, rename) = inbox.unwrap_or_else(|_| panic!("inbox busy"));
        assert_eq!(pending, None, "the rename used navigation's slot");
        assert!(rename.is_some(), "the rename has its own ticket");
        assert!(this.terminal.navigating.is_none());
    });
}

#[gpui::test]
fn navigation_runs_while_rename_pending(cx: &mut gpui::TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    methods(&view, &receipts, cx);
    opened(&view, &fake, cx, space("w1"), RenameAnchor::SpaceRow);
    push(&fake, cx, FieldEvent::Committed("ship".into()));
    let moved = view.update(cx, |this, _| {
        assert!(
            this.shell
                .rename
                .pending
                .as_ref()
                .is_some_and(|p| p.sending()),
            "fixture: the rename waits for its answer"
        );
        let (boot, generation) = (
            crate::board::snapshot(&this.terminal.live)
                .map(|s| s.boot_id.clone())
                .unwrap_or_default(),
            this.terminal.link.generation,
        );
        this.navigate_board("A", Target::Space("w2".into()), &boot, generation)
    });
    assert!(
        moved.is_ok(),
        "a move was refused while a rename waited: {moved:?}"
    );
    assert_eq!(
        methods(&view, &receipts, cx),
        ["workspace.rename", "workspace.focus"]
    );
}

#[gpui::test]
fn palette_runs_while_pending(cx: &mut gpui::TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    methods(&view, &receipts, cx);
    opened(&view, &fake, cx, space("w1"), RenameAnchor::SpaceRow);
    push(&fake, cx, FieldEvent::Committed("ship".into()));
    let ran = view.update(cx, |this, _| {
        assert!(
            this.shell
                .rename
                .pending
                .as_ref()
                .is_some_and(|p| p.sending()),
            "fixture: the rename waits for its answer"
        );
        this.run_action(sheprd_commands::Command::Tab)
    });
    assert!(ran.is_ok(), "a palette command was refused: {ran:?}");
    assert_eq!(
        methods(&view, &receipts, cx),
        ["workspace.rename", "tab.create"]
    );
}

/// The terminal is not the rename's channel (issue: `terminal.input` does
/// not change): a composition started after the field closed survives the
/// host's answer and nothing of the rename reaches the pane.
#[gpui::test]
fn terminal_input_untouched(cx: &mut gpui::TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    sent(&view, &receipts, cx);
    opened(&view, &fake, cx, space("w1"), RenameAnchor::SpaceRow);
    commit(&fake, cx, "ship");
    assert_eq!(phase(&view, cx), None, "fixture: the field closed");
    mark_text(&view, cx, "にほ");
    let before = view.read_with(cx, |this, _| {
        assert!(
            this.shell
                .rename
                .pending
                .as_ref()
                .is_some_and(|p| p.sending()),
            "fixture: the answer is read after the composition starts"
        );
        assert_eq!(this.terminal.input.composition.text, "にほ", "fixture");
        (
            this.terminal.input.retirement_revision(),
            this.terminal.navigating.is_none(),
        )
    });
    assert!(
        sent(&view, &receipts, cx).is_empty(),
        "the rename reached the pane"
    );
    wait(&view, cx, "the host did not answer the rename", |this| {
        this.shell.rename.pending.is_none()
    });
    view.read_with(cx, |this, _| {
        assert_eq!(
            this.terminal.input.composition.text, "にほ",
            "the rename's answer dropped the terminal's composition"
        );
        assert_eq!(
            (
                this.terminal.input.retirement_revision(),
                this.terminal.navigating.is_none(),
            ),
            before,
            "the rename's answer changed the terminal's input or navigation"
        );
    });
}
