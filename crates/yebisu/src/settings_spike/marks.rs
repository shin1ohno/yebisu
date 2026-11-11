// Modified from https://github.com/shin1ohno/herdr-gpui crates/sheprd-gpui/src/toggles.rs at 398c132a06723a0cc1494842a34666d173ec0200
use super::appearance::Theme;
use gpui::{prelude::*, *};
pub(crate) fn switch(theme: &Theme, height: f32, on: bool) -> Div {
    let track = if on { theme.primary() } else { theme.active };
    let thumb = theme.foreground;
    let inset = (height / 8.).round();
    div()
        .w(px((height * 1.64).round()))
        .h(px(height))
        .flex_none()
        .rounded_full()
        .p(px(inset))
        .flex()
        .items_center()
        .bg(rgb(track))
        .when(on, |track| track.justify_end())
        .child(
            div()
                .size(px(height - 2. * inset))
                .rounded_full()
                .bg(rgb(thumb)),
        )
}

pub(super) fn disabled<E: StatefulInteractiveElement>(element: E, disabled: bool) -> E {
    if !disabled {
        return element;
    }
    element.a11y_synthetic_children(|builder| builder.parent_node().set_disabled())
}
