//! The app root's hook into the QA accessibility layer: it publishes
//! `qa.state` on every frame from launch, on every screen, so a test never
//! waits for a marked control to be drawn before it can read the state.
use crate::app::Smoke;
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
            Some(super::layer::publisher(move |cx| {
                view.read(cx).qa_ax_fields()
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
        vec![
            ("screen", json!(screen)),
            ("sheet", sheet),
            ("menu", menu),
            ("taps", json!(self.taps)),
        ]
    }
}
