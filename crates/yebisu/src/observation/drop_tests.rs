//! A QA observation record is never dropped silently (2026-11-19, layout
//! integration: hosted p10 run 44406830068 job 135164241872 lost its
//! `settings-lifecycle` records with no trace in the job or the artifact).
use super::*;
use std::{
    sync::{Mutex, mpsc::Receiver},
    thread::ThreadId,
};

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

/// Stands for a stderr nobody drains: each write says which thread made it,
/// then blocks until the test sends a release.
struct StalledStderr {
    writers: SyncSender<ThreadId>,
    release: Receiver<()>,
    text: Arc<Mutex<Vec<u8>>>,
}

impl Write for StalledStderr {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let _ = self.writers.send(std::thread::current().id());
        let _ = self.release.recv();
        self.text
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Use real stderr with libtest capture disabled so this fails if the
/// recording thread calls `eprintln!` directly, bypassing the reporter.
#[test]
fn a_full_queue_returns_at_once_and_the_caller_never_writes_stderr() {
    const PROBE: &str = "YEBISU_OBSERVATION_STDERR_LOCK_PROBE";
    if std::env::var_os(PROBE).is_none() {
        let output = std::process::Command::new(
            std::env::current_exe().unwrap_or_else(|error| panic!("{error}")),
        )
        .args([
            "--exact",
            "observation::drop_tests::a_full_queue_returns_at_once_and_the_caller_never_writes_stderr",
            "--nocapture",
        ])
        .env(PROBE, "1")
        .output()
        .unwrap_or_else(|error| panic!("{error}"));
        assert!(
            output.status.success(),
            "stderr lock probe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("smoke observation dropped: full (count 1)"),
            "the worker must report the full queue after stderr is unlocked"
        );
        return;
    }

    let (locked, lock_ready) = sync_channel(0);
    let (release, released) = sync_channel::<()>(0);
    let holder = std::thread::spawn(move || {
        let _lock = std::io::stderr().lock();
        let _ = locked.send(());
        let _ = released.recv();
    });
    let (send, records) = sync_channel(1);
    let observation = Observation::injected_with_wait(send, Duration::from_secs(2));
    assert!(observation.try_record(serde_json::json!({"kind": "fill"})));
    lock_ready.recv().unwrap_or_else(|error| panic!("{error}"));

    let (finished, finish) = sync_channel(1);
    let producer = observation.clone();
    let caller = std::thread::spawn(move || {
        producer.record_without_waiting(serde_json::json!({"kind": "lost"}));
        let _ = finished.send(());
    });
    let returned = finish.recv_timeout(Duration::from_millis(500));
    drop(release);
    assert!(holder.join().is_ok());
    assert!(caller.join().is_ok());
    assert!(
        returned.is_ok(),
        "recording waited on a full queue or stderr"
    );
    assert!(observation.wait_for_drop_report(Duration::from_secs(5)));
    assert_eq!(next(&records)["kind"], "fill");
}

/// The reporter coalesces drops even when its sink stalls and writes only
/// from the worker.
#[test]
fn a_stalled_reporter_coalesces_drops_without_holding_the_caller() {
    let (send, records) = sync_channel(1);
    let mut observation = Observation::injected(send);
    let (writers_send, writers) = sync_channel(16);
    let (release, released) = sync_channel(16);
    let text = Arc::new(Mutex::new(Vec::new()));
    observation.report_drops_to(Box::new(StalledStderr {
        writers: writers_send,
        release: released,
        text: text.clone(),
    }));
    assert!(observation.try_record(serde_json::json!({"kind": "fill"})));
    let caller = std::thread::current().id();
    assert!(!observation.try_record(serde_json::json!({"kind": "lost"})));
    let writer = writers
        .recv_timeout(Duration::from_secs(5))
        .unwrap_or_else(|e| panic!("no drop line was written: {e}"));
    assert_ne!(
        writer, caller,
        "the drop line was written on the recording thread"
    );
    // The reporter is now stuck in its first write, like `eprintln!` on a
    // full stderr pipe. Drops keep returning at once.
    let started = Instant::now();
    for _ in 0..200 {
        assert!(!observation.try_record(serde_json::json!({"kind": "lost"})));
    }
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_millis(500),
        "200 drops took {elapsed:?} with stderr stalled"
    );
    assert!(
        !observation.wait_for_drop_report(Duration::from_millis(20)),
        "positive control: the stalled reporter has not printed yet"
    );
    for _ in 0..16 {
        let _ = release.send(());
    }
    assert!(observation.wait_for_drop_report(Duration::from_secs(5)));
    assert!(
        writers.try_iter().all(|thread| thread != caller),
        "a later drop line was written on the recording thread"
    );
    let text = String::from_utf8(
        text.lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone(),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let mut total = 0;
    for line in text.lines() {
        let count = line
            .strip_prefix("smoke observation dropped: full (count ")
            .and_then(|rest| rest.strip_suffix(')'))
            .and_then(|count| count.parse::<u64>().ok())
            .unwrap_or_else(|| panic!("unexpected drop line {line:?}"));
        total += count;
    }
    assert_eq!(total, 201, "{text}");
    assert!(
        text.lines().count() < 201,
        "drops are coalesced while stderr is stalled: {text}"
    );
    assert_eq!(next(&records)["kind"], "fill");
}
