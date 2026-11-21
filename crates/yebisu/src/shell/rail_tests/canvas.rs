use crate::{
    app::Smoke,
    shell::{
        rail::{RailItemKind, RailTarget, rail_items},
        sidebar_status::SidebarStatus,
        style::color,
    },
};
use gpui::{
    Bounds, Entity, LongPressEvent, Modifiers, MouseButton, Pixels, TestAppContext,
    VisualTestContext, px, size,
};
use sheprd_client::protocol::{AgentStatus, ClientMessage, ClientShellSnapshot};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

fn snapshot() -> ClientShellSnapshot {
    serde_json::from_str(include_str!("../../../fixtures/endpoint-snapshot-v1.json"))
        .unwrap_or_else(|error| panic!("endpoint fixture: {error}"))
}

fn show<'a>(
    cx: &'a mut TestAppContext,
    destination: &str,
) -> (Entity<Smoke>, &'a mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1376.), px(1032.)));
    view.update(cx, |this, cx| {
        this.select_host(destination);
        this.actions.board_hidden = true;
        this.shell.fit.window_controls = Some(gpui::Edges::default());
        cx.notify();
    });
    cx.run_until_parked();
    (view, cx)
}

fn bounds(cx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} was not drawn"))
}

fn press_rail_row(cx: &mut VisualTestContext, selector: &'static str) {
    let position = bounds(cx, selector).center();
    cx.simulate_event(LongPressEvent {
        start_position: position,
        position,
        ..LongPressEvent::default()
    });
    cx.run_until_parked();
}

fn click_rail_row(cx: &mut VisualTestContext, selector: &'static str) {
    let position = bounds(cx, selector).center();
    cx.simulate_click(position, Modifiers::none());
    cx.run_until_parked();
}

#[gpui::test]
fn rail_marks_only_the_current_space(cx: &mut TestAppContext) {
    let (view, cx) = show(cx, "pro-dev");
    view.update(cx, |this, _| {
        this.terminal.live.connected = true;
        this.terminal.live.snapshot = Some(Arc::new(snapshot()));
        let items = rail_items(&this.rail_hosts());
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].kind, RailItemKind::Host);
        assert!(!items[0].current, "host should have no current ring");
        assert!(!items[0].accessibility_label().ends_with(", current"));
        assert!(items[1].current, "focused space should have a current ring");
    });
}

#[gpui::test]
fn rail_space_dot_is_the_most_urgent_agent(cx: &mut TestAppContext) {
    let (view, cx) = show(cx, "pro-dev");
    view.update(cx, |this, _| {
        let mut stamped = snapshot();
        let mut working = stamped.agents[0].clone();
        working.agent_status = AgentStatus::Working;
        stamped.agents.insert(0, working);
        this.terminal.live.connected = true;
        this.terminal.live.snapshot = Some(Arc::new(stamped.clone()));
        let rows = rail_items(&this.rail_hosts());
        assert_eq!(rows[1].status, Some(SidebarStatus::NeedsYou));
        assert_eq!(
            rows[1].status.map(SidebarStatus::dot_color),
            Some(color::RED)
        );
        stamped.agents.clear();
        this.terminal.live.snapshot = Some(Arc::new(stamped));
        let rows = rail_items(&this.rail_hosts());
        assert_eq!(
            rows[1].status.map(SidebarStatus::dot_color),
            Some(color::SLATE)
        );
    });
}

#[gpui::test]
fn rail_draws_a_rule_between_hosts(cx: &mut TestAppContext) {
    let (view, cx) = show(cx, "alpha");
    view.update(cx, |this, cx| {
        this.select_host("zeta");
        this.setup.hosts = vec!["zeta".into(), "alpha".into(), "saved-offline".into()];
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |this, _| this
            .rail_hosts()
            .into_iter()
            .map(|host| host.name)
            .collect::<Vec<_>>()),
        ["zeta", "alpha", "saved-offline"],
    );
    let first = bounds(cx, "rail-item-1");
    let second = bounds(cx, "rail-item-2");
    let third = bounds(cx, "rail-item-3");
    let rule = bounds(cx, "rail-host-rule-1");
    assert_eq!(rule.size, size(px(40.), px(1.)));
    assert!(first.bottom() < rule.origin.y && rule.bottom() < second.origin.y);
    assert!(second.bottom() < bounds(cx, "rail-host-rule-2").origin.y);
    assert!(bounds(cx, "rail-host-rule-2").bottom() < third.origin.y);
    assert!(cx.debug_bounds("rail-host-rule-0").is_none());
}

#[gpui::test]
fn rail_labels_use_the_display_name(cx: &mut TestAppContext) {
    let full = "shin1ohno@pro-dev.example.test:2222";
    let (view, cx) = show(cx, full);
    let long = "session-search-with-a-name-too-long-for-one-line";
    view.update(cx, |this, cx| {
        let mut stamped = snapshot();
        stamped.workspaces[0].label = long.to_owned();
        this.terminal.live.connected = true;
        this.terminal.live.snapshot = Some(Arc::new(stamped));
        let items = rail_items(&this.rail_hosts());
        assert_eq!(items[0].name, "pro-dev");
        assert_eq!(items[0].target, RailTarget::Host(full.into()));
        assert!(items[1].accessibility_label().starts_with(long));
        cx.notify();
    });
    cx.run_until_parked();
    assert!(bounds(cx, "rail-label-2").size.height < px(22.));
}

#[gpui::test]
fn rail_footer_buttons_follow_the_canvas(cx: &mut TestAppContext) {
    let (_, cx) = show(cx, "pro-dev");
    let add = bounds(cx, "rail-new-space-icon");
    let divider = bounds(cx, "rail-header-rule");
    let first_host = bounds(cx, "rail-item-1");
    assert_eq!(add.size, size(px(20.), px(20.)));
    assert_eq!(divider.size, size(px(40.), px(1.)));
    assert!(add.bottom() < divider.origin.y && divider.bottom() < first_host.origin.y);
    for name in ["rail-hosts", "rail-settings", "rail-expand"] {
        assert_eq!(bounds(cx, name).size, size(px(64.), px(44.)));
    }
    for name in ["rail-hosts-icon", "rail-settings-icon"] {
        assert_eq!(bounds(cx, name).size, size(px(20.), px(20.)));
    }
    assert!(cx.debug_bounds("rail-expand-icon").is_some());
    assert!(bounds(cx, "rail-hosts").bottom() < bounds(cx, "rail-settings").origin.y);
    assert!(bounds(cx, "rail-settings").bottom() < bounds(cx, "rail-expand").origin.y);
}

#[gpui::test]
fn rail_long_press_opens_the_row_menu(cx: &mut TestAppContext) {
    let (view, cx) = show(cx, "alpha");
    view.update(cx, |this, cx| {
        this.select_host("zeta");
        cx.notify();
    });
    cx.run_until_parked();
    let position = bounds(cx, "rail-item-1").center();
    cx.simulate_mouse_down(position, MouseButton::Left, Modifiers::none());
    press_rail_row(cx, "rail-item-1");
    cx.simulate_event(LongPressEvent {
        start_position: position,
        position,
        phase: gpui::TouchPhase::Ended,
    });
    cx.simulate_mouse_up(position, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(bounds(cx, "rail-row-menu").size.width, px(240.));
    assert!(cx.debug_bounds("rail-menu-new-tab").is_some());
    assert!(cx.debug_bounds("rail-menu-detach").is_some());
    assert!(cx.debug_bounds("rail-menu-open-chat").is_none());
    assert_eq!(
        view.read_with(cx, |this, _| this.terminal.reconnect.target.clone()),
        "zeta"
    );
    let open = bounds(cx, "rail-menu-open").center();
    cx.simulate_click(open, Modifiers::default());
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |this, _| this.terminal.reconnect.target.clone()),
        "alpha"
    );
    assert!(cx.debug_bounds("rail-row-menu").is_none());
}

#[gpui::test]
fn rail_agent_menu_toggles_the_sidebar_pin_and_uses_its_order(cx: &mut TestAppContext) {
    let (view, cx) = show(cx, "alpha");
    let mut shown = snapshot();
    for (pane_id, status) in [
        ("w1:done", AgentStatus::Done),
        ("w1:work", AgentStatus::Working),
    ] {
        let mut pane = shown.panes[0].clone();
        pane.pane_id = pane_id.into();
        pane.label = Some(pane_id.into());
        pane.focused = false;
        shown.panes.push(pane);
        let mut agent = shown.agents[0].clone();
        agent.pane_id = pane_id.into();
        agent.agent_status = status;
        shown.agents.push(agent);
    }
    view.update(cx, |this, cx| {
        this.terminal.live.connected = true;
        this.terminal.live.snapshot = Some(Arc::new(shown));
        cx.notify();
    });
    cx.run_until_parked();
    let agents = |this: &Smoke, _: &gpui::App| {
        this.rail_agent_items()
            .into_iter()
            .filter_map(|item| match &item.target {
                RailTarget::Agent { id, boot, .. } => {
                    let pinned = this.shell.sidebar.is_agent_pinned(id, boot);
                    assert_eq!(item.pinned, pinned);
                    if pinned {
                        assert_eq!(
                            item.accessibility_label(),
                            format!("Pinned: {}, repo, Done", item.name)
                        );
                    }
                    Some((id.pane.clone(), pinned))
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let default_order = [
        ("w1:p1".into(), false),
        ("w1:work".into(), false),
        ("w1:done".into(), false),
    ];
    assert_eq!(view.read_with(cx, agents), default_order);
    // A pinned Done row rises above Working and remains below Needs you.
    press_rail_row(cx, "rail-item-5");
    assert!(cx.debug_bounds("rail-menu-detach").is_none());
    click_rail_row(cx, "rail-menu-pin");
    assert_eq!(
        view.read_with(cx, agents),
        [
            ("w1:p1".into(), false),
            ("w1:done".into(), true),
            ("w1:work".into(), false),
        ]
    );
    press_rail_row(cx, "rail-item-4");
    click_rail_row(cx, "rail-menu-pin");
    assert_eq!(view.read_with(cx, agents), default_order);
}

#[gpui::test]
fn rail_draws_a_disconnected_kept_core_host_as_herdr2(cx: &mut TestAppContext) {
    let (view, cx) = show(cx, "alpha");
    view.update(cx, |this, cx| {
        let mut kept = crate::terminal_view::TerminalView::default();
        kept.reconnect.target = "beta".into();
        kept.core_mode = true;
        kept.live.snapshot = Some(Arc::new(snapshot()));
        this.board.hidden_core.insert("beta".into(), kept);
        cx.notify();
    });
    cx.run_until_parked();
    bounds(cx, "rail-item-2");
    view.read_with(cx, |this, _| {
        let items = rail_items(&this.rail_hosts());
        assert_eq!(
            items[1].accessibility_label(),
            "beta, not connected, Herdr 2"
        );
        assert_eq!(items.len(), 2, "offline spaces must stay hidden");
    });
}

#[gpui::test]
fn rail_menu_refuses_an_agent_after_the_connection_changes(cx: &mut TestAppContext) {
    let (view, cx) = show(cx, "alpha");
    view.update(cx, |this, cx| {
        this.terminal.live.connected = true;
        this.terminal.live.snapshot = Some(Arc::new(snapshot()));
        cx.notify();
    });
    cx.run_until_parked();
    press_rail_row(cx, "rail-item-3");
    view.update(cx, |this, _| {
        this.terminal.link.reset();
    });
    click_rail_row(cx, "rail-menu-pin");
    view.read_with(cx, |this, _| {
        assert!(this.terminal.live.status.contains("menu target changed"));
        assert!(!this.rail_agent_items()[0].pinned);
    });
}

#[gpui::test]
fn rail_new_tab_waits_for_the_drawn_workspace(cx: &mut TestAppContext) {
    let (view, cx) = show(cx, "alpha");
    let (client, receipts) =
        crate::mock_peer::connect_recorded(1).unwrap_or_else(|error| panic!("peer: {error}"));
    view.update(cx, |this, cx| {
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
        this.terminal.open = true;
        cx.notify();
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        this.terminal.live.surface.is_some() && !this.row_tabs().is_empty()
    }) {
        assert!(
            Instant::now() < deadline,
            "private peer did not become ready"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    cx.run_until_parked();
    press_rail_row(cx, "rail-item-3");
    click_rail_row(cx, "rail-menu-new-tab");
    let mut commands = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    while commands.len() < 2 {
        view.update(cx, |this, cx| this.poll_terminal(cx));
        cx.executor().advance_clock(Duration::from_millis(50));
        commands.extend(receipts.try_iter().filter_map(|message| {
            let ClientMessage::ClientShellEndpointRequest { request, .. } = message else {
                return None;
            };
            let command: serde_json::Value =
                serde_json::from_str(&request).unwrap_or_else(|error| panic!("command: {error}"));
            let method = command["method"].as_str()?;
            matches!(method, "workspace.focus" | "tab.create").then(|| {
                (
                    method.to_owned(),
                    command["params"]["workspace_id"]
                        .as_str()
                        .map(str::to_owned),
                )
            })
        }));
        assert!(
            Instant::now() < deadline,
            "expected Focus then New tab: {commands:?}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        commands,
        [
            ("workspace.focus".into(), Some("w2".into())),
            ("tab.create".into(), Some("w2".into())),
        ]
    );
}

#[gpui::test]
fn rail_detach_disconnects_the_selected_herdr2_host(cx: &mut TestAppContext) {
    let (view, cx) = show(cx, "alpha");
    let (backend, _) =
        crate::core_mock::recorded(false).unwrap_or_else(|error| panic!("peer: {error}"));
    view.update(cx, |this, cx| {
        this.connect_core_using(cx, move |_| Ok(backend))
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        this.terminal.live.connected
    }) {
        assert!(
            Instant::now() < deadline,
            "private core did not become ready"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    cx.run_until_parked();
    press_rail_row(cx, "rail-item-1");
    click_rail_row(cx, "rail-menu-detach");
    view.read_with(cx, |this, _| {
        assert!(this.terminal.core.is_none());
        assert!(!this.terminal.live.connected);
        assert!(this.terminal.live.status.starts_with("Detached"));
    });
}

#[gpui::test]
fn rail_menu_closes_on_an_outside_press_that_stays_out_of_the_terminal(cx: &mut TestAppContext) {
    let (view, cx) = show(cx, "rail-overlay");
    let (client, _receipts) =
        crate::mock_peer::connect_quiet().unwrap_or_else(|error| panic!("peer: {error}"));
    view.update(cx, |this, cx| {
        let generation = this.terminal.link.reset();
        this.terminal.link.install(generation, Ok(client));
        this.terminal.open = true;
        this.handoff.select_mode = true;
        cx.notify();
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        this.terminal.live.surface.is_some() && !this.row_tabs().is_empty()
    }) {
        assert!(
            Instant::now() < deadline,
            "private peer did not become ready"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    cx.run_until_parked();
    let position = bounds(cx, "rail-item-1").center();
    cx.simulate_event(LongPressEvent {
        start_position: position,
        position,
        ..LongPressEvent::default()
    });
    cx.run_until_parked();
    let menu = bounds(cx, "rail-row-menu");
    let at = view.read_with(cx, |this, _| this.terminal.geometry.grid.center());
    assert!(
        !menu.contains(&at),
        "fixture: press must be outside the menu"
    );
    let has_selection = |this: &Smoke, _: &gpui::App| this.handoff.selection.is_some();
    assert!(!view.read_with(cx, has_selection));

    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("rail-row-menu").is_none());
    assert!(
        !view.read_with(cx, has_selection),
        "the closing press reached the terminal and started a selection"
    );

    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
    assert!(
        view.read_with(cx, has_selection),
        "fixture: terminal can select"
    );
}
