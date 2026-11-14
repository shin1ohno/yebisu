//! Provisional SDK11 consumer. Source-only until independent/native admission.
//! Native invocation runs without a GPUI/entity borrow on the main dispatch queue.
pub(crate) mod capture;
mod schema;
use crate::observation::Observation;
use gpui::AnyWindowHandle;
use herdr_uikit::{NativeEditAvailability, native_edit_observer as sdk};
use std::sync::mpsc::{Receiver, TryRecvError, sync_channel};

#[derive(Default)]
pub(crate) struct Harness {
    run: Option<sdk::ProcessRun>,
    pending: Option<Receiver<bool>>,
    pub(crate) failed: bool,
    pub(crate) phase: u32,
    pub(crate) end_sent: bool,
}
impl Harness {
    pub(crate) fn complete(&mut self) -> bool {
        let Some(receiver) = &self.pending else {
            return !self.failed;
        };
        match receiver.try_recv() {
            Ok(true) => {
                self.pending = None;
                !self.failed
            }
            Ok(false) | Err(TryRecvError::Disconnected) => {
                self.fail();
                false
            }
            Err(TryRecvError::Empty) => false,
        }
    }
    pub(crate) fn fail(&mut self) {
        self.failed = true;
        if let Some(run) = self.run {
            let _ = sdk::invalidate_run(run);
        }
    }
    pub(crate) fn start(
        &mut self,
        window: AnyWindowHandle,
        observation: &Observation,
        action: sdk::Action,
        origin: sdk::ResponderOrigin,
        policy: Option<NativeEditAvailability>,
        restore: Option<NativeEditAvailability>,
    ) {
        if self.pending.is_some() || self.failed {
            self.fail();
            return;
        }
        let run = match self.run.map(Ok).unwrap_or_else(sdk::begin_run) {
            Ok(run) => run,
            Err(_) => {
                self.fail();
                return;
            }
        };
        self.run = Some(run);
        let arm = match sdk::arm(window, run, sdk::Phase(self.phase)) {
            Ok(arm) => arm,
            Err(_) => {
                self.fail();
                return;
            }
        };
        let disabled = std::env::var_os("YEBISU_P11_OBSERVER_DISABLED").is_some();
        if disabled && sdk::disable(run).is_err() {
            self.fail();
            return;
        }
        let manifest = serde_json::json!({"kind":"p11-arm", "arm":schema::arm(arm),
            "action":format!("{action:?}"), "responder":format!("{origin:?}"),
            "policy":policy.map(|p|[p.copy,p.cut,p.paste,p.select_all]), "disabled":disabled});
        if !observation.try_record(manifest) {
            self.fail();
            return;
        }
        let (sender, receiver) = sync_channel(1);
        self.pending = Some(receiver);
        let observation = observation.clone();
        dispatch2::DispatchQueue::main().exec_async(move || {
            // No mutable app/window/entity borrow survives into this native call.
            let installed = herdr_uikit::set_native_edit_availability(window, policy);
            let invoked = installed && sdk::invoke_native_selector(window, action, origin).is_ok();
            let restored = herdr_uikit::set_native_edit_availability(window, restore);
            let mut valid = invoked && restored;
            match sdk::drain(window, arm) {
                Ok(batch) => {
                    valid &= batch.len == if disabled { 0 } else { 2 };
                    valid &=
                        schema::healthy(batch.status) && batch.status.queued as usize == batch.len;
                    for record in batch.records.into_iter().take(batch.len).flatten() {
                        valid &= observation.try_record(schema::record(record));
                    }
                    valid &= observation.try_record(schema::status("p11-drain", batch.status));
                }
                Err(_) => valid = false,
            }
            match sdk::run_status(run) {
                Ok(status) => {
                    valid &= schema::healthy(status) && status.queued == 0;
                    valid &= observation.try_record(schema::status("p11-status", status));
                }
                Err(_) => valid = false,
            }
            if !valid {
                let _ = sdk::invalidate_run(run);
            }
            if sender.try_send(valid).is_err() {
                let _ = sdk::invalidate_run(run);
            }
        });
    }
    pub(crate) fn finish(&mut self, observation: &Observation) {
        if self.pending.is_some() || self.failed {
            self.fail();
            return;
        }
        if let Some(run) = self.run {
            match sdk::disable(run) {
                Ok(status) if schema::healthy(status) && status.queued == 0 => {
                    if !observation.try_record(schema::status("p11-disabled", status)) {
                        self.fail();
                    }
                }
                _ => self.fail(),
            }
        }
    }
}

/// Called only inside actual KeyDown/Up handlers, never from scheduled work.
pub(crate) fn callback(observation: &Observation, route: &'static str, edge: &'static str) {
    if let Some(key) = sdk::current_native_observation_key()
        && !observation.try_record(serde_json::json!({"kind":"p11-consumer", "key":schema::key(key), "route":route, "edge":edge})) {
        let _ = sdk::invalidate_run(key.arm.run);
    }
}
