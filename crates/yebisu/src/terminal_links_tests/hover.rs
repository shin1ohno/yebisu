//! L66 Command underline (artboard OpenUrl): with Command held, the web
//! address under the resting pointer is underlined on every row it covers;
//! nothing is underlined without Command, during a drag, off the terminal,
//! or over a link that opens nothing.
use super::tap::{command, connected, target};
use crate::{app::Smoke, terminal_links::qa};
use gpui::{Entity, Modifiers, MouseButton, TestAppContext, VisualTestContext};
use std::ops::Range;

fn underlined(view: &Entity<Smoke>, cx: &mut VisualTestContext) -> Vec<(u16, Range<u16>)> {
    view.update(cx, |this, _| this.terminal.link_hover.underlined().to_vec())
}

fn hover(cx: &mut VisualTestContext, at: gpui::Point<gpui::Pixels>, modifiers: Modifiers) {
    cx.simulate_mouse_move(at, None, modifiers);
    cx.run_until_parked();
}

#[gpui::test]
fn command_underlines_the_hovered_web_address_while_held(cx: &mut TestAppContext) {
    let (view, cx, _wire) = connected(cx);
    let at = target(&view, cx, "plain");
    hover(cx, at, Modifiers::default());
    assert_eq!(underlined(&view, cx), [], "underlined without Command");
    // Command pressed while the pointer rests there.
    cx.simulate_modifiers_change(command());
    cx.run_until_parked();
    let shown = underlined(&view, cx);
    let length = u16::try_from(qa::PLAIN.len()).unwrap_or(0);
    assert!(
        matches!(shown.as_slice(), [(12, columns)] if columns.len() == usize::from(length)),
        "the plain address's one row: {shown:?}"
    );
    cx.simulate_modifiers_change(Modifiers::default());
    cx.run_until_parked();
    assert_eq!(underlined(&view, cx), [], "Command released");
    // An address Herdr must still resolve has no known extent: no line.
    let wrapped = target(&view, cx, "wrapped");
    hover(cx, wrapped, command());
    assert_eq!(underlined(&view, cx), [], "an unproven address");
}

#[gpui::test]
fn only_a_resting_pointer_over_an_opening_link_is_underlined(cx: &mut TestAppContext) {
    let (view, cx, _wire) = connected(cx);
    let osc8 = target(&view, cx, "osc8");
    hover(cx, osc8, command());
    let explicit = underlined(&view, cx);
    assert_eq!(explicit.len(), 1, "the OSC 8 link's one row: {explicit:?}");
    for name in ["mailto", "path"] {
        let at = target(&view, cx, name);
        hover(cx, at, command());
        assert_eq!(underlined(&view, cx), [], "{name} opens nothing");
    }
    let plain = target(&view, cx, "plain");
    cx.simulate_mouse_move(plain, Some(MouseButton::Left), command());
    cx.run_until_parked();
    assert_eq!(underlined(&view, cx), [], "a drag is no hover");
    hover(cx, plain, command());
    assert_eq!(underlined(&view, cx).len(), 1, "the plain address");
    hover(cx, gpui::point(gpui::px(1.), gpui::px(1.)), command());
    assert_eq!(underlined(&view, cx), [], "off the terminal");
}
