//! Delete Host (#250, owner decision 2026-11-27): from the Edit sheet and
//! from a Hosts row's left swipe, both through one confirmation. OK
//! disconnects the host, moves the selection to the next saved host (none
//! left: the first-run screen) and starts `Job::Delete`, which forgets the
//! records on disk (`hosts/delete.rs`). The rules live here; `delete_view`
//! draws them.
use crate::{
    app::Smoke,
    hosts::{SavedHosts, next_selection},
    setup::Job,
    terminal_view::TerminalView,
};
use gpui::{Context, ScrollHandle, point, px};
use std::collections::BTreeMap;

/// The confirmation the user answers before anything is deleted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DeleteConfirm {
    pub(crate) host: String,
    /// The name the Hosts list shows for it.
    pub(crate) name: String,
    /// The host has a connection open, which closes first.
    pub(crate) connected: bool,
}

impl DeleteConfirm {
    pub(crate) fn title(&self) -> String {
        format!("Delete {}?", self.name)
    }

    pub(crate) fn lines(&self) -> Vec<&'static str> {
        let mut lines = Vec::new();
        if self.connected {
            lines.push("Yebisu disconnects from it first.");
        }
        lines.push(
            "This iPad forgets the host, its trusted host key, its saved tabs and what reopens at launch. Adding it again starts with the host key check.",
        );
        lines.push(
            "This iPad's public key stays in ~/.ssh/authorized_keys on the host. Remove it there if the host no longer needs it.",
        );
        lines
    }
}

/// Each Hosts row scrolls sideways by the width of its Delete button, so a
/// left swipe shows it. GPUI's own scrolling does the work: a vertical pan
/// still scrolls the list, and a fling stops at the row's edge instead of
/// catching the next tap.
#[derive(Debug, Default)]
pub(crate) struct Swipes(BTreeMap<String, ScrollHandle>);

impl Swipes {
    pub(crate) fn handle(&mut self, host: &str) -> ScrollHandle {
        self.0.entry(host.to_owned()).or_default().clone()
    }

    /// Slides every row back over its Delete button.
    pub(crate) fn close_all(&self) {
        for handle in self.0.values() {
            handle.set_offset(point(px(0.), px(0.)));
        }
    }

    pub(crate) fn retain(&mut self, hosts: &[String]) {
        self.0.retain(|host, _| hosts.contains(host));
    }

    #[cfg(test)]
    pub(crate) fn offset(&self, host: &str) -> Option<f32> {
        self.0.get(host).map(|handle| f32::from(handle.offset().x))
    }
}

/// After a sideways pan ends, a row more than half open opens fully and
/// any other closes. `offset` is the row's scroll (0 closed, `-width` open).
pub(crate) fn settle(offset: f32, width: f32) -> f32 {
    if width > 0. && offset < -width / 2. {
        -width
    } else {
        0.
    }
}

fn disconnect(view: &mut TerminalView) {
    view.core.take();
    view.link.pause();
    view.input.detach();
    view.open = false;
}

fn has_connection(view: &TerminalView) -> bool {
    view.live.connected || view.core.is_some() || view.link.handle.is_some()
}

impl Smoke {
    /// Delete in a row or Delete Host in the Edit sheet: ask first.
    pub(crate) fn hosts_ask_delete(&mut self, host: &str, cx: &mut Context<Self>) {
        if self.setup.busy || !self.setup.hosts.iter().any(|saved| saved == host) {
            return;
        }
        self.shell.hosts_sheet.delete = Some(DeleteConfirm {
            host: host.to_owned(),
            name: self.setup.host_name(host).display().to_owned(),
            connected: self.host_connected(host),
        });
        cx.notify();
    }

    pub(crate) fn hosts_cancel_delete(&mut self, cx: &mut Context<Self>) {
        self.shell.hosts_sheet.delete = None;
        self.shell.hosts_sheet.swipes.close_all();
        cx.notify();
    }

    /// OK: disconnect, move the selection, then delete in the background.
    pub(crate) fn hosts_confirm_delete(&mut self, cx: &mut Context<Self>) {
        if self.setup.busy {
            return;
        }
        let Some(confirm) = self.shell.hosts_sheet.delete.take() else {
            return;
        };
        let sheet = &mut self.shell.hosts_sheet;
        sheet.swipes.close_all();
        // The Edit sheet's host is going: back to the list under it.
        if sheet.add.take().is_some() {
            sheet.list_open = true;
            sheet.name = None;
            // As Cancel: a device-key read that waits for the sheet is dropped.
            self.setup.epoch = self.setup.epoch.wrapping_add(1);
        }
        self.forget_host_connections(&confirm.host, cx);
        self.start_job(Job::Delete(confirm.host), cx);
    }

    /// Any view of `host`, shown or hidden, has a connection open.
    pub(crate) fn host_connected(&self, host: &str) -> bool {
        let shown = self.terminal.reconnect.target == host && has_connection(&self.terminal);
        shown
            || [
                self.board.hidden.get(host),
                self.board.hidden_core.get(host),
            ]
            .into_iter()
            .flatten()
            .any(has_connection)
    }

    /// Closes every connection of `host` and forgets what memory holds for
    /// it. A shown terminal of it gives way to the next saved host, or, with
    /// none left (or when it was not on screen), to no terminal at all.
    pub(crate) fn forget_host_connections(&mut self, host: &str, cx: &mut Context<Self>) {
        if self.terminal.reconnect.target == host {
            let index = self.setup.hosts.iter().position(|saved| saved == host);
            let remaining: Vec<String> = self
                .setup
                .hosts
                .iter()
                .filter(|saved| *saved != host)
                .cloned()
                .collect();
            let next = index.map_or_else(String::new, |index| next_selection(&remaining, index));
            if self.terminal.open && !next.is_empty() {
                // As a sidebar tap: the deleted host's view moves to the
                // hidden ones and is closed below.
                self.select_host(&next);
            } else {
                self.input_transition(Self::clear_selected_terminal);
            }
        }
        for mut view in [
            self.board.hidden.remove(host),
            self.board.hidden_core.remove(host),
        ]
        .into_iter()
        .flatten()
        {
            disconnect(&mut view);
        }
        self.shell.herdr2.forget_host(host);
        self.forget_sidebar_host(host, cx);
        cx.notify();
    }

    fn clear_selected_terminal(&mut self) {
        self.handoff.cancel();
        self.handoff.selection = None;
        self.terminal.input.detach();
        let mut next = TerminalView::default();
        self.carry_terminal_settings(&mut next);
        let mut previous = std::mem::replace(&mut self.terminal, next);
        disconnect(&mut previous);
        self.board.selection = self.board.selection.wrapping_add(1);
        self.smoke_mode = false;
    }

    /// Reply hook (`apply_setup_result`): the host is gone from disk.
    pub(crate) fn hosts_after_delete(&mut self, hosts: &SavedHosts, deleted: &str) {
        if self.setup_input.text.trim() == deleted {
            self.setup_input.set_text(hosts.selected.clone());
        }
        let sheet = &mut self.shell.hosts_sheet;
        sheet.swipes.retain(&hosts.targets);
        if hosts.targets.is_empty() && !self.terminal.open {
            // The first-run screen, as at a first launch: the Add host
            // sheet opens by itself.
            sheet.close();
            sheet.auto_opened = false;
        }
    }
}

#[cfg(test)]
#[path = "delete_tests.rs"]
mod tests;
