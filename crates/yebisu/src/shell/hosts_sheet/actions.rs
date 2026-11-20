//! What each tap in the Hosts and Add host sheets starts. The rules are in
//! `AddHostFlow`; these functions only check them and start the jobs.
use crate::{app::Smoke, setup::Job, shell::add_host::SheetMode};
use gpui::Context;

impl Smoke {
    /// The sheet takes text input away from a terminal that is still open.
    pub(super) fn hosts_sheet_takes_input(&mut self) {
        if !self.terminal.open {
            return;
        }
        self.actions.palette = false;
        self.actions.consumed.clear();
        self.terminal.input.detach();
        self.terminal.input.fence();
    }

    pub(crate) fn hosts_open_add(&mut self, mode: SheetMode, host: &str, cx: &mut Context<Self>) {
        if self.setup.busy {
            return;
        }
        self.setup.pending = None;
        self.setup.fingerprints.clear();
        self.setup_input.set_text(host.to_owned());
        let over_terminal = self.shell.hosts_sheet.over_terminal || self.terminal.open;
        self.shell.hosts_sheet.open_add(mode, host);
        self.shell.hosts_sheet.over_terminal = over_terminal;
        self.hosts_sheet_takes_input();
        cx.notify();
        // A saved host is already trusted: Open signs in at once.
        if mode == SheetMode::Open {
            self.hosts_sign_in(cx);
        } else {
            self.read_device_key(cx);
        }
    }

    pub(crate) fn hosts_close(&mut self, cx: &mut Context<Self>) {
        let in_flight = self
            .shell
            .hosts_sheet
            .add
            .as_ref()
            .is_some_and(|session| session.flow.scanning || session.signing_in);
        if in_flight && self.setup.busy {
            self.setup.busy = false;
            self.setup.epoch = self.setup.epoch.wrapping_add(1);
        }
        self.shell.hosts_sheet.close();
        // Keys went to the sheet while it was open (shell/footer/keys.rs).
        if self.terminal.open {
            self.terminal.needs_focus = true;
        }
        cx.notify();
    }

    /// The host field is `setup_input`; the flow follows it until a job runs
    /// or the host is trusted, then the field follows the flow.
    pub(crate) fn hosts_sync_field(&mut self) {
        let Some(session) = self.shell.hosts_sheet.add.as_mut() else {
            return;
        };
        let text = self.setup_input.text.clone();
        if !session.flow.edit_host(&text) && session.flow.typed_host != text {
            self.setup_input.set_text(session.flow.typed_host.clone());
        }
    }

    pub(crate) fn hosts_scan(&mut self, cx: &mut Context<Self>) {
        self.hosts_sync_field();
        if self.setup.busy {
            return;
        }
        let Some(target) = self
            .shell
            .hosts_sheet
            .add
            .as_mut()
            .and_then(|session| session.begin_scan())
        else {
            return;
        };
        self.start_job(Job::Scan(target), cx);
    }

    pub(crate) fn hosts_cancel_scan(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.shell.hosts_sheet.add.as_mut() else {
            return;
        };
        if !session.flow.scanning || !self.setup.busy {
            return;
        }
        session.cancel_scan();
        self.setup.busy = false;
        self.setup.epoch = self.setup.epoch.wrapping_add(1);
        cx.notify();
    }

    pub(crate) fn hosts_trust(&mut self, cx: &mut Context<Self>) {
        self.hosts_sync_field();
        if self.setup.busy {
            return;
        }
        let typed = match self.shell.hosts_sheet.add.as_ref() {
            Some(session) if session.flow.can_trust() => session.flow.typed_host.trim().to_owned(),
            _ => return,
        };
        // Keep trust bound to the scanned target (app_setup.rs history).
        let Some((target, key)) = self.setup.pending.take() else {
            return;
        };
        if target != typed {
            return;
        }
        let started = self
            .shell
            .hosts_sheet
            .add
            .as_mut()
            .is_some_and(|session| session.flow.begin_trust());
        if started {
            self.start_job(Job::Trust(target, key), cx);
        }
    }

    pub(crate) fn hosts_reject_key(&mut self, cx: &mut Context<Self>) {
        if let Some(session) = self.shell.hosts_sheet.add.as_mut() {
            session.flow.reject_key();
        }
        self.setup.pending = None;
        self.setup.fingerprints.clear();
        cx.notify();
    }

    pub(crate) fn hosts_sign_in(&mut self, cx: &mut Context<Self>) {
        self.hosts_sync_field();
        if self.setup.busy {
            return;
        }
        let Some(target) = self
            .shell
            .hosts_sheet
            .add
            .as_mut()
            .and_then(|session| session.begin_sign_in())
        else {
            return;
        };
        self.setup_input.set_text(target.clone());
        self.start_job(Job::Authenticate(target), cx);
    }
}
