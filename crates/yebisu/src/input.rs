//! UTF-16 input boundary for the P0 smoke field; no terminal commands are sent.
#[cfg(any(target_os = "ios", test))]
use crate::app::Smoke;
#[cfg(any(target_os = "ios", test))]
use gpui::{Bounds, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window};
use std::ops::Range;

#[derive(Default)]
pub(crate) struct Input {
    pub(crate) text: String,
    pub(crate) marked: Option<Range<usize>>,
    pub(crate) selected: Range<usize>,
}

impl Input {
    #[cfg(any(target_os = "ios", test))]
    pub(crate) fn set_text(&mut self, text: String) {
        let end = text.encode_utf16().count();
        self.text = text;
        self.marked = None;
        self.selected = end..end;
    }

    pub(crate) fn byte(&self, utf16: usize) -> usize {
        let mut count = 0;
        for (index, ch) in self.text.char_indices() {
            if count >= utf16 {
                return index;
            }
            count += ch.len_utf16();
        }
        self.text.len()
    }
    pub(crate) fn replace(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        marked: bool,
        selected: Option<Range<usize>>,
    ) {
        let range = range
            .or(self.marked.clone())
            .unwrap_or(self.selected.clone());
        let start = self.byte(range.start);
        let end = self.byte(range.end.max(range.start));
        self.text.replace_range(start..end, text);
        let start = self.text[..start].encode_utf16().count();
        let len = text.encode_utf16().count();
        self.marked = (marked && len > 0).then_some(start..start + len);
        self.selected = selected
            .map(|r| start + r.start.min(len)..start + r.end.max(r.start).min(len))
            .unwrap_or(start + len..start + len);
        self.selected = self.text[..self.byte(self.selected.start)]
            .encode_utf16()
            .count()
            ..self.text[..self.byte(self.selected.end)]
                .encode_utf16()
                .count();
    }
}

#[cfg(any(target_os = "ios", test))]
impl Smoke {
    pub(crate) fn active_focus(&self) -> &gpui::FocusHandle {
        if self.terminal.open {
            &self.terminal_focus
        } else {
            &self.focus
        }
    }
    fn active_input(&self) -> &Input {
        if self.terminal.open {
            &self.terminal.input.composition
        } else if self.smoke_mode {
            &self.input
        } else {
            &self.setup_input
        }
    }
    fn active_input_mut(&mut self) -> &mut Input {
        if self.terminal.open {
            &mut self.terminal.input.composition
        } else if self.smoke_mode {
            &mut self.input
        } else {
            &mut self.setup_input
        }
    }
}

#[cfg(any(target_os = "ios", test))]
impl EntityInputHandler for Smoke {
    fn accepts_text_input(&self, window: &mut Window, _: &mut Context<Self>) -> bool {
        if self.terminal.open {
            self.terminal
                .input
                .accepts(self.active_focus().is_focused(window), false)
        } else {
            self.active_focus().is_focused(window) && (self.smoke_mode || !self.setup.busy)
        }
    }

    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let start = self.active_input().byte(range.start);
        let end = self.active_input().byte(range.end.max(range.start));
        *actual = Some(
            self.active_input().text[..start].encode_utf16().count()
                ..self.active_input().text[..end].encode_utf16().count(),
        );
        Some(self.active_input().text[start..end].to_owned())
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.active_input().selected.clone(),
            reversed: false,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.active_input().marked.clone()
    }
    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.active_input_mut().marked = None;
        if self.terminal.open {
            self.terminal.input.composition = Input::default();
        }
        window.notify_text_input_changed(self.active_focus());
        cx.notify();
    }
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.active_focus().is_focused(window) {
            return;
        }
        if self.terminal.open {
            self.refresh_terminal_input();
            if let Some(event) = self.terminal.input.replace(range, text) {
                self.send_terminal(event);
            }
            window.notify_text_input_changed(self.active_focus());
            cx.notify();
            return;
        }
        if !self.smoke_mode && self.setup.busy {
            return;
        }
        self.active_input_mut().replace(range, text, false, None);
        if !self.smoke_mode {
            self.setup.pending = None;
            self.setup.fingerprints.clear();
        }
        eprintln!("yebisu input committed chars={}", text.chars().count());
        self.observation.record(serde_json::json!({"kind":"input", "input":self.active_input().text, "marked":self.active_input().marked}));
        window.notify_text_input_changed(self.active_focus());
        cx.notify();
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.active_focus().is_focused(window) {
            return;
        }
        if self.terminal.open {
            self.refresh_terminal_input();
            self.terminal.input.mark(text, selected);
            window.notify_text_input_changed(self.active_focus());
            cx.notify();
            return;
        }
        if !self.smoke_mode && self.setup.busy {
            return;
        }
        self.active_input_mut().replace(range, text, true, selected);
        if !self.smoke_mode {
            self.setup.pending = None;
            self.setup.fingerprints.clear();
        }
        eprintln!("yebisu input marked chars={}", text.chars().count());
        self.observation.record(serde_json::json!({"kind":"input", "input":self.active_input().text, "marked":self.active_input().marked}));
        window.notify_text_input_changed(self.active_focus());
        cx.notify();
    }
    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        window: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        if !self.terminal.open {
            return None;
        }
        self.refresh_terminal_input();
        self.terminal.input.destination.as_ref()?;
        let t = &self.terminal;
        let range = if t.input.options.caret_anchor {
            range.start..range.start
        } else {
            range
        };
        Some(t.painter.composition_bounds(
            &t.input.composition.text,
            range,
            t.geometry.cursor(t.live.surface.as_deref()),
            t.geometry.area(t.live.surface.as_deref()),
            &gpui::font("SauceCodePro Nerd Font Mono"),
            window,
        ))
    }
    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        window: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        if !self.terminal.open || !self.terminal.geometry.contains(point) {
            return None;
        }
        let t = &self.terminal;
        t.painter.composition_index(
            &t.input.composition.text,
            point,
            t.geometry.cursor(t.live.surface.as_deref()),
            t.geometry.area(t.live.surface.as_deref()),
            &gpui::font("SauceCodePro Nerd Font Mono"),
            window,
        )
    }
    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.active_input().text.encode_utf16().count())
    }
    fn set_selected_text_range(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let start = self.active_input().byte(range.start);
        let end = self.active_input().byte(range.end.max(range.start));
        self.active_input_mut().selected = self.active_input().text[..start].encode_utf16().count()
            ..self.active_input().text[..end].encode_utf16().count();
        window.notify_text_selection_changed(self.active_focus());
        cx.notify();
    }
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
