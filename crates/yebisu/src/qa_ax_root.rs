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
            count_lifecycle(cx);
            let view = cx.entity();
            Some(super::layer::publisher(move |window, cx| {
                let this = view.read(cx);
                let mut fields = this.qa_ax_fields(window);
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
    pub(crate) fn qa_ax_fields(&self, window: &Window) -> Vec<(&'static str, serde_json::Value)> {
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
        let input_pane = self
            .terminal
            .input
            .destination
            .as_ref()
            .and_then(|d| match &d.target {
                crate::terminal::InputTarget::Pane(id) => Some(id.clone()),
                crate::terminal::InputTarget::Popup(_) => None,
            });
        // The second E5 scene's root (edit_menu_qa_scenes.rs); host tests
        // have no SH1-152 scenes.
        #[cfg(all(target_os = "ios", target_abi = "sim"))]
        let second = self.menu_qa.second;
        #[cfg(not(all(target_os = "ios", target_abi = "sim")))]
        let second = false;
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
            ("herdr_doc", super::core::doc_field()),
            // UI test redesign S4 (the edit-menu tests): where typed input
            // goes, the link generation and the terminal rows (a changed
            // keyboard resizes them), whether typing is paused for a
            // selection (the status bar's "Typing is paused"), the copy
            // setting, which E5 scene this is, and what the mock peers got.
            ("input_pane", input_pane.map_or(Value::Null, Value::from)),
            ("generation", json!(self.terminal.link.generation)),
            (
                "rows",
                live.surface
                    .as_ref()
                    .map_or(Value::Null, |s| json!(s.frame.height)),
            ),
            (
                "selecting",
                json!(self.handoff.select_mode || self.handoff.selection.is_some()),
            ),
            ("copy_on_select", json!(self.handoff.copy_on_select)),
            // The settings worker wrote the current draft (the sheet's
            // "Saved"); null without a settings store.
            (
                "settings_saved",
                self.settings
                    .as_ref()
                    .map_or(Value::Null, |host| json!(host.ready())),
            ),
            // A paste is being prepared or sent (a test waits for it to end
            // before the next case's marker, which the old tests slept for).
            (
                "pasting",
                json!(
                    self.handoff.preparing
                        || self.handoff.workers != 0
                        || self
                            .handoff
                            .core_ticket
                            .as_ref()
                            .is_some_and(|ticket| !ticket.is_finished())
                ),
            ),
            ("second", json!(second)),
            ("peer", super::peer::field()),
            // #229: where the terminal keys show, the keyboard's placement,
            // the drawn latch and the keyboard bar's button count.
            ("key_bar", self.qa_key_bar_field(window)),
            // UI test redesign S6: the P0 smoke screen's input text (what
            // the field draws), the terminal's font size, the local theme
            // and the selected host (P11, P15).
            ("p0_input", json!(self.input.text)),
            ("font_size", json!(self.terminal_font_metrics(window).0)),
            ("theme", json!(self.local_theme.key())),
            ("host", json!(self.terminal.reconnect.target)),
            // L61: the sidebar row whose long-press menu is open, so the
            // edit-menu target test sees that menu still opens.
            (
                "row_menu",
                self.shell
                    .sidebar
                    .long_press
                    .as_ref()
                    .map_or(Value::Null, |menu| {
                        json!(match menu.row {
                            crate::shell::sidebar::long_press::PressedRow::Host(_) => "host",
                            crate::shell::sidebar::long_press::PressedRow::Space { .. } => "space",
                            crate::shell::sidebar::long_press::PressedRow::Forward { .. } =>
                                "forward",
                        })
                    }),
            ),
            // #231: the space row whose menu is open (its row id).
            (
                "row_menu_space",
                self.shell
                    .sidebar
                    .long_press
                    .as_ref()
                    .and_then(|menu| match &menu.row {
                        crate::shell::sidebar::long_press::PressedRow::Space { id, .. } => {
                            Some(json!(id))
                        }
                        _ => None,
                    })
                    .unwrap_or(Value::Null),
            ),
            // #231: the open Close Space confirmation, with what it closes.
            (
                "space_close",
                self.shell
                    .sidebar
                    .close_confirm
                    .as_ref()
                    .map_or(Value::Null, |confirm| {
                        json!({
                            "space": confirm.target.space,
                            "tabs": confirm.question.tabs,
                            "title": confirm.question.title(),
                        })
                    }),
            ),
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
            // The window's scene is foreground-active (not behind Control
            // Center, a system alert or another focused scene).
            ("active", json!(window.is_window_active())),
        ]
    }
}

/// Counts each app lifecycle phase in `qa.state` (`lifecycle_active`, ...),
/// once per process, so a test sees the app went to the background and came
/// back without reading the receipt file. Each count is published with the
/// first frame after it, so `lifecycle_background` shows up after the return.
#[cfg(any(all(target_os = "ios", target_abi = "sim"), test))]
fn count_lifecycle(cx: &mut Context<Smoke>) {
    thread_local! {
        static COUNTING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }
    if COUNTING.with(|counting| counting.replace(true)) {
        return;
    }
    cx.on_app_lifecycle(|phase, _| super::count(super::lifecycle_key(phase)))
        .detach();
}
