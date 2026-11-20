//! The window-wide native Edit policy around the production search field.
//!
//! SH1-152 publishes an explicit terminal policy (Cut only with a terminal
//! selection). While the search field owns focus that policy must be absent,
//! so the SDK's default text-input rules let the field Cut its own text.
#![allow(clippy::unwrap_used)]
use super::regression_tests::install;
use crate::{
    app::Smoke,
    edit_menu::{Availability, MenuPresenter, MenuToken},
    handoff::Selection,
};
use gpui::{AnyWindowHandle, Entity, Pixels, Point, TestAppContext, VisualTestContext};
use std::{cell::RefCell, rc::Rc, sync::Arc};

type Published = Rc<RefCell<Vec<Option<Availability>>>>;
struct Recorder(Published);
impl MenuPresenter for Recorder {
    fn publish(&mut self, _: AnyWindowHandle, availability: Option<Availability>) {
        self.0.borrow_mut().push(availability);
    }
    fn present(&mut self, _: AnyWindowHandle, _: Point<Pixels>) -> Option<MenuToken> {
        None
    }
    fn dismiss(&mut self, _: AnyWindowHandle, _: MenuToken) {}
    fn revoked(&self, _: AnyWindowHandle, _: MenuToken) -> Option<String> {
        None
    }
}

/// Publishes through the same call render uses and returns what the presenter
/// (the SDK boundary on iOS) received.
fn publish(
    view: &Entity<Smoke>,
    cx: &mut VisualTestContext,
    log: &Published,
) -> Option<Availability> {
    log.borrow_mut().clear();
    cx.update(|window, cx| view.update(cx, |this, _| this.publish_native_edits(window)));
    let published = log.borrow().clone();
    assert_eq!(published.len(), 1, "exactly one publication per call");
    published[0]
}

fn select_first_word(view: &Entity<Smoke>, cx: &mut VisualTestContext) {
    view.update(cx, |this, _| {
        this.refresh_terminal_input();
        this.sync_handoff();
        let frame = this.terminal.live.surface.as_ref().unwrap().frame.clone();
        this.handoff.selection = Some(Selection {
            frame: Arc::new(frame),
            anchor: (0, 0),
            end: (4, 0),
            dragging: false,
        });
    });
}

fn assert_terminal_cut(policy: Option<Availability>, phase: &str) {
    let policy = policy.unwrap_or_else(|| panic!("{phase}: terminal policy missing"));
    assert!(
        policy.copy && policy.cut,
        "{phase}: Cut follows terminal selection"
    );
    assert!(!policy.select_all, "{phase}: no terminal Select All");
}

#[gpui::test]
fn search_field_focus_withdraws_the_terminal_edit_policy_only_while_it_has_focus(
    cx: &mut TestAppContext,
) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let log = Published::default();
    let (client, _receipts) = crate::find::peer::connect_quiet(false).unwrap();
    view.update(cx, |this, _| {
        this.edit_menu.presenter = Box::new(Recorder(log.clone()));
    });
    install(&view, cx, "p13-peer", client);
    cx.update(|window, cx| {
        window.activate_window();
        view.update(cx, |this, cx| this.terminal_focus.focus(window, cx));
    });

    // Search closed: SH1-152's explicit terminal policy is unchanged.
    select_first_word(&view, cx);
    assert_terminal_cut(publish(&view, cx, &log), "before search");

    // Search open with field focus: no window-wide policy (SDK defaults).
    let field_focus = cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.open_find(window, cx);
            let bar = this.find.as_ref().unwrap();
            assert!(bar.owns_text_editing(window), "open focuses the field");
            bar.field.read(cx).focus.clone()
        })
    });
    assert!(
        publish(&view, cx, &log).is_none(),
        "field focus keeps a terminal policy"
    );
    cx.update(|window, cx| {
        view.update(cx, |this, _| {
            assert!(this.native_edit_availability(window).is_none());
        })
    });

    // Search open, terminal refocused: the terminal stays explicitly refused.
    cx.update(|window, cx| view.update(cx, |this, cx| this.terminal_focus.focus(window, cx)));
    let refused = publish(&view, cx, &log).unwrap_or_else(|| panic!("terminal policy missing"));
    assert!(
        !refused.copy && !refused.cut && !refused.paste && !refused.select_all,
        "terminal edits during search"
    );
    cx.update(|window, cx| field_focus.focus(window, cx));
    assert!(
        publish(&view, cx, &log).is_none(),
        "field refocus keeps a terminal policy"
    );

    // Search closed again: the terminal policy returns with Cut-on-selection.
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.close_find(window, cx);
            assert!(this.find.is_none() && this.terminal_focus.is_focused(window));
            // Explicit controlled boundary; this is not UIKit retirement proof.
            this.controlled_find_retirement(window);
        })
    });
    select_first_word(&view, cx);
    assert_terminal_cut(publish(&view, cx, &log), "after search");
}
