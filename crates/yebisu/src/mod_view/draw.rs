//! A scene as GPUI elements filling the reserved region. Each node carries the
//! debug selector `mod-node-{index}`; nodes with a callback take taps, and the
//! region takes keys for the node the last tap focused.
use super::{Kind, Scene, Style};
use gpui::{AnyElement, App, Div, FocusHandle, Role, Stateful, Window, div, prelude::*, px, rgb};
use std::rc::Rc;

/// What the drawn region reports to its owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Hit {
    /// A tap on node `index`, the innermost node with a callback under it.
    Tap(usize),
    /// A key while the region has focus (`Keystroke::key`).
    Key(String),
}

/// The owner's handler; it decides whether a hit becomes an event.
pub(crate) type Route = Rc<dyn Fn(Hit, &mut Window, &mut App)>;

/// The dimmed region while the line is down (design decision 7).
const FADED_OPACITY: f32 = 0.4;

/// The region `mod-slot`, sized by its parent, with the scene's root inside.
pub(crate) fn slot(scene: &Scene, faded: bool, focus: &FocusHandle, route: Route) -> AnyElement {
    let root = node_element(scene, scene.root, faded, focus, &route);
    let keys = route;
    div()
        .id("mod-slot")
        .debug_selector(|| "mod-slot".into())
        .relative()
        .size_full()
        .overflow_hidden()
        .track_focus(focus)
        .when(faded, |region| region.opacity(FADED_OPACITY))
        .when(!faded, |region| {
            region.on_key_down(move |event, window, cx| {
                keys(Hit::Key(event.keystroke.key.clone()), window, cx);
            })
        })
        .children(root)
        .into_any_element()
}

fn node_element(
    scene: &Scene,
    index: usize,
    faded: bool,
    focus: &FocusHandle,
    route: &Route,
) -> Option<AnyElement> {
    let node = scene.node(index)?;
    let mut element = div()
        .id(("mod-node", index))
        .debug_selector(move || format!("mod-node-{index}"));
    element = match node.kind {
        Kind::Row => element.flex().flex_row(),
        Kind::Column => element.flex().flex_col(),
        Kind::Button => element
            .role(Role::Button)
            .aria_label(node.text.clone().unwrap_or_default()),
        Kind::Div | Kind::Text => element,
    };
    element = styled(element, &node.style);
    if let Some(text) = &node.text {
        element = element.child(text.clone());
    }
    for &child in &node.children {
        element = element.children(node_element(scene, child, faded, focus, route));
    }
    if !faded && (node.click.is_some() || node.key.is_some()) {
        let route = route.clone();
        let focus = focus.clone();
        let takes_keys = node.key.is_some();
        element = element.on_click(move |_, window, cx| {
            // The innermost node with a callback is the target (R5).
            cx.stop_propagation();
            if takes_keys {
                focus.focus(window, cx);
            }
            route(Hit::Tap(index), window, cx);
        });
    }
    Some(element.into_any_element())
}

fn styled(mut element: Stateful<Div>, style: &Style) -> Stateful<Div> {
    if style.fill {
        element = element.size_full();
    }
    if style.full_width {
        element = element.w_full();
    }
    if style.grow {
        element = element.flex_1();
    }
    if style.center {
        element = element.items_center();
    }
    if let Some(width) = style.width {
        element = element.flex_none().w(px(width));
    }
    if let Some(height) = style.height {
        element = element.flex_none().h(px(height));
    }
    if style.padding > 0.0 {
        element = element.p(px(style.padding));
    }
    if style.gap > 0.0 {
        element = element.gap(px(style.gap));
    }
    if let Some(background) = style.background {
        element = element.bg(rgb(background));
    }
    if let Some(color) = style.color {
        element = element.text_color(rgb(color));
    }
    element
}
