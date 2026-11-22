//! TestDispatcher executes background futures on the test thread. Preserve the
//! production worker boundary while exercising the real blocking receive path.
use crate::{
    core::PasteOutcome,
    handoff_ticket::{PasteTicket, Queued},
};
use std::{
    cell::Cell,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender},
    },
    task::Poll,
    time::Duration,
};

pub(crate) fn outcome(
    executor: &gpui::BackgroundExecutor,
    receive: Receiver<PasteOutcome>,
    ticket: PasteTicket,
    timeout: Duration,
) -> gpui::Task<Result<Queued, String>> {
    let shared = Arc::new(Mutex::new((None, None::<std::task::Waker>, false)));
    PENDING.with(|pending| pending.borrow_mut().push(shared.clone()));
    let worker = shared.clone();
    let start_timeout = TIMEOUT_START.with(Cell::take);
    std::thread::spawn(move || {
        if let Some(start) = start_timeout {
            start
                .recv_timeout(Duration::from_secs(30))
                .unwrap_or_else(|e| panic!("start timeout: {e}"));
        }
        let hooks = ticket.test.clone();
        let result = crate::core_paste_view::await_outcome(receive, ticket, timeout);
        hooks.ui_completion.pass();
        let mut shared = worker.lock().unwrap_or_else(|e| e.into_inner());
        shared.0 = Some(result);
    });
    executor.spawn(std::future::poll_fn(move |cx| {
        let mut shared = shared.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(result) = shared.0.take() {
            shared.2 = true;
            Poll::Ready(result)
        } else {
            shared.1 = Some(cx.waker().clone());
            Poll::Pending
        }
    }))
}

type Completion = Arc<
    Mutex<(
        Option<Result<Queued, String>>,
        Option<std::task::Waker>,
        bool,
    )>,
>;
thread_local! { static PENDING: std::cell::RefCell<Vec<Completion>> = const { std::cell::RefCell::new(Vec::new()) }; }
pub(crate) fn pump() {
    PENDING.with(|pending| {
        pending.borrow_mut().retain(|shared| {
            let mut shared = shared.lock().unwrap_or_else(|e| e.into_inner());
            if shared.2 {
                return false;
            }
            if shared.0.is_some()
                && let Some(waker) = shared.1.take()
            {
                waker.wake();
            }
            true
        })
    });
}

thread_local! { static TIMEOUT_START: Cell<Option<Receiver<()>>> = const { Cell::new(None) }; }

// Hold the real outcome receiver until the fixture observes the delivery phase
// it wants to time out. Only the test worker's start time is controlled here.
pub(super) fn defer_outcome_timeout() -> Sender<()> {
    let (start, wait) = mpsc::channel();
    TIMEOUT_START.with(|slot| assert!(slot.replace(Some(wait)).is_none()));
    start
}
