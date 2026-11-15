//! A dedicated native input entity: its callbacks cannot enqueue terminal input.
use crate::{input::Input, terminal_painter::TerminalPainter};
use gpui::{prelude::*, *};
use sheprd_client::scrollback::MAX_SEARCH_QUERY_BYTES;
use std::ops::Range;

pub(crate) enum Event {
    Changed,
    Close,
    OwnEscape,
    Next(bool),
}
pub(crate) struct Field {
    pub focus: FocusHandle,
    pub input: Input,
    retired: bool,
    bounds: Option<Bounds<Pixels>>,
    painter: TerminalPainter,
    oversized: bool,
}
impl EventEmitter<Event> for Field {}
impl Field {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let mut painter = TerminalPainter::default();
        painter.set_appearance(14., 20., crate::theme::Theme::default());
        Self {
            focus: cx.focus_handle(),
            input: Input::default(),
            retired: false,
            bounds: None,
            painter,
            oversized: false,
        }
    }
    pub fn retire(&mut self) {
        self.retired = true;
        self.input = Input::default();
    }
    fn changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.notify_text_input_changed(&self.focus);
        cx.emit(Event::Changed);
        cx.notify();
    }
    /// Reject large native insertions before constructing a new String.
    fn replace(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        marked: bool,
        selected: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.retired || !self.focus.is_focused(window) {
            return;
        }
        let chosen = range
            .clone()
            .or(self.input.marked.clone())
            .unwrap_or(self.input.selected.clone());
        let removed = self
            .input
            .byte(chosen.end.max(chosen.start))
            .saturating_sub(self.input.byte(chosen.start));
        if self
            .input
            .text
            .len()
            .saturating_sub(removed)
            .saturating_add(text.len())
            > MAX_SEARCH_QUERY_BYTES
        {
            self.oversized = true;
            self.changed(window, cx);
            return;
        }
        self.oversized = false;
        // UIKit uses ordinary replacement for selection deletion inside marked
        // text. Keep the surrounding uncommitted clause, including its offset.
        let retained_mark = self.input.marked.clone().filter(|span| {
            !marked
                && range.as_ref().is_some_and(|range| {
                    range.start <= range.end
                        && span.start <= range.start
                        && range.end <= span.end
                        && range != span
                })
        });
        let removed_utf16 = self.input.text
            [self.input.byte(chosen.start)..self.input.byte(chosen.end.max(chosen.start))]
            .encode_utf16()
            .count();
        self.input.replace(range, text, marked, selected);
        if let Some(span) = retained_mark {
            let end = span
                .end
                .saturating_sub(removed_utf16)
                .saturating_add(text.encode_utf16().count());
            self.input.marked = (span.start < end).then_some(span.start..end);
        }
        self.changed(window, cx);
    }
    pub fn query(&self) -> Option<&str> {
        (!self.oversized).then_some(self.input.text.as_str())
    }
    fn copy(&self, cx: &mut Context<Self>) {
        let start = self.input.byte(self.input.selected.start);
        let end = self.input.byte(self.input.selected.end);
        if start < end {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.input.text[start..end].to_owned(),
            ));
        }
    }
    fn paste(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace(None, &text, false, None, window, cx);
        }
    }
    pub(crate) fn cut(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.retired || !self.focus.is_focused(window) {
            return;
        }
        self.copy(cx);
        // Cut deletes exactly what Copy selected. An IME commit uses None to
        // replace the entire marked clause instead.
        self.replace(
            Some(self.input.selected.clone()),
            "",
            false,
            None,
            window,
            cx,
        );
    }
    pub(crate) fn key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.retired {
            cx.stop_propagation();
            return;
        }
        #[cfg(target_os = "ios")]
        if let Some(action) = herdr_uikit::native_edit_action() {
            match action {
                herdr_uikit::NativeEditAction::Copy => self.copy(cx),
                herdr_uikit::NativeEditAction::Cut => self.cut(window, cx),
                herdr_uikit::NativeEditAction::Paste => self.paste(window, cx),
                herdr_uikit::NativeEditAction::SelectAll => {
                    self.input.selected = 0..self.input.text.encode_utf16().count();
                    self.changed(window, cx);
                }
            }
            cx.stop_propagation();
            window.prevent_default();
            return;
        }
        let key = event.keystroke.key.as_str();
        if key == "escape" {
            if !event.is_held {
                cx.emit(Event::OwnEscape);
                if let Some(marked) = self.input.marked.clone() {
                    self.input.replace(Some(marked), "", false, None);
                    self.changed(window, cx);
                } else {
                    cx.emit(Event::Close);
                }
            }
        } else if key == "enter" && self.input.marked.is_none() {
            if !event.is_held {
                cx.emit(Event::Next(event.keystroke.modifiers.shift));
            }
        } else if event.keystroke.modifiers.platform && matches!(key, "a" | "c" | "x" | "v") {
            match key {
                "a" => {
                    self.input.selected = 0..self.input.text.encode_utf16().count();
                    self.changed(window, cx);
                }
                "c" => self.copy(cx),
                "x" => self.cut(window, cx),
                "v" => self.paste(window, cx),
                _ => {}
            }
        } else if key == "backspace" && self.input.marked.is_none() {
            let mut range = self.input.selected.clone();
            if range.is_empty() {
                let byte = self.input.byte(range.start);
                if let Some(ch) = self.input.text[..byte].chars().next_back() {
                    range.start = range.start.saturating_sub(ch.len_utf16());
                }
            }
            self.replace(Some(range), "", false, None, window, cx);
        } else {
            // Printable/IME events must retain native default insertion.
            // The parent is fenced by the active bar, not propagation alone.
            return;
        }
        cx.stop_propagation();
        window.prevent_default();
    }
    fn cursor(&self) -> Option<Bounds<Pixels>> {
        let bounds = self.bounds?;
        Some(Bounds::new(bounds.origin, size(px(1.), px(20.))))
    }
}
impl Render for Field {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        div()
            .id("terminal-search-field")
            .debug_selector(|| "terminal-search-field".into())
            .track_focus(&self.focus)
            .h(px(28.))
            .w_full()
            .bg(rgb(0x3b4252))
            .on_key_down(cx.listener(Self::key))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| this.focus.focus(window, cx)),
            )
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, cx| {
                        entity.update(cx, |this, cx| {
                            if this.retired {
                                return;
                            }
                            this.bounds = Some(bounds);
                            let text = if this.input.text.is_empty() {
                                "Search terminal"
                            } else {
                                &this.input.text
                            };
                            this.painter.paint_composition(
                                text,
                                bounds.origin,
                                bounds,
                                &font("SauceCodePro Nerd Font Mono"),
                                window,
                            );
                            if let Some(cursor) = this.cursor() {
                                let caret = this.painter.composition_bounds(
                                    &this.input.text,
                                    this.input.selected.end..this.input.selected.end,
                                    cursor,
                                    bounds,
                                    &font("SauceCodePro Nerd Font Mono"),
                                    window,
                                );
                                window.paint_quad(fill(caret, rgb(0xd8dee9)));
                            }
                            let _ = cx;
                        });
                        let focus = entity.read(cx).focus.clone();
                        window.handle_input(&focus, ElementInputHandler::new(bounds, entity), cx);
                    },
                )
                .size_full(),
            )
    }
}
impl EntityInputHandler for Field {
    fn accepts_text_input(&self, window: &mut Window, _: &mut Context<Self>) -> bool {
        !self.retired && self.focus.is_focused(window)
    }
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let start = self.input.byte(range.start);
        let end = self.input.byte(range.end.max(range.start));
        *actual = Some(
            self.input.text[..start].encode_utf16().count()
                ..self.input.text[..end].encode_utf16().count(),
        );
        Some(self.input.text[start..end].to_owned())
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.input.selected.clone(),
            reversed: false,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.input.marked.clone()
    }
    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.input.text.encode_utf16().count())
    }
    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.retired {
            self.input.marked = None;
            self.changed(window, cx);
        }
    }
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace(range, text, false, None, window, cx);
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace(range, text, true, selected, window, cx);
    }
    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        window: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(self.painter.composition_bounds(
            &self.input.text,
            range,
            self.cursor()?,
            self.bounds?,
            &font("SauceCodePro Nerd Font Mono"),
            window,
        ))
    }
    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        window: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        self.painter.composition_index(
            &self.input.text,
            point,
            self.cursor()?,
            self.bounds?,
            &font("SauceCodePro Nerd Font Mono"),
            window,
        )
    }
    fn set_selected_text_range(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.retired {
            return;
        }
        let start = self.input.byte(range.start);
        let end = self.input.byte(range.end.max(range.start));
        self.input.selected = self.input.text[..start].encode_utf16().count()
            ..self.input.text[..end].encode_utf16().count();
        self.changed(window, cx);
    }
}
