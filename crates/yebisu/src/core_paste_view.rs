//! UI reservation and completion for Herdr 2 image pastes.
use crate::{
    app::Smoke,
    core::{Command, PasteOutcome},
    handoff::{CORE_IMAGE_LIMIT, Payload, Prepared},
    handoff_ticket::{Cancellation, PasteTicket, Queued},
};
use gpui::Context;
use std::{
    sync::mpsc::{self, Receiver, RecvTimeoutError},
    time::{Duration, Instant},
};

const OUTCOME_TIMEOUT: Duration = Duration::from_secs(20);
const SETTLE_TIMEOUT: Duration = Duration::from_secs(6);

pub(crate) fn await_outcome(
    receive: Receiver<PasteOutcome>,
    ticket: PasteTicket,
    timeout: Duration,
) -> Result<Queued, String> {
    match receive.recv_timeout(timeout) {
        Ok(outcome) if outcome.serial == ticket.serial => outcome.result,
        Ok(_) => {
            ticket.cancel();
            Err("Paste failed: unexpected host answer".into())
        }
        Err(RecvTimeoutError::Disconnected) => {
            ticket.cancel();
            Err("Paste failed: connection lost".into())
        }
        Err(RecvTimeoutError::Timeout) => {
            let cancellation = ticket.cancel();
            #[cfg(test)]
            ticket
                .test
                .timed_out
                .store(true, std::sync::atomic::Ordering::Release);
            match cancellation {
                Cancellation::Unsent => Err("Paste failed: host did not answer".into()),
                Cancellation::Sent => Ok(Queued),
                Cancellation::Sending => {
                    let deadline = Instant::now() + SETTLE_TIMEOUT;
                    loop {
                        match ticket.delivery() {
                            Some(sheprd_core_client::Delivery::Written) => return Ok(Queued),
                            Some(sheprd_core_client::Delivery::Writing)
                                if Instant::now() < deadline =>
                            {
                                std::thread::sleep(Duration::from_millis(10))
                            }
                            _ => return Err("Paste failed: connection lost".into()),
                        }
                    }
                }
            }
        }
    }
}

impl Smoke {
    pub(crate) fn start_core_image(&mut self, payload: Payload, cx: &mut Context<Self>) {
        let Some(stamp) = self.handoff.stamp.clone() else {
            return;
        };
        let serial = self.handoff.serial;
        let ticket = PasteTicket::new(serial);
        #[cfg(test)]
        let ticket = {
            let mut ticket = ticket;
            ticket.test = self.handoff.core_test.take().unwrap_or_default();
            ticket
        };
        self.handoff.core_ticket = Some(ticket.clone());
        self.handoff.preparing = true;
        self.terminal.live.status = "Preparing image".into();
        self.handoff.workers += 1;
        let work = cx
            .background_executor()
            .spawn(async move { crate::handoff::prepare(payload, CORE_IMAGE_LIMIT) });
        #[cfg(test)]
        let completion_gate = self.handoff.completion_gate.take();
        #[cfg(test)]
        let timeout = self.handoff.core_timeout.unwrap_or(OUTCOME_TIMEOUT);
        #[cfg(not(test))]
        let timeout = OUTCOME_TIMEOUT;
        cx.spawn(async move |weak, cx| {
            let prepared = work.await;
            #[cfg(test)]
            if let Some(gate) = completion_gate {
                gate.await;
            }
            let (reply, receive) = mpsc::sync_channel(1);
            let sent = weak
                .update(cx, |this, _| -> Result<(), String> {
                    this.sync_handoff();
                    if this.handoff.serial != serial
                        || this.handoff.stamp.as_ref() != Some(&stamp)
                        || !ticket.pending()
                    {
                        return Err("Paste cancelled: target changed".into());
                    }
                    let Prepared::Image(bytes) = prepared.map_err(paste_error)? else {
                        return Err("Paste failed: unexpected preparation".into());
                    };
                    this.terminal
                        .core
                        .as_ref()
                        .ok_or("Paste failed: connection lost")?
                        .send(Command::PutImage {
                            destination: stamp.destination,
                            ticket: ticket.clone(),
                            bytes,
                            reply,
                        })
                        .map_err(|_| "Paste failed: connection busy".to_owned())
                })
                .unwrap_or_else(|_| Err("Paste failed: connection lost".into()));
            let result = match sent {
                Ok(()) => {
                    #[cfg(test)]
                    let task = crate::core::paste_test_support::outcome(
                        cx.background_executor(),
                        receive,
                        ticket,
                        timeout,
                    );
                    #[cfg(not(test))]
                    let task = cx
                        .background_executor()
                        .spawn(async move { await_outcome(receive, ticket, timeout) });
                    task.await
                }
                Err(error) => {
                    ticket.cancel();
                    Err(error)
                }
            };
            let _ = weak.update(cx, |this, cx| {
                this.handoff.workers = this.handoff.workers.saturating_sub(1);
                cx.notify();
                this.sync_handoff();
                if this.handoff.serial != serial {
                    return;
                }
                this.handoff.preparing = false;
                this.handoff.refused_tail = false;
                match result {
                    Ok(_) => {
                        this.terminal.live.status = "Paste queued".into();
                        for event in std::mem::take(&mut this.handoff.held) {
                            this.send_terminal(event);
                        }
                    }
                    Err(error) => {
                        this.handoff.cancel();
                        this.terminal.live.status = error;
                    }
                }
            });
        })
        .detach();
        cx.notify();
    }
}

pub(crate) fn paste_error(error: crate::Error) -> String {
    match error {
        crate::Error::Ssh(message) => message,
        other => other.to_string(),
    }
}
