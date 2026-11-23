use crate::app::Smoke;
use gpui::{
    Context, DispatchPhase, HitboxBehavior, LongPressEvent, Modifiers, MouseButton, MouseDownEvent,
    TouchPhase, Window, canvas, prelude::*,
};

/// A pointer press that asks for the edit menu: the secondary button, or the
/// primary button with Control alone (iPadOS's Control-click).
pub(crate) fn secondary_press(button: MouseButton, modifiers: &Modifiers) -> bool {
    match button {
        MouseButton::Right => true,
        MouseButton::Left => modifiers.control && modifiers.number_of_modifiers() == 1,
        _ => false,
    }
}

impl Smoke {
    /// A secondary click on the terminal's content area opens the edit menu
    /// there; anywhere else (sidebar, rail, tab row, status bar, an overlay)
    /// it opens nothing. The SDK leaves UIKit's own secondary-click menu
    /// empty (herdr-uikit af7c496), so this is the only edit menu a pointer
    /// can open.
    pub(crate) fn secondary_click(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        crate::qa_ax::count("menu_secondary");
        if self.find.is_none()
            && !window.default_prevented()
            && self.terminal.geometry.contains(event.position)
            && !self.front_layers.covers(event.position, window)
        {
            if self.present_edit_menu(event.position, window, cx).is_some() {
                crate::qa_ax::count("menu_presented");
            }
            window.prevent_default();
            cx.stop_propagation();
        }
    }

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
