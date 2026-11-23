//! Agent-kind icons: the fork's identity table, the embedded files, and the
//! sidebar AGENTS row, space row and rail row that draw them.
use super::*;
use crate::{
    app::Smoke,
    core::{SessionInfo, TabInfo},
    shell::{
        HerdrKind,
        herdr2_frame::{CoreStatus, session::catalog_content},
        sidebar::{
            SidebarState,
            model::{SpaceInput, snapshot_content, space_entry},
        },
    },
};
use gpui::{Bounds, Entity, Pixels, TestAppContext, VisualTestContext, px, size};
use sheprd_client::protocol::ClientShellSnapshot;
use std::sync::Arc;

/// The fork's `agent_icons!` table (herdr-gpui 9327f4482faeb0b9ccfb086c912a300a98ebad61,
/// crates/sheprd-gpui/src/icons.rs:66-92) in its order, without `omo`.
const FORK: &[(&str, AgentIcon)] = &[
    ("pi", AgentIcon::Pi),
    ("claude", AgentIcon::Claude),
    ("codex", AgentIcon::Codex),
    ("gemini", AgentIcon::Gemini),
    ("cursor", AgentIcon::Cursor),
    ("devin", AgentIcon::Devin),
    ("agy", AgentIcon::Antigravity),
    ("cline", AgentIcon::Cline),
    ("omp", AgentIcon::Omp),
    ("mastracode", AgentIcon::Mastracode),
    ("opencode", AgentIcon::OpenCode),
    ("copilot", AgentIcon::Copilot),
    ("kimi", AgentIcon::Kimi),
    ("kiro", AgentIcon::Kiro),
    ("droid", AgentIcon::Droid),
    ("amp", AgentIcon::Amp),
    ("grok", AgentIcon::Grok),
    ("hermes", AgentIcon::Hermes),
    ("kilo", AgentIcon::Kilo),
    ("qodercli", AgentIcon::Qodercli),
    ("qwen", AgentIcon::Qwen),
    ("letta", AgentIcon::Letta),
    ("maki", AgentIcon::Maki),
    ("muse", AgentIcon::Muse),
];

#[test]
fn every_fork_identity_maps_to_its_icon() {
    for (identity, icon) in FORK {
        assert_eq!(
            AgentIcon::from_identity(Some(identity)),
            *icon,
            "{identity}"
        );
        assert_eq!(icon.label(), *identity);
    }
    assert_eq!(
        AgentIcon::ALL.len(),
        FORK.len() + 1,
        "the table plus generic"
    );
}

#[test]
fn identities_match_without_regard_to_case() {
    for (identity, icon) in FORK {
        let upper = identity.to_ascii_uppercase();
        let mut title = identity.to_string();
        title[..1].make_ascii_uppercase();
        assert_eq!(AgentIcon::from_identity(Some(&upper)), *icon, "{upper}");
        assert_eq!(AgentIcon::from_identity(Some(&title)), *icon, "{title}");
    }
}

#[test]
fn unknown_empty_missing_and_omo_identities_are_generic() {
    for identity in [
        None,
        Some(""),
        Some("unknown"),
        Some("claude code"),
        Some(" claude"),
        Some("codex2"),
        // The fork's PNG mark has no license record (controller 2026-11-23).
        Some("omo"),
        Some("generic"),
    ] {
        assert_eq!(
            AgentIcon::from_identity(identity),
            AgentIcon::Generic,
            "{identity:?}"
        );
    }
}

#[test]
fn each_icon_embeds_its_unchanged_svg_and_no_file_is_unused() {
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/icons");
    for icon in AgentIcon::ALL {
        let path = folder.join(format!("agent-{}.svg", icon.label()));
        let file = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(icon.bytes(), file.as_slice(), "{}", icon.label());
        assert!(icon.bytes().starts_with(b"<svg"), "{}", icon.label());
    }
    let mut shipped = std::fs::read_dir(&folder)
        .unwrap_or_else(|e| panic!("{}: {e}", folder.display()))
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| name.starts_with("agent-"))
        .collect::<Vec<_>>();
    shipped.sort();
    let mut mapped = AgentIcon::ALL
        .iter()
        .map(|icon| format!("agent-{}.svg", icon.label()))
        .collect::<Vec<_>>();
    mapped.sort();
    assert_eq!(shipped, mapped);
}

fn fixture() -> ClientShellSnapshot {
    serde_json::from_str(include_str!("../../../fixtures/endpoint-snapshot-v1.json"))
        .unwrap_or_else(|error| panic!("endpoint fixture: {error}"))
}

#[test]
fn a_generic_daemon_identity_falls_back_to_the_display_name() {
    let mut agent = fixture().agents.remove(0);
    assert_eq!(AgentIcon::of_agent(&agent), AgentIcon::Claude);
    agent.agent = Some("acp".into());
    agent.display_agent = Some("Codex".into());
    assert_eq!(AgentIcon::of_agent(&agent), AgentIcon::Codex);
    agent.agent = Some("claude".into());
    assert_eq!(
        AgentIcon::of_agent(&agent),
        AgentIcon::Claude,
        "identity wins"
    );
    agent.agent = None;
    agent.display_agent = None;
    assert_eq!(AgentIcon::of_agent(&agent), AgentIcon::Generic);
}

#[test]
fn herdr1_rows_and_spaces_carry_the_agent_icon() {
    let state = SidebarState::default();
    let (spaces, lines) = snapshot_content("pro-dev", &fixture(), &state);
    assert_eq!(lines[0].row.icon, AgentIcon::Claude);
    assert_eq!(
        spaces[0].icon,
        Some(AgentIcon::Claude),
        "the agent the row names"
    );
    let empty = space_entry(
        HerdrKind::Herdr1,
        SpaceInput {
            id: "w2".into(),
            name: "empty".into(),
            tabs: 1,
            branch: None,
            current: false,
        },
        &[],
    );
    assert_eq!(empty.detail, "No agent");
    assert_eq!(empty.icon, None, "canvas v27 draws no icon for No agent");
}

fn core_status(address: &str, app: &str) -> CoreStatus {
    CoreStatus {
        address: address.into(),
        status: Some(
            serde_json::from_value(serde_json::json!({
                "app": app,
                "state": "working",
                "title": format!("agent {address}"),
            }))
            .unwrap_or_else(|e| panic!("wire status: {e}")),
        ),
    }
}

#[test]
fn herdr2_rows_take_the_icon_from_the_status_app() {
    let statuses = vec![core_status("t1", "Codex"), core_status("t2", "")];
    let session = SessionInfo {
        name: "default".into(),
        tabs: vec![TabInfo {
            id: "default".into(),
            label: "default".into(),
            active: true,
            panes: vec!["t1".into(), "t2".into()],
            focused_pane: 0,
            vertical: false,
        }],
        confirmed: true,
        core_id: None,
        statuses: Vec::new(),
        created: 0,
    };
    let state = SidebarState::default();
    let (spaces, lines) = catalog_content("pro-dev", 1, "default", &[session], &statuses, &state);
    let icons = lines
        .iter()
        .map(|line| (line.row.identity.pane.as_str(), line.row.icon))
        .collect::<Vec<_>>();
    assert_eq!(
        icons,
        [("t1", AgentIcon::Codex), ("t2", AgentIcon::Generic)]
    );
    assert_eq!(spaces[0].icon, Some(AgentIcon::Codex));
}

fn show(cx: &mut TestAppContext) -> (Entity<Smoke>, &mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1376.), px(1032.)));
    view.update(cx, |this, cx| {
        this.select_host("pro-dev");
        // The board shows the sidebar; hiding it shows the rail.
        this.actions.board_hidden = false;
        this.shell.fit.window_controls = Some(gpui::Edges::default());
        this.terminal.live.connected = true;
        this.terminal.live.snapshot = Some(Arc::new(fixture()));
        cx.notify();
    });
    cx.run_until_parked();
    (view, cx)
}

/// `debug_bounds` takes a `'static` selector; these are built per row.
fn drawn(cx: &mut VisualTestContext, selector: &str) -> Option<Bounds<Pixels>> {
    cx.debug_bounds(selector.to_owned().leak())
}

fn bounds(cx: &mut VisualTestContext, selector: &str) -> Bounds<Pixels> {
    drawn(cx, selector).unwrap_or_else(|| panic!("{selector} was not drawn"))
}

/// The icon is `place.size` square, inside its row, `place.gap` before `next`.
fn assert_icon_before(cx: &mut VisualTestContext, row: &str, icon: &str, next: &str, place: Place) {
    let row = bounds(cx, row);
    let icon = bounds(cx, icon);
    let next = bounds(cx, next);
    assert_eq!(icon.size, size(px(place.size), px(place.size)));
    assert!(row.origin.x <= icon.origin.x && icon.right() <= row.right());
    assert!(row.origin.y <= icon.origin.y && icon.bottom() <= row.bottom());
    assert_eq!(next.origin.x - icon.right(), px(place.gap));
}

#[gpui::test]
fn the_agents_row_draws_the_icon_before_the_name(cx: &mut TestAppContext) {
    let (_, cx) = show(cx);
    let row = "sidebar-agent-pro-dev-w1:p1";
    assert_icon_before(
        cx,
        row,
        &format!("{row}-agent-icon-claude"),
        &format!("{row}-name"),
        AGENTS,
    );
}

#[gpui::test]
fn the_space_row_draws_the_icon_of_the_agent_it_names(cx: &mut TestAppContext) {
    let (_, cx) = show(cx);
    let row = "sidebar-space-pro-dev-w1";
    assert_icon_before(
        cx,
        row,
        &format!("{row}-agent-icon-claude"),
        &format!("{row}-detail"),
        SPACE,
    );
}

#[test]
fn the_canvas_sizes_and_ink() {
    assert_eq!(AGENTS, Place { size: 14., gap: 5. });
    assert_eq!(SPACE, Place { size: 13., gap: 5. });
    assert_eq!(RAIL, Place { size: 12., gap: 4. });
    assert_eq!(INK, 0xacb2bb);
}
