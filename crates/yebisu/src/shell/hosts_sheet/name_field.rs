use crate::{
    hosts::MAX_HOST_NAME_BYTES,
    input::Input,
    shell::style::{color, metric, radius, text},
};
use gpui::{prelude::*, *};
use std::ops::Range;

pub(super) struct NameField {
    input: Input,
    focus: FocusHandle,
    manual: bool,
    suggested_for: Option<String>,
    edited: bool,
    enabled: bool,
    layout: Option<(ShapedLine, Bounds<Pixels>)>,
}

impl NameField {
    pub fn new(saved: Option<String>, cx: &mut Context<Self>) -> Self {
        let mut input = Input::default();
        let manual = saved.is_some();
        input.set_text(saved.unwrap_or_default());
        Self {
            input,
            focus: cx.focus_handle(),
            manual,
            suggested_for: None,
            edited: false,
            enabled: true,
            layout: None,
        }
    }

    pub fn suggest(&mut self, address: &str, name: String, cx: &mut Context<Self>) {
        if !self.manual {
            self.input.set_text(name);
            self.suggested_for = Some(address.to_owned());
            self.edited = true;
            cx.notify();
        }
    }

    pub fn name_to_save(&self, target: &str) -> Option<String> {
        let address = target
            .split_once('@')
            .map_or(target, |(_, address)| address);
        (self.edited && (self.manual || self.suggested_for.as_deref() == Some(address)))
            .then(|| self.input.text.trim().to_owned())
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn edit(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        marked: bool,
        selected: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.accepts_text_input(window, cx) || text.chars().any(char::is_control) {
            return;
        }
        let chosen = range
            .as_ref()
            .or(self.input.marked.as_ref())
            .unwrap_or(&self.input.selected);
        let start = self.input.byte(chosen.start);
        let end = self.input.byte(chosen.end.max(chosen.start));
        if self.input.text.len() - (end - start) + text.len() > MAX_HOST_NAME_BYTES {
            return;
        }
        self.input.replace(range, text, marked, selected);
        self.layout = None;
        self.edited = true;
        self.manual = true;
        window.notify_text_input_changed(&self.focus);
        cx.notify();
    }

    fn key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !self.accepts_text_input(window, cx) || self.input.marked.is_some() {
            return;
        }
        let key = event.keystroke.key.as_str();
        let platform = event.keystroke.modifiers.platform;
        let mut range = self.input.selected.clone();
        match (platform, key) {
            (true, "a") => self.input.selected = 0..self.input.text.encode_utf16().count(),
            (true, "c" | "x") => {
                if !range.is_empty() {
                    let text =
                        &self.input.text[self.input.byte(range.start)..self.input.byte(range.end)];
                    cx.write_to_clipboard(ClipboardItem::new_string(text.to_owned()));
                    if key == "x" {
                        self.edit(Some(range), "", false, None, window, cx);
                    }
                }
            }
            (true, "v") => {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    self.edit(None, &text, false, None, window, cx);
                }
            }
            (false, "backspace" | "delete" | "left" | "right") => {
                let before = self.input.text[..self.input.byte(range.start)]
                    .chars()
                    .next_back()
                    .map_or(0, char::len_utf16);
                let after = self.input.text[self.input.byte(range.end)..]
                    .chars()
                    .next()
                    .map_or(0, char::len_utf16);
                if range.is_empty() {
                    if matches!(key, "left" | "backspace") {
                        range.start -= before;
                    } else {
                        range.end += after;
                    }
                }
                match key {
                    "left" => self.input.selected = range.start..range.start,
                    "right" => self.input.selected = range.end..range.end,
                    _ if !range.is_empty() => self.edit(Some(range), "", false, None, window, cx),
                    _ => {}
                }
            }
            (false, "enter" | "escape") => window.dismiss_virtual_keyboard(),
            _ => return,
        }
        window.notify_text_input_changed(&self.focus);
        cx.notify();
        cx.stop_propagation();
        window.prevent_default();
    }

    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let focused = self.focus.is_focused(window);
        let placeholder = self.input.text.is_empty() && !focused;
        let (label, ink) = if placeholder {
            ("Optional", color::MUTED)
        } else {
            (self.input.text.as_str(), color::TEXT)
        };
        let line = window.text_system().shape_line(
            label.into(),
            px(text::BASE),
            &[TextRun {
                len: label.len(),
                font: window.text_style().font(),
                color: rgb(ink).into(),
                background_color: None,
                underline: None,
                strikethrough: None,
            }],
            None,
        );
        let caret = line.x_for_index(self.input.byte(self.input.selected.end));
        let offset = if focused {
            (caret + px(1.) - bounds.size.width).max(px(0.))
        } else {
            px(0.)
        };
        let text_bounds = Bounds::new(bounds.origin - point(offset, px(0.)), bounds.size);
        if focused {
            let start = line.x_for_index(self.input.byte(self.input.selected.start));
            window.paint_quad(fill(
                Bounds::new(
                    point(text_bounds.left() + start.min(caret), bounds.top() + px(9.)),
                    size(
                        (caret - start).abs().max(px(1.)),
                        bounds.size.height - px(18.),
                    ),
                ),
                rgb(color::ACCENT),
            ));
        }
        let _ = line.paint(
            text_bounds.origin,
            bounds.size.height,
            TextAlign::Left,
            None,
            window,
            cx,
        );
        self.layout = (!placeholder).then_some((line, text_bounds));
    }
}

impl Render for NameField {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        div()
            .id("host-name-field")
            .debug_selector(|| "host-name-field".into())
            .track_focus(&self.focus)
            .h(px(super::view::HOST_CONTROL_HEIGHT))
            .px(px(metric::INSET))
            .rounded(px(radius::CONTROL))
            .border_1()
            .border_color(rgb(color::RAISED))
            .bg(rgb(color::BACKGROUND))
            .overflow_hidden()
            .on_key_down(cx.listener(Self::key))
            .on_click(cx.listener(|this, _, window, cx| {
                if this.enabled {
                    this.focus.focus(window, cx);
                    window.request_virtual_keyboard();
                }
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

impl EntityInputHandler for NameField {
    fn accepts_text_input(&self, window: &mut Window, _: &mut Context<Self>) -> bool {
        self.enabled && self.focus.is_focused(window)
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
        self.edit(range, text, false, None, window, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit(range, text, true, selected, window, cx);
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
