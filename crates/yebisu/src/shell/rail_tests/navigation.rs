use super::{canvas::show, *};
use crate::{app::Smoke, board::snapshot, shell::herdr2_frame::CoreStatus};
use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext};
use std::time::{Duration, Instant};

const HOST: &str = "rail-navigation";
const SESSION: &str = "yebisu";

fn wait_for(
    view: &Entity<Smoke>,
    cx: &mut VisualTestContext,
    description: &str,
    condition: impl Fn(&Smoke) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        condition(this)
    }) {
        assert!(Instant::now() < deadline, "{description}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn start_herdr1(view: &Entity<Smoke>, cx: &mut VisualTestContext) {
    let client =
        crate::mock_peer::connect_burst(1).unwrap_or_else(|error| panic!("Herdr 1 peer: {error}"));
    view.update(cx, |this, _| {
        this.select_host_kind(HOST, HerdrKind::Herdr1);
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
    });
    wait_for(view, cx, "Herdr 1 snapshot", |this| {
        this.terminal.live.connected && snapshot(&this.terminal.live).is_some()
    });
}

fn start_herdr2(view: &Entity<Smoke>, cx: &mut VisualTestContext, mixed: bool) {
    if mixed {
        start_herdr1(view, cx);
    }
    let (backend, _) =
        crate::core_mock::recorded(false).unwrap_or_else(|error| panic!("Herdr 2 peer: {error}"));
    view.update(cx, |this, cx| {
        this.terminal.core_test_backend = Some(backend);
        this.connect_named_core(HOST.into(), SESSION.into(), Some("t1".into()), cx)
            .unwrap_or_else(|error| panic!("connect Herdr 2: {error}"));
    });
    wait_for(view, cx, "Herdr 2 catalog and initial focus", |this| {
        this.terminal.live.connected
            && snapshot(&this.terminal.live)
                .is_some_and(|shown| shown.focused_pane_id.as_deref() == Some("t1"))
            && this
                .terminal
                .core
                .as_ref()
                .is_some_and(|core| !core.sessions(this.terminal.live.input_epoch).is_empty())
    });
    view.read_with(cx, |this, _| {
        assert_eq!(this.terminal_for(HOST, HerdrKind::Herdr1).is_some(), mixed);
    });
}

fn click_row(
    view: &Entity<Smoke>,
    cx: &mut VisualTestContext,
    description: &str,
    matches_row: impl Fn(&RailItem) -> bool,
) {
    cx.run_until_parked();
    let index = view.read_with(cx, |this, _| {
        rail_items(&this.rail_hosts())
            .into_iter()
            .chain(this.rail_agent_items())
            .position(|item| matches_row(&item))
            .unwrap_or_else(|| panic!("missing rail row for {description}"))
    });
    let selectors = [
        "rail-item-1",
        "rail-item-2",
        "rail-item-3",
        "rail-item-4",
        "rail-item-5",
        "rail-item-6",
        "rail-item-7",
        "rail-item-8",
    ];
    let selector = selectors[index];
    let row = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} was not drawn"));
    cx.simulate_click(row.center(), Modifiers::none());
    cx.run_until_parked();
}

fn opens_herdr2_space(cx: &mut TestAppContext, mixed: bool) {
    let (view, cx) = show(cx, "");
    start_herdr2(&view, cx, mixed);
    let (backend, _) =
        crate::core_mock::recorded(false).unwrap_or_else(|error| panic!("second core: {error}"));
    view.update(cx, |this, cx| {
        this.terminal
            .core
            .as_ref()
            .unwrap_or_else(|| panic!("first core"))
            .publish_discovery(None, HOST, br#"["setup",["c0ffee",[]]]"#)
            .unwrap_or_else(|error| panic!("session discovery: {error}"));
        if mixed {
            this.select_host_kind(HOST, HerdrKind::Herdr1);
        }
        this.terminal.core_test_backend = Some(backend);
        cx.notify();
    });
    click_row(&view, cx, "Herdr 2 space setup", |item| {
        item.kind == RailItemKind::Space && item.herdr == HerdrKind::Herdr2 && item.name == "setup"
    });
    wait_for(&view, cx, "opened the setup session on Herdr 2", |this| {
        this.terminal.herdr_kind() == HerdrKind::Herdr2
            && this.terminal.live.connected
            && this
                .terminal
                .core
                .as_ref()
                .is_some_and(|core| core.name() == "setup")
    });
}

fn opens_herdr2_agent(cx: &mut TestAppContext, mixed: bool) {
    let (view, cx) = show(cx, "");
    start_herdr2(&view, cx, mixed);
    view.update(cx, |this, cx| {
        let report: CoreStatus = CoreStatus {
            address: "t2".into(),
            status: Some(
                serde_json::from_value(serde_json::json!({
                    "app": "claude", "state": "working", "title": "session agent"
                }))
                .unwrap_or_else(|error| panic!("agent status: {error}")),
            ),
        };
        this.terminal
            .core
            .as_ref()
            .unwrap_or_else(|| panic!("core session"))
            .statuses_mailbox()
            .publish(vec![report]);
        if mixed {
            this.select_host_kind(HOST, HerdrKind::Herdr1);
        }
        cx.notify();
    });
    click_row(&view, cx, "Herdr 2 agent yebisu/t2", |item| {
        matches!(&item.target, RailTarget::Agent { id, .. }
            if id.kind == HerdrKind::Herdr2 && id.pane == "yebisu\0t2")
    });
    wait_for(&view, cx, "focused t2 on Herdr 2", |this| {
        this.terminal.herdr_kind() == HerdrKind::Herdr2
            && snapshot(&this.terminal.live)
                .is_some_and(|shown| shown.focused_pane_id.as_deref() == Some("t2"))
    });
}

#[gpui::test]
fn rail_navigation_opens_a_space_on_a_herdr2_only_host(cx: &mut TestAppContext) {
    opens_herdr2_space(cx, false);
}

#[gpui::test]
fn rail_navigation_opens_a_herdr2_space_from_herdr1_on_the_same_host(cx: &mut TestAppContext) {
    opens_herdr2_space(cx, true);
}

#[gpui::test]
fn rail_navigation_opens_an_agent_on_a_herdr2_only_host(cx: &mut TestAppContext) {
    opens_herdr2_agent(cx, false);
}

#[gpui::test]
fn rail_navigation_opens_a_herdr2_agent_from_herdr1_on_the_same_host(cx: &mut TestAppContext) {
    opens_herdr2_agent(cx, true);
}

#[gpui::test]
fn rail_navigation_still_opens_a_herdr1_space(cx: &mut TestAppContext) {
    let (view, cx) = show(cx, "");
    start_herdr1(&view, cx);
    click_row(&view, cx, "Herdr 1 space second", |item| {
        item.kind == RailItemKind::Space && item.herdr == HerdrKind::Herdr1 && item.name == "second"
    });
    wait_for(&view, cx, "focused Herdr 1's second space", |this| {
        snapshot(&this.terminal.live)
            .is_some_and(|shown| shown.focused_workspace_id.as_deref() == Some("w2"))
    });
}

#[gpui::test]
fn rail_navigation_still_opens_a_herdr1_agent(cx: &mut TestAppContext) {
    let (view, cx) = show(cx, "");
    start_herdr1(&view, cx);
    click_row(&view, cx, "Herdr 1 agent w2:p2", |item| {
        matches!(&item.target, RailTarget::Agent { id, .. }
            if id.kind == HerdrKind::Herdr1 && id.pane == "w2:p2")
    });
    wait_for(
        &view,
        cx,
        "focused the Herdr 1 agent in the second space",
        |this| {
            snapshot(&this.terminal.live)
                .is_some_and(|shown| shown.focused_pane_id.as_deref() == Some("w2:p2"))
        },
    );
}

#[gpui::test]
fn rail_navigation_unpinned_agent_label_reads_the_space(cx: &mut TestAppContext) {
    let (view, cx) = show(cx, "");
    start_herdr1(&view, cx);
    view.read_with(cx, |this, _| {
        let agent = this
            .rail_agent_items()
            .into_iter()
            .find(|item| matches!(&item.target, RailTarget::Agent { id, .. } if id.pane == "w1:p1"))
            .unwrap_or_else(|| panic!("unpinned agent in repo"));
        assert!(!agent.pinned);
        assert_eq!(
            agent.accessibility_label(),
            format!("{}, repo, Needs you", agent.name)
        );
    });
}
