//! P13 adapter over the real Link reader mailbox and coherent terminal painter.
use crate::{
    app::Smoke,
    find::{
        Search, State, adapter,
        field::{Event, Field},
    },
    live::Live,
};
use gpui::{prelude::*, *};
use sheprd_client::{
    Method,
    protocol::PaneSurfacePane,
    scrollback::{SearchDirection, TextPoint},
};
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

pub(crate) struct FindBar {
    pub field: Entity<Field>,
    pub search: Search,
    inbox: Arc<Mutex<Live>>,
    _changed: Subscription,
}
pub(crate) fn available(terminal: &crate::terminal_view::TerminalView) -> bool {
    terminal.open
        && !terminal.core_mode
        && terminal.core.is_none()
        && !terminal.reconnect.suspended
        && terminal.navigating.is_none()
        && adapter::capable(&terminal.live)
        && adapter::scope(
            &terminal.live,
            &terminal.reconnect.target,
            terminal.link.generation,
        )
        .is_some()
}
fn pane<'a>(live: &'a Live, id: &str) -> Option<&'a PaneSurfacePane> {
    live.surface
        .as_ref()?
        .panes
        .iter()
        .find(|p| p.pane_id == id)
}
fn viewport_top(pane: &PaneSurfacePane) -> u32 {
    pane.scroll.map_or(0, |s| {
        u32::try_from(
            s.max_offset_from_bottom
                .saturating_sub(s.offset_from_bottom),
        )
        .unwrap_or(u32::MAX)
    })
}
impl Smoke {
    pub(crate) fn open_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.find.is_some() {
            if let Some(bar) = &self.find {
                let focus = bar.field.read(cx).focus.clone();
                focus.focus(window, cx);
                window.request_virtual_keyboard();
            }
            return;
        }
        if !available(&self.terminal) {
            self.terminal.live.status = State::Unsupported.label().into();
            cx.notify();
            return;
        }
        let inbox = self.terminal.link.inbox.clone();
        let Ok(live) = inbox.try_lock() else {
            return;
        };
        if !adapter::capable(&live) {
            return;
        }
        let Some(scope) = adapter::scope(
            &live,
            &self.terminal.reconnect.target,
            self.terminal.link.generation,
        ) else {
            return;
        };
        drop(live);
        self.handoff.cancel();
        self.handoff.selection = None;
        self.handoff.select_mode = false;
        self.actions.palette = false;
        self.terminal.input.detach();
        let field = cx.new(Field::new);
        let changed = cx.subscribe_in(&field, window, |this, field, event, window, cx| {
            match event {
                Event::Close => {
                    this.close_find(window, cx);
                    return;
                }
                Event::Changed => {
                    if let Some(bar) = &mut this.find {
                        let field = field.read(cx);
                        if field.input.marked.is_none() {
                            if let Some(query) = field.query() {
                                bar.search.set_query(query);
                            } else {
                                bar.search.set_query("");
                                bar.search.state = State::Oversized;
                            }
                        }
                    }
                }
                Event::Next(backward) => {
                    if let Some(bar) = &mut this.find {
                        bar.search.step(if *backward {
                            SearchDirection::Backward
                        } else {
                            SearchDirection::Forward
                        });
                    }
                }
            }
            this.poll_find(cx);
            cx.notify();
        });
        let focus = field.read(cx).focus.clone();
        focus.focus(window, cx);
        window.request_virtual_keyboard();
        self.find = Some(FindBar {
            field,
            search: Search::new(scope, true),
            inbox,
            _changed: changed,
        });
        window.notify_text_input_changed(&self.terminal_focus);
        cx.notify();
    }
    fn retire_find(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(mut bar) = self.find.take() else {
            return false;
        };
        bar.field.update(cx, |field, _| field.retire());
        bar.search.close();
        // Invalidation is also enforced by dropped owner and identity checks;
        // a busy reader cannot hold UI teardown hostage.
        if let Ok(mut live) = bar.inbox.try_lock() {
            live.search.clear();
        }
        self.terminal.input.detach();
        true
    }
    pub(crate) fn close_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.retire_find(cx) {
            return;
        }
        self.refresh_terminal_input();
        if self.terminal.open {
            self.terminal.input.fresh_gesture();
            self.terminal_focus.focus(window, cx);
        } else {
            self.focus.focus(window, cx);
        }
        window.notify_text_input_changed(self.active_focus());
        cx.notify();
    }
    pub(crate) fn poll_find(&mut self, cx: &mut Context<Self>) {
        let Some(bar) = self.find.as_mut() else {
            return;
        };
        let identity = self.terminal.open
            && !self.terminal.core_mode
            && self.terminal.navigating.is_none()
            && !self.terminal.reconnect.suspended
            && Arc::ptr_eq(&bar.inbox, &self.terminal.link.inbox)
            && bar.search.scope.host == self.terminal.reconnect.target;
        let valid = if identity {
            bar.inbox.try_lock().ok().map(|live| {
                adapter::capable(&live)
                    && adapter::scope(
                        &live,
                        &self.terminal.reconnect.target,
                        self.terminal.link.generation,
                    )
                    .as_ref()
                        == Some(&bar.search.scope)
            })
        } else {
            Some(false)
        };
        if valid == Some(false) {
            self.retire_find(cx);
            self.terminal.needs_focus = true;
            cx.notify();
            return;
        }
        if valid.is_none() {
            return;
        }
        let now = Instant::now();
        let Ok(mut live) = bar.inbox.try_lock() else {
            return;
        };
        // Recheck under the same lock used for enqueue and registration.
        if adapter::scope(
            &live,
            &self.terminal.reconnect.target,
            self.terminal.link.generation,
        )
        .as_ref()
            != Some(&bar.search.scope)
        {
            return;
        }
        if bar.search.expire(now) {
            live.search.clear();
            cx.notify();
        }
        if let Some((scope, id)) = live.search.ticket.clone()
            && let Some(answer) = live.search.answer.take()
        {
            let applied = bar.search.answer(&scope, &id, answer);
            live.search.clear();
            if applied {
                if let Some(result) = &bar.search.result
                    && let Some(current) = result.current_match()
                    && let Some(pane) = pane(&live, &result.pane_id)
                    && pane.content_revision == result.content_revision
                    && let Some(scroll) = pane.scroll
                    && Method::PaneScroll.advertised_in(&live.methods)
                    && (current.start.row < viewport_top(pane)
                        || current.end.row
                            >= viewport_top(pane).saturating_add(u32::from(pane.inner_rect.height)))
                    && let Some(handle) = &self.terminal.link.handle
                {
                    let top = current
                        .start
                        .row
                        .saturating_sub(u32::from(pane.inner_rect.height) / 2);
                    let offset = scroll.max_offset_from_bottom.saturating_sub(u64::from(top));
                    if handle
                        .request(
                            &scope.boot,
                            Method::PaneScroll,
                            serde_json::json!({"pane_id":scope.pane,"offset_from_bottom":offset}),
                        )
                        .is_err()
                    {
                        bar.search.state = State::SendFailed;
                    }
                }
                cx.notify();
            }
        }
        let Some(pane) = pane(&live, &bar.search.scope.pane) else {
            return;
        };
        let cursor = TextPoint {
            row: viewport_top(pane).saturating_add(u32::from(pane.inner_rect.height)),
            col: 0,
        };
        if let Some(params) = bar.search.next(pane.content_revision, cursor) {
            let Some(handle) = &self.terminal.link.handle else {
                bar.search.send_failed();
                cx.notify();
                return;
            };
            // ClientHandle uses monotonic IDs within one connection. Reopen
            // shares that allocator; reconnect gets an entirely different inbox.
            match handle.copy_search(&bar.search.scope.boot, &params) {
                Ok(id) => {
                    live.search.ticket = Some((bar.search.scope.clone(), id.clone()));
                    live.search.answer = None;
                    bar.search.sent(id, &params, now);
                }
                Err(_) => bar.search.send_failed(),
            }
            cx.notify();
        }
    }
    pub(crate) fn render_find(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(bar) = &self.find else {
            let enabled = available(&self.terminal);
            return div()
                .id("terminal-search-open")
                .px_4()
                .py_2()
                .child(if enabled {
                    "Find"
                } else {
                    State::Unsupported.label()
                })
                .when(enabled, |view| {
                    view.on_click(cx.listener(|this, _, window, cx| this.open_find(window, cx)))
                })
                .into_any_element();
        };
        let label = bar
            .search
            .result
            .as_ref()
            .filter(|_| bar.search.state == State::Matches)
            .map(|r| {
                format!(
                    "{} / {}",
                    r.current_global.unwrap_or(0).saturating_add(1),
                    r.total
                )
            })
            .unwrap_or_else(|| bar.search.state.label().into());
        div()
            .id("terminal-search")
            .px_4()
            .py_2()
            .flex()
            .flex_col()
            .gap_2()
            .child(bar.field.clone())
            .child(label)
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        div()
                            .id("search-previous")
                            .child("Previous")
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(bar) = &mut this.find {
                                    bar.search.step(SearchDirection::Backward);
                                }
                                this.poll_find(cx);
                            })),
                    )
                    .child(div().id("search-next").child("Next").on_click(cx.listener(
                        |this, _, _, cx| {
                            if let Some(bar) = &mut this.find {
                                bar.search.step(SearchDirection::Forward);
                            }
                            this.poll_find(cx);
                        },
                    )))
                    .child(
                        div()
                            .id("search-close")
                            .child("Close")
                            .on_click(cx.listener(Self::close_find_click)),
                    ),
            )
            .into_any_element()
    }
    fn close_find_click(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.close_find(window, cx);
    }
    pub(crate) fn p13_qa_controls(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        #[cfg(all(target_os = "ios", target_abi = "sim"))]
        {
            self.p13_qa(cx)
        }
        #[cfg(not(all(target_os = "ios", target_abi = "sim")))]
        {
            let _ = cx;
            None
        }
    }
    pub(crate) fn find_highlights(&self) -> Vec<crate::terminal_painter::Highlight> {
        let mut out = Vec::new();
        let Some(bar) = &self.find else {
            return out;
        };
        let Some(result) = &bar.search.result else {
            return out;
        };
        let Ok(authoritative) = bar.inbox.try_lock() else {
            return out;
        };
        if !Arc::ptr_eq(&bar.inbox, &self.terminal.link.inbox)
            || adapter::scope(
                &authoritative,
                &self.terminal.reconnect.target,
                self.terminal.link.generation,
            )
            .as_ref()
                != Some(&bar.search.scope)
        {
            return out;
        }
        drop(authoritative);
        if adapter::scope(
            &self.terminal.live,
            &self.terminal.reconnect.target,
            self.terminal.link.generation,
        )
        .as_ref()
            != Some(&bar.search.scope)
        {
            return out;
        }
        let Some(pane) = pane(&self.terminal.live, &result.pane_id) else {
            return out;
        };
        if pane.content_revision != result.content_revision {
            return out;
        }
        if pane.inner_rect.width == 0 || pane.inner_rect.height == 0 {
            return out;
        }
        let top = viewport_top(pane);
        let bottom = top.saturating_add(u32::from(pane.inner_rect.height));
        for (index, range) in result.matches.iter().enumerate() {
            for row in range.start.row.max(top)..=range.end.row.min(bottom.saturating_sub(1)) {
                let start = if row == range.start.row {
                    range.start.col
                } else {
                    0
                };
                let end = if row == range.end.row {
                    range.end.col.saturating_add(1)
                } else {
                    pane.inner_rect.width
                }
                .min(pane.inner_rect.width);
                if start >= end {
                    continue;
                }
                if let Ok(y) = u16::try_from(row - top) {
                    if out.len() == 4096 {
                        return out;
                    }
                    out.push(crate::terminal_painter::Highlight {
                        row: pane.inner_rect.y.saturating_add(y),
                        columns: pane.inner_rect.x.saturating_add(start)
                            ..pane.inner_rect.x.saturating_add(end),
                        tint: if result.current == u32::try_from(index).ok() {
                            crate::terminal_painter::Tint::CurrentMatch
                        } else {
                            crate::terminal_painter::Tint::Match
                        },
                    });
                }
            }
        }
        out
    }
}

#[cfg(test)]
#[path = "find/adapter_tests.rs"]
mod tests;
