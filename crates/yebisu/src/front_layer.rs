//! Explicit hitboxes for overlays which must keep scroll propagation.
use gpui::{AnyElement, Hitbox, HitboxBehavior, Pixels, Point, Window, canvas, div, prelude::*};
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Default)]
pub(crate) struct FrontLayers(Rc<RefCell<Vec<Hitbox>>>);
impl FrontLayers {
    pub fn clear(&self) {
        self.0.borrow_mut().clear();
    }
    pub fn covers(&self, position: Point<Pixels>, window: &Window) -> bool {
        self.0
            .borrow()
            .iter()
            .any(|hitbox| hitbox.is_hovered_at(position, window))
    }
}

pub(crate) fn front_layer(element: AnyElement, registry: FrontLayers) -> impl IntoElement {
    div().relative().child(element).child(
        canvas(
            move |bounds, window, _| {
                registry
                    .0
                    .borrow_mut()
                    .push(window.insert_hitbox(bounds, HitboxBehavior::Normal));
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full(),
    )
}
