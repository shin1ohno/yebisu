//! Add host consumes bounded snapshots. The browser lives on a dedicated
//! worker so waiting for a Bonjour socket never holds a GPUI executor thread.
#[cfg(target_os = "ios")]
mod bonjour;
#[cfg(any(target_os = "ios", test))]
mod reply;

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

pub(super) const MAX_DEVICES: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Device {
    pub name: String,
    pub address: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Status {
    #[default]
    Searching,
    WaitingForPermission,
    Finished,
    Unavailable,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Snapshot {
    pub status: Status,
    pub devices: Vec<Device>,
}

#[derive(Default)]
pub(super) struct Mailbox {
    cancelled: AtomicBool,
    latest: Mutex<Option<Snapshot>>,
}

impl Mailbox {
    pub fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub fn publish(&self, status: Status, devices: impl IntoIterator<Item = Device>) {
        let mut devices: Vec<_> = devices.into_iter().take(MAX_DEVICES).collect();
        devices.sort();
        devices.dedup();
        if let Ok(mut latest) = self.latest.lock() {
            *latest = Some(Snapshot { status, devices });
        }
    }
}

pub(super) struct Search(Arc<Mailbox>);

impl Search {
    pub fn start(browser: impl FnOnce(&Mailbox) + Send + 'static) -> Self {
        let mailbox = Arc::new(Mailbox::default());
        let worker = Arc::clone(&mailbox);
        if let Err(error) = thread::Builder::new()
            .name("yebisu-bonjour".into())
            .spawn(move || browser(&worker))
        {
            tracing::warn!(%error, "could not start Bonjour discovery");
            mailbox.publish(Status::Unavailable, []);
        }
        Self(mailbox)
    }

    pub fn take_latest(&self) -> Option<Snapshot> {
        self.0.latest.try_lock().ok()?.take()
    }
}

impl Drop for Search {
    fn drop(&mut self) {
        self.0.cancelled.store(true, Ordering::Release);
    }
}

#[cfg(target_os = "ios")]
pub(super) fn browse(mailbox: &Mailbox) {
    // Simulator QA can render the actual sheet without relying on LAN access.
    #[cfg(all(target_abi = "sim", feature = "test-support"))]
    if std::env::var_os("YEBISU_QA_NEARBY").is_some() {
        mailbox.publish(
            Status::Finished,
            [
                Device {
                    name: "pro-dev".into(),
                    address: "pro-dev.local".into(),
                },
                Device {
                    name: "Studio".into(),
                    address: "studio.local".into(),
                },
            ],
        );
        return;
    }
    bonjour::browse(mailbox);
}
