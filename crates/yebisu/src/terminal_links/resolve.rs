//! Asking Herdr where an address at a row edge ends. The surface carries no
//! soft-wrap flag, so a candidate read across rows, or one that runs into
//! the right edge, is not opened as read (`Link::unproven`). A Herdr 1 daemon
//! that advertises `pane.link.resolve` answers from its own terminal, which
//! knows its soft wraps, with the cells the address covers; only that
//! answer, read from the shown frame, opens, and a joined address is shown
//! whole in the status line as it opens. Herdr 2 has no such request, so
//! there the press opens nothing and says why.
use super::{Refusal, plain};
use crate::{app::Smoke, live::Live, terminal::in_rect};
use gpui::Context;
use serde_json::{Value, json};
use sheprd_client::{Method, protocol::PaneSurfaceFrame};
use std::time::{Duration, Instant};

/// How long a press waits for Herdr's answer before giving up.
const TIMEOUT: Duration = Duration::from_secs(5);

/// The most cells an answer may name, the bound a press reads anyway.
const MAX_REGIONS: usize = super::wrap::MAX_WRAPPED_ROWS as usize;

/// The one resolution in flight on a connection and its answer, kept in the
/// connection's mailbox so the reader thread can file the answer.
#[derive(Clone, Default)]
pub(crate) struct Mailbox {
    ticket: Option<Ticket>,
    answer: Option<Value>,
}

#[derive(Clone)]
struct Ticket {
    request: String,
    boot: String,
    /// The frame cell of the pane's first row and column.
    origin: (u16, u16),
    /// The pane-relative cell pressed.
    cell: (u16, u16),
    sent: Instant,
}

/// Files Herdr's answer to the resolution in flight; any other is ignored.
pub(crate) fn receive(live: &mut Live, id: &str, response: &Value) {
    let mailbox = &mut live.link_resolve;
    if mailbox
        .ticket
        .as_ref()
        .is_some_and(|ticket| ticket.request == id)
        && mailbox.answer.is_none()
    {
        mailbox.answer = Some(response.clone());
    }
}

/// Files a request the connection did not send as a failed answer.
pub(crate) fn reject(live: &mut Live, id: Option<&str>) {
    if let Some(id) = id {
        receive(live, id, &json!({"error": "not sent"}));
    }
}

/// The cells Herdr says the address covers, pane-relative, as (row, first
/// column, last column); `None` for an error or a malformed answer.
fn regions(answer: &Value) -> Option<Vec<(u16, u16, u16)>> {
    if answer.get("error").is_some() {
        return None;
    }
    let list = answer.pointer("/result/regions")?.as_array()?;
    if list.len() > MAX_REGIONS {
        return None;
    }
    list.iter()
        .map(|region| {
            let number = |key: &str| u16::try_from(region.get(key)?.as_u64()?).ok();
            let (row, start, end) = (number("row")?, number("start_col")?, number("end_col")?);
            (start <= end).then_some((row, start, end))
        })
        .collect()
}

/// The text the shown frame draws in `regions` of the pane at `origin`,
/// each region trimmed of the blank a wrapped wide glyph leaves.
fn region_text(
    surface: &PaneSurfaceFrame,
    origin: (u16, u16),
    regions: &[(u16, u16, u16)],
) -> Option<String> {
    let frame = &surface.frame;
    let width = usize::from(frame.width);
    let mut text = String::new();
    for &(row, start, end) in regions {
        let row = usize::from(origin.1.checked_add(row)?);
        let (start, end) = (
            usize::from(origin.0.checked_add(start)?),
            usize::from(origin.0.checked_add(end)?),
        );
        if row >= usize::from(frame.height) || end >= width {
            return None;
        }
        let cells = frame.cells.get(row * width + start..=row * width + end)?;
        let continuing = super::continuations(cells);
        let part: String = cells
            .iter()
            .zip(continuing)
            .filter(|(_, continuing)| !continuing)
            .map(|(cell, _)| super::shown(cell))
            .collect();
        text.push_str(part.trim_end());
    }
    Some(text)
}

impl Smoke {
    /// Asks Herdr where the address at frame cell (`column`, `row`) ends.
    /// Returns false when this connection cannot ask: Herdr 2, or a daemon
    /// that does not advertise the request.
    pub(super) fn resolve_link(
        &mut self,
        surface: &PaneSurfaceFrame,
        (column, row): (u16, u16),
    ) -> bool {
        if self.terminal.core_mode {
            return false;
        }
        let Some(handle) = &self.terminal.link.handle else {
            return false;
        };
        let Ok(mut live) = self.terminal.link.inbox.try_lock() else {
            return false;
        };
        if !Method::PaneLinkResolve.advertised_in(&live.methods) {
            return false;
        }
        // The pane under the cell; a surface without panes is the focused
        // pane over the whole frame.
        let pane = surface
            .panes
            .iter()
            .find(|pane| in_rect(pane.inner_rect, column, row));
        let (pane_id, origin, revision) = match pane {
            Some(pane) => (
                pane.pane_id.clone(),
                (pane.inner_rect.x, pane.inner_rect.y),
                Some(pane.content_revision),
            ),
            None => {
                let Some(id) = live
                    .snapshot
                    .as_ref()
                    .and_then(|snapshot| snapshot.focused_pane_id.clone())
                else {
                    return false;
                };
                (id, (0, 0), None)
            }
        };
        let cell = (column - origin.0, row - origin.1);
        let mut params = json!({"pane_id": pane_id, "viewport_row": cell.1, "col": cell.0});
        if let Some(revision) = revision {
            params["content_revision"] = json!(revision);
        }
        let Ok(request) = handle.request(&surface.boot_id, Method::PaneLinkResolve, params) else {
            return false;
        };
        live.link_resolve = Mailbox {
            ticket: Some(Ticket {
                request,
                boot: surface.boot_id.clone(),
                origin,
                cell,
                sent: Instant::now(),
            }),
            answer: None,
        };
        true
    }

    /// Opens what Herdr's answer names, once it arrived; called every poll.
    pub(crate) fn poll_link_resolve(&mut self, cx: &mut Context<Self>) {
        let (ticket, answer) = {
            let Ok(mut live) = self.terminal.link.inbox.try_lock() else {
                return;
            };
            let mailbox = &mut live.link_resolve;
            let Some(ticket) = mailbox.ticket.clone() else {
                return;
            };
            let answer = mailbox.answer.take();
            if answer.is_none() && ticket.sent.elapsed() < TIMEOUT {
                return;
            }
            *mailbox = Mailbox::default();
            (ticket, answer)
        };
        let opened = self.open_resolved(&ticket, answer.as_ref(), cx);
        if let Err(reason) = opened {
            self.refuse_link(&reason);
        }
        cx.notify();
    }

    fn open_resolved(
        &mut self,
        ticket: &Ticket,
        answer: Option<&Value>,
        cx: &mut Context<Self>,
    ) -> Result<(), Refusal> {
        let answer = answer.ok_or(Refusal::Unanswered)?;
        let regions = regions(answer).ok_or(Refusal::Unanswered)?;
        let (_, surface) = self.terminal.coherent_frame().ok_or(Refusal::Changed)?;
        if surface.boot_id != ticket.boot {
            return Err(Refusal::Changed);
        }
        let pressed = regions.iter().any(|&(row, start, end)| {
            row == ticket.cell.1 && (start..=end).contains(&ticket.cell.0)
        });
        if !pressed {
            return Err(Refusal::NoAddress);
        }
        let text = region_text(&surface, ticket.origin, &regions).ok_or(Refusal::Changed)?;
        let url = plain::web(&text).ok_or(Refusal::NoAddress)?;
        // A wrapped address is shown whole: no row of the screen shows it so.
        let joined = (regions.len() > 1).then_some(url.as_str());
        self.open_web_link(&url, joined, cx);
        Ok(())
    }
}
