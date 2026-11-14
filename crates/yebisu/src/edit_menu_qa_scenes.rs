//! Two real GPUI/UIKit scenes, kept out of the product's window-creation flow.
use crate::{app::Smoke, edit_menu_qa_native as native, edit_menu_qa_sh1_152::case};
use gpui::{App, AppContext, Context, Window, WindowHandle, WindowOptions};
use std::{cell::RefCell, rc::Rc, time::Duration};

#[derive(Default)]
struct Scenes {
    first: Option<WindowHandle<Smoke>>,
    first_menu: Option<Rc<native::Capture>>,
}
thread_local! { static SCENES: RefCell<Scenes> = RefCell::default(); }

pub(crate) fn open_second(window: &Window, stage: usize, cx: &mut Context<Smoke>) {
    SCENES.with_borrow_mut(|scenes| scenes.first = window.window_handle().downcast());
    create_second(stage, cx);
}

fn create_second(stage: usize, cx: &mut App) {
    let opened = cx.open_window(WindowOptions::default(), |window, cx| {
        let view = cx.new(Smoke::new);
        let weak = view.downgrade();
        cx.on_app_lifecycle(move |phase, cx| { let _ = weak.update(cx, |this, cx| this.lifecycle(phase, cx)); }).detach();
        view.update(cx, |this, cx| {
            this.menu_qa.stage = stage;
            this.menu_qa.second = true;
            this.terminal.reconnect.begin("qa@private-peer".into());
            let generation = this.terminal.link.reset();
            this.terminal.link.install(generation, crate::mock_peer::connect());
            this.terminal.open = true;
            this.terminal.needs_focus = true;
            cx.spawn(async |weak, cx| {
                // Move to w1:p3 once this scene's own peer connection has a destination.
                for attempt in 0..40 {
                    cx.background_executor().timer(Duration::from_millis(250)).await;
                    let done = weak.update(cx, |this, cx| {
                        this.poll_terminal(cx);
                        this.refresh_terminal_input();
                        this.sync_handoff();
                        if this.handoff.stamp.is_none() && attempt < 39 {
                            return false;
                        }
                        let result = this.run_action(sheprd_commands::Command::FocusRight);
                        this.observation.record(serde_json::json!({"kind":"sh1-152-second-ready", "accepted":result.is_ok(), "attempt":attempt}));
                        cx.notify();
                        true
                    });
                    if done.unwrap_or(true) {
                        break;
                    }
                }
            }).detach();
            cx.notify();
        });
        let focus = view.read(cx).terminal_focus.clone();
        focus.focus(window, cx);
        view
    });
    if let Err(error) = opened {
        tracing::error!(%error, "SH1-152 second scene failed");
    }
}

pub(crate) fn captured(capture: Rc<native::Capture>, stage: usize) -> Option<Vec<bool>> {
    if case() != "E5c" {
        return None;
    }
    if stage == 0 {
        SCENES.with_borrow_mut(|scenes| scenes.first_menu = Some(capture));
        None
    } else {
        let first = SCENES.with_borrow(|scenes| scenes.first_menu.clone());
        first.map(|first| native::replay(&first.commands))
    }
}

pub(crate) fn close_first(cx: &mut Context<Smoke>) -> bool {
    let first = SCENES.with_borrow_mut(|scenes| scenes.first.take());
    first.is_some_and(|handle| {
        handle
            .update(cx, |_, window, _| window.remove_window())
            .is_ok()
    })
}
