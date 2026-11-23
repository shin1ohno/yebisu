//! Agent-kind marks before agent names in the sidebar, its space rows and the
//! rail. The table follows the fork's `AgentIcon::from_identity`
//! (herdr-gpui crates/sheprd-gpui/src/icons.rs:34), keyed on Herdr's canonical
//! `agent_label` identity. The SVGs are copied unchanged into `assets/icons/`
//! and embedded in the binary, as the notification sounds are; assets/icons/README.md
//! and NOTICE carry their sources and licenses.
use crate::shell::style::color;
use gpui::{Div, div, prelude::*, px, rgb, svg};
use sheprd_client::protocol::ClientShellAgent;

/// Sizes and gaps from canvas v27 (Main.dc.html, Rail.dc.html, SidebarMixed.dc.html):
/// AGENTS rows 14 pt, the space row's second line 13 pt, the rail 12 pt, with
/// 5 pt before the name (4 pt in the rail).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Place {
    pub(crate) size: f32,
    pub(crate) gap: f32,
}

pub(crate) const AGENTS: Place = Place { size: 14., gap: 5. };
pub(crate) const SPACE: Place = Place { size: 13., gap: 5. };
pub(crate) const RAIL: Place = Place { size: 12., gap: 4. };
/// The canvas ink for every agent icon, #acb2bb (the secondary text color).
pub(crate) const INK: u32 = color::MUTED;

/// Declares each mark once: its variant, the identity and the embedded file
/// `assets/icons/agent-<identity>.svg`.
macro_rules! agent_icons {
    ($($variant:ident => $label:literal,)+) => {
        /// An agent's kind as its icon shows it.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub(crate) enum AgentIcon {
            $($variant,)+
            Generic,
        }

        impl AgentIcon {
            #[cfg(test)]
            pub(crate) const ALL: &[Self] = &[$(Self::$variant,)+ Self::Generic];

            /// The icon for a daemon identity, compared without regard to
            /// case. Unknown, empty and missing identities are generic.
            pub(crate) fn from_identity(identity: Option<&str>) -> Self {
                let Some(identity) = identity else {
                    return Self::Generic;
                };
                $(if identity.eq_ignore_ascii_case($label) {
                    return Self::$variant;
                })+
                Self::Generic
            }

            /// The identity the icon is named after (`generic` for the fallback).
            pub(crate) const fn label(self) -> &'static str {
                match self {
                    $(Self::$variant => $label,)+
                    Self::Generic => "generic",
                }
            }

            pub(crate) const fn bytes(self) -> &'static [u8] {
                match self {
                    $(Self::$variant => include_bytes!(concat!(
                        "../../../../../assets/icons/agent-", $label, ".svg"
                    )),)+
                    Self::Generic => {
                        include_bytes!("../../../../../assets/icons/agent-generic.svg")
                    }
                }
            }
        }
    };
}

// The fork's table in its order (Herdr's `src/detect/mod.rs` `agent_label`).
// `omo` is left out: the fork draws it from agent-omo.png, which has no
// license record, so Omo shows the generic icon (controller decision 2026-11-23).
agent_icons! {
    Pi => "pi",
    Claude => "claude",
    Codex => "codex",
    Gemini => "gemini",
    Cursor => "cursor",
    Devin => "devin",
    Antigravity => "agy",
    Cline => "cline",
    Omp => "omp",
    Mastracode => "mastracode",
    OpenCode => "opencode",
    Copilot => "copilot",
    Kimi => "kimi",
    Kiro => "kiro",
    Droid => "droid",
    Amp => "amp",
    Grok => "grok",
    Hermes => "hermes",
    Kilo => "kilo",
    Qodercli => "qodercli",
    Qwen => "qwen",
    Letta => "letta",
    Maki => "maki",
    Muse => "muse",
}

impl AgentIcon {
    /// A Herdr 1 agent: its canonical identity, then (as the fork's iPad
    /// chrome does for ACP's generic identity) its display name.
    pub(crate) fn of_agent(agent: &ClientShellAgent) -> Self {
        match Self::from_identity(agent.agent.as_deref()) {
            Self::Generic => Self::from_identity(agent.display_agent.as_deref()),
            icon => icon,
        }
    }

    /// The mark at `place`'s size in `INK`. `row` names the row for the
    /// debug selector `<row>-agent-icon-<label>`.
    pub(crate) fn draw(self, row: &str, place: Place) -> Div {
        let selector = format!("{row}-agent-icon-{}", self.label());
        div()
            .debug_selector(move || selector)
            .flex_none()
            .size(px(place.size))
            .child(
                svg()
                    .data(self.bytes())
                    .size(px(place.size))
                    .flex_none()
                    .text_color(rgb(INK)),
            )
    }
}

#[cfg(test)]
#[path = "agent_icon_tests.rs"]
mod tests;
