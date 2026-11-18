//! L4: the sidebar filter. Owner: L4.
//!
//! The magnifier narrows spaces and agents by what the user types. This file
//! holds the matching rule, the empty-result outcome and the query kept in
//! `RailState`; the sidebar list (L3) calls them. The field sits in the
//! sidebar header: a tap focuses it and hardware keys edit the query
//! (`filter_edit`); Escape clears it, Return leaves it. The keys stop at the
//! field, so none reaches the terminal while it has focus.
use super::sidebar::{AgentRow, model::AgentLine};
use crate::app::Smoke;
use gpui::{
    AnyElement, Context, FocusHandle, KeyDownEvent, Keystroke, MouseButton, Role, Window, div,
    prelude::*, px, rgb,
};

/// Label of the button that clears an active filter.
pub(crate) const CLEAR_LABEL: &str = "Clear filter";
/// The field's text while the query is empty, and its spoken name.
pub(crate) const FILTER_PLACEHOLDER: &str = "Filter";
pub(crate) const FILTER_LABEL: &str = "Filter spaces and agents";
/// The field's text, and its reason, while it cannot take keys: no hardware
/// keyboard is known (the software keyboard does not open for it yet).
pub(crate) const FILTER_NEEDS_KEYBOARD: &str = "Filter · hardware keyboard";
pub(crate) const FILTER_NEEDS_KEYBOARD_LABEL: &str =
    "Filter spaces and agents, needs a hardware keyboard";
/// The field's text, and its reason, on Herdr 2: the Herdr 2 sidebar does
/// not filter yet, so the field takes no keys there.
pub(crate) const FILTER_HERDR1_ONLY: &str = "Filter · Herdr 1 only";
pub(crate) const FILTER_HERDR1_ONLY_LABEL: &str = "Filter, not available for Herdr 2 yet";
/// Longest query the field keeps, in characters.
pub(crate) const MAX_QUERY_CHARS: usize = 64;

/// What one key press in the focused field does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FilterEdit {
    /// The query becomes this text.
    Set(String),
    /// Escape: clear the query and leave the field.
    Clear,
    /// Return: keep the query and leave the field.
    Leave,
    /// Not the field's key (a shortcut with Command, Control or Option):
    /// it goes on to the app.
    Pass,
}

/// The edit `keystroke` makes to `query`. Backspace removes the last
/// character; a printable character is appended up to `MAX_QUERY_CHARS`.
pub(crate) fn filter_edit(query: &str, keystroke: &Keystroke) -> FilterEdit {
    let modifiers = keystroke.modifiers;
    if modifiers.platform || modifiers.control || modifiers.alt || modifiers.function {
        return FilterEdit::Pass;
    }
    match keystroke.key.as_str() {
        "escape" => return FilterEdit::Clear,
        "enter" => return FilterEdit::Leave,
        "backspace" => {
            let mut text = query.to_owned();
            text.pop();
            return FilterEdit::Set(text);
        }
        _ => {}
    }
    let typed = keystroke
        .key_char
        .as_deref()
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_control());
    let text: String = query.chars().chain(typed).take(MAX_QUERY_CHARS).collect();
    FilterEdit::Set(text)
}

/// The AGENTS lines a query leaves, in their order (`filter_rows`).
pub(crate) fn filter_lines(lines: Vec<AgentLine>, query: &str) -> Vec<AgentLine> {
    let rows: Vec<AgentRow> = lines.iter().map(|line| line.row.clone()).collect();
    let kept: Vec<&AgentRow> = filter_rows(&rows, query);
    lines
        .into_iter()
        .filter(|line| kept.iter().any(|row| row.identity == line.row.identity))
        .collect()
}

/// True when every word of `query` appears in `text`, ignoring case. An empty
/// query matches everything.
pub(crate) fn matches(query: &str, text: &str) -> bool {
    let haystack = text.to_lowercase();
    query
        .split_whitespace()
        .all(|word| haystack.contains(&word.to_lowercase()))
}

/// The rows a query leaves, in their order. A row matches by its name or its
/// host, so `pro-dev` lists that host's agents.
pub(crate) fn filter_rows<'a>(rows: &'a [AgentRow], query: &str) -> Vec<&'a AgentRow> {
    rows.iter()
        .filter(|row| matches(query, &format!("{} {}", row.name, row.identity.host)))
        .collect()
}

/// What the list shows for a query and the number of rows it leaves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FilterOutcome {
    /// No query: the list is whole (an empty list is L3's "New space" state).
    Unfiltered,
    Results(usize),
    /// The query left nothing: say so and offer the way out (D7).
    Empty {
        clear_label: &'static str,
    },
}

pub(crate) fn filter_outcome(query: &str, shown: usize) -> FilterOutcome {
    if query.trim().is_empty() {
        FilterOutcome::Unfiltered
    } else if shown == 0 {
        FilterOutcome::Empty {
            clear_label: CLEAR_LABEL,
        }
    } else {
        FilterOutcome::Results(shown)
    }
}

impl Smoke {
    pub(crate) fn sidebar_filter(&self) -> &str {
        &self.shell.rail.filter
    }

    pub(crate) fn set_sidebar_filter(&mut self, query: &str) {
        self.shell.rail.filter = query.to_owned();
    }

    pub(crate) fn clear_sidebar_filter(&mut self) {
        self.shell.rail.filter.clear();
    }

    fn filter_focus(&mut self, cx: &mut Context<Self>) -> FocusHandle {
        self.shell
            .rail
            .filter_focus
            .get_or_insert_with(|| cx.focus_handle())
            .clone()
    }

    /// A hardware key in the focused field. Every key the field takes stops
    /// here, so it never reaches the terminal or the window's key line.
    fn filter_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match filter_edit(&self.shell.rail.filter, &event.keystroke) {
            FilterEdit::Pass => return,
            FilterEdit::Set(text) => self.set_sidebar_filter(&text),
            FilterEdit::Clear => {
                self.clear_sidebar_filter();
                self.leave_filter(window, cx);
            }
            FilterEdit::Leave => self.leave_filter(window, cx),
        }
        cx.stop_propagation();
        cx.notify();
    }

    /// Hands the keys back to the terminal.
    fn leave_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.terminal_focus.focus(window, cx);
    }

    /// Called once per frame after the sidebar column: when the focused
    /// field was not drawn (rail, hidden sidebar) or no hardware keyboard is
    /// known any more, the keys go back to the terminal, since GPUI would
    /// send them to no one.
    pub(crate) fn release_hidden_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let drawn = std::mem::take(&mut self.shell.rail.filter_drawn);
        if self.filter_focused(window) && (!drawn || !self.shell.fit.hardware_keyboard) {
            self.leave_filter(window, cx);
        }
    }

    /// Whether the sidebar filter field holds the keys.
    pub(crate) fn filter_focused(&self, window: &Window) -> bool {
        self.shell
            .rail
            .filter_focus
            .as_ref()
            .is_some_and(|focus| focus.is_focused(window))
    }

    /// The focus the terminal's text input is registered under: the filter
    /// field's while it holds the keys, else the terminal's. The field takes
    /// only hardware key presses, so text that still comes through the
    /// platform text input (software keyboard, dictation, IME) reaches
    /// `release_filter_for_text_input` instead of being dropped.
    pub(crate) fn text_input_focus(&self, window: &Window) -> FocusHandle {
        match &self.shell.rail.filter_focus {
            Some(focus) if focus.is_focused(window) => focus.clone(),
            _ => self.terminal_focus.clone(),
        }
    }

    /// Text arrived through the platform text input while the field held the
    /// keys (re-review d68b H1): the software keyboard is the input path, so
    /// the hardware keyboard flag is stale. A keyboard detached after a
    /// hardware press, with the software keyboard floating or split, leaves
    /// it set: herdr-uikit reports no inset for an undocked keyboard. Forget
    /// the flag and hand the keys back to the terminal, which takes the text.
    pub(crate) fn release_filter_for_text_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.filter_focused(window) {
            self.shell.fit.hardware_keyboard = false;
            self.leave_filter(window, cx);
            cx.notify();
        }
    }

    /// The field that takes no keys, with its reason.
    fn filter_unavailable(text: &'static str, label: &'static str) -> AnyElement {
        div()
            .id("sidebar-filter-unavailable")
            .debug_selector(|| "sidebar-filter-unavailable".to_owned())
            .role(Role::SearchInput)
            .aria_label(label)
            .flex_1()
            .min_w_0()
            .h(px(32.))
            .flex()
            .items_center()
            .px_2()
            .overflow_hidden()
            .whitespace_nowrap()
            .text_ellipsis()
            .text_color(rgb(0x6b7280))
            .child(text)
            .into_any_element()
    }

    /// The filter field in the sidebar header: the magnifier, the query (or
    /// "Filter"), and × while a query is set.
    pub(crate) fn render_filter_field(&mut self, cx: &mut Context<Self>) -> AnyElement {
        // Herdr 2's sidebar does not read the query: the field would take
        // keys and filter nothing (review F4).
        if self.terminal.core_mode {
            return Self::filter_unavailable(FILTER_HERDR1_ONLY, FILTER_HERDR1_ONLY_LABEL);
        }
        // Only hardware keys reach the field; without a hardware keyboard a
        // focused field would hold the keys no one can type (review F1).
        if !self.shell.fit.hardware_keyboard && self.shell.rail.filter.is_empty() {
            return Self::filter_unavailable(FILTER_NEEDS_KEYBOARD, FILTER_NEEDS_KEYBOARD_LABEL);
        }
        self.shell.rail.filter_drawn = true;
        let focus = self.filter_focus(cx);
        let query = self.shell.rail.filter.clone();
        let shown = if query.is_empty() {
            FILTER_PLACEHOLDER.to_owned()
        } else {
            query.clone()
        };
        div()
            .id("sidebar-filter")
            .debug_selector(|| "sidebar-filter".to_owned())
            .role(Role::SearchInput)
            .aria_label(FILTER_LABEL)
            .track_focus(&focus)
            .flex_1()
            .min_w_0()
            .h(px(32.))
            .flex()
            .items_center()
            .gap_1()
            .px_2()
            .rounded_md()
            .bg(rgb(0x1c1c22))
            .focus(|style| style.bg(rgb(0x2b2933)))
            .text_color(rgb(if query.is_empty() { 0x6b7280 } else { 0xd8dee9 }))
            .on_mouse_down(
                MouseButton::Left,
                // Without a hardware keyboard a set query stays visible with
                // its ×; `release_hidden_filter` gives a focus taken then back
                // to the terminal in the same frame.
                cx.listener(move |_, _, window, cx| {
                    focus.focus(window, cx);
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_key_down(cx.listener(Self::filter_key))
            .child(div().flex_none().child("⌕"))
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .child(shown),
            )
            .when(!query.is_empty(), |field| {
                field.child(
                    div()
                        .id("sidebar-filter-clear")
                        .debug_selector(|| "sidebar-filter-clear".to_owned())
                        .role(Role::Button)
                        .aria_label(CLEAR_LABEL)
                        .flex_none()
                        .px_1()
                        .child("×")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.clear_sidebar_filter();
                            cx.notify();
                        })),
                )
            })
            .into_any_element()
    }

    /// The empty-result row: the button that clears the filter. `None` while
    /// no filter is set; the list shows it when `filter_outcome` is `Empty`.
    pub(crate) fn render_filter_clear(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.shell.rail.filter.trim().is_empty() {
            return None;
        }
        Some(
            div()
                .id("filter-clear")
                .debug_selector(|| "filter-clear".to_owned())
                .role(Role::Button)
                .aria_label(CLEAR_LABEL)
                .min_h(px(44.))
                .px_3()
                .flex()
                .items_center()
                .text_color(rgb(0x88c0d0))
                .child(CLEAR_LABEL)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.clear_sidebar_filter();
                    cx.notify();
                }))
                .into_any_element(),
        )
    }
}

#[cfg(test)]
#[path = "filter_tests.rs"]
mod tests;
