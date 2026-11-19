//! Simulator-only UIKit side of the QA accessibility layer (`qa_ax`).
//!
//! The layer is one transparent UIView over the owner window's UIWindow (the
//! window of the GPUI view that publishes, `layer::Windows`) with user
//! interaction off, so every touch falls through to the GPUI view. Its
//! `accessibilityElements` are the marked controls plus `qa.state`. When
//! another window takes the layer over (the second E5 scene), the view moves
//! to that window and its elements are rebuilt there.
#![allow(
    unsafe_code,
    reason = "main-thread QA calls public UIKit accessibility methods"
)]
use super::{ids::STATE, layer::Control};
use objc2::{
    class,
    encode::{Encode, Encoding},
    msg_send,
    rc::{Allocated, Retained},
    runtime::AnyObject,
};
use objc2_foundation::{NSArray, NSString};
use std::{cell::RefCell, ffi::c_void, ptr::NonNull};

#[repr(C)]
#[derive(Clone, Copy)]
struct Point {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Size {
    width: f64,
    height: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Rect {
    origin: Point,
    size: Size,
}

// SAFETY: the layouts match CGPoint, CGSize and CGRect on 64-bit iOS.
unsafe impl Encode for Point {
    const ENCODING: Encoding = Encoding::Struct("CGPoint", &[f64::ENCODING, f64::ENCODING]);
}
// SAFETY: as above.
unsafe impl Encode for Size {
    const ENCODING: Encoding = Encoding::Struct("CGSize", &[f64::ENCODING, f64::ENCODING]);
}
// SAFETY: as above.
unsafe impl Encode for Rect {
    const ENCODING: Encoding = Encoding::Struct("CGRect", &[Point::ENCODING, Size::ENCODING]);
}

fn rect(frame: [f32; 4]) -> Rect {
    Rect {
        origin: Point {
            x: f64::from(frame[0]),
            y: f64::from(frame[1]),
        },
        size: Size {
            width: f64::from(frame[2]),
            height: f64::from(frame[3]),
        },
    }
}

#[link(name = "UIKit", kind = "framework")]
unsafe extern "C" {
    static UIAccessibilityTraitButton: u64;
    static UIAccessibilityTraitStaticText: u64;
    static UIAccessibilityLayoutChangedNotification: u32;
    fn UIAccessibilityPostNotification(notification: u32, argument: *mut AnyObject);
}

struct Layer {
    /// The UIWindow the view was added to.
    window: Retained<AnyObject>,
    view: Retained<AnyObject>,
    state: Option<Retained<AnyObject>>,
}

thread_local! {
    static LAYER: RefCell<Option<Layer>> = const { RefCell::new(None) };
}

/// UIViewAutoresizingFlexibleWidth | UIViewAutoresizingFlexibleHeight.
const FLEXIBLE_SIZE: usize = 2 | 16;

unsafe fn key_window() -> Option<Retained<AnyObject>> {
    let app: *mut AnyObject = unsafe { msg_send![class!(UIApplication), sharedApplication] };
    let windows: Retained<NSArray<AnyObject>> = unsafe { msg_send![app, windows] };
    let mut first = None;
    for window in windows {
        let key: bool = unsafe { msg_send![&*window, isKeyWindow] };
        if key {
            return Some(window);
        }
        first.get_or_insert(window);
    }
    first
}

/// The UIWindow that shows `view` (the GPUI view of the publishing window),
/// or the key window when there is no view.
unsafe fn target_window(view: Option<NonNull<c_void>>) -> Option<Retained<AnyObject>> {
    match view {
        Some(view) => unsafe {
            let view = &*view.as_ptr().cast::<AnyObject>();
            msg_send![view, window]
        },
        None => unsafe { key_window() },
    }
}

unsafe fn new_view(window: &AnyObject) -> Retained<AnyObject> {
    unsafe {
        let bounds: Rect = msg_send![window, bounds];
        let view: Allocated<AnyObject> = msg_send![class!(UIView), alloc];
        let view: Retained<AnyObject> = msg_send![view, initWithFrame: bounds];
        let _: () = msg_send![&*view, setUserInteractionEnabled: false];
        let _: () = msg_send![&*view, setAutoresizingMask: FLEXIBLE_SIZE];
        let _: () = msg_send![&*view, setIsAccessibilityElement: false];
        let _: () = msg_send![window, addSubview: &*view];
        view
    }
}

unsafe fn element(
    container: &AnyObject,
    id: &str,
    label: &str,
    traits: u64,
    frame: Rect,
) -> Retained<AnyObject> {
    unsafe {
        let element: Allocated<AnyObject> = msg_send![class!(UIAccessibilityElement), alloc];
        let element: Retained<AnyObject> =
            msg_send![element, initWithAccessibilityContainer: container];
        let _: () = msg_send![&*element, setAccessibilityIdentifier: &*NSString::from_str(id)];
        let _: () = msg_send![&*element, setAccessibilityLabel: &*NSString::from_str(label)];
        let _: () = msg_send![&*element, setAccessibilityTraits: traits];
        let _: () = msg_send![&*element, setAccessibilityFrameInContainerSpace: frame];
        element
    }
}

/// Publishes `controls` and the `qa.state` value in the UIWindow of `view`
/// (the publishing window's GPUI view; the key window without one). With
/// `layout_changed`, or in a window the layer was not in, the element list
/// is rebuilt and UIKit is told the layout changed; otherwise only the state
/// value is replaced. False when there is no window to attach the layer to.
pub(crate) fn apply(
    view: Option<NonNull<c_void>>,
    controls: &[Control],
    state: &str,
    layout_changed: bool,
) -> bool {
    LAYER.with(|layer| {
        let mut layer = layer.borrow_mut();
        // SAFETY: GPUI paints on the main thread, which owns UIKit; `view` is
        // the publishing window's live GPUI view (herdr-uikit retains it).
        let Some(window) = (unsafe { target_window(view) }) else {
            return false;
        };
        let moved = layer
            .as_ref()
            .is_some_and(|layer| !std::ptr::eq(&*layer.window, &*window));
        if moved && let Some(old) = layer.take() {
            // SAFETY: main thread, as above.
            let _: () = unsafe { msg_send![&*old.view, removeFromSuperview] };
        }
        if layer.is_none() {
            // SAFETY: main thread, as above.
            let view = unsafe { new_view(&window) };
            *layer = Some(Layer {
                window,
                view,
                state: None,
            });
        }
        let Some(layer) = layer.as_mut() else {
            return false;
        };
        // SAFETY: main thread, as above; every object is a live UIKit object.
        unsafe {
            if layout_changed || layer.state.is_none() {
                let mut elements = Vec::with_capacity(controls.len() + 1);
                for control in controls {
                    elements.push(element(
                        &layer.view,
                        control.id,
                        control.label,
                        UIAccessibilityTraitButton,
                        rect(control.frame),
                    ));
                }
                let state_element = element(
                    &layer.view,
                    STATE,
                    "QA state",
                    UIAccessibilityTraitStaticText,
                    rect([0.0, 0.0, 1.0, 1.0]),
                );
                elements.push(state_element.clone());
                layer.state = Some(state_element);
                let refs: Vec<&AnyObject> = elements.iter().map(|e| &**e).collect();
                let array = NSArray::from_slice(&refs);
                let _: () = msg_send![&*layer.view, setAccessibilityElements: &*array];
                if let Some(superview) = {
                    let s: Option<Retained<AnyObject>> = msg_send![&*layer.view, superview];
                    s
                } {
                    let _: () = msg_send![&*superview, bringSubviewToFront: &*layer.view];
                }
                UIAccessibilityPostNotification(
                    UIAccessibilityLayoutChangedNotification,
                    std::ptr::null_mut(),
                );
            }
            if let Some(state_element) = &layer.state {
                let _: () =
                    msg_send![&**state_element, setAccessibilityValue: &*NSString::from_str(state)];
            }
        }
        true
    })
}
