//! The names a rename changes, wherever they are drawn (issue #143 B-8):
//! the sidebar's space and agent rows, the rail, its menu header, the tab
//! row, the space switcher and list, the pane chip and the Space menu.
//!
//! While a name is being sent it is drawn in place of the host's, muted;
//! once accepted, in the normal color until a newer snapshot names it. A
//! label being edited draws nothing (the field covers it, in the same box)
//! and carries the slot canvas that reports where the field goes.
use super::{RenameAnchor, RenameObject, RenameTarget, slot::slot_canvas};
use crate::{
    app::Smoke,
    shell::{
        HerdrKind,
        rail::{RailItem, RailTarget},
    },
};
use gpui::{Div, IntoElement, ParentElement, Styled, div, rgb, transparent_black};

impl Smoke {
    /// The name `object` shows now: the pending one, or `host`.
    pub(crate) fn shown_name(&self, boot: &str, object: &RenameObject, host: String) -> String {
        self.rename_label(boot, object)
            .map_or(host, |(pending, _)| pending)
    }

    /// `label` with `name` in it, renamable over `anchor`: the pending name
    /// and its color, an empty box of the same size while it is edited, and
    /// the anchor's slot then.
    pub(crate) fn renamable(
        &self,
        label: Div,
        boot: &str,
        anchor: RenameAnchor,
        object: RenameObject,
        name: String,
    ) -> Div {
        let (name, ink) = match self.rename_label(boot, &object) {
            Some((pending, ink)) => (pending, Some(ink)),
            None => (name, None),
        };
        let label = match ink {
            Some(ink) => label.text_color(rgb(ink)),
            None => label,
        };
        if self.shell.rename.editing(anchor, &object) {
            let slots = self.shell.rename.slots.clone();
            label
                .relative()
                .child(div().text_color(transparent_black()).child(name))
                .child(slot_canvas(slots, anchor, object).into_any_element())
        } else {
            label.child(name)
        }
    }

    /// A rail item's renamable object: its space, or its agent's pane.
    pub(crate) fn rail_rename_target(&self, item: &RailItem) -> Option<RenameTarget> {
        let (host, kind, object, boot, generation) = match &item.target {
            RailTarget::Host(_) => return None,
            RailTarget::Space {
                host,
                kind,
                workspace_id,
                boot,
                generation,
            } => (
                host.clone(),
                *kind,
                RenameObject::Space(workspace_id.clone()),
                boot,
                *generation,
            ),
            RailTarget::Agent {
                id,
                boot,
                generation,
                ..
            } => (
                id.host.clone(),
                id.kind,
                RenameObject::Pane(id.pane.clone()),
                boot,
                *generation,
            ),
        };
        Some(RenameTarget {
            host,
            kind,
            boot: boot.clone(),
            generation,
            object,
        })
    }

    /// Rail items with the pending names (Herdr 1 spaces and agents).
    pub(crate) fn rail_names(&self, items: &mut [RailItem]) {
        for item in items {
            if let Some(target) = self.rail_rename_target(item)
                && target.kind == HerdrKind::Herdr1
            {
                item.name = self.shown_name(&target.boot, &target.object, item.name.clone());
            }
        }
    }
}

/// What the terminal painter needs for the pane chips (`PaneChip`), taken
/// from `RenameState` before the paint borrows the terminal.
#[derive(Clone, Default)]
pub(crate) struct ChipRename {
    pub(crate) slots: super::slot::Slots,
    /// The pane whose chip is being edited.
    pub(crate) editing: Option<String>,
    /// A pane's pending name and its color.
    pub(crate) pending: Option<(String, String, u32)>,
}

impl ChipRename {
    pub(crate) fn pending_for(&self, pane: &str) -> Option<(String, u32)> {
        self.pending
            .as_ref()
            .filter(|(id, _, _)| id == pane)
            .map(|(_, text, ink)| (text.clone(), *ink))
    }

    pub(crate) fn editing(&self, pane: &str) -> bool {
        self.editing.as_deref() == Some(pane)
    }

    /// The chip of `pane` was drawn at `bounds` inside `clip`.
    pub(crate) fn report(
        &self,
        pane: &str,
        bounds: gpui::Bounds<gpui::Pixels>,
        clip: gpui::Bounds<gpui::Pixels>,
    ) {
        self.slots.report(
            RenameAnchor::PaneChip,
            &RenameObject::Pane(pane.to_owned()),
            bounds,
            clip,
        );
    }
}

/// How a pane chip draws its name: a pending rename's color, or nothing
/// (the rename field covers it).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ChipLook {
    pub(crate) ink: Option<u32>,
    pub(crate) hidden: bool,
}
