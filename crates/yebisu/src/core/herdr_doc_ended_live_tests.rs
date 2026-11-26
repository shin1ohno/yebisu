//! Ended `herdr-app` spaces against a real herdr-core. Opt in with
//! `YEBISU_HERDR_CORE_TEST_BINARY=/path/to/herdr-core cargo test -p yebisu
//! herdr_doc_live_ended -- --ignored --test-threads=1`. The core is one
//! server of the private fleet of `core/space_fleet.rs` in the session
//! `test-ended`; the test never reaches a running core and ends only its
//! own. It checks the end reasons herdr-core gives (an exit, a kill, a
//! server stop of a terminal kept or not) and that the reader removes the
//! spaces those end, and only those, across a restart.
use super::*;
use crate::core::space_fleet::Fleet;
use serde_json::{Value, json};
use sheprd_core_client::wire::{Run, Terminal};

const SESSION: &str = "test-ended";
const PERIOD: Duration = Duration::from_millis(200);
const BOUND: Duration = Duration::from_secs(20);

fn space(id: &str, terminal: &str) -> Value {
    let tab = format!("tb{id}");
    json!({"id": id, "name": "", "folder": "/w", "tabs": [{"id": tab, "name": "",
        "root": {"type": "block", "id": tab, "content": {"terminal": terminal}}}]})
}

fn listed(fleet: &Fleet, address: &str) -> Option<Terminal> {
    fleet
        .list(SESSION)
        .into_iter()
        .find(|terminal| terminal.address == address)
}

fn wait_for(what: &str, done: impl Fn() -> bool) {
    let deadline = Instant::now() + BOUND;
    while !done() {
        assert!(Instant::now() < deadline, "timed out: {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn ids(value: &Value) -> Vec<String> {
    value["spaces"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|space| space["id"].as_str().map(str::to_owned))
        .collect()
}

fn reader(fleet: &mut Fleet) -> (DocSync, DocWriter) {
    let backend = fleet.bridge(SESSION);
    DocSync::start(
        DocPlan {
            source: DocSource::Core,
            period: PERIOD,
        },
        (&backend, "test"),
        DocMailbox::default(),
        Arc::new(Mutex::new(Live::default())),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap_or_else(|e| panic!("reader: {e}"))
    .unwrap_or_else(|| panic!("a Core plan reads"))
}

#[test]
#[ignore = "needs YEBISU_HERDR_CORE_TEST_BINARY"]
fn herdr_doc_live_ended_spaces_after_exit_kill_and_restart() {
    assert!(
        SESSION.starts_with("test-"),
        "HC_SESSION must start with test-"
    );
    let mut fleet = Fleet::start();
    fleet.add(SESSION);
    let setup = fleet.bridge(SESSION);
    let core = setup.core();
    let run = |argv: &[&str], keep: bool| {
        let argv = argv.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>();
        core.run(&Run {
            argv: &argv,
            keep,
            ..Run::default()
        })
        .unwrap_or_else(|e| panic!("run {argv:?}: {e}"))
        .address
    };
    let exits = run(&["sh", "-c", "exit 0"], false);
    let beside = run(&["sh", "-c", "exit 0"], false);
    let killed = run(&["sleep", "600"], false);
    let unkept = run(&["sleep", "600"], false);
    let kept = run(&["sleep", "600"], true);
    core.kill(&killed).unwrap_or_else(|e| panic!("kill: {e}"));
    let reason = |address: &str| listed(&fleet, address).and_then(|t| t.reason);
    wait_for("the exit", || reason(&exits).as_deref() == Some("exited"));
    wait_for("the other exit", || {
        reason(&beside).as_deref() == Some("exited")
    });
    wait_for("the kill", || reason(&killed).as_deref() == Some("killed"));
    // spP is a split: the kept terminal beside one that exited.
    let mut split = space("spP", &kept);
    split["tabs"][0]["root"] = json!({"type": "row", "id": "tbspP", "children": [
        {"type": "block", "id": "pK", "content": {"terminal": kept}},
        {"type": "block", "id": "pB", "content": {"terminal": beside}}]});
    let layout = json!({"spaces": [space("spE", &exits), space("spK", &killed),
        space("spU", &unkept), split]});
    core.doc_set(herdr_doc::NAME, &layout, 0)
        .unwrap_or_else(|e| panic!("set: {e}"));
    let doc = |probe: &CoreBackend| {
        probe
            .core()
            .doc_get(herdr_doc::NAME)
            .unwrap_or_else(|e| panic!("get: {e}"))
            .value
    };
    // The exit and the kill end their spaces; the live ones stay.
    let first = reader(&mut fleet);
    wait_for("spE and spK removed", || {
        ids(&doc(&setup)) == ["spU", "spP"]
    });
    // The split loses its ended pane; the live one takes its share.
    wait_for("the ended pane removed", || {
        doc(&setup)["spaces"][1]["tabs"][0]["root"]["children"]
            == json!([{"type": "block", "id": "pK", "content": {"terminal": kept},
                "size": {"weight": 2.0}}])
    });
    std::thread::sleep(PERIOD * 5);
    assert_eq!(
        ids(&doc(&setup)),
        ["spU", "spP"],
        "a live space was removed"
    );
    // A restart: the first reader's connection is gone with the old boot.
    drop(setup);
    fleet.restart(SESSION);
    drop(first);
    let unkept_after = listed(&fleet, &unkept);
    let kept_after = listed(&fleet, &kept);
    eprintln!("after restart: unkept {unkept_after:?}, kept {kept_after:?}");
    let probe = fleet.bridge(SESSION);
    let _second = reader(&mut fleet);
    match unkept_after
        .as_ref()
        .map(|t| (t.state, t.reason.clone(), t.keep))
    {
        // Not kept: its program is gone for good, so is its space.
        Some((TerminalState::Finished, Some(reason), false)) if reason == "server_stop" => {
            wait_for("spU removed", || ids(&doc(&probe)) == ["spP"]);
        }
        other => panic!("not kept, after a restart: {other:?}"),
    }
    // Kept: herdr-core gave it a shell; its space stays.
    std::thread::sleep(PERIOD * 5);
    assert_eq!(
        ids(&doc(&probe)),
        ["spP"],
        "kept: {:?}",
        listed(&fleet, &kept)
    );
}
