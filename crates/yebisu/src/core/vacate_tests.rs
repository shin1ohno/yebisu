use super::{
    SessionInfo,
    vacate::{
        Expect, GRACE, Stopped, Vacancies, Watch, next_space, protected, stop_command, stop_result,
        vacant,
    },
};
use sheprd_core_client::wire::Terminal;
use std::time::{Duration, Instant};

fn terminal(state: &str) -> Terminal {
    serde_json::from_value(serde_json::json!({"address": "t1", "state": state}))
        .unwrap_or_else(|e| panic!("terminal: {e}"))
}

fn row(name: &str, confirmed: bool) -> SessionInfo {
    SessionInfo {
        name: name.into(),
        tabs: Vec::new(),
        confirmed,
        core_id: confirmed.then(|| format!("id-{name}")),
        statuses: Vec::new(),
        created: 0,
        doc: None,
    }
}

fn pair(name: &str, id: &str) -> (String, Expect) {
    (name.into(), Expect::seen(id.into(), Some(7)))
}

#[test]
fn only_the_default_space_is_protected() {
    assert!(protected("default"));
    assert!(!protected("space-1795252668300"));
    assert!(!protected("Default"));
}

#[test]
fn a_space_is_vacant_with_no_tab_and_only_finished_terminals() {
    assert!(vacant(0, &[]));
    assert!(vacant(0, &[terminal("finished"), terminal("finished")]));
    // A live terminal in no tab keeps the space.
    assert!(!vacant(0, &[terminal("finished"), terminal("live")]));
    // So does a state this client does not know.
    assert!(!vacant(0, &[terminal("parked")]));
    // A tab keeps it, whatever the list says.
    assert!(!vacant(1, &[]));
}

#[test]
fn a_polled_space_is_due_only_after_two_snapshots_at_least_five_seconds_apart() {
    let t0 = Instant::now();
    let mut seen = Vacancies::default();
    seen.observe(t0, vec![pair("space-1", "a")]);
    assert!(
        seen.due(t0).is_empty(),
        "the first snapshot alone never stops"
    );
    seen.observe(t0 + Duration::from_secs(1), vec![pair("space-1", "a")]);
    assert!(seen.due(t0 + Duration::from_secs(1)).is_empty());
    let almost = t0 + GRACE - Duration::from_millis(1);
    seen.observe(almost, vec![pair("space-1", "a")]);
    assert!(seen.due(almost).is_empty(), "4.999 s is not 5 s");
    let later = t0 + GRACE;
    seen.observe(later, vec![pair("space-1", "a")]);
    assert_eq!(seen.due(later), vec![pair("space-1", "a")]);
}

#[test]
fn a_space_that_filled_in_between_starts_again() {
    let t0 = Instant::now();
    let mut seen = Vacancies::default();
    seen.observe(t0, vec![pair("space-1", "a")]);
    seen.observe(t0 + Duration::from_secs(3), Vec::new());
    seen.observe(t0 + Duration::from_secs(6), vec![pair("space-1", "a")]);
    assert!(
        seen.due(t0 + Duration::from_secs(6)).is_empty(),
        "vacant again since 6 s, not since 0 s"
    );
}

#[test]
fn a_restarted_server_starts_the_grace_again() {
    let t0 = Instant::now();
    let mut seen = Vacancies::default();
    seen.observe(t0, vec![pair("space-1", "a")]);
    seen.observe(t0 + Duration::from_secs(6), vec![pair("space-1", "b")]);
    assert!(seen.due(t0 + Duration::from_secs(6)).is_empty());
}

#[test]
fn a_failed_stop_is_not_retried_until_the_server_changes() {
    let t0 = Instant::now();
    let mut seen = Vacancies::default();
    seen.observe(t0, vec![pair("space-1", "a")]);
    let t1 = t0 + Duration::from_secs(6);
    seen.observe(t1, vec![pair("space-1", "a")]);
    assert_eq!(seen.due(t1).len(), 1);
    seen.failed("space-1", &pair("space-1", "a").1);
    let t2 = t1 + Duration::from_secs(6);
    seen.observe(t2, vec![pair("space-1", "a")]);
    assert!(seen.due(t2).is_empty());
    let t3 = t2 + Duration::from_secs(6);
    seen.observe(t3, vec![pair("space-1", "b")]);
    seen.observe(t3 + GRACE, vec![pair("space-1", "b")]);
    assert_eq!(seen.due(t3 + GRACE), vec![pair("space-1", "b")]);
}

#[test]
fn the_connected_space_looks_at_once_then_every_five_seconds_while_busy() {
    let t0 = Instant::now();
    let mut watch = Watch::default();
    assert!(!watch.due(false, t0));
    assert!(watch.due(true, t0), "its last tab just went");
    assert!(!watch.due(true, t0 + Duration::from_secs(1)), "not twice");
    watch.busy(t0);
    assert!(!watch.due(true, t0 + Duration::from_secs(4)));
    assert!(
        watch.due(true, t0 + GRACE),
        "a terminal may have ended since"
    );
    // A new tab ends the vacancy; closing it again is a new event.
    assert!(!watch.due(false, t0 + Duration::from_secs(9)));
    assert!(watch.due(true, t0 + Duration::from_secs(9)));
}

#[test]
fn selection_takes_the_next_confirmed_space_else_the_last() {
    let order = [
        row("a", true),
        row("b", true),
        row("c", false),
        row("d", true),
    ];
    // The one that takes its place.
    assert_eq!(next_space(&order, "a").map(|n| n.name), Some("b".into()));
    // An unconfirmed one cannot be connected to: pass over it.
    assert_eq!(next_space(&order, "b").map(|n| n.name), Some("d".into()));
    // Nothing after it: the last one remaining.
    assert_eq!(next_space(&order, "d").map(|n| n.name), Some("b".into()));
    assert_eq!(
        next_space(&order, "d").and_then(|n| n.core_id),
        Some("id-b".into())
    );
    // Nothing else to go to.
    assert_eq!(next_space(&[row("a", true)], "a"), None);
    assert_eq!(next_space(&[row("a", true), row("b", false)], "a"), None);
}

#[test]
fn the_stop_command_asks_the_socket_and_never_starts_a_server() {
    let expect = Expect {
        core_id: "0123abcd-ef".into(),
        pid: Some(4242),
        min_uptime_ms: Some(900),
    };
    let command =
        stop_command("space-1795252668300", &expect).unwrap_or_else(|e| panic!("command: {e}"));
    // Python on the host, the script on stdin, the arguments after it.
    assert!(
        command.contains(
            "exec python3 -I - space-1795252668300 0123abcd-ef 4242 900 <<'YEBISU_CORE_STOP'"
        ),
        "{command}"
    );
    assert!(command.contains(include_str!("../shell/herdr2_frame/stop.py")));
    // `hc server status` starts a server on Unix: no `hc` runs, and the
    // script starts no process.
    let script = include_str!("../shell/herdr2_frame/stop.py");
    let wrapper = command.replace(script, "");
    assert!(
        !wrapper.contains("hc ") && !wrapper.contains("status"),
        "{wrapper}"
    );
    assert!(!script.contains("subprocess") && !script.contains("os.exec"));
    // The script asks `server.status` for pid, uptime and live, and stops in
    // the same connection.
    assert!(script.contains("\"server.status\"") && script.contains("\"server.stop\""));
    assert!(script.contains("FileNotFoundError, ConnectionRefusedError"));
    // Unknown values are `-`.
    let any = Expect::seen("abc".into(), None);
    let command = stop_command("space-1", &any).unwrap_or_else(|e| panic!("command: {e}"));
    assert!(
        command.contains("space-1 abc - - <<'YEBISU_CORE_STOP'"),
        "{command}"
    );
}

#[test]
fn the_stop_command_refuses_a_bad_session_or_core_id() {
    let id = |id: &str| Expect::seen(id.into(), None);
    assert!(
        stop_command("a/b", &id("id")).is_err(),
        "a path is no session"
    );
    assert!(stop_command("space-1", &id("")).is_err());
    assert!(stop_command("space-1", &id("x'; reboot; '")).is_err());
}

#[test]
fn the_stop_status_tells_stopped_busy_and_another_server() {
    assert_eq!(stop_result(Some(0), b"stopped").ok(), Some(Stopped::Done));
    assert_eq!(
        stop_result(Some(4), b"a terminal is running").ok(),
        Some(Stopped::Busy)
    );
    for code in [3, 5, 6, 127, 1] {
        assert!(stop_result(Some(code), b"x").is_err(), "{code}");
    }
    assert!(stop_result(None, b"").is_err());
}
