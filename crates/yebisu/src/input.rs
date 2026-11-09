//! UTF-16 input boundary for the P0 smoke field; no terminal commands are sent.
#[cfg(target_os = "ios")]
use crate::app::Smoke;
#[cfg(target_os = "ios")]
use gpui::{Bounds, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window};
use std::ops::Range;

#[derive(Default)]
pub(crate) struct Input {
    pub(crate) text: String,
    pub(crate) marked: Option<Range<usize>>,
    selected: Range<usize>,
}

impl Input {
    fn byte(&self, utf16: usize) -> usize {
        let mut count = 0;
        for (index, ch) in self.text.char_indices() {
            if count >= utf16 {
                return index;
            }
            count += ch.len_utf16();
        }
        self.text.len()
    }
    fn replace(
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
    }
}

#[cfg(target_os = "ios")]
impl EntityInputHandler for Smoke {
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
    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input.marked = None;
        window.notify_text_input_changed(&self.focus);
        cx.notify();
    }
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.input.replace(range, text, false, None);
        eprintln!("yebisu input committed chars={}", text.chars().count());
        self.observation.record(serde_json::json!({"kind":"input", "input":self.input.text, "marked":self.input.marked}));
        window.notify_text_input_changed(&self.focus);
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
        self.input.replace(range, text, true, selected);
        eprintln!("yebisu input marked chars={}", text.chars().count());
        self.observation.record(serde_json::json!({"kind":"input", "input":self.input.text, "marked":self.input.marked}));
        window.notify_text_input_changed(&self.focus);
        cx.notify();
    }
    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        // P0 deliberately withholds candidate geometry until shaped text is available.
        None
    }
    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.input.text.encode_utf16().count())
    }
    fn set_selected_text_range(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let start = self.input.byte(range.start);
        let end = self.input.byte(range.end.max(range.start));
        self.input.selected = self.input.text[..start].encode_utf16().count()
            ..self.input.text[..end].encode_utf16().count();
        window.notify_text_selection_changed(&self.focus);
        cx.notify();
    }
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
