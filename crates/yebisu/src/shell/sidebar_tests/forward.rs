//! Issue 140, board 43: a host's forwards under its row in four states, a
//! listening row's tap opening `http://127.0.0.1:{local}/`, the rows kept
//! when the host is collapsed, and none in the rail.
use crate::{
    app::Smoke,
    forwards::{
        Forwards, OPEN_HINT, Reason, State,
        worker::{self, Connector, Link},
    },
    live::Live,
    mock_peer,
    setup::Setup,
    shell::{
        HerdrKind,
        sidebar::{
            forward_row::sidebar_line,
            model::{HostEntry, HostLink},
        },
        sidebar_list::{ListRow, merge, with_forwards},
    },
};
use gpui::{Modifiers, TestAppContext, VisualTestContext, px, size};
use herdr_ssh::forward;
use sheprd_client::ClientEvent;
use std::{
    net::TcpStream,
    num::NonZeroU16,
    sync::Arc,
    time::{Duration, Instant},
};

const HOST: &str = "me@pro-dev";
/// The forwarded port in the rendered tests; `any_port` listens wherever
/// the system chooses, so it need not be free here.
const REMOTE: u16 = 45678;
const ROW: &str = "sidebar-forward-me@pro-dev-45678";

struct Open;

impl Connector for Open {
    fn trusted(&self, _: &str) -> crate::Result<()> {
        Ok(())
    }
    fn connect(&self, _: &str) -> Result<Box<dyn Link>, Reason> {
        Ok(Box::new(Open))
    }
}

impl Link for Open {
    fn open(&self, _: u16) -> Result<(), forward::Error> {
        Ok(())
    }
    fn relay(&self, _: u16, _: TcpStream) -> Result<(), forward::Error> {
        Ok(())
    }
    fn wait_end(&self, _: Duration) -> Option<forward::Error> {
        None
    }
    fn stop(&self) {}
}

fn any_port(_: u16) -> std::io::Result<std::net::TcpListener> {
    worker::listen(0)
}

fn port(number: u16) -> NonZeroU16 {
    NonZeroU16::new(number).unwrap_or_else(|| panic!("port 0"))
}

#[test]
fn four_states_text() {
    let listening = State::Listening {
        local: 3000,
        nobody_yet: false,
    };
    assert_eq!(listening.sidebar_text(3000), "⇄ 3000 → 127.0.0.1:3000");
    assert_eq!(State::Connecting.sidebar_text(5173), "⇄ 5173 · connecting…");
    assert_eq!(State::Paused.sidebar_text(4000), "⇄ 4000 · paused");
    assert_eq!(
        State::Ended(Reason::ConnectionLost).sidebar_text(8080),
        "⇄ 8080 · ended"
    );
    let moved = State::Listening {
        local: 10080,
        nobody_yet: true,
    };
    assert_eq!(moved.sidebar_text(80), "⇄ 80 → 127.0.0.1:10080");
    assert_eq!(sidebar_line(80, moved), "80 → 127.0.0.1:10080");
}

#[test]
fn with_forwards_inserts_after_host() {
    let mut forwards = Forwards::with(Arc::new(Open), any_port);
    for remote in [3000, 5173] {
        if let Err(error) = forwards.start(HOST, port(remote)) {
            panic!("{error}");
        }
    }
    if let Err(error) = forwards.start("me@mini", port(8080)) {
        panic!("{error}");
    }
    let hosts = [
        HostEntry::new(HOST, HerdrKind::Herdr1, HostLink::Connected),
        HostEntry::new("me@mini", HerdrKind::Herdr1, HostLink::Connected),
        HostEntry::new("me@none", HerdrKind::Herdr1, HostLink::Connected),
    ];
    let rows = with_forwards(merge(&hosts, &Setup::default()), &forwards);
    let shape: Vec<String> = rows
        .iter()
        .map(|row| match row {
            ListRow::Host(host) => format!("host {}", host.name.destination()),
            ListRow::Space(space) => format!("space {}", space.host),
            ListRow::Forward(forward) => format!("forward {} {}", forward.host, forward.remote),
        })
        .collect();
    assert_eq!(
        shape,
        [
            "host me@pro-dev",
            "forward me@pro-dev 3000",
            "forward me@pro-dev 5173",
            "host me@mini",
            "forward me@mini 8080",
            "host me@none",
        ]
    );
}

/// A frame with a connected terminal on `HOST` and one listening forward.
fn frame(cx: &mut TestAppContext) -> (gpui::Entity<Smoke>, &mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.simulate_resize(size(px(1200.), px(900.)));
    view.update(cx, |this, _| {
        let mut live = Live::default();
        live.connected = true;
        live.apply(ClientEvent::Snapshot(Arc::new(
            serde_json::from_str(mock_peer::SNAPSHOT).unwrap_or_else(|e| panic!("fixture {e}")),
        )));
        this.terminal.live = live;
        this.terminal.open = true;
        this.terminal.reconnect.target = HOST.into();
        this.setup.hosts = vec![HOST.into()];
        this.forwards = Forwards::with(Arc::new(Open), any_port);
        if let Err(error) = this.forwards.start(HOST, port(REMOTE)) {
            panic!("{error}");
        }
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    while !view.update(cx, |this, cx| {
        this.poll_forwards(cx);
        matches!(
            this.forwards.get(HOST, REMOTE).map(|forward| forward.state),
            Some(State::Listening { .. })
        )
    }) {
        assert!(Instant::now() < deadline, "the forward did not listen");
        std::thread::sleep(Duration::from_millis(5));
    }
    cx.run_until_parked();
    (view, cx)
}

#[gpui::test]
fn tap_listening_opens_url(cx: &mut TestAppContext) {
    let (view, cx) = frame(cx);
    let local = view.update(cx, |this, _| {
        match this.forwards.get(HOST, REMOTE).map(|f| f.state) {
            Some(State::Listening { local, .. }) => local,
            other => panic!("{other:?}"),
        }
    });
    let row = cx
        .debug_bounds(ROW)
        .unwrap_or_else(|| panic!("no forward row in the sidebar"));
    cx.simulate_click(row.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        cx.opened_url().as_deref(),
        Some(format!("http://127.0.0.1:{local}/").as_str())
    );
    view.update(cx, |this, _| {
        assert_eq!(this.notifications.visible_status(), OPEN_HINT);
    });
}

#[gpui::test]
fn rows_survive_collapse(cx: &mut TestAppContext) {
    let (view, cx) = frame(cx);
    assert!(cx.debug_bounds(ROW).is_some());
    view.update(cx, |this, cx| {
        this.shell.sidebar.toggle_host(HOST);
        assert!(this.shell.sidebar.host_collapsed(HOST));
        cx.notify();
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds(ROW).is_some(),
        "a collapsed host keeps its forward rows"
    );
}

#[gpui::test]
fn rail_has_no_forward_rows(cx: &mut TestAppContext) {
    let (view, cx) = frame(cx);
    assert!(cx.debug_bounds(ROW).is_some(), "positive control");
    view.update(cx, |this, cx| {
        this.actions.board_hidden = true;
        cx.notify();
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds(ROW).is_none(),
        "the rail draws no forward rows"
    );
}
