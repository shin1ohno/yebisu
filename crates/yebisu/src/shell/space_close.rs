//! Close Space from a sidebar space row's menu (#231). Owner: L3.
//!
//! The menu (`sidebar/long_press.rs`, opened by a long press or a secondary
//! click) lists Close Space beside Rename. Close asks first only when
//! something could be lost (owner decision 2026-11-26):
//!
//! - Herdr 2: a live terminal of the space reported a status of its own (an
//!   agent) or runs a program other than a shell in its foreground
//!   (`core/running.rs`); shells alone close at once.
//! - Herdr 1: its snapshot names agents, never the foreground program, so a
//!   pane is never known to be an idle shell. Close asks whenever the space
//!   has a pane (controller decision 2026-11-26: fail safe; an agent, or
//!   anything unknown, asks).
//!
//! The close itself is the daemon's own: Herdr 1's `workspace.close`, sent
//! like a tab's `tab.close` (`tab_row.rs`), and on Herdr 2 the space leaves
//! the `herdr-app` document through the checked write path
//! (`Intent::RemoveSpace`) before its terminals end (`doc_tabs.rs`
//! `close_space`).
use super::{
    HerdrKind,
    herdr2_frame::session::split_space_identity,
    host_menus::{MenuScope, menu_name},
    style::{color, metric, radius, shadow, text},
    tab_row::HOST_UNAVAILABLE,
};
use crate::{app::Smoke, board::snapshot, core::DocSpace};
use gpui::{
    AnyElement, Context, Pixels, Point, Role, anchored, deferred, div, prelude::*, px, rgb,
};
use sheprd_client::{Method, protocol::ClientShellSnapshot};

const CONFIRM_WIDTH: f32 = 280.;
/// Why a Herdr 1 host that advertises no `workspace.close` cannot close.
pub(crate) const NO_CLOSE_METHOD: &str = "This host can't close spaces";
/// Why a close shown for a space that is gone or moved is refused.
pub(crate) const CLOSE_TARGET_CHANGED: &str = "This space changed; open its menu again";

/// What may be lost, as the confirmation says it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Running {
    /// Herdr 2: this many terminals run a program or an agent.
    Programs(usize),
    /// Herdr 1: this many agents; other programs are not reported.
    Agents(usize),
}

/// Why Close Space asks first, and what it closes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Question {
    pub(crate) tabs: usize,
    pub(crate) running: Running,
}

impl Question {
    /// "Close 2 tabs of this space?"
    pub(crate) fn title(&self) -> String {
        match self.tabs {
            1 => "Close 1 tab of this space?".to_owned(),
            n => format!("Close {n} tabs of this space?"),
        }
    }

    /// What runs there, as far as the host tells.
    pub(crate) fn detail(&self) -> String {
        match self.running {
            Running::Programs(1) => "A program is running in it and will end.".to_owned(),
            Running::Programs(n) => format!("{n} programs are running in it and will end."),
            Running::Agents(0) => "Herdr 1 can't tell whether a program is running.".to_owned(),
            Running::Agents(1) => "An agent is running in it and will end.".to_owned(),
            Running::Agents(n) => format!("{n} agents are running in it and will end."),
        }
    }
}

/// Herdr 1's space `id`: `None` closes at once (a space with no pane),
/// else the question (any pane may run a program the snapshot does not
/// name).
pub(crate) fn herdr1_question(snapshot: &ClientShellSnapshot, id: &str) -> Option<Question> {
    if !snapshot.panes.iter().any(|pane| pane.workspace_id == id) {
        return None;
    }
    Some(Question {
        tabs: snapshot
            .tabs
            .iter()
            .filter(|tab| tab.workspace_id == id)
            .count(),
        running: Running::Agents(
            snapshot
                .agents
                .iter()
                .filter(|agent| agent.workspace_id == id)
                .count(),
        ),
    })
}

/// Herdr 2's document space: `None` closes at once (shells only).
pub(crate) fn herdr2_question(space: &DocSpace) -> Option<Question> {
    (!space.busy.is_empty()).then_some(Question {
        tabs: space.tabs.len(),
        running: Running::Programs(space.busy.len()),
    })
}

/// The space a Close was shown for, on the connection it was shown on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CloseTarget {
    pub(crate) host: String,
    pub(crate) kind: HerdrKind,
    pub(crate) boot: String,
    pub(crate) generation: u64,
    /// The row's id: Herdr 1's workspace id, Herdr 2's `default\0<space>`.
    pub(crate) space: String,
    pub(crate) name: String,
}

/// An open confirmation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CloseConfirm {
    pub(crate) target: CloseTarget,
    pub(crate) question: Question,
    pub(crate) at: Point<Pixels>,
}

impl Smoke {
    /// The document space a Herdr 2 row names, as the drawn `default`
    /// session last read it.
    fn doc_space(&self, identity: &str) -> Option<DocSpace> {
        let (session, Some(id)) = split_space_identity(identity) else {
            return None;
        };
        if session != super::herdr2_frame::HERDR2_SESSION {
            return None;
        }
        self.doc_names()?
            .spaces
            .into_iter()
            .find(|space| space.id == id)
    }

    /// Close Space picked from the menu shown for `shown` at `at`: closes
    /// at once, or opens the confirmation (module docs).
    pub(crate) fn begin_space_close(
        &mut self,
        shown: &MenuScope,
        name: &str,
        at: Point<Pixels>,
    ) -> crate::Result<()> {
        let space = shown
            .space
            .clone()
            .ok_or(crate::Error::NativeMenu("no space was shown"))?;
        let changed = || crate::Error::Ssh(CLOSE_TARGET_CHANGED.into());
        let shown_snapshot = snapshot(&self.terminal.live).ok_or_else(changed)?;
        let kind = self.terminal.herdr_kind();
        let target = CloseTarget {
            host: shown.host.clone(),
            kind,
            boot: shown_snapshot.boot_id.clone(),
            generation: self.terminal.link.generation,
            space: space.clone(),
            name: name.to_owned(),
        };
        let question = match kind {
            HerdrKind::Herdr1 => herdr1_question(shown_snapshot, &space),
            HerdrKind::Herdr2 => herdr2_question(&self.doc_space(&space).ok_or_else(changed)?),
        };
        match question {
            None => self.close_space_now(&target),
            Some(question) => {
                crate::qa_ax::count("space_close_asked");
                self.shell.sidebar.close_confirm = Some(CloseConfirm {
                    target,
                    question,
                    at,
                });
                Ok(())
            }
        }
    }

    /// The confirmation's Close.
    pub(crate) fn confirm_space_close(&mut self) -> crate::Result<()> {
        let confirm = self
            .shell
            .sidebar
            .close_confirm
            .take()
            .ok_or(crate::Error::NativeMenu("no close to confirm"))?;
        self.close_space_now(&confirm.target)
    }

    /// Closes `target` if it is still the selected host's, on the same
    /// connection, and still there.
    pub(crate) fn close_space_now(&mut self, target: &CloseTarget) -> crate::Result<()> {
        if self.terminal.reconnect.target != target.host
            || self.terminal.link.generation != target.generation
            || self.terminal.herdr_kind() != target.kind
        {
            return Err(crate::Error::Ssh(CLOSE_TARGET_CHANGED.into()));
        }
        match target.kind {
            HerdrKind::Herdr1 => self.input_transition(|this| this.close_workspace(target))?,
            HerdrKind::Herdr2 => {
                let space = self
                    .doc_space(&target.space)
                    .ok_or_else(|| crate::Error::Ssh(CLOSE_TARGET_CHANGED.into()))?;
                self.try_core_command(crate::core::Command::CloseDocSpace(space.id))?;
            }
        }
        crate::qa_ax::count("space_close_sent");
        self.observation.record(serde_json::json!({
            "kind": "space-close",
            "herdr": match target.kind {
                HerdrKind::Herdr1 => 1,
                HerdrKind::Herdr2 => 2,
            },
        }));
        Ok(())
    }

    /// Herdr 1's `workspace.close`, with the checks a tab's close has
    /// (`close_tab_by_id_inner`).
    fn close_workspace(&mut self, target: &CloseTarget) -> crate::Result<()> {
        if self.input_modal() {
            return Err(crate::Error::Settings(
                "terminal action blocked by settings".into(),
            ));
        }
        if self.terminal.reconnect.suspended || self.terminal.live.version_mismatch.get().is_some()
        {
            return Err(crate::Error::Ssh(HOST_UNAVAILABLE.into()));
        }
        self.handoff.cancel();
        self.handoff.selection = None;
        self.actions.palette = false;
        let terminal = &mut self.terminal;
        let mut live = terminal
            .link
            .inbox
            .try_lock()
            .map_err(|_| crate::Error::Ssh("Host busy".into()))?;
        let shown =
            snapshot(&live).ok_or_else(|| crate::Error::Ssh("Waiting for snapshot".into()))?;
        if !Method::WorkspaceClose.advertised_in(&live.methods) {
            return Err(crate::Error::Ssh(NO_CLOSE_METHOD.into()));
        }
        if !live.connected
            || live.surface.as_ref().is_some_and(|s| s.popup.is_some())
            || terminal.navigating.is_some()
        {
            return Err(crate::Error::Ssh(HOST_UNAVAILABLE.into()));
        }
        if shown.boot_id != target.boot
            || !shown
                .workspaces
                .iter()
                .any(|w| w.workspace_id == target.space)
        {
            return Err(crate::Error::Ssh(CLOSE_TARGET_CHANGED.into()));
        }
        let revision = shown.revision;
        let handle = terminal
            .link
            .handle
            .as_ref()
            .ok_or_else(|| crate::Error::Ssh("Disconnected".into()))?;
        let request = handle
            .request(
                &target.boot,
                Method::WorkspaceClose,
                serde_json::json!({ "workspace_id": target.space }),
            )
            .map_err(|e| crate::Error::Ssh(e.to_string()))?;
        terminal.navigating = Some(crate::board::Navigation {
            boot: target.boot.clone(),
            revision,
            target: crate::board::Target::Command,
            request: request.clone(),
        });
        terminal.input.detach();
        live.pending_request = Some(request);
        live.completed_request = None;
        live.failed_request = None;
        Ok(())
    }

    /// The open confirmation, drawn over everything where the menu was.
    pub(crate) fn render_space_close_confirm(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let confirm = self.shell.sidebar.close_confirm.clone()?;
        let button = |id: &'static str, label: &'static str, ink: u32| {
            div()
                .id(id)
                .role(Role::Button)
                .aria_label(label)
                .relative()
                .flex()
                .items_center()
                .h(px(metric::MENU_ITEM))
                .px(px(metric::SIDE))
                .text_color(rgb(ink))
                .child(label)
        };
        let card = div()
            .id("sidebar-space-close")
            .w(px(CONFIRM_WIDTH))
            .py(px(metric::MENU_PADDING))
            .rounded(px(radius::POPOVER))
            .bg(rgb(color::RAISED))
            .text_color(rgb(color::TEXT))
            .shadow(vec![shadow::POPOVER.box_shadow()])
            .occlude()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.shell.sidebar.close_confirm = None;
                cx.notify();
            }))
            .child(
                div()
                    .px(px(metric::SIDE))
                    .py(px(metric::MENU_PADDING))
                    .child(confirm.question.title())
                    .child(
                        div()
                            .text_size(px(text::SECONDARY))
                            .text_color(rgb(color::MUTED))
                            .child(format!(
                                "{} {}",
                                menu_name(&confirm.target.name),
                                confirm.question.detail()
                            )),
                    ),
            )
            .child(
                button("sidebar-space-close-cancel", "Cancel", color::TEXT)
                    .children(crate::qa_ax::mark(
                        crate::qa_ax::ids::SIDEBAR_SPACE_CLOSE_CANCEL,
                        "Cancel close",
                    ))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.shell.sidebar.close_confirm = None;
                        cx.notify();
                    })),
            )
            .child(
                button("sidebar-space-close-confirm", "Close Space", color::RED)
                    .children(crate::qa_ax::mark(
                        crate::qa_ax::ids::SIDEBAR_SPACE_CLOSE_CONFIRM,
                        "Close space",
                    ))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Err(error) = this.confirm_space_close() {
                            this.terminal.live.status = error.to_string();
                        }
                        cx.notify();
                    })),
            );
        Some(
            deferred(anchored().position(confirm.at).snap_to_window().child(card))
                .with_priority(1)
                .into_any_element(),
        )
    }
}

#[cfg(test)]
#[path = "space_close_tests.rs"]
mod tests;
