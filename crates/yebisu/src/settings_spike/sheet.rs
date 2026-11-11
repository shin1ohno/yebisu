// Modified from https://github.com/shin1ohno/herdr-gpui crates/sheprd-gpui/src/menu/settings_sheet.rs at 398c132a06723a0cc1494842a34666d173ec0200
//! Actual menu-layer shell compiled on iOS, mounted only in host tests.
use super::{appearance::Theme, draft::Draft, editor::SettingsWindow, metrics, motion::Toggle};
use crate::app::Smoke;
use gpui::{prelude::*, *};
use std::time::Instant;

pub(crate) struct SettingsSheet {
    pub(super) editor: Entity<SettingsWindow>,
    draft: Draft,
    return_focus: FocusHandle,
    appear: Toggle,
    pub(super) open: bool,
    closing: bool,
}
pub(super) fn sheet_size(visible: Size<Pixels>) -> Size<Pixels> {
    size(
        (visible.width - px(32.))
            .max(px(0.))
            .min(px(metrics::SETTINGS_SHEET_WIDTH)),
        (visible.height - px(32.))
            .max(px(0.))
            .min(px(metrics::SETTINGS_SHEET_HEIGHT)),
    )
}
impl SettingsSheet {
    pub(crate) fn install(
        source: Entity<Smoke>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let draft = Draft {
            source: source.downgrade(),
        };
        let editor = cx.new(|cx| SettingsWindow::new(draft.clone(), cx));
        let return_focus = if source.read(cx).terminal.open {
            source.read(cx).terminal_focus.clone()
        } else {
            source.read(cx).focus.clone()
        };
        editor.read(cx).focus.clone().focus(window, cx);
        let mut appear = Toggle::default();
        appear.set(true, Instant::now(), cfg!(test));
        Self {
            editor,
            draft,
            return_focus,
            appear,
            open: true,
            closing: false,
        }
    }
    pub(super) fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editor
            .update(cx, |editor, cx| editor.flush_for_dismiss(cx));
        self.closing = true;
        self.finish_dismiss(window, cx);
    }
    fn finish_dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.closing && self.editor.read(cx).sheet_ready(cx) {
            self.open = false;
            self.closing = false;
            self.appear.set(false, Instant::now(), cfg!(test));
            self.return_focus.focus(window, cx);
        }
        cx.notify();
    }
}
impl Render for SettingsSheet {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.closing {
            self.draft.flush(cx);
            self.finish_dismiss(window, cx);
        }
        let now = Instant::now();
        let shown = self.appear.shown(now);
        if self.appear.moving(now) || (self.closing && self.draft.error(cx).is_none()) {
            window.request_animation_frame();
        }
        let theme = Theme::default();
        let size = sheet_size(window.fully_visible_bounds().size);
        let panel = div()
            .id("menu-panel")
            .debug_selector(|| "settings-spike-panel".into())
            .w(size.width)
            .h(size.height)
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(metrics::SHEET_RADIUS))
            .border_1()
            .border_color(rgb(theme.active))
            .bg(rgb(theme.surface))
            .shadow_lg()
            .opacity(shown)
            .top(px((1. - shown) * 10.))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(self.editor.clone());
        div()
            .id("settings-spike-overlay")
            .debug_selector(|| "settings-spike-overlay".into())
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(
                (theme.background << 8) | (168. * shown.clamp(0., 1.)).round() as u32,
            ))
            .occlude()
            .when(!self.open && !self.appear.moving(now), |overlay| {
                overlay.hidden()
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    cx.stop_propagation();
                    this.dismiss(window, cx);
                }),
            )
            .on_scroll(|_, _, cx| cx.stop_propagation())
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                cx.stop_propagation();
                if event.keystroke.key == "escape" {
                    this.dismiss(window, cx);
                }
            }))
            .child(panel)
    }
}
