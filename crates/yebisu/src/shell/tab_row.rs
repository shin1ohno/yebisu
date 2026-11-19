//! L2a: the 36 pt tab row. Owner: L2a.
//!
//! Tabs scroll sideways inside their own strip; + (new tab), split and ...
//! (the overflow menu) are pinned to the right edge and never scroll away
//! (plan v2 7.5). Each tab carries its own 32×36 × button, a separate hit
//! from the tab body (S-9).
use crate::{app::Smoke, board::snapshot, layout::TAB_ROW_HEIGHT};
use gpui::{AnyElement, Context, div, prelude::*, px, rgb};
use sheprd_client::Method;
use sheprd_commands::Command;

/// Width of a tab's × button.
pub(crate) const CLOSE_WIDTH: f32 = 32.0;
/// Width of each pinned action (+, split, ...), canvas Main.dc.html.
pub(crate) const PINNED_ACTION_WIDTH: f32 = 44.0;
/// The glyph of the button that opens the ... menu.
pub(crate) const MORE_GLYPH: &str = "…";
/// The pinned actions, left to right.
pub(crate) const PINNED_ACTIONS: [TabHit; 3] = [TabHit::NewTab, TabHit::Split, TabHit::Overflow];
/// A tab never grows past this; a longer name is cut with an ellipsis so it
/// cannot push its own × out.
pub(crate) const MAX_TAB_WIDTH: f32 = 240.0;
/// The refusal while the host is suspended or speaks another version.
pub(crate) const HOST_UNAVAILABLE: &str = "Host unavailable or command pending";
/// Left padding before a tab's label.
const TAB_PADDING: f32 = 12.0;

/// Tab row metrics (plan v2 7.5 `tab_row_metrics`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct TabRowMetrics {
    pub(crate) height: f32,
    pub(crate) close_width: f32,
    pub(crate) close_height: f32,
}

/// What a tap on the tab row hits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TabHit {
    Select(usize),
    Close(usize),
    NewTab,
    Split,
    Overflow,
}

pub(crate) fn tab_row_metrics() -> TabRowMetrics {
    TabRowMetrics {
        height: TAB_ROW_HEIGHT,
        close_width: CLOSE_WIDTH,
        close_height: TAB_ROW_HEIGHT,
    }
}

/// Width the pinned actions take at the right edge.
pub(crate) fn pinned_width() -> f32 {
    PINNED_ACTION_WIDTH * PINNED_ACTIONS.len() as f32
}

/// Hit test for a tap at `x` given each tab's width and the row width; the
/// pinned actions (+, split, ...) stay at the right edge.
///
/// `x` is measured from the row's left edge with the tab strip scrolled to
/// its start. Tabs that do not fit run under the pinned actions, which win
/// every tap in their area.
pub(crate) fn tab_hit(x: f32, tab_widths: &[f32], row_width: f32) -> Option<TabHit> {
    if !(0.0..row_width).contains(&x) {
        return None;
    }
    let pinned_left = (row_width - pinned_width()).max(0.0);
    if x >= pinned_left {
        let slot = ((x - pinned_left) / PINNED_ACTION_WIDTH).floor();
        // `slot` is in 0..3 because x < row_width; the cast cannot truncate.
        return PINNED_ACTIONS.get(slot as usize).copied();
    }
    let mut left = 0.0;
    for (index, width) in tab_widths.iter().enumerate() {
        let width = width.clamp(0.0, MAX_TAB_WIDTH);
        let right = left + width;
        if x < right {
            let close_left = right - CLOSE_WIDTH.min(width);
            return Some(if x >= close_left {
                TabHit::Close(index)
            } else {
                TabHit::Select(index)
            });
        }
        left = right;
    }
    None
}

/// One tab of the focused space: (tab id, label, focused).
type TabEntry = (String, String, bool);

impl Smoke {
    pub(super) fn row_tabs(&self) -> Vec<TabEntry> {
        if self.terminal.core_mode {
            // L8: one Herdr 2 session is one tab (q4).
            return crate::shell::herdr2_frame::session::session_tabs();
        }
        snapshot(&self.terminal.live)
            .map(|s| {
                s.tabs
                    .iter()
                    .filter(|t| Some(&t.workspace_id) == s.focused_workspace_id.as_ref())
                    .map(|t| (t.tab_id.clone(), t.label.clone(), t.focused))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(crate) fn render_tab_row(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let tabs = self.row_tabs();
        let herdr1 = !self.terminal.core_mode;
        let strip = div()
            .id("shell-tab-strip")
            .flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_x_scroll()
            .children(
                tabs.into_iter()
                    .enumerate()
                    .map(|(index, (id, label, focused))| {
                        self.tab_element(index, id, label, focused, cx)
                    }),
            );
        let pinned = div()
            .flex()
            .flex_none()
            .h_full()
            .child(
                self.pinned_button("shell-new-tab", "+", herdr1, None, cx, |this, cx| {
                    this.tab_row_command(Command::Tab, cx)
                }),
            )
            .child(
                self.pinned_button("shell-split", "◫", herdr1, None, cx, |this, cx| {
                    this.tab_row_command(Command::SplitRight, cx)
                }),
            )
            .child(self.pinned_button(
                "shell-more",
                MORE_GLYPH,
                true,
                crate::qa_ax::mark(crate::qa_ax::ids::TABS_MORE, MORE_GLYPH),
                cx,
                |this, cx| {
                    this.shell.frame.overflow_open ^= true;
                    cx.notify();
                },
            ));
        Some(
            div()
                .id("shell-tab-row")
                .flex()
                .flex_none()
                .w_full()
                .h(px(TAB_ROW_HEIGHT))
                .bg(rgb(0x1c1c22))
                .text_color(rgb(0xd8dee9))
                .child(strip)
                .child(pinned)
                .into_any_element(),
        )
    }

    fn tab_element(
        &self,
        index: usize,
        id: String,
        label: String,
        focused: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let host = self.terminal.reconnect.target.clone();
        let generation = self.terminal.link.generation;
        let boot = snapshot(&self.terminal.live)
            .map(|s| s.boot_id.clone())
            .unwrap_or_default();
        let (select_host, select_id, select_boot) = (host.clone(), id.clone(), boot.clone());
        // Herdr 2's one session tab has no Herdr 1 tab to focus or close.
        let herdr1 = !self.terminal.core_mode;
        div()
            .id(("shell-tab", index))
            .flex()
            .flex_none()
            .items_center()
            .max_w(px(MAX_TAB_WIDTH))
            .h_full()
            .bg(rgb(if focused { 0x2b2933 } else { 0x1c1c22 }))
            .child(
                div()
                    .id(("shell-tab-body", index))
                    .debug_selector(move || format!("shell-tab-body-{index}"))
                    .flex_1()
                    .min_w_0()
                    .pl(px(TAB_PADDING))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .child(format!("{} · {label}", index + 1))
                    .when(herdr1, |body| {
                        body.on_click(cx.listener(move |this, _, window, cx| {
                            if let Err(e) =
                                this.focus_tab(&select_host, &select_id, &select_boot, generation)
                            {
                                this.terminal.live.status = e.to_string();
                            }
                            window.dismiss_virtual_keyboard();
                            cx.notify();
                        }))
                    }),
            )
            .when(herdr1, |tab| {
                tab.child(
                    div()
                        .id(("shell-tab-close", index))
                        .flex_none()
                        .w(px(CLOSE_WIDTH))
                        .h(px(TAB_ROW_HEIGHT))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(rgb(0xacb2bb))
                        .child("×")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if let Err(e) = this.close_tab_by_id(&host, &id, &boot, generation) {
                                this.terminal.live.status = e.to_string();
                            }
                            window.dismiss_virtual_keyboard();
                            cx.stop_propagation();
                            cx.notify();
                        })),
                )
            })
            .into_any_element()
    }

    fn pinned_button(
        &self,
        id: &'static str,
        glyph: &'static str,
        enabled: bool,
        qa_mark: Option<AnyElement>,
        cx: &mut Context<Self>,
        action: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> AnyElement {
        div()
            .id(id)
            .flex_none()
            .w(px(PINNED_ACTION_WIDTH))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .text_color(rgb(if enabled { 0xd8dee9 } else { 0x4c566a }))
            .child(glyph)
            .when(enabled, |view| {
                view.on_click(cx.listener(move |this, _, _, cx| action(this, cx)))
            })
            .children(qa_mark)
            .into_any_element()
    }

    fn tab_row_command(&mut self, command: Command, cx: &mut Context<Self>) {
        if let Err(e) = self.run_action(command) {
            self.terminal.live.status = e.to_string();
        }
        cx.notify();
    }

    /// Closes the tab `id` without focusing it first (S-9: × closes and does
    /// not select). It takes the same path as `run_action`'s Close tab: the
    /// input transition (native text retirement), the Settings fence, the
    /// handoff cleanup, and the suspended / version-mismatch refusals.
    pub(crate) fn close_tab_by_id(
        &mut self,
        host: &str,
        id: &str,
        boot: &str,
        generation: u64,
    ) -> crate::Result<()> {
        self.input_transition(|this| this.close_tab_by_id_inner(host, id, boot, generation))
    }

    fn close_tab_by_id_inner(
        &mut self,
        host: &str,
        id: &str,
        boot: &str,
        generation: u64,
    ) -> crate::Result<()> {
        if self.input_modal() {
            return Err(crate::Error::Settings(
                "terminal action blocked by settings".into(),
            ));
        }
        self.handoff.cancel();
        self.handoff.selection = None;
        if self.terminal.reconnect.target != host || self.terminal.link.generation != generation {
            return Err(crate::Error::Ssh("Stale tab; tap again".into()));
        }
        if self.terminal.reconnect.suspended || self.terminal.live.version_mismatch.get().is_some()
        {
            return Err(crate::Error::Ssh(HOST_UNAVAILABLE.into()));
        }
        self.actions.palette = false;
        let terminal = &mut self.terminal;
        let mut live = terminal
            .link
            .inbox
            .try_lock()
            .map_err(|_| crate::Error::Ssh("Host busy".into()))?;
        let s = snapshot(&live).ok_or_else(|| crate::Error::Ssh("Waiting for snapshot".into()))?;
        if !live.connected
            || live.version_mismatch.get().is_some()
            || live.surface.as_ref().is_some_and(|s| s.popup.is_some())
            || terminal.navigating.is_some()
            || s.boot_id != boot
            || !s.tabs.iter().any(|t| t.tab_id == id)
            || !Method::TabClose.advertised_in(&live.methods)
        {
            return Err(crate::Error::Ssh("Tab unavailable".into()));
        }
        let revision = s.revision;
        let handle = terminal
            .link
            .handle
            .as_ref()
            .ok_or_else(|| crate::Error::Ssh("Disconnected".into()))?;
        let request = handle
            .request(boot, Method::TabClose, serde_json::json!({ "tab_id": id }))
            .map_err(|e| crate::Error::Ssh(e.to_string()))?;
        terminal.navigating = Some(crate::board::Navigation {
            boot: boot.into(),
            revision,
            target: crate::board::Target::Command,
            request: request.clone(),
        });
        terminal.input.detach();
        live.pending_request = Some(request);
        live.completed_request = None;
        live.failed_request = None;
        Ok(())
    }
}

#[cfg(test)]
#[path = "tab_row_tests.rs"]
mod tests;
