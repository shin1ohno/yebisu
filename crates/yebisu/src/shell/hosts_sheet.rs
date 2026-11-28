//! L7: the Hosts sheet and the first-run screen (S-14). Owner: L7.
//!
//! The sheet lists the saved hosts, opens the three-step Add host sheet, and
//! shows this iPad's public key. `HostsSheetState` holds the rules (what
//! opens when, how a job reply moves the flow); `view` draws it and
//! `actions` starts the background jobs. Every rule is tested on the state,
//! not through pixels.
use super::{
    HerdrKind,
    add_host::{AddHostSession, AddHostStep, ScanOutcome, SheetMode},
};
use crate::{app::Smoke, setup::Reply};
use gpui::{AnyElement, Context, Window};
use sheprd_client::ssh::HostKeyScan;

mod actions;
pub(crate) mod delete;
mod delete_view;
mod name_field;
#[path = "../nearby/mod.rs"]
mod nearby;
mod nearby_view;
mod view;

/// Developer controls live in the Hosts sheet and only in a Simulator build.
pub(crate) const fn developer_buttons_allowed(ios: bool, simulator: bool) -> bool {
    ios && simulator
}

/// Labels of the developer controls for a build; empty outside the Simulator.
pub(crate) fn developer_items(allowed: bool) -> Vec<&'static str> {
    if allowed {
        vec![
            "Developer: simulate save failure",
            "Developer: P0 rendering check",
        ]
    } else {
        Vec::new()
    }
}

pub(crate) const DEVELOPER_BUTTONS: bool =
    developer_buttons_allowed(cfg!(target_os = "ios"), cfg!(target_abi = "sim"));

#[derive(Debug, Default)]
pub(crate) struct HostsSheetState {
    /// The host list sheet is open.
    pub(crate) list_open: bool,
    /// The Add host sheet, when open. It sits over the list.
    pub(crate) add: Option<AddHostSession>,
    name: Option<gpui::Entity<name_field::NameField>>,
    nearby: nearby_view::NearbySheet,
    /// The first-run auto-open already happened (it happens once).
    pub(crate) auto_opened: bool,
    /// The sheet was opened over a connected terminal (footer Hosts); any other
    /// sheet belongs to the first-run frame and closes when a terminal opens.
    pub(crate) over_terminal: bool,
    /// A terminal was open since the first-run frame was last drawn.
    pub(crate) saw_terminal: bool,
    /// Sign-in found a Herdr to open; the view opens it after the reply.
    pub(crate) pending_open: Option<HerdrKind>,
    /// The open Delete Host confirmation (#250).
    pub(crate) delete: Option<delete::DeleteConfirm>,
    /// Each Hosts row's sideways scroll (a left swipe shows its Delete).
    pub(crate) swipes: delete::Swipes,
}

impl HostsSheetState {
    pub(crate) fn open_list(&mut self) {
        self.list_open = true;
    }

    pub(crate) fn open_add(&mut self, mode: SheetMode, host: &str) {
        self.delete = None;
        self.swipes.close_all();
        self.name = None;
        self.nearby = nearby_view::NearbySheet::default();
        self.add = Some(AddHostSession::new(mode, host));
    }

    pub(crate) fn close(&mut self) {
        self.list_open = false;
        self.add = None;
        self.delete = None;
        self.swipes.close_all();
        self.name = None;
        self.nearby = nearby_view::NearbySheet::default();
        self.over_terminal = false;
    }

    /// Called every frame. A sheet of the first-run frame does not stay on
    /// top of a terminal that opened behind it.
    pub(crate) fn note_terminal(&mut self, open: bool) {
        if !open {
            return;
        }
        self.saw_terminal = true;
        if self.visible() && !self.over_terminal {
            self.close();
        }
    }

    pub(crate) fn visible(&self) -> bool {
        self.list_open || self.add.is_some()
    }

    /// With no host saved the Add host sheet opens by itself, once, after the
    /// startup load answered; with hosts saved the list opens instead. The
    /// same happens again when a terminal that was open has closed.
    pub(crate) fn auto_open(&mut self, loaded: bool, host_count: usize) -> bool {
        let returning = std::mem::take(&mut self.saw_terminal);
        if returning {
            self.auto_opened = false;
        }
        if self.auto_opened || !(loaded || returning) {
            return false;
        }
        self.auto_opened = true;
        if host_count == 0 {
            self.open_add(SheetMode::Add, "");
        } else {
            self.open_list();
        }
        true
    }

    /// Move the Add host flow with a finished background job. Replies for a
    /// job the user cancelled never get here (`Setup::epoch`).
    pub(crate) fn observe(&mut self, result: &crate::Result<Reply>) {
        let Some(session) = self.add.as_mut() else {
            return;
        };
        match result {
            Ok(Reply::Scanned(target, scan, fingerprints)) => {
                let outcome = match scan {
                    HostKeyScan::New(_) => ScanOutcome::New {
                        fingerprint: fingerprints.clone(),
                    },
                    HostKeyScan::Trusted => ScanOutcome::Pinned {
                        fingerprint: fingerprints.clone(),
                    },
                    HostKeyScan::Refused(failure) => ScanOutcome::Failed(failure.to_string()),
                };
                session.finish_scan(target, outcome);
            }
            Ok(Reply::Mismatch {
                target,
                expected,
                seen,
            }) => session.finish_scan(
                target,
                ScanOutcome::Mismatch {
                    expected: expected.clone(),
                    seen: seen.clone(),
                },
            ),
            Ok(Reply::Saved(..)) => session.finish_trust(Ok(())),
            Ok(Reply::SignedIn(_, _, probe)) => {
                self.pending_open = session.finish_sign_in(Ok(*probe));
            }
            Ok(_) => {}
            Err(error) => {
                let reason = error.to_string();
                if session.signing_in {
                    self.pending_open = session.finish_sign_in(Err(reason));
                } else if session.flow.scanning && session.flow.step == AddHostStep::Trust {
                    session.finish_trust(Err(reason));
                } else if session.flow.scanning {
                    let target = session.flow.typed_host.trim().to_owned();
                    session.finish_scan(&target, ScanOutcome::Failed(reason));
                }
            }
        }
    }
}

impl Smoke {
    /// Footer Hosts: open the host list. `true` means handled.
    pub(crate) fn open_hosts(&mut self, cx: &mut Context<Self>) -> bool {
        self.shell.hosts_sheet.open_list();
        self.shell.hosts_sheet.over_terminal = self.terminal.open;
        self.hosts_sheet_takes_input();
        cx.notify();
        true
    }

    /// The Hosts or Add host sheet is on screen. Integrator files that guard
    /// terminal input with `settings_modal` should guard with this too.
    pub(crate) fn hosts_sheet_modal(&self) -> bool {
        self.shell.hosts_sheet.visible() && self.terminal.open
    }

    pub(crate) fn render_hosts_sheet(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        self.draw_hosts_sheet(window, cx)
    }

    /// The empty frame (and, through `render_hosts_sheet`, the sheet over it)
    /// whenever no terminal is open. `None` only while a terminal is open.
    pub(crate) fn render_first_run(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        self.draw_first_run(window, cx)
    }

    /// Reply hook: called after `Setup::apply`, opens the Herdr sign-in found.
    pub(crate) fn open_pending_herdr(&mut self, cx: &mut Context<Self>) {
        let Some(kind) = self.shell.hosts_sheet.pending_open.take() else {
            return;
        };
        if let Some(session) = &self.shell.hosts_sheet.add {
            let host = session.flow.typed_host.trim().to_owned();
            self.setup_input.set_text(host);
        }
        self.shell.hosts_sheet.close();
        match kind {
            HerdrKind::Herdr1 => self.connect_terminal(cx),
            HerdrKind::Herdr2 => self.connect_core(cx),
        }
    }
}

#[cfg(test)]
#[path = "hosts_sheet_tests.rs"]
mod tests;
