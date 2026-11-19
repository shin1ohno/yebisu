//! A QA observation record is never dropped silently (2026-11-19, layout
//! integration: hosted p10 run 44406830068 job 135164241872 lost its
//! `settings-lifecycle` records with no trace in the job or the artifact).
use super::*;
use std::sync::mpsc::Receiver;

fn next(records: &Receiver<Value>) -> Value {
    records
        .recv_timeout(Duration::from_secs(1))
        .unwrap_or_else(|e| panic!("{e}"))
}

/// A record that finds the queue full is counted, and the next record that
/// gets in is preceded by one `observation-dropped` record with the count, so
/// the oracle reading the file sees the gap.
#[test]
fn a_dropped_record_is_reported_in_the_file() {
    let (send, records) = sync_channel(2);
    let observation = Observation::injected(send);
    assert!(observation.try_record(serde_json::json!({"kind": "first"})));
    assert!(observation.try_record(serde_json::json!({"kind": "second"})));
    assert!(
        !observation.try_record(serde_json::json!({"kind": "lost"})),
        "a full queue with no wait drops the record"
    );
    assert_eq!(next(&records)["kind"], "first");
    assert_eq!(next(&records)["kind"], "second");
    assert!(observation.try_record(serde_json::json!({"kind": "after"})));
    let notice = next(&records);
    assert_eq!(
        notice["kind"], "observation-dropped",
        "the gap is reported before the next record: {notice}"
    );
    assert_eq!(notice["count"], 1);
    assert!(notice["time"].is_f64());
    assert_eq!(next(&records)["kind"], "after");
    assert!(records.try_recv().is_err(), "one notice per gap");
}

/// The count is shared by every clone of one observation (the app hands
/// clones to its views), so a drop in one clone is reported by another.
#[test]
fn clones_share_the_drop_count() {
    let (send, records) = sync_channel(2);
    let first = Observation::injected(send);
    let second = first.clone();
    assert!(first.try_record(serde_json::json!({"kind": "fill"})));
    assert!(first.try_record(serde_json::json!({"kind": "fill"})));
    assert!(!first.try_record(serde_json::json!({"kind": "lost"})));
    assert_eq!(next(&records)["kind"], "fill");
    assert_eq!(next(&records)["kind"], "fill");
    assert!(second.try_record(serde_json::json!({"kind": "after"})));
    assert_eq!(next(&records)["kind"], "observation-dropped");
}

/// With a wait, a writer that is behind slows the sender down instead of
/// losing records: a burst five times the queue bound arrives whole.
#[test]
fn a_full_queue_waits_for_the_writer() {
    let (send, records) = sync_channel(2);
    let observation = Observation::injected_with_wait(send, Duration::from_secs(5));
    let reader = std::thread::spawn(move || {
        let mut kinds = Vec::new();
        while let Ok(value) = records.recv_timeout(Duration::from_secs(5)) {
            std::thread::sleep(Duration::from_millis(2));
            kinds.push(value["kind"].as_str().unwrap_or_default().to_owned());
            if kinds.len() == 10 {
                break;
            }
        }
        kinds
    });
    for index in 0..10 {
        assert!(
            observation.try_record(serde_json::json!({"kind": format!("burst-{index}")})),
            "record {index} was dropped instead of waiting"
        );
    }
    let kinds = reader.join().unwrap_or_else(|_| panic!("reader panicked"));
    let expected: Vec<String> = (0..10).map(|index| format!("burst-{index}")).collect();
    assert_eq!(kinds, expected);
}

/// At its limit the writer says so in the file instead of closing the queue,
/// and later records do not turn into `disconnected` drops.
#[test]
fn the_writer_limit_is_written_to_the_file() {
    let (sender, receiver) = sync_channel(8);
    for index in 0..4 {
        sender
            .send(serde_json::json!({"kind": "record", "index": index}))
            .unwrap_or_else(|e| panic!("{e}"));
    }
    let writer = std::thread::spawn(move || {
        let mut file = Vec::new();
        write_records_limited(&mut file, receiver, 2, None).map(|()| file)
    });
    sender
        .send(serde_json::json!({"kind": "late"}))
        .unwrap_or_else(|e| panic!("the queue closed at the limit: {e}"));
    drop(sender);
    let file = writer
        .join()
        .unwrap_or_else(|_| panic!("writer panicked"))
        .unwrap_or_else(|e| panic!("{e}"));
    let text = String::from_utf8(file).unwrap_or_else(|e| panic!("{e}"));
    let lines: Vec<Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap_or_else(|e| panic!("{e}")))
        .collect();
    assert_eq!(lines.len(), 3, "{text}");
    assert_eq!(lines[2]["kind"], "observation-limit");
    assert_eq!(lines[2]["written"], 2);
}
