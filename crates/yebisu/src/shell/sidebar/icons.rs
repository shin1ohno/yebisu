//! Sidebar glyphs from the canvas. Controls supply their own accessibility names.
use crate::shell::style::{color, metric, radius};
use gpui::{Div, Role, Stateful, Styled, Svg, div, prelude::*, px, rgb, svg};

// Main.dc.html:27, :31, :34, :48, :87, :107, :121, :124.
const LARGE: f32 = 20.;
const STANDARD: f32 = 18.;
const CHEVRON: f32 = 14.;
pub(super) const PIN: f32 = 12.;

#[derive(Clone, Copy)]
pub(crate) enum Icon {
    Collapse,
    Search,
    Add,
    Right,
    Down,
    Hosts,
    Settings,
    Pin,
}

impl Icon {
    pub(crate) fn draw(self, ink: u32) -> Svg {
        let (size, view_box, stroke, shape) = match self {
            Self::Collapse => (
                LARGE,
                20,
                1.5,
                r#"<rect x="2.5" y="3.5" width="15" height="13" rx="2"/><path d="M7.5 3.5v13"/>"#,
            ),
            Self::Search => (
                STANDARD,
                20,
                1.5,
                r#"<circle cx="9" cy="9" r="5.5"/><path d="m13 13 4 4"/>"#,
            ),
            Self::Add => (STANDARD, 20, 1.5, r#"<path d="M10 4v12M4 10h12"/>"#),
            Self::Right => (CHEVRON, 14, 1.5, r#"<path d="m5 3 4 4-4 4"/>"#),
            Self::Down => (CHEVRON, 14, 1.5, r#"<path d="m3 5 4 4 4-4"/>"#),
            Self::Hosts => (
                STANDARD,
                20,
                1.5,
                r#"<rect x="3" y="4" width="14" height="5" rx="1.5"/><rect x="3" y="11" width="14" height="5" rx="1.5"/>"#,
            ),
            Self::Settings => (
                LARGE,
                20,
                1.5,
                r#"<circle cx="10" cy="10" r="2.5"/><path d="M10 2.5v2M10 15.5v2M2.5 10h2M15.5 10h2M4.7 4.7l1.4 1.4M13.9 13.9l1.4 1.4M4.7 15.3l1.4-1.4M13.9 6.1l1.4-1.4"/>"#,
            ),
            Self::Pin => (
                PIN,
                24,
                2.,
                r#"<path stroke-linecap="round" stroke-linejoin="round" d="M16 3 21 8 17 9 13 13 13 17 7 11 11 11 15 7ZM10 14l-7 7"/>"#,
            ),
        };
        let data = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {view_box} {view_box}" fill="none" stroke="white" stroke-width="{stroke}">{shape}</svg>"#
        );
        svg()
            .data(data.as_bytes())
            .size(px(size))
            .flex_none()
            .text_color(rgb(ink))
    }

    pub(crate) fn button(
        self,
        id: &'static str,
        label: impl Into<gpui::SharedString>,
        enabled: bool,
    ) -> Stateful<Div> {
        self.button_with_ink(
            id,
            label,
            if enabled {
                color::MUTED
            } else {
                color::DISABLED
            },
        )
    }

    /// `button` drawn in `ink` (the magnifier is accent while filtering).
    pub(crate) fn button_with_ink(
        self,
        id: &'static str,
        label: impl Into<gpui::SharedString>,
        ink: u32,
    ) -> Stateful<Div> {
        div()
            .id(id)
            .debug_selector(move || id.to_owned())
            .role(Role::Button)
            .aria_label(label.into())
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(metric::TOUCH))
            .rounded(px(radius::CONTROL))
            .child(self.draw(ink))
    }
}
