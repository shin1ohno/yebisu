//! Herdr 2 workspace. Each connection owns an immutable mailbox and bounded FIFO.
#[cfg(test)]
mod image_tests;
mod paste;
mod paste_path;
#[cfg(test)]
pub(crate) mod paste_test_support;
#[cfg(test)]
pub(crate) mod publication_probe;
#[cfg(test)]
mod regression_tests;
mod screen;
mod store;
#[cfg(test)]
mod tests;
mod worker;

use crate::{live::Live, terminal_input::Destination};
use sheprd_backend::{ClientPaneInputEvent, Size};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc::{self, SyncSender},
};

pub(crate) enum Command {
    Input(Destination, ClientPaneInputEvent),
    PutImage {
        destination: Destination,
        ticket: crate::handoff_ticket::PasteTicket,
        bytes: Vec<u8>,
        reply: SyncSender<PasteOutcome>,
    },
    Resize(Size),
    Split,
    Focus(usize),
    Orientation,
    #[cfg(test)]
    PauseInput(crate::handoff::PublicationGate),
    #[cfg(test)]
    ArmPublicationProbe(Box<publication_probe::AfterCompare>),
    #[cfg(test)]
    FinishPublicationProbe(publication_probe::Key, SyncSender<publication_probe::Key>),
}

pub(super) struct Queued {
    command: Command,
    input_epoch: u64,
}

pub(crate) struct PasteOutcome {
    pub serial: u64,
    pub result: Result<crate::handoff_ticket::Queued, String>,
}

pub(crate) struct Session {
    queue: SyncSender<Queued>,
    input_epoch: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
    #[cfg(test)]
    pub finished: Arc<AtomicBool>,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}
impl Session {
    pub fn start(
        host: String,
        generation: u64,
        inbox: Arc<Mutex<Live>>,
        connect: impl FnOnce() -> crate::Result<sheprd_core_client::CoreBackend> + Send + 'static,
        root: Option<std::path::PathBuf>,
    ) -> crate::Result<Self> {
        let (queue, receive) = mpsc::sync_channel(256);
        let stop = Arc::new(AtomicBool::new(false));
        let input_epoch = Arc::new(AtomicU64::new(0));
        let worker_epoch = input_epoch.clone();
        let shared = inbox.clone();
        let finished = Arc::new(AtomicBool::new(false));
        let completed = finished.clone();
        let stopped = stop.clone();
        std::thread::Builder::new()
            .name("yebisu-core".into())
            .spawn(move || {
                let result = (|| {
                    let backend = connect()?;
                    // Hello may finish after this connection was retired. Never spawn,
                    // attach or persist a terminal for that obsolete completion.
                    if stopped.load(Ordering::Acquire) {
                        return Ok(());
                    }
                    worker::run(
                        backend,
                        host,
                        generation,
                        &shared,
                        (&stopped, &worker_epoch),
                        receive,
                        root,
                    )
                })();
                if let Err(error) = result {
                    tracing::warn!(%error, "Herdr 2 connection stopped");
                    if let Ok(mut live) = shared.lock() {
                        live.connected = false;
                        live.status = error.to_string();
                        live.dirty = true;
                    }
                }
                completed.store(true, Ordering::Release);
            })?;
        Ok(Self {
            queue,
            input_epoch,
            stop,
            #[cfg(test)]
            finished,
        })
    }
    // Cancels queued consumer commands. Already-published SDK transport writes
    // require an SDK writer fence and are not claimed cancelled by this epoch.
    pub fn invalidate_input(&self) {
        self.input_epoch.fetch_add(1, Ordering::AcqRel);
    }
    #[cfg(test)]
    pub(crate) fn probe_input_epoch(&self) -> u64 {
        self.input_epoch.load(Ordering::Acquire)
    }
    pub fn send(&self, command: Command) -> crate::Result<()> {
        self.queue
            .try_send(Queued {
                command,
                input_epoch: self.input_epoch.load(Ordering::Acquire),
            })
            .map_err(|_| crate::Error::CoreQueue)
    }
}

#[cfg(target_os = "ios")]
pub(crate) fn connect(host: &str) -> crate::Result<sheprd_core_client::CoreBackend> {
    use std::time::Duration;
    let scan =
        sheprd_client::ssh::scan_host_key(host).map_err(|e| crate::Error::Ssh(e.to_string()))?;
    crate::hosts::authentication_gate(&scan)?;
    let command = sheprd_client::core_connect_command("default")
        .map_err(|e| crate::Error::Ssh(e.to_string()))?;
    let (stream, tunnel) = sheprd_client::exec_ssh_stream(host, &command)
        .map_err(|e| crate::Error::Ssh(e.to_string()))?;
    stream.set_read_timeout(Some(Duration::from_millis(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let stream = SshStream {
        stream,
        _tunnel: Arc::new(tunnel),
    };
    Ok(sheprd_core_client::CoreBackend::new(
        sheprd_core_client::Core::over(stream)?,
        10000,
    ))
}
#[cfg(target_os = "ios")]
struct SshStream {
    stream: sheprd_client::Stream,
    _tunnel: Arc<herdr_ssh::Tunnel>,
}
#[cfg(target_os = "ios")]
impl std::io::Read for SshStream {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        std::io::Read::read(&mut self.stream, bytes)
    }
}
#[cfg(target_os = "ios")]
impl std::io::Write for SshStream {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        std::io::Write::write(&mut self.stream, bytes)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        std::io::Write::flush(&mut self.stream)
    }
}
