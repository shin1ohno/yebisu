//! The `smoke observation dropped: <reason>` lines, printed off the thread
//! that drops the record. A record is offered while drawing
//! (`record_window_controls`), so a synchronous `eprintln!` there held the
//! UI thread for as long as stderr blocked: a pipe nobody drains, or the
//! stderr lock held by another thread (review of PR 87 at 16ef79e). The
//! dropping thread now only bumps counters and wakes this module's thread
//! without waiting; that thread prints the lines, coalescing the drops it
//! finds into one line per reason with their count. QA checks still fail on
//! the line (`scripts/ios/app_stderr.py`), and the file still gets its
//! `observation-dropped` record (`Observation::try_record`).
#[cfg(test)]
use std::time::{Duration, Instant};
use std::{
    io::Write,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc::{SyncSender, sync_channel},
    },
};

/// Why a record did not get into the queue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Reason {
    Full,
    Disconnected,
}

impl Reason {
    fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Disconnected => "disconnected",
        }
    }
}

/// Drops waiting to be printed, per reason, and test-only delivery counters.
#[derive(Debug, Default)]
struct Counts {
    full: AtomicU64,
    disconnected: AtomicU64,
    #[cfg(test)]
    counted: AtomicU64,
    #[cfg(test)]
    printed: AtomicU64,
}

/// The dropping side of the reporter. Clones share one thread; the thread
/// ends when the last clone is gone.
#[derive(Clone, Debug)]
pub(crate) struct DropReport {
    counts: Arc<Counts>,
    /// Capacity 1: a full channel already holds a wake the thread has not
    /// taken, and the thread reads the counters after taking it.
    wake: SyncSender<()>,
}

impl DropReport {
    /// Starts the thread that prints into `out` (stderr in the app).
    pub(crate) fn spawn(mut out: Box<dyn Write + Send>) -> Self {
        let counts = Arc::new(Counts::default());
        let (wake, woken) = sync_channel::<()>(1);
        let printer = counts.clone();
        std::thread::spawn(move || {
            for () in woken.iter() {
                print(&printer, &mut out);
            }
        });
        Self { counts, wake }
    }

    /// Counts one drop and wakes the thread. Never blocks and never writes.
    pub(crate) fn count(&self, reason: Reason) {
        let counter = match reason {
            Reason::Full => &self.counts.full,
            Reason::Disconnected => &self.counts.disconnected,
        };
        counter.fetch_add(1, Ordering::AcqRel);
        #[cfg(test)]
        self.counts.counted.fetch_add(1, Ordering::AcqRel);
        // A full channel already has a pending wake.
        let _ = self.wake.try_send(());
    }

    /// Waits up to `bound` until every drop counted so far is printed.
    #[cfg(test)]
    pub(crate) fn wait_printed(&self, bound: Duration) -> bool {
        let deadline = Instant::now() + bound;
        loop {
            let counted = self.counts.counted.load(Ordering::Acquire);
            if self.counts.printed.load(Ordering::Acquire) >= counted {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

/// One line per reason with drops pending: `smoke observation dropped:
/// <reason> (count <n>)`. A failed write is not retried; the file's
/// `observation-dropped` record still carries the count.
fn print(counts: &Counts, out: &mut dyn Write) {
    for (reason, counter) in [
        (Reason::Full, &counts.full),
        (Reason::Disconnected, &counts.disconnected),
    ] {
        let count = counter.swap(0, Ordering::AcqRel);
        if count == 0 {
            continue;
        }
        let line = format!(
            "smoke observation dropped: {} (count {count})\n",
            reason.as_str()
        );
        let _ = out.write_all(line.as_bytes()).and_then(|()| out.flush());
        #[cfg(test)]
        counts.printed.fetch_add(count, Ordering::AcqRel);
    }
}

/// A test sink: each complete line goes to the channel without its newline.
#[cfg(test)]
pub(crate) struct Lines {
    sender: SyncSender<String>,
    pending: Vec<u8>,
}

#[cfg(test)]
impl Lines {
    pub(crate) fn new(sender: SyncSender<String>) -> Self {
        Self {
            sender,
            pending: Vec::new(),
        }
    }
}

#[cfg(test)]
impl Write for Lines {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.pending.extend_from_slice(buf);
        while let Some(end) = self.pending.iter().position(|&byte| byte == b'\n') {
            let line: Vec<u8> = self.pending.drain(..=end).collect();
            let text = String::from_utf8_lossy(&line[..end]).into_owned();
            let _ = self.sender.send(text);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
