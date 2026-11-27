//! Close Space from a space row's menu (#231): which spaces ask first, the
//! one request a close sends, a secondary click opening the same menu, and
//! a stale confirmation refused.
use super::{
    CLOSE_TARGET_CHANGED, OPEN_TO_CLOSE, Question, Running, herdr1_question, herdr2_question,
    named_close_gate, named_question,
};
use crate::{
    app::Smoke,
    core::{DocSpace, TabInfo},
    shell::{
        ChromeState, HerdrKind,
        host_menus::{self, MenuItemId, MenuScope, SpaceMenuItem},
        search::tests::{Receipts, terminal, wait},
        sidebar::long_press::PressedRow,
    },
};
use gpui::{
    Entity, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, VisualTestContext, point, px,
};
use std::time::Duration;

const CLOSE: MenuItemId = MenuItemId::Space(SpaceMenuItem::Close);

/// The endpoint requests of `method` the peer received since the last
/// call, as their params.
fn requests(
    view: &Entity<Smoke>,
    receipts: &Receipts,
    cx: &mut VisualTestContext,
    method: &str,
) -> Vec<serde_json::Value> {
    for _ in 0..5 {
        view.update(cx, |this, cx| this.poll_terminal(cx));
        cx.run_until_parked();
    }
    let mut seen = Vec::new();
    while let Ok(message) = receipts.recv_timeout(Duration::from_millis(50)) {
        if let sheprd_client::protocol::ClientMessage::ClientShellEndpointRequest {
            request, ..
        } = message
        {
            let value: serde_json::Value =
                serde_json::from_str(&request).unwrap_or_else(|e| panic!("request: {e}"));
            if value["method"] == method {
                seen.push(value["params"].clone());
            }
        }
    }
    seen
}

fn space_row(host: &str, id: &str) -> PressedRow {
    PressedRow::Space {
        host: host.into(),
        id: id.into(),
        name: format!("Space · {id}"),
    }
}

/// The selected host's name.
fn host(view: &Entity<Smoke>, cx: &mut VisualTestContext) -> String {
    view.read_with(cx, |this, _| this.terminal.reconnect.target.clone())
}

#[gpui::test]
fn herdr1_close_asks_then_sends_one_workspace_close(cx: &mut gpui::TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    let host = host(&view, cx);
    requests(&view, &receipts, cx, "workspace.close");
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            // Not the focused space (w1): Close names any space of the host.
            this.sidebar_long_press(space_row(&host, "w2"), point(px(40.), px(200.)));
            let menu = this.shell.sidebar.long_press.clone();
            assert!(
                menu.is_some_and(|m| m.entries.iter().any(|e| e.item == CLOSE && e.enabled)),
                "Close Space is listed and enabled"
            );
            assert!(this.pick_long_press(CLOSE, window, cx).is_ok());
            // The fixture's w2 has a pane and an agent: it asks.
            let asked = this.shell.sidebar.close_confirm.clone();
            assert_eq!(
                asked.map(|c| c.question),
                Some(Question {
                    tabs: 1,
                    running: Running::Agents(1)
                })
            );
        })
    });
    assert!(
        requests(&view, &receipts, cx, "workspace.close").is_empty(),
        "nothing is closed before the confirmation"
    );
    view.update(cx, |this, _| {
        assert!(this.confirm_space_close().is_ok());
        assert!(
            this.confirm_space_close().is_err(),
            "a confirmation runs once"
        );
    });
    assert_eq!(
        requests(&view, &receipts, cx, "workspace.close"),
        [serde_json::json!({"workspace_id": "w2"})],
        "exactly one workspace.close, naming the row's space"
    );
    // The peer took w2 out; w1 and its tab and panes stay.
    wait(&view, cx, "the snapshot without w2", |this| {
        crate::board::snapshot(&this.terminal.live)
            .is_some_and(|s| s.workspaces.iter().all(|w| w.workspace_id != "w2"))
    });
    view.update(cx, |this, _| {
        let shown = crate::board::snapshot(&this.terminal.live)
            .unwrap_or_else(|| panic!("fixture: snapshot"));
        let ids = |items: Vec<&str>| items.join(",");
        assert_eq!(
            ids(shown
                .workspaces
                .iter()
                .map(|w| w.workspace_id.as_str())
                .collect()),
            "w1"
        );
        assert_eq!(
            ids(shown.tabs.iter().map(|t| t.tab_id.as_str()).collect()),
            "w1:t1"
        );
        assert_eq!(
            ids(shown.panes.iter().map(|p| p.pane_id.as_str()).collect()),
            "w1:p1,w1:p3"
        );
    });
}

/// Herdr 1 fails safe (controller decision 2026-11-26): its snapshot names
/// no foreground program, so a space with any pane asks, agent or not; only
/// a space without a pane closes at once.
#[gpui::test]
fn herdr1_asks_unless_the_space_has_no_pane(cx: &mut gpui::TestAppContext) {
    let (view, _receipts, cx) = terminal(cx);
    view.update(cx, |this, _| {
        let mut shown = crate::board::snapshot(&this.terminal.live)
            .cloned()
            .unwrap_or_else(|| panic!("fixture: snapshot"));
        assert_eq!(
            herdr1_question(&shown, "w1").map(|q| q.running),
            Some(Running::Agents(1)),
            "an agent asks"
        );
        shown.agents.retain(|agent| agent.workspace_id != "w1");
        let unknown = herdr1_question(&shown, "w1");
        assert_eq!(
            unknown.map(|q| q.running),
            Some(Running::Agents(0)),
            "a pane whose program is unknown asks too"
        );
        assert_eq!(
            unknown.map(|q| q.detail()).as_deref(),
            Some("Herdr 1 can't tell whether a program is running.")
        );
        shown.panes.retain(|pane| pane.workspace_id != "w1");
        assert_eq!(herdr1_question(&shown, "w1"), None, "no pane: at once");
        assert_eq!(herdr1_question(&shown, "w9"), None);
    });
}

fn doc_space(tabs: usize, busy: &[&str]) -> DocSpace {
    DocSpace {
        id: "sp1".into(),
        name: "Build".into(),
        auto_name: false,
        folder: "/src".into(),
        tabs: (0..tabs)
            .map(|i| TabInfo {
                id: format!("tb{i}"),
                label: (i + 1).to_string(),
                panes: vec![format!("t{i}")],
                active: false,
                focused_pane: 0,
                vertical: false,
                auto_name: true,
            })
            .collect(),
        busy: busy.iter().map(|t| (*t).to_owned()).collect(),
    }
}

/// Herdr 2: shells alone close at once; a program or an agent asks.
#[test]
fn herdr2_asks_only_when_something_runs() {
    assert_eq!(herdr2_question(&doc_space(2, &[])), None);
    let asked = herdr2_question(&doc_space(2, &["t1"]));
    assert_eq!(
        asked,
        Some(Question {
            tabs: 2,
            running: Running::Programs(1)
        })
    );
    assert_eq!(
        asked.map(|q| q.title()).as_deref(),
        Some("Close 2 tabs of this space?")
    );
    assert_eq!(
        asked.map(|q| q.detail()).as_deref(),
        Some("A program is running in it and will end.")
    );
}

/// A confirmation shown on one connection is refused on the next one, and
/// sends nothing.
#[gpui::test]
fn a_stale_confirmation_is_refused(cx: &mut gpui::TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    let host = host(&view, cx);
    requests(&view, &receipts, cx, "workspace.close");
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.sidebar_long_press(space_row(&host, "w2"), point(px(40.), px(200.)));
            assert!(this.pick_long_press(CLOSE, window, cx).is_ok());
            assert!(this.shell.sidebar.close_confirm.is_some());
            this.terminal.link.generation += 1;
            let refused = this.confirm_space_close().err().map(|e| e.to_string());
            assert!(
                refused
                    .as_deref()
                    .is_some_and(|e| e.contains(CLOSE_TARGET_CHANGED)),
                "{refused:?}"
            );
        })
    });
    assert!(requests(&view, &receipts, cx, "workspace.close").is_empty());
}

/// Another host's space row lists Close disabled, and a pick is refused.
#[gpui::test]
fn another_hosts_row_cannot_close(cx: &mut gpui::TestAppContext) {
    let (view, receipts, cx) = terminal(cx);
    requests(&view, &receipts, cx, "workspace.close");
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.sidebar_long_press(space_row("elsewhere", "w2"), point(px(40.), px(200.)));
            let menu = this.shell.sidebar.long_press.clone();
            assert!(menu.is_some_and(|m| {
                m.entries
                    .iter()
                    .any(|e| e.item == CLOSE && !e.enabled && e.reason.is_some())
            }));
            assert!(this.pick_long_press(CLOSE, window, cx).is_err());
            assert!(this.shell.sidebar.close_confirm.is_none());
        })
    });
    assert!(requests(&view, &receipts, cx, "workspace.close").is_empty());
}

/// The middle of the painted `selector`.
fn middle(cx: &mut VisualTestContext, selector: &'static str) -> gpui::Point<gpui::Pixels> {
    let at = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} is not painted"));
    point(
        at.origin.x + at.size.width / 2.,
        at.origin.y + at.size.height / 2.,
    )
}

fn click(
    cx: &mut VisualTestContext,
    at: gpui::Point<gpui::Pixels>,
    button: MouseButton,
    modifiers: Modifiers,
) {
    cx.simulate_event(MouseDownEvent {
        button,
        position: at,
        modifiers,
        click_count: 1,
        first_mouse: false,
    });
    cx.simulate_event(MouseUpEvent {
        button,
        position: at,
        modifiers,
        click_count: 1,
    });
    cx.run_until_parked();
}

/// Closes the menu and draws again, so no painted menu covers the row.
fn close_menu(view: &Entity<Smoke>, cx: &mut VisualTestContext) {
    view.update(cx, |this, cx| {
        this.shell.sidebar.long_press = None;
        cx.notify();
    });
    cx.run_until_parked();
}

fn open_row(view: &Entity<Smoke>, cx: &mut VisualTestContext) -> Option<String> {
    view.read_with(cx, |this, _| {
        this.shell
            .sidebar
            .long_press
            .as_ref()
            .and_then(|menu| match &menu.row {
                PressedRow::Space { id, .. } => Some(id.clone()),
                _ => None,
            })
    })
}

/// A secondary click and a Control-click on a space row open that row's
/// menu (the long press's), without moving to the space.
#[gpui::test]
fn a_secondary_click_opens_the_rows_menu(cx: &mut gpui::TestAppContext) {
    let (view, _receipts, cx) = terminal(cx);
    let host = host(&view, cx);
    for _ in 0..3 {
        view.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
    }
    let selector: &'static str = Box::leak(format!("sidebar-space-{host}-w2").into_boxed_str());
    let at = middle(cx, selector);
    click(cx, at, MouseButton::Right, Modifiers::none());
    assert_eq!(open_row(&view, cx).as_deref(), Some("w2"));
    close_menu(&view, cx);
    // Control-click: the primary button with Control alone.
    let control = Modifiers {
        control: true,
        ..Modifiers::none()
    };
    click(cx, at, MouseButton::Left, control);
    assert_eq!(open_row(&view, cx).as_deref(), Some("w2"));
    view.read_with(cx, |this, _| {
        assert!(
            this.terminal.navigating.is_none(),
            "a Control-click does not move to the space"
        );
    });
    close_menu(&view, cx);
    // A plain click still moves there, and opens no menu.
    click(cx, at, MouseButton::Left, Modifiers::none());
    assert_eq!(open_row(&view, cx), None);
}

/// #249: of the Herdr 2 rows the document does not hold, only the
/// connected `space-<ms>` session's own row closes; `default` never does,
/// and another session's row says to open it first.
#[test]
fn only_the_connected_named_row_closes() {
    assert_eq!(named_close_gate("space-17", Some("space-17")), Ok(()));
    assert_eq!(
        named_close_gate("space-17", Some("default")),
        Err(OPEN_TO_CLOSE)
    );
    assert_eq!(named_close_gate("space-17", None), Err(OPEN_TO_CLOSE));
    assert_eq!(
        named_close_gate("default", Some("default")),
        Err(crate::core::NAMED_CLOSE_PROTECTED)
    );
    assert_eq!(
        named_close_gate("default\0sp1", Some("default")),
        Err(CLOSE_TARGET_CHANGED)
    );
}

/// The connected `space-<ms>` row asks by #231's rule: shells alone close
/// at once, a program or an agent asks.
#[test]
fn a_named_space_asks_only_when_something_runs() {
    assert_eq!(named_question(1, &[]), None);
    assert_eq!(
        named_question(2, &["t1".into(), "t2".into()]),
        Some(Question {
            tabs: 2,
            running: Running::Programs(2)
        })
    );
}

/// The long-press menu of Herdr 2 rows the document does not hold: Rename
/// keeps its reason, Close has its own (#249: it said Rename's), and the
/// connected `space-<ms>` row's Close is enabled.
#[test]
fn close_has_its_own_reason_on_rows_outside_the_document() {
    let state = ChromeState {
        kind: HerdrKind::Herdr2,
        connected: true,
        host_selected: true,
        ..ChromeState::default()
    };
    let current = MenuScope {
        host: "pro-dev".into(),
        space: Some("herdr-2".into()),
    };
    let row = MenuScope {
        host: "pro-dev".into(),
        space: Some("space-17".into()),
    };
    let entries = |connected: Option<&str>| {
        let mut entries = host_menus::space_long_press_for(&state, &row, &current);
        let close = named_close_gate("space-17", connected).err();
        host_menus::refuse_document_items(&mut entries, &state, close);
        entries
    };
    let find = |entries: &[host_menus::MenuEntry], item| {
        entries
            .iter()
            .find(|e| e.item == MenuItemId::Space(item))
            .cloned()
            .unwrap_or_else(|| panic!("{item:?} not listed"))
    };
    let elsewhere = entries(Some("default"));
    let rename = find(&elsewhere, SpaceMenuItem::Rename);
    let close = find(&elsewhere, SpaceMenuItem::Close);
    assert!(!rename.enabled && !close.enabled);
    assert_eq!(
        rename.reason,
        Some(crate::shell::rename::herdr2::NOT_IN_DOCUMENT)
    );
    assert_eq!(close.reason, Some(OPEN_TO_CLOSE));
    let own = entries(Some("space-17"));
    assert!(!find(&own, SpaceMenuItem::Rename).enabled);
    assert!(find(&own, SpaceMenuItem::Close).enabled);
}
