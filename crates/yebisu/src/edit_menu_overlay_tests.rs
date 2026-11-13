use super::*;
use gpui::{Context, Modifiers, MouseButton, Render, Window, div, prelude::*};

struct Overlay {
    view: Entity<Smoke>,
    at: Point<Pixels>,
    occlude: bool,
    taps: Rc<RefCell<usize>>,
}
impl Render for Overlay {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let taps = self.taps.clone();
        let button = div()
            .id("overlay-button")
            .size_full()
            .when(self.occlude, |d| d.occlude())
            .on_click(move |_, _, _| *taps.borrow_mut() += 1)
            .child("Overlay")
            .into_any_element();
        let registry = self.view.read(cx).front_layers.clone();
        div().relative().size_full().child(self.view.clone()).child(
            div()
                .absolute()
                .left(self.at.x)
                .top(self.at.y)
                .w(px(80.))
                .h(px(50.))
                .child(if self.occlude {
                    button
                } else {
                    crate::front_layer::front_layer(button, registry).into_any_element()
                }),
        )
    }
}

#[gpui::test]
fn h4_front_layer_and_occlusion_preserve_overlay_tap_after_redraw(cx: &mut TestAppContext) {
    for occlude in [false, true] {
        let (view, cx, calls) = fixture(cx);
        let at = position(&view, cx);
        let taps = Rc::new(RefCell::new(0));
        cx.update(|window, cx| {
            window.replace_root(cx, |_, _| Overlay {
                view: view.clone(),
                at,
                occlude,
                taps: taps.clone(),
            });
        });
        cx.run_until_parked();
        for pass in 1..=2 {
            let center = at + point(px(30.), px(20.));
            hold(cx, center);
            touch(cx, TouchPhase::Ended, center);
            assert!(calls.borrow().presented.is_empty());
            assert_eq!(*taps.borrow(), pass);
            assert!(view.read_with(cx, |this, _| this.handoff.selection.is_none()));
            view.update(cx, |_, cx| cx.notify());
            cx.run_until_parked();
        }
    }
}

#[gpui::test]
fn h4_outside_terminal_and_existing_error_overlay_never_present(cx: &mut TestAppContext) {
    let (view, cx, calls) = fixture(cx);
    for _ in 0..2 {
        let grid = view.read_with(cx, |this, _| this.terminal.geometry.grid);
        for at in [
            point(px(-1.), px(-1.)),
            grid.origin - point(px(1.), px(1.)),
            point(grid.origin.x - px(1.), grid.origin.y + px(30.)),
        ] {
            hold(cx, at);
            touch(cx, TouchPhase::Ended, at);
            assert!(calls.borrow().presented.is_empty());
            assert!(view.read_with(cx, |this, _| this.handoff.selection.is_none()));
        }
        view.update(cx, |this, cx| {
            this.settings_error = Some("fixture error".into());
            cx.notify();
        });
        cx.run_until_parked();
        let error = cx
            .debug_bounds("settings-error")
            .unwrap_or_else(|| panic!("error overlay not painted"));
        let at = error.origin + point(error.size.width / 2., error.size.height / 2.);
        hold(cx, at);
        touch(cx, TouchPhase::Ended, at);
        assert!(calls.borrow().presented.is_empty());
        assert!(view.read_with(cx, |this, _| this.handoff.selection.is_none()));
        view.update(cx, |this, cx| {
            this.settings_error = None;
            cx.notify();
        });
        cx.run_until_parked();
    }
}

#[gpui::test]
fn h5_secondary_click_and_primary_drag_keep_separate_meanings(cx: &mut TestAppContext) {
    let (view, cx, calls) = fixture(cx);
    let at = position(&view, cx);
    cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::default());
    assert_eq!(calls.borrow().presented.len(), 1);
    cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::default());
    assert_eq!(calls.borrow().presented.len(), 1);
    assert!(view.read_with(cx, |this, _| this.handoff.selection.is_none()));
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(
        at + point(px(60.), px(0.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_up(
        at + point(px(60.), px(0.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    assert_eq!(calls.borrow().presented.len(), 1);
    assert!(view.read_with(cx, |this, _| {
        this.handoff
            .selection
            .as_ref()
            .is_some_and(|s| s.anchor != s.end)
    }));
}
