//! L9: the Mods status item and overlay (Q3). Owner: L9.
//!
//! Mods shows in the status bar only while L0 has withdrawn a requested,
//! version-matched mod region; a region with a mismatched version is absent,
//! not withdrawn, and never counts (q1). Tapping Mods lists the withdrawn
//! regions in overlay O (ModsMax.dc.html, SplitHalf.dc.html).
use super::{
    style::{color, radius, text, weight},
    tab_row::icons::Icon,
};
use crate::{
    app::Smoke,
    layout::{DIALOG_WIDTH, OVERLAY_MARGIN, Region, Regions},
};
use gpui::{AnyElement, Context, Role, Window, div, prelude::*, px, rgb};

// ModsMax.dc.html:190–192.
const MODS_ICON_GAP: f32 = 6.;
const MODS_BADGE_SIZE: f32 = 16.;

/// "Mods n" only when `withdrawn` > 0.
pub(crate) fn mods_item(withdrawn: usize) -> Option<String> {
    (withdrawn > 0).then(|| format!("Mods {withdrawn}"))
}

/// The mod regions to lay out: the runtime's `request`. Simulator builds
/// take a fixed request from `YEBISU_LAYOUT_MODS_QA` (`defaults` for
/// artboard 11, `maximum` for artboard 10) so the Mods item can be checked
/// before the mod runtime lands (plan v2 7.15, Simulator).
pub(crate) fn requested(request: Regions) -> Regions {
    #[cfg(all(target_os = "ios", target_abi = "sim"))]
    {
        static QA: std::sync::OnceLock<Option<Regions>> = std::sync::OnceLock::new();
        let qa = QA.get_or_init(
            || match std::env::var("YEBISU_LAYOUT_MODS_QA").ok().as_deref() {
                Some("defaults") => Some(Regions::defaults()),
                Some("maximum") => Some(Regions::maximum()),
                _ => None,
            },
        );
        if let Some(qa) = qa {
            return *qa;
        }
    }
    request
}

/// A withdrawn region's name, as the canvas' accessibility labels word it.
pub(crate) fn region_label(region: Region) -> &'static str {
    match region {
        Region::M1 => "sidebar bottom",
        Region::M2 => "right dock",
        Region::M3 => "bottom band",
        Region::M4 => "top band",
    }
}

/// The Mods button's accessibility label, e.g. "Mods, 2 withdrawn: sidebar
/// bottom, top band"; `None` when nothing is withdrawn.
pub(crate) fn mods_accessibility_label(withdrawn: &[Region]) -> Option<String> {
    if withdrawn.is_empty() {
        return None;
    }
    let names: Vec<&str> = withdrawn.iter().copied().map(region_label).collect();
    Some(format!(
        "Mods, {} withdrawn: {}",
        withdrawn.len(),
        names.join(", ")
    ))
}

/// The overlay's rows, one per withdrawn region in region order.
pub(crate) fn mods_overlay_rows(withdrawn: &[Region]) -> Vec<String> {
    withdrawn
        .iter()
        .map(|region| format!("{region:?} · {}", region_label(*region)))
        .collect()
}

impl Smoke {
    pub(super) fn render_mods_status(
        &self,
        withdrawn: &[Region],
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let label = mods_accessibility_label(withdrawn)?;
        Some(
            div()
                .id("shell-status-mods")
                .debug_selector(|| "shell-status-mods".into())
                .role(Role::Button)
                .aria_label(label)
                .flex_none()
                .h_full()
                .flex()
                .items_center()
                .gap(px(MODS_ICON_GAP))
                .whitespace_nowrap()
                .text_color(rgb(color::MUTED))
                .child(Icon::Mods.muted())
                .child("Mods")
                .child(
                    div()
                        .debug_selector(|| "shell-status-mods-count".into())
                        .min_w(px(MODS_BADGE_SIZE))
                        .h(px(MODS_BADGE_SIZE))
                        .px_1()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(radius::CONTROL))
                        .bg(rgb(color::SLATE))
                        .text_color(rgb(color::TEXT))
                        .text_size(px(text::SMALL))
                        .font_weight(weight::BOLD)
                        .child(withdrawn.len().to_string()),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.toggle_mods_overlay();
                    cx.notify();
                }))
                .into_any_element(),
        )
    }

    /// Opens or closes the Mods list (the status bar's Mods item).
    pub(crate) fn toggle_mods_overlay(&mut self) {
        self.shell.fit.mods_open = !self.shell.fit.mods_open;
    }

    pub(crate) fn render_mods_overlay(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.shell.fit.mods_open {
            return None;
        }
        let withdrawn = self.frame_layout(window).withdrawn;
        // The list closes by itself once every region fits again.
        if withdrawn.is_empty() {
            self.shell.fit.mods_open = false;
            return None;
        }
        let width =
            DIALOG_WIDTH.min((f32::from(window.viewport_size().width) - OVERLAY_MARGIN).max(0.0));
        let rows = mods_overlay_rows(&withdrawn);
        Some(
            div()
                .id("shell-mods-overlay")
                .debug_selector(|| "shell-mods-overlay".into())
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .on_click(cx.listener(|this, _, _, cx| {
                    this.shell.fit.mods_open = false;
                    cx.notify();
                }))
                .child(
                    div()
                        .id("shell-mods-list")
                        .w(px(width))
                        .flex()
                        .flex_col()
                        .gap_1()
                        .p_4()
                        .rounded_md()
                        .bg(rgb(0x1c1c22))
                        .border_1()
                        .border_color(rgb(0x2b2933))
                        .text_color(rgb(0xd8dee9))
                        .child(
                            div()
                                .text_sm()
                                .child(mods_item(withdrawn.len()).unwrap_or_default()),
                        )
                        .children(
                            rows.into_iter()
                                .map(|row| div().text_sm().text_color(rgb(0xacb2bb)).child(row)),
                        ),
                )
                .into_any_element(),
        )
    }
}

#[cfg(test)]
#[path = "mods_tests.rs"]
mod tests;
