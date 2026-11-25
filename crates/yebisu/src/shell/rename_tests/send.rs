//! One request per commit, in herdr-gpui's own shapes (issue AC6, AC9).
use super::*;
use crate::shell::rename::{
    FIND_OPEN, HERDR2, NO_METHOD, NOT_CONNECTED, OTHER_HOST, RenameTarget, SHEET_OPEN, WAITING,
};

#[gpui::test]
fn shapes(cx: &mut gpui::TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    renames(&view, &receipts, cx);
    for (object, anchor, expected) in [
        (
            space("w1"),
            RenameAnchor::SpaceRow,
            r#"workspace.rename {"workspace_id":"w1","label":"build"}"#,
        ),
        (
            tab("w1:t1"),
            RenameAnchor::TabLabel,
            r#"tab.rename {"tab_id":"w1:t1","label":"build"}"#,
        ),
        (
            pane("w1:p1"),
            RenameAnchor::PaneChip,
            r#"pane.rename {"pane_id":"w1:p1","label":"build"}"#,
        ),
    ] {
        opened(&view, &fake, cx, object, anchor);
        commit(&fake, cx, "build");
        assert_eq!(
            renames(&view, &receipts, cx),
            [expected],
            "exactly one request in the B-7 shape"
        );
    }
}

#[gpui::test]
fn one_request_per_commit(cx: &mut gpui::TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    renames(&view, &receipts, cx);
    opened(&view, &fake, cx, space("w2"), RenameAnchor::SpaceRow);
    push(&fake, cx, FieldEvent::Committed("  ship\nit  ".into()));
    // A late second commit (the SDK sends one) changes nothing.
    push(&fake, cx, FieldEvent::Committed("again".into()));
    close(&fake, cx, FieldDismiss::Committed, FieldRestored::Text);
    assert_eq!(
        renames(&view, &receipts, cx),
        [r#"workspace.rename {"workspace_id":"w2","label":"ship it"}"#]
    );
}

#[gpui::test]
fn pane_empty_sends_empty_label(cx: &mut gpui::TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    renames(&view, &receipts, cx);
    opened(&view, &fake, cx, pane("w1:p1"), RenameAnchor::PaneChip);
    let request = fake.request();
    assert!(request.allow_empty, "a pane may be emptied");
    assert_eq!(request.placeholder, "Automatic name");
    commit(&fake, cx, "");
    assert_eq!(
        renames(&view, &receipts, cx),
        [r#"pane.rename {"pane_id":"w1:p1","label":""}"#]
    );
}

#[gpui::test]
fn tab_space_empty_never_sent(cx: &mut gpui::TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    renames(&view, &receipts, cx);
    for (object, anchor) in [
        (tab("w1:t1"), RenameAnchor::TabLabel),
        (space("w1"), RenameAnchor::SpaceRow),
    ] {
        opened(&view, &fake, cx, object, anchor);
        assert!(
            !fake.request().allow_empty,
            "a tab or space is never emptied"
        );
        commit(&fake, cx, "   ");
    }
    assert!(
        renames(&view, &receipts, cx).is_empty(),
        "an empty name was sent"
    );
}

#[gpui::test]
fn unchanged_sends_nothing(cx: &mut gpui::TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    let fake = install_fake(&view, cx);
    renames(&view, &receipts, cx);
    opened(&view, &fake, cx, space("w1"), RenameAnchor::SpaceRow);
    assert_eq!(
        fake.request().text,
        "repo",
        "the field starts with the name"
    );
    commit(&fake, cx, "repo");
    assert!(renames(&view, &receipts, cx).is_empty());
}

/// Each refusal of issue §C, in its order: an earlier one wins.
#[gpui::test]
fn refusal_texts_in_order(cx: &mut gpui::TestAppContext) {
    let (view, _receipts, cx) = terminal(cx);
    view.update(cx, |this, _| {
        let target = this
            .rename_target(space("w1"))
            .unwrap_or_else(|| panic!("fixture: target"));
        assert_eq!(this.rename_unavailable(&target), None, "fixture: renamable");
        let other_host = RenameTarget {
            host: "B".into(),
            ..target.clone()
        };
        assert_eq!(this.rename_unavailable(&other_host), Some(OTHER_HOST));
        // A Herdr 2 pane is refused before the host is (decision 8); a Herdr 2
        // space or tab is the document's and goes on to the host (#144 D3).
        let herdr2 = RenameTarget {
            kind: crate::shell::HerdrKind::Herdr2,
            object: pane("w1:p1"),
            ..other_host.clone()
        };
        assert_eq!(this.rename_unavailable(&herdr2), Some(HERDR2));
        let herdr2_space = RenameTarget {
            kind: crate::shell::HerdrKind::Herdr2,
            ..other_host.clone()
        };
        assert_eq!(this.rename_unavailable(&herdr2_space), Some(OTHER_HOST));
        let gone = RenameTarget {
            object: space("w9"),
            ..target.clone()
        };
        assert_eq!(
            this.rename_unavailable(&gone),
            Some("This space was closed.")
        );
        this.terminal
            .live
            .methods
            .retain(|m| m != "workspace.rename");
        assert_eq!(this.rename_unavailable(&target), Some(NO_METHOD));
        this.terminal.live.connected = false;
        assert_eq!(this.rename_unavailable(&target), Some(NOT_CONNECTED));
        this.settings_modal = true;
        assert_eq!(this.rename_unavailable(&herdr2), Some(SHEET_OPEN));
        this.settings_modal = false;
        this.terminal.live.connected = true;
        this.terminal.live.methods.push("workspace.rename".into());
        this.shell.rename.pending = Some(crate::shell::rename::PendingLabel {
            boot: target.boot.clone(),
            object: target.object.clone(),
            text: "x".into(),
            sent_revision: 0,
            doc: false,
            request: "r".into(),
            state: crate::shell::rename::send::PendingState::Sending,
            sent_at: std::time::Instant::now(),
        });
        assert_eq!(this.rename_unavailable(&target), Some(WAITING));
    });
    let _ = FIND_OPEN;
}
