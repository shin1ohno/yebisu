use super::{PasteOutcome, paste_path};
use crate::{
    handoff_ticket::{PasteTicket, Queued},
    terminal_input::Destination,
};
use sheprd_core_client::{CoreBackend, CoreTerminal, Error, wire::Put};
use std::{
    collections::BTreeMap,
    sync::mpsc::{self, Receiver, Sender, SyncSender},
};

pub(super) struct Disconnect(pub CoreBackend);
impl Drop for Disconnect {
    fn drop(&mut self) {
        self.0.core().disconnect();
    }
}

struct Pending {
    destination: Destination,
    address: String,
    bytes: usize,
    ticket: PasteTicket,
    reply: SyncSender<PasteOutcome>,
}

pub(super) struct Uploads {
    pending: BTreeMap<u64, Pending>,
    send: Sender<(u64, Result<Put, Error>)>,
    receive: Receiver<(u64, Result<Put, Error>)>,
}
impl Default for Uploads {
    fn default() -> Self {
        let (send, receive) = mpsc::channel();
        Self {
            pending: BTreeMap::new(),
            send,
            receive,
        }
    }
}
impl Uploads {
    pub fn start(
        &mut self,
        backend: &CoreBackend,
        destination: Destination,
        address: String,
        bytes: Vec<u8>,
        ticket: PasteTicket,
        reply: SyncSender<PasteOutcome>,
    ) {
        if !ticket.pending() || backend.core().is_lost() {
            reject(ticket, reply, "Paste cancelled: target changed");
            return;
        }
        if !backend
            .core()
            .hello()
            .methods
            .iter()
            .any(|method| method == "terminal.put")
        {
            reject(ticket, reply, "Host core lacks terminal.put");
            return;
        }
        let serial = ticket.serial;
        self.pending.insert(
            serial,
            Pending {
                destination,
                address: address.clone(),
                bytes: bytes.len(),
                ticket: ticket.clone(),
                reply,
            },
        );
        let backend = backend.clone();
        let send = self.send.clone();
        let spawned = std::thread::Builder::new()
            .name("yebisu-put".into())
            .spawn(move || {
                let result = if ticket.pending() && !backend.core().is_lost() {
                    #[cfg(test)]
                    if ticket
                        .test
                        .put_busy
                        .load(std::sync::atomic::Ordering::Acquire)
                    {
                        return {
                            let _ = send.send((serial, Err(Error::Busy)));
                        };
                    }
                    backend.core().put(&address, "paste.png", &bytes)
                } else {
                    Err(Error::Lost)
                };
                let _ = send.send((serial, result));
            });
        if let Err(error) = spawned
            && let Some(pending) = self.pending.remove(&serial)
        {
            reject(
                pending.ticket,
                pending.reply,
                &format!("Paste failed: {error}"),
            );
        }
    }
    pub fn drain(
        &mut self,
        current: Option<&Destination>,
        address: &str,
        terminal: &mut CoreTerminal,
        stopped: bool,
    ) {
        for (serial, result) in self.receive.try_iter() {
            let Some(pending) = self.pending.remove(&serial) else {
                continue;
            };
            let result = (|| -> Result<Queued, String> {
                let put = result.map_err(message)?;
                if put.bytes != pending.bytes {
                    return Err("Paste failed: unexpected host answer".into());
                }
                let path = paste_path::escape(&put.path)?;
                if stopped || current != Some(&pending.destination) || address != pending.address {
                    return Err("Paste cancelled: target changed".into());
                }
                #[cfg(test)]
                pending.ticket.test.before_input.pass();
                pending.ticket.queue(|| {
                    #[cfg(test)]
                    if pending
                        .ticket
                        .test
                        .input_busy
                        .load(std::sync::atomic::Ordering::Acquire)
                    {
                        return Err(message(Error::Busy));
                    }
                    terminal
                        .input_gated(&sheprd_backend::ClientPaneInputEvent::Paste(path))
                        .map_err(message)
                })
            })();
            if result.is_err() {
                pending.ticket.fail();
            }
            #[cfg(test)]
            if pending
                .ticket
                .test
                .notification
                .armed
                .load(std::sync::atomic::Ordering::Acquire)
            {
                std::thread::spawn(move || {
                    pending.ticket.test.notification.pass();
                    let _ = pending.reply.try_send(PasteOutcome { serial, result });
                });
                continue;
            }
            let _ = pending.reply.try_send(PasteOutcome { serial, result });
        }
    }
}

pub(super) fn reject(ticket: PasteTicket, reply: SyncSender<PasteOutcome>, message: &str) {
    ticket.fail();
    let _ = reply.try_send(PasteOutcome {
        serial: ticket.serial,
        result: Err(message.into()),
    });
}

pub(super) fn message(error: Error) -> String {
    match error {
        Error::Busy => "Paste failed: connection busy".into(),
        Error::Lost => "Paste failed: connection lost".into(),
        Error::Timeout(_) => "Paste failed: host did not answer".into(),
        Error::Invalid(_, _) => "Paste failed: unexpected host answer".into(),
        Error::Refused { kind, message, .. } => {
            if kind
                .as_deref()
                .is_some_and(|k| matches!(k, "finished" | "not_found" | "machine_unknown"))
            {
                "Paste failed: terminal ended".into()
            } else {
                message
            }
        }
        error => format!("Paste failed: {error}"),
    }
}
