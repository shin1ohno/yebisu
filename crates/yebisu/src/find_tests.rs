#![allow(clippy::unwrap_used)]
use super::*;
use serde_json::json;
fn scope() -> Scope {
    Scope {
        host: "host-a".into(),
        generation: 7,
        epoch: 2,
        boot: "boot-a".into(),
        pane: "p1".into(),
    }
}
fn answer(pane: &str) -> serde_json::Value {
    json!({"result":{"type":"pane_copy_search","pane_id":pane,"content_revision":4,
        "matches":[{"start":{"row":2,"col":1},"end":{"row":2,"col":3}}],
        "total":1,"current":0,"current_global":0}})
}
fn flight(query: &str) -> Search {
    let mut search = Search::new(scope(), true);
    search.set_query(query);
    let params = search.next(4, TextPoint::default()).unwrap();
    search.sent("collision".into(), &params, Instant::now());
    search
}
#[test]
fn utf8_contract_and_latest_query_are_bounded() {
    let mut search = flight("ASCII");
    for _ in 0..500 {
        search.set_query("日本語");
    }
    assert!(search.next(4, TextPoint::default()).is_none());
    assert!(!search.answer(&scope(), "collision", decode(&answer("p1"))));
    let next = search.next(4, TextPoint::default()).unwrap();
    assert_eq!(next.query, "日本語");
    search.set_query(&"あ".repeat(1366));
    assert_eq!(search.state, State::Oversized);
    assert!(search.query.is_empty());
    assert!(search.next(4, TextPoint::default()).is_none());
}
#[test]
fn wrong_scope_and_id_cannot_consume_a_flight() {
    let mut search = flight("find");
    let expected = scope();
    for wrong in [
        Scope {
            host: "b".into(),
            ..expected.clone()
        },
        Scope {
            generation: 8,
            ..expected.clone()
        },
        Scope {
            epoch: 3,
            ..expected.clone()
        },
        Scope {
            boot: "boot-b".into(),
            ..expected.clone()
        },
        Scope {
            pane: "p2".into(),
            ..expected.clone()
        },
    ] {
        assert!(!search.answer(&wrong, "collision", decode(&answer("p1"))));
    }
    assert!(!search.answer(&expected, "other", decode(&answer("p1"))));
    assert!(search.answer(&expected, "collision", decode(&answer("p1"))));
    assert_eq!(search.state, State::Matches);
}
#[test]
fn empty_timeout_close_and_unsupported_never_replay() {
    let mut search = flight("x");
    search.set_query("");
    assert!(!search.answer(&scope(), "collision", decode(&answer("p1"))));
    assert_eq!(search.state, State::Empty);
    let mut search = flight("x");
    search.set_query("newer");
    assert!(search.expire(Instant::now() + TIMEOUT));
    assert_eq!(search.state, State::TimedOut);
    assert!(search.next(4, TextPoint::default()).is_none());
    search.close();
    assert!(!search.answer(&scope(), "collision", decode(&answer("p1"))));
    let mut unsupported = Search::new(scope(), false);
    unsupported.set_query("x");
    unsupported.step(SearchDirection::Forward);
    assert!(unsupported.next(4, TextPoint::default()).is_none());
    assert_eq!(unsupported.state, State::Unsupported);
}
#[test]
fn malformed_oversized_and_no_match_have_distinct_states() {
    assert_eq!(decode(&json!({})), Err(State::Malformed));
    let mut body = answer("p1");
    body["result"]["current"] = json!(42);
    assert_eq!(decode(&body), Err(State::Malformed));
    body["result"]["matches"] = json!(vec![json!({}); MAX_MATCHES + 1]);
    assert_eq!(decode(&body), Err(State::Oversized));
    assert_eq!(
        decode(&json!({"padding":"a".repeat(MAX_RESPONSE_BYTES)})),
        Err(State::Oversized)
    );
    let empty = json!({"result":{"type":"pane_copy_search","pane_id":"p1","content_revision":4,"matches":[],"total":0}});
    let mut search = flight("missing");
    assert!(search.answer(&scope(), "collision", decode(&empty)));
    assert_eq!(search.state, State::NoMatch);
    let mut search = flight("x");
    search.answer(&scope(), "collision", decode(&answer("p2")));
    assert_eq!(search.state, State::Malformed);
}
#[test]
fn next_previous_use_typed_daemon_coordinates() {
    let mut search = flight("日本語");
    search.answer(&scope(), "collision", decode(&answer("p1")));
    search.step(SearchDirection::Forward);
    let next = search.next(4, TextPoint::default()).unwrap();
    assert_eq!(next.direction, SearchDirection::Forward);
    assert_eq!(
        next.previous,
        search.result.as_ref().unwrap().current_match()
    );
    search.step(SearchDirection::Backward);
    assert_eq!(
        search.next(4, TextPoint::default()).unwrap().direction,
        SearchDirection::Backward
    );
}

fn live() -> crate::live::Live {
    use sheprd_client::{ClientEvent, protocol::ClientShellSnapshot};
    use std::sync::Arc;
    let mut live = crate::live::Live::default();
    live.connected = true;
    let snapshot: ClientShellSnapshot = serde_json::from_str(crate::mock_peer::SNAPSHOT).unwrap();
    live.apply(ClientEvent::Snapshot(Arc::new(snapshot)));
    live.apply(ClientEvent::Surface(Arc::new(crate::find::peer::surface())));
    live
}
#[test]
fn reader_mailbox_retains_early_response_and_rejects_focus_before_ui_poll() {
    use sheprd_client::ClientEvent;
    use std::sync::Arc;
    let mut live = live();
    let scope = adapter::scope(&live, "host", 1).unwrap();
    live.search.ticket = Some((scope.clone(), "collision".into()));
    // Reader observes response immediately after send, before any UI poll.
    live.apply(ClientEvent::Response {
        request_id: "collision".into(),
        response: answer(&scope.pane),
    });
    assert!(live.search.answer.as_ref().unwrap().is_ok());
    live.search.clear();
    live.search.ticket = Some((scope.clone(), "collision".into()));
    let mut snapshot = live.snapshot.as_deref().unwrap().clone();
    snapshot.focused_pane_id = Some("other".into());
    live.apply(ClientEvent::Snapshot(Arc::new(snapshot)));
    live.apply(ClientEvent::Response {
        request_id: "collision".into(),
        response: answer(&scope.pane),
    });
    assert!(live.search.answer.is_none());
    assert!(live.search.ticket.is_none());
}
#[test]
fn disconnect_and_reopened_mailboxes_do_not_share_colliding_ids() {
    use sheprd_client::ClientEvent;
    let mut old = live();
    let scope = adapter::scope(&old, "host", 1).unwrap();
    old.search.ticket = Some((scope.clone(), "collision".into()));
    old.apply(ClientEvent::Disconnected {
        reason: "test".into(),
        ssh: None,
    });
    old.apply(ClientEvent::Response {
        request_id: "collision".into(),
        response: answer(&scope.pane),
    });
    assert!(old.search.answer.is_none());
    let fresh = live();
    assert!(fresh.search.ticket.is_none());
    assert!(fresh.search.answer.is_none());
}
