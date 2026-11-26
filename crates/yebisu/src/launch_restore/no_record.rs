//! A launch with no record (owner decision 2026-11-26): the first launch of
//! a build that keeps one, or a record that cannot be read. Without this the
//! sidebar showed no Herdr 2 space until a new space was made.
//!
//! Saved hosts carry no Herdr kind, so each one is asked off the UI thread,
//! by the read-only listing a restored space uses (it never starts a
//! server), whether Herdr 2 lists a space there. A host that does opens
//! `default` as a hidden connection, so the sidebar lists its spaces; a host
//! that lists none is skipped; a listing that fails shows its reason on the
//! host's Herdr 2 row, which waits for Reconnect. The #170 rules hold: one
//! try, and only under `Return: Automatic`. A record that lists nothing is
//! the user's choice and never comes here.
//!
//! Unlike a restore, a terminal the user opened first does not cancel this:
//! a hidden connection replaces nothing, and `still_waiting` skips a host
//! whose Herdr 2 the user already opened.
use super::Place;
use crate::{app::Smoke, diagnostics, shell::herdr2_frame::HERDR2_SESSION};
use gpui::Context;

impl Smoke {
    pub(super) fn open_default_spaces(&mut self, cx: &mut Context<Self>) {
        if self.shell.hosts_sheet.add.is_some() {
            return;
        }
        let automatic = self.terminal.reconnect.automatic;
        self.observation.record(serde_json::json!({
            "kind": "launch-default", "automatic": automatic,
            "hosts": self.setup.hosts.len(),
        }));
        if !automatic {
            diagnostics::record("herdr2_launch_default_tap");
            return;
        }
        for host in self.setup.hosts.clone() {
            let listed = self.list_spaces(&host, cx);
            cx.spawn(async move |weak, cx| {
                let listed = listed.await;
                let _ = weak.update(cx, |this, cx| {
                    this.launch.pending = this.launch.pending.saturating_sub(1);
                    match listed {
                        Ok(spaces) if spaces.is_empty() => {
                            diagnostics::record("herdr2_launch_default_none");
                        }
                        Ok(_) => {
                            diagnostics::record("herdr2_launch_default");
                            this.open_restored_space(
                                &host,
                                HERDR2_SESSION.to_owned(),
                                None,
                                Place::Hidden,
                                cx,
                            );
                        }
                        Err(error) => {
                            diagnostics::record("herdr2_launch_default_failed");
                            this.fail_restored_space(&host, HERDR2_SESSION, &error, Place::Hidden);
                        }
                    }
                    cx.notify();
                });
            })
            .detach();
        }
    }
}
