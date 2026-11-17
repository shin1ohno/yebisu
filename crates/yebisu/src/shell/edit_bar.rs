//! L5: the edit bar shown only while selecting. Owner: L5.
//!
//! While a selection pauses typing (`Handoff::blocks_input`), the bar offers
//! Copy, Paste, Paste image and Cancel, and the status bar explains why typing
//! is paused. Cancel ends the selection and resumes typing the same way the
//! `Select` toggle does (`toggle_selection_inner`). The frame (L2a) places the
//! bar and the status text; this module owns what they contain.
use super::{ChromeState, EntryAction, EntryPoint, HerdrKind, Place};
use crate::app::Smoke;
use gpui::{AnyElement, Context, Window, div, prelude::*, px, rgb};

/// Status bar wording while typing is paused for a selection (design D5).
pub(crate) const PAUSED_TYPING_STATUS: &str = "Typing is paused until you cancel the selection";
/// Why Paste image is disabled on Herdr 2 (design D6).
pub(crate) const HERDR2_IMAGE_REASON: &str = "Herdr 2 accepts text only";
/// Accessibility name of the Cancel button (design canvas, Selection).
pub(crate) const CANCEL_LABEL: &str = "Cancel selection and resume typing";
/// Bar height, also the minimum touch target.
pub(crate) const EDIT_BAR_HEIGHT: f32 = 44.0;

const COPY_HINT: &str = "⌘C copies";
const ELLIPSIS: char = '…';
// Estimated button widths at the design's 16 px padding, used only to decide
// whether Paste image still fits one row (plan v2 7.10, narrow widths).
const COPY_WIDTH: f32 = 64.0;
const PASTE_WIDTH: f32 = 68.0;
const PASTE_IMAGE_WIDTH: f32 = 112.0;
const DIVIDER_WIDTH: f32 = 1.0;
const BAR_MARGIN: f32 = 10.0;

#[derive(Debug, Default)]
pub(crate) struct EditBarState {
    /// Width the frame offers the bar; `None` until the frame reports it.
    available_width: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EditBarItem {
    Copy,
    Paste,
    PasteImage,
    Cancel,
}

impl EditBarItem {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Copy => "Copy",
            Self::Paste => "Paste",
            Self::PasteImage => "Paste image",
            Self::Cancel => CANCEL_LABEL,
        }
    }

    /// The product entry point the item serves; Cancel leaves Select.
    pub(crate) fn action(self) -> EntryAction {
        match self {
            Self::Copy => EntryAction::Copy,
            Self::Paste => EntryAction::Paste,
            Self::PasteImage => EntryAction::PasteImage,
            Self::Cancel => EntryAction::Select,
        }
    }

    fn width(self) -> f32 {
        match self {
            Self::Copy => COPY_WIDTH,
            Self::Paste => PASTE_WIDTH,
            Self::PasteImage => PASTE_IMAGE_WIDTH,
            Self::Cancel => EDIT_BAR_HEIGHT,
        }
    }
}

/// The bar's buttons: all four while selecting, none otherwise.
pub(crate) fn edit_bar_items(selecting: bool) -> Vec<EditBarItem> {
    if selecting {
        vec![
            EditBarItem::Copy,
            EditBarItem::Paste,
            EditBarItem::PasteImage,
            EditBarItem::Cancel,
        ]
    } else {
        Vec::new()
    }
}

/// The status bar text while typing is paused for a selection.
pub(crate) fn paused_typing_status(selecting: bool) -> Option<&'static str> {
    selecting.then_some(PAUSED_TYPING_STATUS)
}

/// The full status line, `Selected 7 characters · ⌘C copies · Typing is
/// paused until you cancel the selection`, cut to `max_columns` characters:
/// the copy hint goes first, then the count, and what is left ends in `…`.
pub(crate) fn selection_status_line(selected_chars: usize, max_columns: usize) -> String {
    let unit = if selected_chars == 1 {
        "character"
    } else {
        "characters"
    };
    let count = format!("Selected {selected_chars} {unit}");
    let candidates = [
        format!("{count} · {COPY_HINT} · {PAUSED_TYPING_STATUS}"),
        format!("{count} · {PAUSED_TYPING_STATUS}"),
        PAUSED_TYPING_STATUS.to_owned(),
    ];
    for line in &candidates {
        if line.chars().count() <= max_columns {
            return line.clone();
        }
    }
    let keep = max_columns.saturating_sub(1);
    let mut cut: String = PAUSED_TYPING_STATUS.chars().take(keep).collect();
    if max_columns > 0 {
        cut.push(ELLIPSIS);
    }
    cut
}

/// Buttons that fit the bar's row and buttons moved to the `…` menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EditBarFit {
    pub(crate) shown: Vec<EditBarItem>,
    pub(crate) overflow: Vec<EditBarItem>,
}

/// One row at `width`: when the four buttons do not fit, Paste image moves to
/// the `…` menu; Copy, Paste and Cancel always stay on the bar.
pub(crate) fn edit_bar_fit(selecting: bool, width: f32) -> EditBarFit {
    let items = edit_bar_items(selecting);
    let needed = BAR_MARGIN * 2.0
        + items.iter().map(|item| item.width()).sum::<f32>()
        + DIVIDER_WIDTH * items.len().saturating_sub(1) as f32;
    if width >= needed {
        return EditBarFit {
            shown: items,
            overflow: Vec::new(),
        };
    }
    let (overflow, shown) = items
        .into_iter()
        .partition(|item| *item == EditBarItem::PasteImage);
    EditBarFit { shown, overflow }
}

/// Entry points the bar contributes to `chrome_entry_points`: none outside a
/// selection; Paste image is disabled with a reason on Herdr 2.
pub(crate) fn edit_bar_entry_points(state: &ChromeState) -> Vec<EntryPoint> {
    edit_bar_items(state.selecting)
        .into_iter()
        .map(|item| {
            let image_blocked = item == EditBarItem::PasteImage && state.kind == HerdrKind::Herdr2;
            EntryPoint {
                action: item.action(),
                place: Place::EditBar,
                enabled: !image_blocked,
                reason: image_blocked.then_some(HERDR2_IMAGE_REASON),
            }
        })
        .collect()
}

impl Smoke {
    /// Typing is paused for a selection: Select mode, or a live selection
    /// that is not a passive highlight kept after typing resumed.
    pub(crate) fn is_selecting(&self) -> bool {
        self.handoff.blocks_input()
    }

    /// The frame reports the width it can give the bar (`None` clears it).
    pub(crate) fn set_edit_bar_width(&mut self, width: Option<f32>) {
        self.shell.edit_bar.available_width = width;
    }

    /// The status line for the selection, or `None` when typing is not paused.
    pub(crate) fn selection_status(&self, max_columns: usize) -> Option<String> {
        paused_typing_status(self.is_selecting())?;
        let chars = self
            .handoff
            .selection
            .as_ref()
            .and_then(|selection| selection.text())
            .map_or(0, |text| text.chars().count());
        Some(selection_status_line(chars, max_columns))
    }

    pub(crate) fn render_edit_bar(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let selecting = self.is_selecting();
        if !selecting {
            return None;
        }
        let items = match self.shell.edit_bar.available_width {
            Some(width) => edit_bar_fit(selecting, width).shown,
            None => edit_bar_items(selecting),
        };
        let kind = if self.terminal.core_mode {
            HerdrKind::Herdr2
        } else {
            HerdrKind::Herdr1
        };
        let mut bar = div()
            .id("edit-bar")
            .debug_selector(|| "edit-bar".into())
            .absolute()
            .left(px(BAR_MARGIN))
            .top(px(4.0))
            .flex()
            .bg(rgb(0x2b2933))
            .rounded(px(10.0))
            .overflow_hidden()
            .occlude();
        for (index, item) in items.into_iter().enumerate() {
            if index > 0 {
                bar = bar.child(div().w(px(DIVIDER_WIDTH)).bg(rgb(0x3b3945)));
            }
            let enabled = !(item == EditBarItem::PasteImage && kind == HerdrKind::Herdr2);
            bar = bar.child(edit_bar_button(item, enabled, cx));
        }
        Some(bar.into_any_element())
    }

    /// Cancel: clears the selection and resumes typing (`input.rs:153`).
    pub(crate) fn cancel_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input_transition(|this| this.end_selection());
        self.terminal_focus.focus(window, cx);
        window.request_virtual_keyboard();
        cx.notify();
    }

    /// The state half of Cancel: the same end state as turning `Select` off
    /// (`toggle_selection_inner`), also when a selection exists without Select
    /// mode (drag or double-tap start), where toggling would turn Select on.
    pub(crate) fn end_selection(&mut self) {
        self.handoff.passive_highlight = false;
        self.handoff.select_mode = false;
        self.handoff.cancel();
        self.terminal.input.detach();
        self.handoff.selection = None;
        if !self.find_handoff.paused {
            self.terminal.input.fresh_gesture();
        }
    }
}

fn edit_bar_button(item: EditBarItem, enabled: bool, cx: &mut Context<Smoke>) -> AnyElement {
    let id = match item {
        EditBarItem::Copy => "edit-bar-copy",
        EditBarItem::Paste => "edit-bar-paste",
        EditBarItem::PasteImage => "edit-bar-paste-image",
        EditBarItem::Cancel => "edit-bar-cancel",
    };
    let (label, width) = match item {
        EditBarItem::Cancel => ("×", Some(EDIT_BAR_HEIGHT)),
        other => (other.label(), None),
    };
    let button = div()
        .id(id)
        .h(px(EDIT_BAR_HEIGHT))
        .flex()
        .items_center()
        .justify_center()
        .text_color(rgb(0xd8dee9))
        .map(|button| match width {
            Some(width) => button.w(px(width)),
            None => button.px(px(16.0)),
        })
        .child(label);
    if !enabled {
        return button.opacity(0.4).into_any_element();
    }
    button
        .on_click(cx.listener(move |this, _, window, cx| {
            match item {
                EditBarItem::Copy => this.copy_selection(cx),
                EditBarItem::Paste => this.start_paste(false, cx),
                EditBarItem::PasteImage => this.start_paste(true, cx),
                EditBarItem::Cancel => this.cancel_selection(window, cx),
            }
            cx.notify();
        }))
        .into_any_element()
}

#[cfg(test)]
#[path = "edit_bar_tests.rs"]
mod tests;
