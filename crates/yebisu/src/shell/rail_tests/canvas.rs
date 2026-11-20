use crate::{
    app::Smoke,
    shell::{
        rail::{RailItemKind, RailTarget, rail_items},
        sidebar_status::SidebarStatus,
        style::color,
    },
};
use gpui::{
    Bounds, Entity, LongPressEvent, Modifiers, Pixels, TestAppContext, VisualTestContext, px, size,
};
use sheprd_client::protocol::ClientShellSnapshot;
use std::sync::Arc;

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
        working.agent_status = sheprd_client::protocol::AgentStatus::Working;
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
    cx.simulate_event(LongPressEvent {
        start_position: position,
        position,
        ..LongPressEvent::default()
    });
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
