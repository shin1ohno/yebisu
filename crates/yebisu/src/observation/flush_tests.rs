//! `flush` (hosted p10, run 44406830068 job 135164241872): records still in
//! the queue die with a process that UIKit suspends and kills, so a lifecycle
//! callback that may precede a suspension waits until they are in the file.
use super::*;
use gpui::AppContext as _;
use std::sync::{Mutex, mpsc::Receiver};

/// A writer that starts `delay` late and writes into a shared buffer,
/// counting into `progress` like the app's writer.
fn slow_writer(
    receiver: Receiver<Value>,
    progress: Arc<Progress>,
    delay: Duration,
) -> Arc<Mutex<Vec<u8>>> {
    let file = Arc::new(Mutex::new(Vec::new()));
    let sink = file.clone();
    std::thread::spawn(move || {
        std::thread::sleep(delay);
        let mut buffer = Vec::new();
        for value in receiver.iter() {
            let _ = write_jsonl_line(&mut buffer, &value);
            sink.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone_from(&buffer);
            progress.written.fetch_add(1, Ordering::AcqRel);
        }
    });
    file
}

fn lines(file: &Arc<Mutex<Vec<u8>>>) -> usize {
    file.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .iter()
        .filter(|&&b| b == b'\n')
        .count()
}

/// `flush` returns only once every record sent before it is in the file.
#[test]
fn flush_waits_until_the_records_are_written() {
    let (sender, receiver) = sync_channel(16);
    let progress = Arc::new(Progress::default());
    let file = slow_writer(receiver, progress.clone(), Duration::from_millis(150));
    let observation = Observation::injected_with_progress(sender, progress);
    for index in 0..5 {
        assert!(observation.try_record(serde_json::json!({"kind": "record", "index": index})));
    }
    assert_eq!(observation.flush(Duration::from_secs(5)), Ok(()));
    assert_eq!(
        lines(&file),
        5,
        "flush returned before the records were written"
    );
}

/// A writer that never gets there: `flush` gives up at its bound and says
/// how many records are still unwritten.
#[test]
fn flush_reports_what_a_stalled_writer_left() {
    let (sender, _receiver) = sync_channel(16);
    let observation = Observation::injected_with_progress(sender, Arc::new(Progress::default()));
    assert!(observation.try_record(serde_json::json!({"kind": "record"})));
    assert!(observation.try_record(serde_json::json!({"kind": "record"})));
    let started = Instant::now();
    assert_eq!(observation.flush(Duration::from_millis(50)), Err(2));
    assert!(started.elapsed() >= Duration::from_millis(50));
}

/// The app's writer counts what it writes, and what it discards at its limit.
#[test]
fn the_writer_counts_written_and_discarded_records() {
    let (sender, receiver) = sync_channel(8);
    let progress = Progress::default();
    for index in 0..4 {
        sender
            .send(serde_json::json!({"kind": "record", "index": index}))
            .unwrap_or_else(|e| panic!("{e}"));
    }
    drop(sender);
    let mut file = Vec::new();
    write_records_limited(&mut file, receiver, 2, Some(&progress))
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(progress.written.load(Ordering::Acquire), 4);
}

/// Inactive (the callback before a suspension) waits for the writer; a
/// writer that does not finish is reported in the file.
#[gpui::test]
fn inactive_flushes_and_reports_a_stalled_writer(cx: &mut gpui::TestAppContext) {
    let (sender, records) = sync_channel(64);
    let view = cx.new(crate::app::Smoke::new);
    let started = Instant::now();
    view.update(cx, |this, cx| {
        this.observation =
            Observation::injected_with_progress(sender, Arc::new(Progress::default()));
        this.lifecycle(gpui::AppLifecyclePhase::Inactive, cx);
    });
    assert!(
        started.elapsed() >= LIFECYCLE_FLUSH,
        "Inactive did not wait for the writer"
    );
    let kinds: Vec<Value> = records.try_iter().collect();
    let timeout = kinds
        .iter()
        .find(|record| record["kind"] == "observation-flush-timeout")
        .unwrap_or_else(|| panic!("no flush timeout record: {kinds:?}"));
    assert_eq!(timeout["phase"], "Inactive");
    assert!(
        timeout["pending"]
            .as_u64()
            .is_some_and(|pending| pending >= 1)
    );
}

/// Active (no suspension follows) does not wait.
#[gpui::test]
fn active_does_not_wait_for_the_writer(cx: &mut gpui::TestAppContext) {
    let (sender, records) = sync_channel(64);
    let view = cx.new(crate::app::Smoke::new);
    let started = Instant::now();
    view.update(cx, |this, cx| {
        this.observation =
            Observation::injected_with_progress(sender, Arc::new(Progress::default()));
        this.lifecycle(gpui::AppLifecyclePhase::Active, cx);
    });
    assert!(started.elapsed() < LIFECYCLE_FLUSH);
    assert!(
        records
            .try_iter()
            .all(|record| record["kind"] != "observation-flush-timeout")
    );
}
