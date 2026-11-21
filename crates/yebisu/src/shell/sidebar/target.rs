//! L3: notification targets on the agent identity (S-2). Owner: L3.
//!
//! A notification's recorded pane is resolved by (host, Herdr kind, pane id),
//! the identity of the AGENTS row, never by its name or its recorded tab.
use super::AgentIdentity;
use crate::{notifications::Context, shell::HerdrKind};
use sheprd_client::protocol::ClientShellSnapshot;

/// Where a notification's agent is now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Destination {
    pub(crate) workspace: String,
    pub(crate) tab: String,
    pub(crate) pane: String,
}

/// The agent a notification is about, on the connection that delivered it.
pub(crate) fn notification_identity(target: &Context) -> Option<AgentIdentity> {
    target.pane.as_ref().map(|pane| AgentIdentity {
        host: target.host.clone(),
        kind: target.herdr,
        pane: pane.clone(),
    })
}

/// Resolves a notification's agent in one connection's newest snapshot,
/// wherever its tab or space is now and whatever it is called. Another
/// connection (host or Herdr kind) or another daemon boot never resolves.
pub(crate) fn resolve_target(
    target: &Context,
    agent: &AgentIdentity,
    connection: (&str, HerdrKind),
    snapshot: &ClientShellSnapshot,
) -> Option<Destination> {
    if (agent.host.as_str(), agent.kind) != connection
        || notification_identity(target).as_ref() != Some(agent)
        || target.boot != snapshot.boot_id
    {
        return None;
    }
    snapshot
        .panes
        .iter()
        .find(|p| p.pane_id == agent.pane)
        .map(|p| Destination {
            workspace: p.workspace_id.clone(),
            tab: p.tab_id.clone(),
            pane: p.pane_id.clone(),
        })
}
