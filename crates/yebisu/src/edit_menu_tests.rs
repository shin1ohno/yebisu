//! The production adapters with real GPUI touch timers and private-peer state.
use crate::{
    app::Smoke,
    edit_menu::{Availability, EditAction, MenuPresenter, MenuToken},
    mock_peer,
};
use gpui::{
    AnyWindowHandle, Entity, Pixels, Point, TestAppContext, TouchEvent, TouchId, TouchPhase,
    VisualTestContext, point, px,
};
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

#[path = "edit_menu_overlay_tests.rs"]
mod overlays;
#[path = "edit_menu_state_tests.rs"]
mod states;

#[derive(Default)]
struct Calls {
    availability: Option<Availability>,
    presented: Vec<(MenuToken, bool)>,
    dismissed: Vec<MenuToken>,
    fail: bool,
    revoked: bool,
}
struct Recorder(Rc<RefCell<Calls>>);
impl MenuPresenter for Recorder {
    fn publish(&mut self, _: AnyWindowHandle, availability: Option<Availability>) {
        self.0.borrow_mut().availability = availability;
    }
    fn present(&mut self, _: AnyWindowHandle, _: Point<Pixels>) -> Option<MenuToken> {
        let mut calls = self.0.borrow_mut();
        let token = MenuToken::Recorded(calls.presented.len() as u64 + 1);
        let paste = calls.availability.is_some_and(|a| a.paste);
        calls.presented.push((token, paste));
        (!calls.fail).then_some(token)
    }
    fn dismiss(&mut self, _: AnyWindowHandle, token: MenuToken) {
        self.0.borrow_mut().dismissed.push(token);
    }
    fn revoked(&self, _: AnyWindowHandle, _: MenuToken) -> Option<String> {
        self.0.borrow().revoked.then(|| "ResignedOutside".into())
    }
}

fn fixture(cx: &mut TestAppContext) -> (Entity<Smoke>, &mut VisualTestContext, Rc<RefCell<Calls>>) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let calls = Rc::new(RefCell::new(Calls::default()));
    view.update(cx, |this, _| {
        this.edit_menu.presenter = Box::new(Recorder(calls.clone()));
        this.select_host("menu-fixture");
        this.terminal.open = true;
        this.actions.board_hidden = true;
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, mock_peer::connect());
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let ready = view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            this.refresh_terminal_input();
            this.sync_handoff();
            this.terminal.live.surface.is_some() && this.handoff.stamp.is_some()
        });
        if ready {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "private peer did not become ready"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    cx.update(|window, cx| {
        window.activate_window();
        view.update(cx, |this, cx| {
            this.focus.focus(window, cx);
            cx.notify();
        });
    });
    cx.run_until_parked();
    (view, cx, calls)
}

fn position(view: &Entity<Smoke>, cx: &VisualTestContext) -> Point<Pixels> {
    view.read_with(cx, |this, _| {
        this.terminal.geometry.grid.origin + point(px(40.), px(40.))
    })
}
fn touch(cx: &mut VisualTestContext, phase: TouchPhase, position: Point<Pixels>) {
    cx.simulate_event(TouchEvent {
        id: TouchId(1),
        phase,
        position,
        predicted_position: None,
        force: None,
    });
}
fn hold(cx: &mut VisualTestContext, at: Point<Pixels>) {
    touch(cx, TouchPhase::Started, at);
    cx.executor().advance_clock(Duration::from_millis(501));
    cx.run_until_parked();
}

#[gpui::test]
fn h1_long_press_focuses_publishes_and_never_selects(cx: &mut TestAppContext) {
    let (view, cx, calls) = fixture(cx);
    let at = position(&view, cx);
    hold(cx, at);
    touch(cx, TouchPhase::Moved, at + point(px(2.), px(0.)));
    touch(cx, TouchPhase::Ended, at);
    assert_eq!(
        calls.borrow().presented,
        vec![(MenuToken::Recorded(1), true)]
    );
    cx.update(|window, cx| {
        view.update(cx, |this, _| {
            assert!(this.terminal_focus.is_focused(window));
            assert!(this.handoff.selection.is_none());
            assert!(!this.handoff.select_mode);
            assert!(this.terminal.input.next().is_none());
        })
    });
}

#[gpui::test]
fn h2_h3_short_tap_and_movement_do_not_present(cx: &mut TestAppContext) {
    for moved in [false, true] {
        let (view, cx, calls) = fixture(cx);
        let at = position(&view, cx);
        touch(cx, TouchPhase::Started, at);
        if moved {
            touch(cx, TouchPhase::Moved, at + point(px(12.), px(0.)));
        }
        cx.executor()
            .advance_clock(Duration::from_millis(if moved { 501 } else { 100 }));
        cx.run_until_parked();
        touch(cx, TouchPhase::Ended, at);
        assert!(calls.borrow().presented.is_empty());
        if !moved {
            assert!(view.read_with(cx, |this, _| this.handoff.selection.is_some()));
        }
    }
}

#[gpui::test]
fn h6_h7_cancel_and_failed_presentation_keep_next_tap_working(cx: &mut TestAppContext) {
    for fail in [false, true] {
        let (view, cx, calls) = fixture(cx);
        calls.borrow_mut().fail = fail;
        let at = position(&view, cx);
        hold(cx, at);
        touch(
            cx,
            if fail {
                TouchPhase::Ended
            } else {
                TouchPhase::Cancelled
            },
            at,
        );
        assert!(view.read_with(cx, |this, _| this.handoff.selection.is_none()));
        assert_eq!(calls.borrow().dismissed.len(), usize::from(!fail));
        touch(cx, TouchPhase::Started, at);
        touch(cx, TouchPhase::Ended, at);
        assert!(view.read_with(cx, |this, _| this.handoff.selection.is_some()));
    }
}

#[gpui::test]
fn h8_new_menu_replaces_old_token(cx: &mut TestAppContext) {
    let (view, cx, calls) = fixture(cx);
    let at = position(&view, cx);
    for _ in 0..2 {
        hold(cx, at);
        touch(cx, TouchPhase::Ended, at);
    }
    assert_eq!(calls.borrow().presented.len(), 2);
    assert_eq!(calls.borrow().dismissed, vec![MenuToken::Recorded(1)]);
    assert_eq!(
        view.read_with(cx, |this, _| this
            .edit_menu
            .ticket
            .as_ref()
            .map(|t| t.token)),
        Some(MenuToken::Recorded(2))
    );
}

#[gpui::test]
fn h13_inactive_ticket_does_not_return_and_h15_sdk_revocation_is_final(cx: &mut TestAppContext) {
    for sdk in [false, true] {
        let (view, cx, calls) = fixture(cx);
        let at = position(&view, cx);
        hold(cx, at);
        touch(cx, TouchPhase::Ended, at);
        if sdk {
            calls.borrow_mut().revoked = true;
            cx.update(|window, cx| view.update(cx, |this, _| this.edit_menu_check(window)));
        } else {
            cx.deactivate_window();
            cx.update(|window, _| window.activate_window());
            cx.run_until_parked();
        }
        cx.update(|window, cx| {
            view.update(cx, |this, cx| {
                for action in [EditAction::Copy, EditAction::Paste] {
                    this.native_menu_action(action, Some(MenuToken::Recorded(1)), window, cx);
                }
                assert!(this.edit_menu.ticket.is_none());
                assert_eq!(
                    (
                        this.handoff.clipboard_reads,
                        this.handoff.clipboard_writes,
                        this.handoff.workers
                    ),
                    (0, 0, 0)
                );
            })
        });
        assert_eq!(calls.borrow().dismissed.len(), 1);
        calls.borrow_mut().revoked = false;
        hold(cx, at);
        touch(cx, TouchPhase::Ended, at);
        cx.write_to_clipboard(gpui::ClipboardItem::new_string("fresh-paste".into()));
        cx.update(|window, cx| {
            view.update(cx, |this, cx| {
                this.native_menu_action(
                    EditAction::Paste,
                    Some(MenuToken::Recorded(2)),
                    window,
                    cx,
                );
                assert_eq!(this.handoff.clipboard_reads, 1);
            })
        });
    }
}

#[gpui::test]
fn h14_ordinary_native_paste_counts_one_read(cx: &mut TestAppContext) {
    let (view, cx, _) = fixture(cx);
    cx.write_to_clipboard(gpui::ClipboardItem::new_string("ordinary-paste".into()));
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal_focus.focus(window, cx);
            this.native_menu_action(EditAction::Paste, None, window, cx);
            assert_eq!(this.handoff.clipboard_reads, 1);
        })
    });
}
