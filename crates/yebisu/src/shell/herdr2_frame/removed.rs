//! A removed Herdr 2 space takes its place in the selection (see
//! `core::vacate`): the worker stopped the space's server because its last tab
//! closed, so the view still showing it must move on.
use crate::{
    app::Smoke,
    core::{SessionTarget, vacate::Removal},
};
use gpui::Context;

impl Smoke {
    /// Moves every Herdr 2 view whose own space was removed to the space that
    /// takes its place (the official app's rule, `herdr-app/src/model.rs`
    /// `remove_space`: the next space in the order, else the last), or detaches
    /// it when the host has no other space. The removed space is forgotten as
    /// the host's selected one, so a later reconnect never starts it again.
    pub(crate) fn poll_removed_space(&mut self, cx: &mut Context<Self>) {
        if let Some(removal) = self.terminal.core.as_ref().and_then(|core| core.removal()) {
            let host = self.terminal.reconnect.target.clone();
            self.move_selection_from(&host, removal, cx);
        }
        let hidden = self
            .board
            .hidden_core
            .iter()
            .filter_map(|(host, terminal)| Some((host.clone(), terminal.core.as_ref()?.removal()?)))
            .collect::<Vec<_>>();
        for (host, removal) in hidden {
            self.shell.herdr2.forget_session(&host, &removal.name);
            match removal.next {
                Some(next) => {
                    self.shell.herdr2.confirm_session(&host, &next.name);
                    self.resume_hidden_core(host, next.name, cx);
                }
                None => {
                    if let Some(terminal) = self.board.hidden_core.get_mut(&host) {
                        terminal.core.take();
                        terminal.live.connected = false;
                        terminal.live.status = format!("Herdr 2 · space {} removed", removal.name);
                    }
                }
            }
        }
    }

    fn move_selection_from(&mut self, host: &str, removal: Removal, cx: &mut Context<Self>) {
        // Its server is gone: nothing may remember this session as connected
        // (`remember_connected_core` would select the removed space again).
        self.terminal.live.connected = false;
        let moved = removal.next.as_ref().is_some_and(|next| {
            self.connect_core_target(
                SessionTarget {
                    host: host.to_owned(),
                    name: next.name.clone(),
                    focus: None,
                    expected_core_id: next.core_id.clone(),
                },
                cx,
            )
            .is_ok()
        });
        if !moved {
            self.detach_core();
        }
        self.shell.herdr2.forget_session(host, &removal.name);
        if let Some(next) = removal.next.filter(|_| moved) {
            self.shell.herdr2.confirm_session(host, &next.name);
        }
        cx.notify();
    }
}
