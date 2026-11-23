//! The rail's agent rows draw the agent-kind icon before the label; host and
//! space rows draw none.
use super::*;
use crate::{
    app::Smoke,
    shell::sidebar::agent_icon::{AgentIcon, RAIL},
};
use gpui::{Bounds, Entity, Pixels, TestAppContext, VisualTestContext, px, size};
use sheprd_client::protocol::ClientShellSnapshot;
use std::sync::Arc;

fn fixture() -> ClientShellSnapshot {
    serde_json::from_str(include_str!("../../../fixtures/endpoint-snapshot-v1.json"))
        .unwrap_or_else(|error| panic!("endpoint fixture: {error}"))
}

fn collapsed(cx: &mut TestAppContext) -> (Entity<Smoke>, &mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1376.), px(1032.)));
    view.update(cx, |this, cx| {
        this.select_host("pro-dev");
        this.actions.board_hidden = true;
        this.shell.fit.window_controls = Some(gpui::Edges::default());
        this.shell.frame.sidebar_collapsed = true;
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

#[gpui::test]
fn the_rail_agent_row_draws_the_icon_before_its_label(cx: &mut TestAppContext) {
    let (view, cx) = collapsed(cx);
    let index = view.read_with(cx, |this, _| {
        let rows = rail_items(&this.rail_hosts(), &this.setup);
        assert!(rows.iter().all(|row| row.icon.is_none()));
        let agents = this.rail_agent_items();
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].icon, Some(AgentIcon::Claude));
        rows.len() + 1
    });
    let row = bounds(cx, &format!("rail-item-{index}"));
    let icon = bounds(cx, &format!("rail-item-{index}-agent-icon-claude"));
    let label = bounds(cx, &format!("rail-label-{index}"));
    assert_eq!(icon.size, size(px(RAIL.size), px(RAIL.size)));
    assert!(row.origin.x <= icon.origin.x && icon.right() <= row.right());
    assert!(row.origin.y <= icon.origin.y && icon.bottom() <= row.bottom());
    assert_eq!(label.origin.x - icon.right(), px(RAIL.gap));
    for other in 1..index {
        for icon in AgentIcon::ALL {
            let selector = format!("rail-item-{other}-agent-icon-{}", icon.label());
            assert!(drawn(cx, &selector).is_none(), "{selector}");
        }
    }
}
