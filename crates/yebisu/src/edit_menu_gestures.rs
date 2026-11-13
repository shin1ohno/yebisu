use crate::app::Smoke;
use gpui::{
    Context, DispatchPhase, HitboxBehavior, LongPressEvent, TouchPhase, canvas, prelude::*,
};

impl Smoke {
    pub(crate) fn edit_menu_canvas(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let paint = self.terminal_paint_callback(cx);
        let entity = cx.entity().downgrade();
        canvas(
            |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
            move |bounds, hitbox, window, cx| {
                window.on_mouse_event(move |event: &LongPressEvent, phase, window, cx| {
                    if phase != DispatchPhase::Bubble {
                        return;
                    }
                    let _ = entity.update(cx, |this, cx| match event.phase {
                        TouchPhase::Started
                            if !window.default_prevented()
                                && hitbox.is_hovered_at(event.start_position, window)
                                && this.terminal.geometry.contains(event.start_position)
                                && !this.front_layers.covers(event.start_position, window) =>
                        {
                            this.edit_menu.gesture =
                                this.present_edit_menu(event.start_position, window, cx);
                            window.prevent_default();
                            cx.stop_propagation();
                        }
                        TouchPhase::Cancelled => {
                            if let Some(token) = this.edit_menu.gesture.take()
                                && this
                                    .edit_menu
                                    .ticket
                                    .as_ref()
                                    .is_some_and(|ticket| ticket.token == token)
                            {
                                this.dismiss_edit_menu(window);
                            }
                        }
                        _ => {}
                    });
                });
                paint(bounds, (), window, cx);
            },
        )
        .size_full()
    }
}
