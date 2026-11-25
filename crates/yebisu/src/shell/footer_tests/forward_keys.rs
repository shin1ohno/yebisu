//! Issue 140: while the forward sheet is open, hardware keys and typed text
//! go to its port field; neither a KeyDown nor typed text reaches the
//! terminal or the remote shell (the same kind of input as the sidebar
//! filter's, TODO.md). Closing the sheet gives the keys back.
use super::*;
use crate::shell::footer::keys::KeyOwner;
use sheprd_client::protocol::ClientPaneInputEvent;

fn pane_input(
    view: &Entity<Smoke>,
    cx: &mut VisualTestContext,
    wire: &mpsc::Receiver<ClientMessage>,
) -> Vec<ClientPaneInputEvent> {
    view.update(cx, |this, _| {
        this.terminal.size = None;
        this.refresh_terminal_input();
        this.flush_terminal();
    });
    cx.run_until_parked();
    drain(wire, Duration::from_millis(300))
        .into_iter()
        .flat_map(|message| match message {
            ClientMessage::ClientShellPaneInput { events, .. } => events,
            _ => Vec::new(),
        })
        .collect()
}

#[gpui::test]
fn forward_sheet_keys_never_reach_terminal(cx: &mut TestAppContext) {
    let (view, cx, wire) = connected(cx);
    view.update(cx, |this, _| this.setup.hosts = vec!["sandbox".into()]);
    cx.update(|window, cx| view.update(cx, |this, cx| this.terminal_focus.focus(window, cx)));
    cx.run_until_parked();
    drain(&wire, Duration::from_millis(100));
    // Positive control: with the terminal focused, a letter reaches the shell.
    cx.simulate_keystrokes("a");
    assert!(
        !pane_input(&view, cx, &wire).is_empty(),
        "positive control: the letter must reach the shell"
    );
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.open_forward_sheet("sandbox".into(), window, cx)
        })
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            assert_eq!(this.key_owner(), KeyOwner::ForwardSheet);
            assert!(this.input_modal());
            assert!(!this.terminal_owns_text_input());
            assert!(
                !this.terminal_focus.is_focused(window),
                "terminal kept focus"
            );
            let field = this
                .shell
                .forward_sheet
                .field
                .clone()
                .unwrap_or_else(|| panic!("no port field"));
            assert!(
                field.read(cx).focus_handle().is_focused(window),
                "the port field has no focus"
            );
        })
    });
    cx.simulate_keystrokes("l s enter ctrl-c");
    // Typed text (IME commits, insertText:) lands in the field: letters are
    // refused there, digits kept.
    cx.simulate_input("ls");
    cx.simulate_input("12");
    cx.run_until_parked();
    let after = pane_input(&view, cx, &wire);
    assert!(
        after.is_empty(),
        "input reached the remote shell while the forward sheet was open: {after:?}"
    );
    view.update(cx, |this, cx| {
        assert!(
            this.forward_sheet_open(),
            "Return on an empty field keeps the sheet"
        );
        let text = this
            .shell
            .forward_sheet
            .field
            .as_ref()
            .map(|field| field.read(cx).text().to_owned());
        assert_eq!(text.as_deref(), Some("12"), "the field keeps digits only");
    });
    cx.update(|window, cx| view.update(cx, |this, cx| this.close_forward_sheet(window, cx)));
    cx.run_until_parked();
    cx.update(|window, cx| {
        view.update(cx, |this, _| {
            assert_eq!(this.key_owner(), KeyOwner::Terminal);
            assert!(this.terminal_owns_text_input());
            assert!(this.terminal_focus.is_focused(window), "focus not returned");
        })
    });
    cx.simulate_keystrokes("z");
    assert!(
        !pane_input(&view, cx, &wire).is_empty(),
        "keys did not return to the terminal"
    );
}
