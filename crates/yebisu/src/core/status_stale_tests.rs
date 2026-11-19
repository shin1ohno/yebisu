//! Review 0df6 SF3: a list that keeps failing does not leave the last
//! statuses on screen as if they were current; one failure alone keeps them.
use super::*;
use std::time::Instant;

const BOUND: Duration = Duration::from_secs(10);

fn raw(address: &str) -> CoreStatus {
    CoreStatus {
        address: address.into(),
        status: None,
    }
}

/// A listing that answers `answers` in order, reports each call, then blocks
/// until the test ends.
fn scripted(
    answers: Vec<Option<Vec<CoreStatus>>>,
) -> (List, mpsc::Receiver<usize>, mpsc::Sender<()>) {
    let (called_tx, called) = mpsc::channel();
    let (release, wait) = mpsc::channel::<()>();
    let mut answers = answers.into_iter();
    let mut count = 0;
    let list: List = Box::new(move || {
        count += 1;
        let _ = called_tx.send(count);
        answers.next().unwrap_or_else(|| {
            let _ = wait.recv();
            None
        })
    });
    (list, called, release)
}

/// Runs the poll until its `calls`-th list call has started.
fn run(
    answers: Vec<Option<Vec<CoreStatus>>>,
    calls: usize,
) -> (
    StatusMailbox,
    Arc<Mutex<Live>>,
    StatusPoll,
    mpsc::Sender<()>,
) {
    let (list, called, release) = scripted(answers);
    let mailbox = StatusMailbox::default();
    let inbox = Arc::new(Mutex::new(Live::default()));
    let poll = StatusPoll::start(
        list,
        mailbox.clone(),
        inbox.clone(),
        Arc::new(AtomicBool::new(false)),
        Duration::from_millis(1),
    )
    .unwrap_or_else(|e| panic!("poll: {e}"));
    let deadline = Instant::now() + BOUND;
    loop {
        let call = called
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap_or_else(|e| panic!("list call {calls} not within {BOUND:?}: {e}"));
        if call == calls {
            break;
        }
    }
    (mailbox, inbox, poll, release)
}

#[test]
fn failing_lists_drop_the_last_statuses() {
    // Call 4 blocks, so calls 1 to 3 (one success, two failures) are done.
    let (mailbox, inbox, poll, release) = run(vec![Some(vec![raw("w1:p1")]), None, None], 4);
    assert_eq!(
        mailbox.latest(),
        None,
        "two failed lists in a row left the last statuses on screen"
    );
    assert!(inbox.lock().unwrap_or_else(PoisonError::into_inner).dirty);
    drop(release);
    drop(poll);
}

#[test]
fn one_failed_list_keeps_the_last_statuses() {
    let (mailbox, _inbox, poll, release) = run(vec![Some(vec![raw("w1:p1")]), None], 3);
    assert_eq!(mailbox.latest(), Some(vec![raw("w1:p1")]));
    drop(release);
    drop(poll);
}

#[test]
fn a_success_resets_the_failure_count() {
    let answers = vec![
        Some(vec![raw("w1:p1")]),
        None,
        Some(vec![raw("w1:p2")]),
        None,
    ];
    let (mailbox, _inbox, poll, release) = run(answers, 5);
    assert_eq!(mailbox.latest(), Some(vec![raw("w1:p2")]));
    drop(release);
    drop(poll);
}
