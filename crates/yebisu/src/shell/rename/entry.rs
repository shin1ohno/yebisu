//! Where a rename starts (issue #143 B-2): a double tap on a space row's
//! name or a tab's label, the long-press menus (`sidebar/long_press.rs`,
//! `rail/menu.rs`, `tab_row/menu.rs`), and the palette and the menu bar
//! (`Rename Tab`, `Rename Space`, `Rename Pane`, run by `actions.rs`).
use super::{HERDR2, RenameAnchor, RenameObject, RenameTarget};
use crate::{
    app::Smoke,
    board::snapshot,
    shell::{
        HerdrKind,
        sidebar_list::ListSpace,
        style::{text, weight},
    },
};
use gpui::{AnyElement, ClickEvent, Context, ElementId, div, prelude::*, px};
use sheprd_commands::Command;

impl Smoke {
    /// A space row's name. A double tap renames the space: the first tap
    /// goes on to the row (it moves there), the second stops here, so the
    /// row moves once. Herdr 2 rows only say why not.
    pub(crate) fn space_name(&self, row: &ListSpace, cx: &mut Context<Self>) -> AnyElement {
        let space = &row.entry;
        let object = RenameObject::Space(space.id.clone());
        let target = RenameTarget {
            host: row.host.clone(),
            kind: row.kind,
            boot: row.boot.clone(),
            generation: row.generation,
            object: object.clone(),
        };
        let label = div()
            .min_w_0()
            .truncate()
            .text_size(px(text::NAME))
            .when(space.current, |name| name.font_weight(weight::BOLD));
        let selector = format!("sidebar-space-name-{}-{}", row.host, space.id);
        div()
            .id(ElementId::Name(selector.clone().into()))
            .debug_selector(move || selector)
            .relative()
            .min_w_0()
            .when(row.kind == HerdrKind::Herdr1 && space.current, |name| {
                name.children(crate::qa_ax::mark(
                    crate::qa_ax::ids::SIDEBAR_CURRENT_SPACE,
                    "Current space",
                ))
            })
            .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                if event.click_count() != 2 {
                    return;
                }
                cx.stop_propagation();
                if target.kind == HerdrKind::Herdr2 {
                    this.terminal.live.status = HERDR2.into();
                    cx.notify();
                    return;
                }
                this.begin_rename(target.clone(), RenameAnchor::SpaceRow);
                cx.notify();
            }))
            .child(self.renamable(
                label,
                &row.boot,
                RenameAnchor::SpaceRow,
                object,
                space.name.clone(),
            ))
            .into_any_element()
    }

    /// The palette's and the menu bar's rename commands' target: the
    /// focused tab, space or pane of the selected host, and where its field
    /// goes.
    pub(crate) fn command_rename_target(
        &self,
        command: Command,
    ) -> Option<(RenameTarget, RenameAnchor)> {
        let shown = snapshot(&self.terminal.live)?;
        let (object, anchor) = match command {
            Command::RenameTab => (
                RenameObject::Tab(shown.focused_tab_id.clone()?),
                RenameAnchor::TabLabel,
            ),
            Command::RenameWorkspace => (
                RenameObject::Space(shown.focused_workspace_id.clone()?),
                RenameAnchor::SpaceRow,
            ),
            Command::RenamePane => (
                RenameObject::Pane(shown.focused_pane_id.clone()?),
                RenameAnchor::PaneChip,
            ),
            _ => return None,
        };
        Some((self.rename_target(object)?, anchor))
    }

    /// Runs a palette or menu bar rename command (`run_action_inner`).
    pub(crate) fn run_rename_command(&mut self, command: Command) -> crate::Result<()> {
        let (target, anchor) = self
            .command_rename_target(command)
            .ok_or_else(|| crate::Error::Ssh("Target unavailable".into()))?;
        if let Some(reason) = self.rename_unavailable(&target) {
            return Err(crate::Error::Ssh(reason.into()));
        }
        self.actions.palette = false;
        self.begin_rename(target, anchor);
        Ok(())
    }
}
