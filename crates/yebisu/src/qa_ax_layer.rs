//! What the QA accessibility layer publishes, and when (Simulator and host
//! tests only; `qa_ax::mark` and the root hook reach it only when the layer
//! is on).
//!
//! Each marked control paints a canvas that records its frame for this frame
//! and counts the presses GPUI hit-tests to it. The root publisher, a
//! deferred canvas drawn after everything else, collects the frame's marks
//! with the root fields and hands the result to UIKit when it changed.
use gpui::{
    AnyElement, App, Bounds, DispatchPhase, HitboxBehavior, MouseDownEvent, MouseUpEvent, Pixels,
    Window, canvas, deferred, prelude::*,
};
use serde_json::Value;
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
};

/// `qa.state` fields the layer owns; `report` refuses them.
pub(crate) const RESERVED: [&str; 4] = ["seq", "launch", "presses", "duplicates"];

/// One exposed control: identifier, label and frame in window points
/// (x, y, width, height).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Control {
    pub(crate) id: &'static str,
    pub(crate) label: &'static str,
    pub(crate) frame: [f32; 4],
}

/// What `qa.state` reports.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct State {
    /// Grows by one whenever another field changes, so a reader can tell a
    /// new value from a repeated one.
    pub(crate) seq: u64,
    /// The app's process id: a relaunch changes it.
    pub(crate) launch: u32,
    /// Presses GPUI delivered to each marked control (down and up inside
    /// it, not covered by an occluding element).
    pub(crate) presses: BTreeMap<&'static str, u64>,
    /// Identifiers marked more than once in the last frame (the first one
    /// painted is exposed).
    pub(crate) duplicates: Vec<&'static str>,
    /// The root fields and what QA modes reported.
    pub(crate) fields: BTreeMap<&'static str, Value>,
}

/// The `qa.state` value: one line of JSON with the fields flattened.
pub(crate) fn state_json(state: &State) -> String {
    let mut object = serde_json::Map::new();
    for (key, value) in &state.fields {
        object.insert((*key).to_owned(), value.clone());
    }
    object.insert("seq".to_owned(), state.seq.into());
    object.insert("launch".to_owned(), state.launch.into());
    let presses = state
        .presses
        .iter()
        .map(|(id, count)| ((*id).to_owned(), Value::from(*count)))
        .collect();
    object.insert("presses".to_owned(), Value::Object(presses));
    object.insert("duplicates".to_owned(), state.duplicates.clone().into());
    Value::Object(object).to_string()
}

/// The next published value, or `None` when nothing changed since
/// `published`. A change in any state field other than `seq` bumps `seq`;
/// a moved control republishes the layout with the same `seq`.
pub(crate) fn next(
    published: Option<&(Vec<Control>, State)>,
    controls: Vec<Control>,
    mut state: State,
) -> Option<(Vec<Control>, State)> {
    if let Some((old_controls, old_state)) = published {
        state.seq = old_state.seq;
        if *old_controls == controls && *old_state == state {
            return None;
        }
        if *old_state != state {
            state.seq += 1;
        }
    } else {
        state.seq = 1;
    }
    Some((controls, state))
}

/// One frame's marks in identifier order, keeping the first one painted for
/// an identifier, and the identifiers painted more than once.
pub(crate) fn collapse(painted: Vec<Control>) -> (Vec<Control>, Vec<&'static str>) {
    let mut seen = BTreeMap::new();
    let mut duplicates = BTreeSet::new();
    for control in painted {
        if seen.contains_key(control.id) {
            duplicates.insert(control.id);
        } else {
            seen.insert(control.id, control);
        }
    }
    (
        seen.into_values().collect(),
        duplicates.into_iter().collect(),
    )
}

/// Which window owns the layer (UI test redesign S4, the two E5 scenes).
/// There is one layer and one `qa.state`, so XCTest never sees two elements
/// with one identifier; it lives in the owner's UIWindow, and only the owner's
/// marks and root fields are published. A window is numbered when it first
/// draws. The first window to draw owns the layer; a newer window takes it
/// over when it draws while active, so the second scene owns the layer once
/// it is focused. Ownership never moves back to an older window: a test that
/// closes the newest window ends with no state to read, by name.
#[derive(Debug, Default)]
pub(crate) struct Windows {
    order: BTreeMap<u64, u64>,
    numbered: u64,
    owner: Option<u64>,
}

impl Windows {
    /// Window `id` drew a frame (`active`: its scene is foreground-active);
    /// true when it owns the layer after this frame.
    pub(crate) fn draw(&mut self, id: u64, active: bool) -> bool {
        let numbered = &mut self.numbered;
        let order = *self.order.entry(id).or_insert_with(|| {
            *numbered += 1;
            *numbered
        });
        let owner = match self.owner {
            None => id,
            Some(owner) if owner != id && active && order > self.order[&owner] => id,
            Some(owner) => owner,
        };
        self.owner = Some(owner);
        owner == id
    }
}

#[derive(Default)]
struct Registry {
    painted: Vec<Control>,
    presses: BTreeMap<&'static str, u64>,
    armed: BTreeSet<&'static str>,
    fields: BTreeMap<&'static str, Value>,
    published: Option<(Vec<Control>, State)>,
    /// This thread built the root publisher, so its fields are published.
    publishing: bool,
    windows: Windows,
    /// The window the published layer belongs to.
    published_by: Option<u64>,
}

thread_local! {
    static REGISTRY: RefCell<Registry> = RefCell::new(Registry::default());
}

/// Sets a field for the next published value; false for a reserved key, and
/// on a thread that never built the root publisher: the registry is per
/// thread, so a field set there would never reach `qa.state` (review F4).
pub(crate) fn report(key: &'static str, value: Value) -> bool {
    if RESERVED.contains(&key) {
        return false;
    }
    REGISTRY.with(|registry| {
        let mut registry = registry.borrow_mut();
        if registry.publishing {
            registry.fields.insert(key, value);
        }
        registry.publishing
    })
}

fn frame(bounds: Bounds<Pixels>) -> [f32; 4] {
    [
        f32::from(bounds.origin.x),
        f32::from(bounds.origin.y),
        f32::from(bounds.size.width),
        f32::from(bounds.size.height),
    ]
}

/// The mark element (see `qa_ax::mark`). Its hitbox never blocks the
/// control's own; it only tells whether GPUI's hit test reached the control.
pub(crate) fn marker(id: &'static str, label: &'static str) -> AnyElement {
    canvas(
        |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
        move |bounds, hitbox, window, _| {
            REGISTRY.with(|registry| {
                registry.borrow_mut().painted.push(Control {
                    id,
                    label,
                    frame: frame(bounds),
                });
            });
            let down = hitbox.clone();
            window.on_mouse_event(move |_: &MouseDownEvent, phase, window, _| {
                if phase == DispatchPhase::Capture && down.is_hovered(window) {
                    REGISTRY.with(|registry| registry.borrow_mut().armed.insert(id));
                }
            });
            window.on_mouse_event(move |_: &MouseUpEvent, phase, window, _| {
                if phase != DispatchPhase::Capture {
                    return;
                }
                let hovered = hitbox.is_hovered(window);
                let counted = REGISTRY.with(|registry| {
                    let mut registry = registry.borrow_mut();
                    let armed = registry.armed.remove(id);
                    if armed && hovered {
                        *registry.presses.entry(id).or_default() += 1;
                    }
                    armed && hovered
                });
                if counted {
                    // A control whose press changes nothing else still gets
                    // a frame, so the count is published.
                    window.refresh();
                }
            });
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
    .into_any_element()
}

/// The root publisher: a deferred canvas drawn after every other element
/// (including other deferred ones), so it sees every mark of the frame.
/// `fields` reads the root's own `qa.state` fields (it is given the window
/// for what only the window knows, such as the keyboard).
pub(crate) fn publisher(
    fields: impl Fn(&Window, &App) -> Vec<(&'static str, Value)> + 'static,
) -> AnyElement {
    publishing_thread();
    deferred(
        canvas(
            |_, _, _| (),
            move |_, _, window, cx| {
                let id = window.window_handle().window_id().as_u64();
                if !owns(id, window.is_window_active()) {
                    return;
                }
                if !publish(id, native_view(window), fields(window, cx)) {
                    // No window to attach the layer to yet: try next frame.
                    window.refresh();
                }
            },
        )
        .absolute()
        .size_0(),
    )
    .with_priority(usize::MAX)
    .into_any_element()
}

/// Whether window `id` owns the layer after this frame (`Windows::draw`). A
/// window that does not drops the marks it painted, so they never reach the
/// owner's layer.
fn owns(id: u64, active: bool) -> bool {
    REGISTRY.with(|registry| {
        let mut registry = registry.borrow_mut();
        let owns = registry.windows.draw(id, active);
        if !owns {
            registry.painted.clear();
        }
        owns
    })
}

/// The GPUI view of `window` (herdr-uikit's UIView), whose UIWindow the
/// layer is attached to.
#[cfg(all(target_os = "ios", target_abi = "sim"))]
fn native_view(window: &Window) -> Option<std::ptr::NonNull<std::ffi::c_void>> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match HasWindowHandle::window_handle(window).ok()?.as_raw() {
        RawWindowHandle::UiKit(handle) => Some(handle.ui_view),
        _ => None,
    }
}

#[cfg(not(all(target_os = "ios", target_abi = "sim")))]
fn native_view(_: &Window) -> Option<std::ptr::NonNull<std::ffi::c_void>> {
    None
}

/// Marks this thread as the one whose registry the root publisher reads.
fn publishing_thread() {
    REGISTRY.with(|registry| registry.borrow_mut().publishing = true);
}

/// Publishes this frame's marks and fields if they changed; false when the
/// UIKit layer could not be attached (nothing is recorded as published).
fn publish(
    window: u64,
    view: Option<std::ptr::NonNull<std::ffi::c_void>>,
    root: Vec<(&'static str, Value)>,
) -> bool {
    REGISTRY.with(|registry| {
        let mut registry = registry.borrow_mut();
        // A new owner: publish even an unchanged state, so the layer moves to
        // its window (`native::apply` rebuilds the elements there); `seq`
        // keeps growing.
        let new_owner = registry.published_by != Some(window);
        for (key, value) in root {
            if !RESERVED.contains(&key) {
                registry.fields.insert(key, value);
            }
        }
        let (controls, duplicates) = collapse(std::mem::take(&mut registry.painted));
        let state = State {
            seq: 0,
            launch: std::process::id(),
            presses: registry.presses.clone(),
            duplicates,
            fields: registry.fields.clone(),
        };
        let fresh = next(registry.published.as_ref(), controls, state);
        let Some((controls, state)) =
            fresh.or_else(|| new_owner.then(|| registry.published.clone()).flatten())
        else {
            return true;
        };
        let layout_changed = new_owner
            || registry
                .published
                .as_ref()
                .is_none_or(|(old, _)| *old != controls);
        #[cfg(all(target_os = "ios", target_abi = "sim"))]
        if !super::native::apply(view, &controls, &state_json(&state), layout_changed) {
            return false;
        }
        #[cfg(not(all(target_os = "ios", target_abi = "sim")))]
        let _ = (layout_changed, view);
        registry.published = Some((controls, state));
        registry.published_by = Some(window);
        true
    })
}

/// What was last published on this thread (host tests read it).
#[cfg(test)]
pub(crate) fn published() -> Option<(Vec<Control>, State)> {
    REGISTRY.with(|registry| registry.borrow().published.clone())
}

#[cfg(test)]
#[path = "qa_ax_layer_tests.rs"]
mod tests;
