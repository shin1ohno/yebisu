//! A dedicated native input entity: its callbacks cannot enqueue terminal input
//! while it is the search field. Once retired it stays GPUI's platform input
//! handler until the next draw; only then, behind the terminal's own guards,
//! does it hand callbacks to the terminal input path.
use crate::{app::Smoke, input::Input, terminal_painter::TerminalPainter};
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
    successor: Option<(WeakEntity<Smoke>, u64)>,
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
            successor: None,
        }
    }
    /// `frame` is the owner's render count when search closed.
    pub fn retire(&mut self, successor: WeakEntity<Smoke>, frame: u64) {
        self.retired = true;
        self.input = Input::default();
        self.successor = Some((successor, frame));
    }
    /// Runs `f` on the terminal input path for a retired field's native
    /// callback, or returns None and the field keeps its inert retired reply.
    ///
    /// GPUI swaps the platform input handler only in `Window::draw` (gpui-pre
    /// 0.3.12 window.rs:3685-3727), so the live UIKit client created at close
    /// still calls this field until the first post-close frame. The SDK never
    /// routes a retired client here (herdr-uikit 99d7928
    /// src/window/input_target.rs:75-80, 217). `Smoke` is never leased on
    /// this path: GPUI's platform handler enters through
    /// `AsyncWindowContext::update`, which refuses a borrowed app, and the
    /// owner drops its last handle to this field when it retires it.
    fn forward<R>(
        &self,
        cx: &mut App,
        f: impl FnOnce(&mut Smoke, &mut Context<Smoke>) -> R,
    ) -> Option<R> {
        let (successor, frame) = self.successor.as_ref()?;
        let smoke = successor.upgrade()?;
        smoke.update(cx, |smoke, cx| {
            smoke.inherits_retired_field(*frame).then(|| f(smoke, cx))
        })
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
// Each callback first offers itself to the terminal input path (`forward`);
// a refused or unretired callback keeps this field's own reply unchanged.
impl EntityInputHandler for Field {
    fn accepts_text_input(&self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if let Some(accepted) = self.forward(cx, |s, cx| s.accepts_text_input(window, cx)) {
            return accepted;
        }
        !self.retired && self.focus.is_focused(window)
    }
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual: &mut Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        if let Some(text) = self.forward(cx, |s, cx| {
            s.text_for_range(range.clone(), actual, window, cx)
        }) {
            return text;
        }
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
        ignore_disabled_input: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        if let Some(selection) = self.forward(cx, |s, cx| {
            s.selected_text_range(ignore_disabled_input, window, cx)
        }) {
            return selection;
        }
        Some(UTF16Selection {
            range: self.input.selected.clone(),
            reversed: false,
        })
    }
    fn marked_text_range(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        if let Some(marked) = self.forward(cx, |s, cx| s.marked_text_range(window, cx)) {
            return marked;
        }
        self.input.marked.clone()
    }
    fn text_length_utf16(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<usize> {
        if let Some(length) = self.forward(cx, |s, cx| s.text_length_utf16(window, cx)) {
            return length;
        }
        Some(self.input.text.encode_utf16().count())
    }
    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .forward(cx, |s, cx| s.unmark_text(window, cx))
            .is_some()
        {
            return;
        }
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
        if self
            .forward(cx, |s, cx| {
                s.replace_text_in_range(range.clone(), text, window, cx)
            })
            .is_some()
        {
            return;
        }
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
        if self
            .forward(cx, |s, cx| {
                s.replace_and_mark_text_in_range(range.clone(), text, selected.clone(), window, cx)
            })
            .is_some()
        {
            return;
        }
        self.replace(range, text, true, selected, window, cx);
    }
    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        element: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        if let Some(bounds) = self.forward(cx, |s, cx| {
            s.bounds_for_range(range.clone(), element, window, cx)
        }) {
            return bounds;
        }
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
        cx: &mut Context<Self>,
    ) -> Option<usize> {
        if let Some(index) =
            self.forward(cx, |s, cx| s.character_index_for_point(point, window, cx))
        {
            return index;
        }
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
        if self
            .forward(cx, |s, cx| {
                s.set_selected_text_range(range.clone(), window, cx)
            })
            .is_some()
        {
            return;
        }
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
