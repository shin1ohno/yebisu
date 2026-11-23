//! L4 rail rules, one topic per file. Names follow plan v2 7.9.
use super::*;
use crate::shell::{HerdrKind, sidebar_status::SidebarStatus};

#[path = "rail_tests/a11y.rs"]
mod a11y;
#[path = "rail_tests/agent_icon.rs"]
mod agent_icon;
#[path = "rail_tests/canvas.rs"]
mod canvas;
#[path = "rail_tests/collapse.rs"]
mod collapse;
#[path = "rail_tests/entries.rs"]
mod entries;
#[path = "rail_tests/navigation.rs"]
mod navigation;
#[path = "rail_tests/new_space.rs"]
mod new_space;
#[path = "rail_tests/render.rs"]
mod render;
#[path = "rail_tests/scale.rs"]
mod scale;
#[path = "rail_tests/text.rs"]
mod text;
#[path = "rail_tests/toggle.rs"]
mod toggle;

pub(super) fn space(id: &str, label: &str, agents: &[SidebarStatus]) -> RailSpace {
    RailSpace {
        workspace_id: id.to_owned(),
        number: 1,
        label: label.to_owned(),
        agents: agents.to_vec(),
        focused: false,
    }
}

pub(super) fn host(name: &str, connected: bool, spaces: Vec<RailSpace>) -> RailHost {
    RailHost {
        name: name.to_owned(),
        kind: HerdrKind::Herdr1,
        connected,
        current: false,
        boot: "boot-1".to_owned(),
        generation: 3,
        spaces,
    }
}
