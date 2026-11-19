//! The app root's hook into the QA accessibility layer: it publishes
//! `qa.state` on every frame from launch, on every screen, so a test never
//! waits for a marked control to be drawn before it can read the state.
use crate::app::Smoke;
#[cfg(any(all(target_os = "ios", target_abi = "sim"), test))]
use gpui::Window;
use gpui::{AnyElement, Context};

impl Smoke {
    /// The root publisher (`layer::publisher`), or `None` unless the layer
    /// is on. Add it as the last child of the root element.
    pub(crate) fn qa_ax_root(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !super::enabled() {
            return None;
        }
        #[cfg(any(all(target_os = "ios", target_abi = "sim"), test))]
        {
            let view = cx.entity();
            Some(super::layer::publisher(move |window, cx| {
                let this = view.read(cx);
                let mut fields = this.qa_ax_fields();
                fields.extend(this.qa_ax_window_fields(window));
                fields
            }))
        }
        #[cfg(not(any(all(target_os = "ios", target_abi = "sim"), test)))]
        {
            let _ = cx;
            None
        }
    }

    /// The root fields of `qa.state`: which screen is up, and what covers it
    /// (a test checks these before pressing, because `isHittable` does not
    /// see a GPUI menu or sheet over a control).
    #[cfg(any(all(target_os = "ios", target_abi = "sim"), test))]
    pub(crate) fn qa_ax_fields(&self) -> Vec<(&'static str, serde_json::Value)> {
        use serde_json::{Value, json};
        let screen = if self.terminal.open {
            "terminal"
        } else if self.smoke_mode {
            "p0"
        } else {
            "first-run"
        };
        let hosts = &self.shell.hosts_sheet;
        let sheet = if self.settings_modal {
            json!("settings")
        } else if hosts.add.is_some() {
            json!("add-host")
        } else if hosts.list_open {
            json!("hosts")
        } else {
            Value::Null
        };
        let menu = if self.shell.frame.overflow_open {
            json!("overflow")
        } else {
            Value::Null
        };
        let live = &self.terminal.live;
        let pane = crate::board::snapshot(live).and_then(|s| s.focused_pane_id.clone());
        vec![
            ("screen", json!(screen)),
            ("sheet", sheet),
            ("menu", menu),
            ("taps", json!(self.taps)),
            // The open settings page (`Page::id`), null with the sheet closed.
            (
                "settings_page",
                if self.settings_modal {
                    json!(self.settings_page.id())
                } else {
                    Value::Null
                },
            ),
            // The terminal's density and how a selection starts.
            ("compact", json!(self.terminal.compact)),
            ("drag_select", json!(self.handoff.drag_select)),
            // The terminal link: connected, the last surface revision drawn
            // (null before the first frame) and the focused pane's id.
            ("connected", json!(live.connected)),
            (
                "surface",
                live.surface
                    .as_ref()
                    .map_or(Value::Null, |s| json!(s.surface_revision)),
            ),
            ("pane", pane.map_or(Value::Null, Value::from)),
            // Herdr 2's pane layout as the core worker last published it.
            ("core", super::core::layout_field()),
        ]
    }

    /// The root fields only the window knows: the docked keyboard's height
    /// in points (0 without one; above about 120 it is the software
    /// keyboard, `status_bar::SOFTWARE_KEYBOARD_MIN_HEIGHT`) and the
    /// viewport (width, height).
    #[cfg(any(all(target_os = "ios", target_abi = "sim"), test))]
    pub(crate) fn qa_ax_window_fields(
        &self,
        window: &Window,
    ) -> Vec<(&'static str, serde_json::Value)> {
        use crate::{layout::Size, shell::fit};
        use serde_json::json;
        let viewport = window.viewport_size();
        let size = Size::new(f32::from(viewport.width), f32::from(viewport.height));
        let ime = self.platform_insets(window).ime;
        let keyboard = fit::docked_keyboard_height(size, fit::ime_keyboard(size, &ime));
        vec![
            ("keyboard", json!(keyboard.round())),
            ("viewport", json!([size.width.round(), size.height.round()])),
        ]
    }
}
