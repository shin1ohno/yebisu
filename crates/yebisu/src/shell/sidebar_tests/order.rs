use super::*;
use SidebarStatus::*;

#[test]
fn agents_order_needs_you_error_working_done_idle_unknown() {
    let mut rows = vec![
        row("h", "p1", "unknown", Unknown, 9),
        row("h", "p2", "idle", Idle, 8),
        row("h", "p3", "done", Done, 7),
        row("h", "p4", "working", Working, 6),
        row("h", "p5", "error", Error, 5),
        row("h", "p6", "needs", NeedsYou, 0),
    ];
    order_agents(&mut rows);
    assert_eq!(
        names(&rows),
        ["needs", "error", "working", "done", "idle", "unknown"]
    );
}

#[test]
fn same_status_newest_received_first() {
    let mut rows = vec![
        row("a", "p1", "older", Working, 3),
        row("a", "p2", "newest", Working, 7),
        row("a", "p3", "middle", Working, 5),
    ];
    order_agents(&mut rows);
    assert_eq!(names(&rows), ["newest", "middle", "older"]);
}

#[test]
fn cross_daemon_seq_is_ignored() {
    let mut state = SidebarState::default();
    let seen = |pane: &str, status, seq| Observed {
        pane: pane.into(),
        status,
        seq,
    };
    // First batches: host a reports a high daemon seq, host b a low one.
    state.observe("a", HerdrKind::Herdr1, "boot-a", &[seen("p", Idle, 100)]);
    state.observe("b", HerdrKind::Herdr1, "boot-b", &[seen("p", Idle, 1)]);
    // a changes first, b later; b's seq stays far below a's.
    state.observe("a", HerdrKind::Herdr1, "boot-a", &[seen("p", Working, 101)]);
    state.observe("b", HerdrKind::Herdr1, "boot-b", &[seen("p", Working, 2)]);
    let id = |host: &str| AgentIdentity {
        host: host.into(),
        kind: HerdrKind::Herdr1,
        pane: "p".into(),
    };
    let mut rows = vec![
        row("a", "p", "on-a", Working, state.received(&id("a"))),
        row("b", "p", "on-b", Working, state.received(&id("b"))),
    ];
    order_agents(&mut rows);
    // This iPad received b's change last, so b is on top despite seq 2 < 101.
    assert_eq!(names(&rows), ["on-b", "on-a"]);
    // The same daemon's seq still detects a change of the same agent.
    let before = state.received(&id("a"));
    state.observe("a", HerdrKind::Herdr1, "boot-a", &[seen("p", Working, 102)]);
    assert!(state.received(&id("a")) > before);
}

#[test]
fn initial_batch_orders_by_host_then_name() {
    let mut state = SidebarState::default();
    let batch = |names: &[&str]| {
        names
            .iter()
            .map(|n| Observed {
                pane: (*n).into(),
                status: Working,
                seq: 1,
            })
            .collect::<Vec<_>>()
    };
    state.observe("zeta", HerdrKind::Herdr1, "z", &batch(&["b", "a"]));
    state.observe("alpha", HerdrKind::Herdr1, "x", &batch(&["d", "c"]));
    let mut rows = Vec::new();
    for (host, panes) in [("zeta", ["b", "a"]), ("alpha", ["d", "c"])] {
        for pane in panes {
            let identity = AgentIdentity {
                host: host.into(),
                kind: HerdrKind::Herdr1,
                pane: pane.into(),
            };
            rows.push(row(host, pane, pane, Working, state.received(&identity)));
        }
    }
    assert!(
        rows.iter().all(|r| r.received == 0),
        "a first batch is stamped 0"
    );
    order_agents(&mut rows);
    assert_eq!(names(&rows), ["c", "d", "a", "b"]);
    // After a reconnect (new boot) the host's agents are a first batch again.
    state.observe(
        "zeta",
        HerdrKind::Herdr1,
        "z",
        &[Observed {
            pane: "a".into(),
            status: Done,
            seq: 2,
        }],
    );
    let a = AgentIdentity {
        host: "zeta".into(),
        kind: HerdrKind::Herdr1,
        pane: "a".into(),
    };
    assert!(state.received(&a) > 0);
    state.observe(
        "zeta",
        HerdrKind::Herdr1,
        "z2",
        &[Observed {
            pane: "a".into(),
            status: Done,
            seq: 1,
        }],
    );
    assert_eq!(state.received(&a), 0);
}
