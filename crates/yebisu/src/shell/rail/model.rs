//! The rail's rows as plain data: hosts and their spaces, with the counts of
//! agents that need the user. No GPUI type, so the rules are tested directly.
use crate::shell::{HerdrKind, sidebar::AgentIdentity, sidebar_status::SidebarStatus};
use crate::{hosts::HostName, shell::rail_badges::badge_accessibility};

/// One space of a host, as the rail needs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RailSpace {
    pub(crate) workspace_id: String,
    pub(crate) number: usize,
    pub(crate) label: String,
    /// The status of every agent in the space (the badge counts).
    pub(crate) agents: Vec<SidebarStatus>,
    /// The space the terminal shows now.
    pub(crate) focused: bool,
}

/// One host and its spaces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RailHost {
    pub(crate) name: String,
    pub(crate) kind: HerdrKind,
    pub(crate) connected: bool,
    /// The host the terminal shows now.
    pub(crate) current: bool,
    /// Board target guards (a stale tap is refused, not applied).
    pub(crate) boot: String,
    pub(crate) generation: u64,
    pub(crate) spaces: Vec<RailSpace>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RailItemKind {
    Host,
    Space,
    Agent,
}

/// What a tap on the row does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RailTarget {
    Host(String),
    Space {
        host: String,
        workspace_id: String,
        boot: String,
        generation: u64,
    },
    Agent {
        id: AgentIdentity,
        workspace_id: String,
        boot: String,
        generation: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RailItem {
    pub(crate) kind: RailItemKind,
    /// The full name; the button shows it cut off, speech reads all of it.
    pub(crate) name: String,
    pub(crate) host: String,
    pub(crate) space: Option<String>,
    pub(crate) herdr: HerdrKind,
    /// A space's status; `None` for a host row.
    pub(crate) status: Option<SidebarStatus>,
    pub(crate) connected: bool,
    pub(crate) current: bool,
    pub(crate) pinned: bool,
    pub(crate) needs: usize,
    pub(crate) error: usize,
    pub(crate) boot: String,
    pub(crate) generation: u64,
    pub(crate) target: RailTarget,
}

fn herdr_text(kind: HerdrKind) -> &'static str {
    match kind {
        HerdrKind::Herdr1 => "Herdr 1",
        HerdrKind::Herdr2 => "Herdr 2",
    }
}

impl RailItem {
    /// The spoken name includes the full title and any exact badge counts (S-8).
    pub(crate) fn accessibility_label(&self) -> String {
        if let (true, Some(space), Some(status)) = (self.pinned, &self.space, self.status) {
            return format!("Pinned: {}, {space}, {}", self.name, status.label());
        }
        let mut parts = vec![self.name.clone()];
        match (self.kind, self.status) {
            (RailItemKind::Host, _) => {
                parts.push(if self.connected {
                    "connected".to_owned()
                } else {
                    "not connected".to_owned()
                });
                parts.push(herdr_text(self.herdr).to_owned());
            }
            (RailItemKind::Space | RailItemKind::Agent, status) => {
                parts.push(self.host.clone());
                if let Some(status) = status {
                    parts.push(status.label().to_owned());
                }
                parts.extend(badge_accessibility(self.needs, self.error));
            }
        }
        if self.current {
            parts.push("current".to_owned());
        }
        parts.join(", ")
    }
}

fn count(agents: &[SidebarStatus], wanted: SidebarStatus) -> usize {
    agents.iter().filter(|status| **status == wanted).count()
}

fn space_name(space: &RailSpace) -> String {
    if space.label.trim().is_empty() {
        format!("Space {}", space.number)
    } else {
        space.label.clone()
    }
}

/// Hosts in the order given, each followed by its spaces; a host that is not
/// connected lists no spaces (a previous connection's spaces are not kept).
pub(crate) fn rail_items(hosts: &[RailHost]) -> Vec<RailItem> {
    let mut items = Vec::new();
    for host in hosts {
        let display = HostName::new(&host.name).to_string();
        items.push(RailItem {
            kind: RailItemKind::Host,
            name: display.clone(),
            host: display.clone(),
            space: None,
            herdr: host.kind,
            status: None,
            connected: host.connected,
            current: false,
            pinned: false,
            needs: 0,
            error: 0,
            boot: host.boot.clone(),
            generation: host.generation,
            target: RailTarget::Host(host.name.clone()),
        });
        if !host.connected {
            continue;
        }
        for space in &host.spaces {
            items.push(RailItem {
                kind: RailItemKind::Space,
                name: space_name(space),
                host: display.clone(),
                space: None,
                herdr: host.kind,
                status: Some(
                    space
                        .agents
                        .iter()
                        .copied()
                        .min_by_key(|status| status.rank())
                        .unwrap_or(SidebarStatus::NoStatus),
                ),
                connected: true,
                current: host.current && space.focused,
                pinned: false,
                needs: count(&space.agents, SidebarStatus::NeedsYou),
                error: count(&space.agents, SidebarStatus::Error),
                boot: host.boot.clone(),
                generation: host.generation,
                target: RailTarget::Space {
                    host: host.name.clone(),
                    workspace_id: space.workspace_id.clone(),
                    boot: host.boot.clone(),
                    generation: host.generation,
                },
            });
        }
    }
    items
}
