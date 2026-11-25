//! The forward sheet's Remote port field: an `Input` with its own focus
//! handle, as the Hosts sheet's name field is (`hosts_sheet/name_field.rs`),
//! so typed text and IME callbacks land here and never in the terminal.
//! It keeps ASCII digits only, at most five; Return asks the sheet to
//! forward and Escape to close.
use crate::{
    input::Input,
    shell::style::{color, metric, radius, text},
};
use gpui::{prelude::*, *};
use std::ops::Range;

/// The most digits a port has (65535).
pub(crate) const MAX_DIGITS: usize = 5;
/// What the empty field shows.
pub(crate) const PLACEHOLDER: &str = "3000";

/// What the field asks of the sheet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PortEvent {
    Submit,
    Close,
}

pub(crate) struct PortField {
    input: Input,
    focus: FocusHandle,
    /// The sheet shows an input error: the border is the danger color.
    pub(crate) invalid: bool,
    layout: Option<(ShapedLine, Bounds<Pixels>)>,
}

impl EventEmitter<PortEvent> for PortField {}

impl PortField {
    pub(crate) fn new(cx: &mut Context<Self>) -> Self {
        Self {
            input: Input::default(),
            focus: cx.focus_handle(),
            invalid: false,
            layout: None,
        }
    }

    pub(crate) fn focus_handle(&self) -> FocusHandle {
        self.focus.clone()
    }

    pub(crate) fn text(&self) -> &str {
        &self.input.text
    }

    /// Puts a chip's number in the field, the caret after it.
    pub(crate) fn set_text(&mut self, text: String, cx: &mut Context<Self>) {
        self.input.set_text(text);
        self.layout = None;
        cx.notify();
    }

    /// Whether `text` may go in place of `range`: digits only, and the
    /// field stays at most [`MAX_DIGITS`] long.
    fn admits(&self, range: &Range<usize>, text: &str) -> bool {
        let start = self.input.byte(range.start);
        let end = self.input.byte(range.end.max(range.start));
        text.bytes().all(|byte| byte.is_ascii_digit())
            && self.input.text.len() - (end - start) + text.len() <= MAX_DIGITS
    }

    fn edit(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.accepts_text_input(window, cx) {
            return;
        }
        // The software keyboard's Return arrives as a typed line break.
        if text.contains(['\n', '\r']) {
            cx.emit(PortEvent::Submit);
            return;
        }
        let chosen = range
            .clone()
            .or(self.input.marked.clone())
            .unwrap_or(self.input.selected.clone());
        if !self.admits(&chosen, text) {
            return;
        }
        // Digits never compose, so nothing stays marked.
        self.input.replace(Some(chosen), text, false, None);
        self.input.marked = None;
        self.layout = None;
        window.notify_text_input_changed(&self.focus);
        cx.notify();
    }

    fn key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !self.accepts_text_input(window, cx) {
            return;
        }
        let key = event.keystroke.key.as_str();
        let platform = event.keystroke.modifiers.platform;
        let mut range = self.input.selected.clone();
        match (platform, key) {
            (false, "enter") => cx.emit(PortEvent::Submit),
            (false, "escape") => cx.emit(PortEvent::Close),
            (true, "a") => self.input.selected = 0..self.input.text.len(),
            (true, "v") => {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    self.edit(None, text.trim(), window, cx);
                }
            }
            (false, "backspace" | "delete" | "left" | "right") => {
                // Digits are one byte and one UTF-16 unit each.
                if range.is_empty() {
                    if matches!(key, "left" | "backspace") {
                        range.start = range.start.saturating_sub(1);
                    } else {
                        range.end = (range.end + 1).min(self.input.text.len());
                    }
                }
                match key {
                    "left" => self.input.selected = range.start..range.start,
                    "right" => self.input.selected = range.end..range.end,
                    _ if !range.is_empty() => self.edit(Some(range), "", window, cx),
                    _ => {}
                }
            }
            _ => return,
        }
        window.notify_text_input_changed(&self.focus);
        cx.notify();
        cx.stop_propagation();
        window.prevent_default();
    }

    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let focused = self.focus.is_focused(window);
        let placeholder = self.input.text.is_empty();
        let (label, ink) = if placeholder {
            (PLACEHOLDER, color::MUTED)
        } else {
            (self.input.text.as_str(), color::TEXT)
        };
        let mut font = window.text_style().font();
        // Tabular digits: every digit is as wide as the next.
        font.features = FontFeatures(std::sync::Arc::new(vec![("tnum".into(), 1)]));
        let line = window.text_system().shape_line(
            label.to_owned().into(),
            px(FONT_SIZE),
            &[TextRun {
                len: label.len(),
                font,
                color: rgb(ink).into(),
                background_color: None,
                underline: None,
                strikethrough: None,
            }],
            None,
        );
        let caret_index = if placeholder {
            0
        } else {
            self.input.byte(self.input.selected.end)
        };
        let caret = line.x_for_index(caret_index);
        if focused {
            let start = if placeholder {
                caret
            } else {
                line.x_for_index(self.input.byte(self.input.selected.start))
            };
            window.paint_quad(fill(
                Bounds::new(
                    point(bounds.left() + start.min(caret), bounds.top() + px(12.)),
                    size(
                        (caret - start).abs().max(px(1.)),
                        bounds.size.height - px(24.),
                    ),
                ),
                rgb(color::ACCENT),
            ));
        }
        let _ = line.paint(
            bounds.origin,
            bounds.size.height,
            TextAlign::Left,
            None,
            window,
            cx,
        );
        self.layout = (!placeholder).then_some((line, bounds));
    }
}

/// The field's digits (board 42: 15 pt).
pub(crate) const FONT_SIZE: f32 = 15.;
pub(crate) const WIDTH: f32 = 200.;
pub(crate) const HEIGHT: f32 = 48.;

impl Render for PortField {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let border = if self.invalid {
            color::RED
        } else {
            color::RAISED
        };
        div()
            .id("forward-port-field")
            .debug_selector(|| "forward-port-field".into())
            .track_focus(&self.focus)
            .w(px(WIDTH))
            .h(px(HEIGHT))
            .px(px(metric::INSET))
            .rounded(px(radius::CONTROL))
            .border_1()
            .border_color(rgb(border))
            .bg(rgb(color::BACKGROUND))
            .text_size(px(text::BASE))
            .overflow_hidden()
            .on_key_down(cx.listener(Self::key))
            .on_click(cx.listener(|this, _, window, cx| {
                this.focus.focus(window, cx);
                window.request_virtual_keyboard();
            }))
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, cx| {
                        entity.update(cx, |this, cx| this.paint(bounds, window, cx));
                        let focus = entity.read(cx).focus.clone();
                        window.handle_input(&focus, ElementInputHandler::new(bounds, entity), cx);
                    },
                )
                .size_full(),
            )
    }
}

impl EntityInputHandler for PortField {
    fn accepts_text_input(&self, window: &mut Window, _: &mut Context<Self>) -> bool {
        self.focus.is_focused(window)
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

    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.input.text.encode_utf16().count())
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.input.marked.clone()
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.input.marked = None;
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit(range, text, window, cx);
    }

    /// Composition (an IME's marked text) is refused: a port has no
    /// characters to compose, so a Japanese IME's kana never stay.
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit(range, text, window, cx);
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let (line, bounds) = self.layout.as_ref()?;
        let start = line.x_for_index(self.input.byte(range.start));
        let end = line.x_for_index(self.input.byte(range.end.max(range.start)));
        Some(Bounds::new(
            bounds.origin + point(start.min(end), px(0.)),
            size((end - start).abs().max(px(1.)), bounds.size.height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let (line, bounds) = self.layout.as_ref()?;
        let index = line.closest_index_for_x(point.x - bounds.left());
        Some(self.input.text.get(..index)?.encode_utf16().count())
    }
}
