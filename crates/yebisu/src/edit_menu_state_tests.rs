use super::*;
use crate::{
    edit_menu::{EditMenuTicket, SelectionKey, menu_action_allowed},
    handoff::Selection,
};
use std::sync::Arc;

#[gpui::test]
fn h9_h10_every_consumer_revocation_rejects_before_clipboard_and_without_redraw(
    cx: &mut TestAppContext,
) {
    for cause in 0..6 {
        let (view, cx, calls) = fixture(cx);
        let at = position(&view, cx);
        hold(cx, at);
        touch(cx, TouchPhase::Ended, at);
        cx.update(|window, cx| {
            view.update(cx, |this, cx| {
                match cause {
                    0 => this.board.selection += 1,
                    1 => this.terminal.open = false,
                    2 => this.terminal.reconnect.suspended = true,
                    3 => this.actions.palette = true,
                    4 => this.focus.focus(window, cx),
                    _ => {
                        let frame = this
                            .terminal
                            .live
                            .surface
                            .as_ref()
                            .unwrap_or_else(|| panic!("surface"))
                            .frame
                            .clone();
                        this.handoff.selection = Some(Selection {
                            frame: Arc::new(frame),
                            anchor: (1, 2),
                            end: (6, 2),
                            dragging: false,
                        });
                    }
                }
                for action in [EditAction::Paste, EditAction::Copy] {
                    this.native_menu_action(action, Some(MenuToken::Recorded(1)), window, cx);
                }
                assert!(this.edit_menu.ticket.is_none(), "cause {cause}");
                assert_eq!(
                    (
                        this.handoff.clipboard_reads,
                        this.handoff.clipboard_writes,
                        this.handoff.workers
                    ),
                    (0, 0, 0),
                    "cause {cause}"
                );
                assert!(this.terminal.input.next().is_none());
            })
        });
        assert_eq!(calls.borrow().dismissed.len(), 1, "cause {cause}");
    }
}

#[gpui::test]
fn h11_dismissed_menu_leaves_regular_input_available(cx: &mut TestAppContext) {
    let (view, cx, calls) = fixture(cx);
    let at = position(&view, cx);
    hold(cx, at);
    touch(cx, TouchPhase::Ended, at);
    cx.update(|window, cx| view.update(cx, |this, _| this.dismiss_edit_menu(window)));
    cx.simulate_input("x");
    let deadline = Instant::now() + Duration::from_secs(5);
    let received = loop {
        cx.run_until_parked();
        let messages: Vec<_> = calls
            .borrow()
            .receipts
            .as_ref()
            .unwrap_or_else(|| panic!("receipt channel"))
            .try_iter()
            .collect();
        let events: Vec<_> = messages
            .into_iter()
            .filter_map(|m| match m {
                sheprd_client::protocol::ClientMessage::ClientShellPaneInput {
                    pane_id,
                    events,
                    ..
                } => Some((pane_id, events)),
                _ => None,
            })
            .collect();
        if !events.is_empty() {
            break events;
        }
        assert!(
            Instant::now() < deadline,
            "ordinary input after menu dismissal was lost"
        );
        std::thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(
        received,
        vec![(
            "w1:p1".into(),
            vec![sheprd_client::protocol::ClientPaneInputEvent::TextCommit(
                "x".into()
            )]
        )]
    );
    view.update(cx, |this, _| {
        assert!(this.handoff.selection.is_none());
        assert!(!this.handoff.select_mode);
        assert!(!this.handoff.preparing);
    });
}

#[gpui::test]
fn token_contract_binds_selection_all_destination_fields_and_active_state(cx: &mut TestAppContext) {
    let (view, cx, _) = fixture(cx);
    view.update(cx, |this, _| {
        let frame = Arc::new(
            this.terminal
                .live
                .surface
                .as_ref()
                .unwrap_or_else(|| panic!("surface"))
                .frame
                .clone(),
        );
        let selection = Selection {
            frame: frame.clone(),
            anchor: (1, 2),
            end: (6, 2),
            dragging: false,
        };
        let key = SelectionKey::from(&selection);
        let ticket = EditMenuTicket {
            token: MenuToken::Recorded(1),
            stamp: this.handoff.stamp.clone(),
            selection: Some(key.clone()),
        };
        let stamp = this
            .handoff
            .stamp
            .clone()
            .unwrap_or_else(|| panic!("stamp"));
        for action in [EditAction::Copy, EditAction::Paste] {
            assert!(menu_action_allowed(
                Some(&ticket),
                ticket.token,
                Some(&stamp),
                Some(&key),
                action,
                true
            ));
            assert!(!menu_action_allowed(
                Some(&ticket),
                ticket.token,
                Some(&stamp),
                Some(&key),
                action,
                false
            ));
            assert!(!menu_action_allowed(
                None,
                ticket.token,
                Some(&stamp),
                Some(&key),
                action,
                true
            ));
            assert!(!menu_action_allowed(
                Some(&ticket),
                MenuToken::Recorded(2),
                Some(&stamp),
                Some(&key),
                action,
                true
            ));
            assert!(!menu_action_allowed(
                Some(&ticket),
                ticket.token,
                None,
                Some(&key),
                action,
                true
            ));
            for field in 0..5 {
                let mut changed = stamp.clone();
                match field {
                    0 => changed.selection += 1,
                    1 => changed.destination.generation += 1,
                    2 => changed.destination.epoch += 1,
                    3 => changed.destination.boot.push('x'),
                    _ => {
                        changed.destination.target =
                            crate::terminal::InputTarget::Pane("other".into())
                    }
                }
                assert!(!menu_action_allowed(
                    Some(&ticket),
                    ticket.token,
                    Some(&changed),
                    Some(&key),
                    action,
                    true
                ));
            }
        }
        for action in [EditAction::Cut, EditAction::SelectAll] {
            assert!(!menu_action_allowed(
                Some(&ticket),
                ticket.token,
                Some(&stamp),
                Some(&key),
                action,
                true
            ));
        }
        for change in 0..3 {
            let mut changed = Selection {
                frame: frame.clone(),
                anchor: selection.anchor,
                end: selection.end,
                dragging: false,
            };
            match change {
                0 => changed.frame = Arc::new((*frame).clone()),
                1 => changed.anchor.0 += 1,
                _ => changed.end.0 += 1,
            }
            assert!(!menu_action_allowed(
                Some(&ticket),
                ticket.token,
                Some(&stamp),
                Some(&SelectionKey::from(&changed)),
                EditAction::Copy,
                true
            ));
        }
    });
}
