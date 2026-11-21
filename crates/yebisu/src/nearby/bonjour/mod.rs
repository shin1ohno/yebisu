use super::{Device, MAX_DEVICES, Mailbox, Status};
use ffi::{Browser, Event, POLICY_DENIED, Service};
use std::{
    collections::BTreeMap,
    thread,
    time::{Duration, Instant},
};

mod ffi;

const POLL: Duration = Duration::from_millis(100);
const SEARCH_TIME: Duration = Duration::from_secs(10);
const PERMISSION_WAIT: Duration = Duration::from_secs(60);
const PERMISSION_RETRY: Duration = Duration::from_secs(1);

pub(super) fn browse(mailbox: &Mailbox) {
    let permission_deadline = Instant::now() + PERMISSION_WAIT;
    while !mailbox.cancelled() {
        match run(mailbox) {
            Ok(()) => return,
            Err(error) if error.0 == POLICY_DENIED && Instant::now() < permission_deadline => {
                mailbox.publish(Status::WaitingForPermission, []);
                let until = (Instant::now() + PERMISSION_RETRY).min(permission_deadline);
                while Instant::now() < until && !mailbox.cancelled() {
                    thread::sleep(POLL.min(until.saturating_duration_since(Instant::now())));
                }
            }
            Err(error) => {
                tracing::warn!(code = error.0, "Bonjour discovery stopped");
                mailbox.publish(Status::Unavailable, []);
                return;
            }
        }
    }
}

fn run(mailbox: &Mailbox) -> Result<(), ffi::Error> {
    let mut browser = Browser::new()?;
    let mut services: BTreeMap<Service, Option<Device>> = BTreeMap::new();
    let search_ends = Instant::now() + SEARCH_TIME;
    let mut status = Status::Searching;
    mailbox.publish(status, []);
    while !mailbox.cancelled() {
        let mut changed = false;
        for event in browser.poll()? {
            match event {
                Event::Found(service) => {
                    if services.len() < MAX_DEVICES
                        && !services.contains_key(&service)
                        && browser.resolve(&service)?
                    {
                        services.insert(service, None);
                    }
                }
                Event::Lost(service) => {
                    browser.cancel_resolution(&service);
                    changed |= services.remove(&service).flatten().is_some();
                }
                Event::Resolved(service, device) => {
                    browser.cancel_resolution(&service);
                    if let Some(entry) = services.get_mut(&service) {
                        *entry = Some(device);
                        changed = true;
                    }
                }
                Event::ResolveFailed(service) => {
                    browser.cancel_resolution(&service);
                    services.remove(&service);
                }
                Event::Error(error) => return Err(error),
            }
        }
        if status == Status::Searching && Instant::now() >= search_ends {
            status = Status::Finished;
            changed = true;
        }
        if changed {
            mailbox.publish(status, services.values().flatten().cloned());
        }
    }
    Ok(())
}
