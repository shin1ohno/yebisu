//! The attached terminals and screens of the tab in use, moved unchanged
//! out of `worker.rs` to keep it under the 1,000-line rule (#144 D2).
use super::active_layout;
use crate::core::{
    ended,
    screen::{self, Screen},
    store::{Saved, Workspace},
};
use sheprd_backend::{Backend as _, Role, Size, Terminal as _};
use sheprd_core_client::{CoreBackend, CoreTerminal};

#[derive(Default)]
pub(super) struct Active {
    pub(super) terminals: Vec<CoreTerminal>,
    pub(super) screens: Vec<Screen>,
}

impl Active {
    /// Attaches the tab in use. A terminal whose program already finished
    /// cannot be attached (herdr-core refuses with `finished`): its pane
    /// leaves the layout, and a tab left empty closes (`ended`). The caller
    /// saves the layout. Any other failure is the connection's.
    pub(super) fn attach(
        backend: &CoreBackend,
        workspace: &mut Workspace,
        size: Size,
        column: Option<crate::shell::single_pane::ColumnMetrics>,
    ) -> crate::Result<Self> {
        loop {
            let Some(saved) = active_layout(workspace) else {
                return Ok(Self::default());
            };
            let initial = screen::pane_size(size, saved.ids.len(), saved.vertical, column)?;
            let mut terminals = Vec::with_capacity(saved.ids.len());
            let mut gone = None;
            for id in &saved.ids {
                match backend.attach(id, initial, Role::Active) {
                    Ok(terminal) => terminals.push(terminal),
                    Err(error) if ended::finished(&error) => {
                        gone = Some(id.clone());
                        break;
                    }
                    Err(error) => return Err(error.into()),
                }
            }
            let Some(id) = gone else {
                let screens = terminals.iter().map(|_| Screen::default()).collect();
                return Ok(Self { terminals, screens });
            };
            tracing::info!(terminal = %id.as_str(), "Herdr 2 terminal finished; its pane closes");
            // Each pass removes one pane, so this ends.
            drop(terminals);
            ended::remove_terminal(workspace, &id);
        }
    }

    /// Closes the panes whose program ended while another pane of the tab
    /// remains; the last pane keeps its final screen. True when one closed.
    pub(super) fn close_ended(&mut self, saved: &mut Saved) -> crate::Result<bool> {
        let before = saved.ids.get(saved.active).cloned();
        let mut closed = false;
        while self.screens.len() > 1 {
            let Some(index) = self.screens.iter().position(|screen| screen.ended) else {
                break;
            };
            self.terminals.remove(index);
            self.screens.remove(index);
            ended::remove_pane(saved, index);
            closed = true;
        }
        // The pane that takes over tells its program it has the focus.
        if closed
            && saved.ids.get(saved.active) != before.as_ref()
            && !self.screens[saved.active].ended
        {
            self.terminals[saved.active].focus(true)?;
        }
        Ok(closed)
    }

    /// Sends each drawn pane its size; an ended or hidden pane keeps its own.
    pub(super) fn resize(
        &mut self,
        saved: &Saved,
        size: Size,
        column: Option<crate::shell::single_pane::ColumnMetrics>,
    ) -> crate::Result<()> {
        let sizes = screen::pane_sizes(
            size,
            self.terminals.len(),
            saved.vertical,
            saved.active,
            column,
        )?;
        for ((terminal, screen), sent) in self.terminals.iter_mut().zip(&self.screens).zip(sizes) {
            if let (false, Some(sent)) = (screen.ended, sent) {
                terminal.resize(sent)?;
            }
        }
        Ok(())
    }

    pub(super) fn focus(&mut self, saved: &mut Saved, next: usize) -> crate::Result<()> {
        if !self.screens[saved.active].ended {
            self.terminals[saved.active].focus(false)?;
        }
        saved.active = next;
        if !self.screens[next].ended {
            self.terminals[next].focus(true)?;
        }
        Ok(())
    }
}
