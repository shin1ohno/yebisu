//! Explicit fixed-field serialization; no text/clipboard/host/error payloads.
use herdr_uikit::native_edit_observer as sdk;
use serde_json::{Value, json};
pub(super) fn arm(a: sdk::Arm) -> Value {
    json!({"run":format!("{:032x}",a.run.0),"epoch":a.epoch,"view":a.view_generation,"phase":a.phase.0})
}
pub(super) fn key(k: sdk::ObservationKey) -> Value {
    json!({"arm":arm(k.arm),"invocation":k.invocation})
}
pub(super) fn healthy(s: sdk::Status) -> bool {
    s.schema_version == 1
        && !s.sticky_invalid
        && s.invalid_reasons == 0
        && s.dropped_total == 0
        && s.outstanding == 0
        && s.reserved == 0
}
pub(super) fn status(kind: &'static str, s: sdk::Status) -> Value {
    json!({"kind":kind,"schema":s.schema_version,"run":format!("{:032x}",s.run.0),"epoch":s.latest_epoch,"enabled":s.enabled,
        "dropped":s.dropped_total,"invalid":s.invalid_reasons,"sticky":s.sticky_invalid,"outstanding":s.outstanding,"reserved":s.reserved,"queued":s.queued,"sequence":s.snapshot_sequence})
}
pub(super) fn record(r: sdk::Record) -> Value {
    let (event, handled, fallback) = match r.event {
        sdk::Event::Entry => ("Entry", None, None),
        sdk::Event::Outcome(sdk::Outcome::Refused) => ("Refused", None, None),
        sdk::Event::Outcome(sdk::Outcome::Dispatched {
            key_down_handled,
            fallback,
        }) => (
            "Dispatched",
            Some(key_down_handled),
            Some(format!("{fallback:?}")),
        ),
        sdk::Event::Outcome(sdk::Outcome::Aborted) => ("Aborted", None, None),
        sdk::Event::Outcome(sdk::Outcome::Invalidated) => ("Invalidated", None, None),
    };
    json!({"kind":"p11-sdk","schema":r.schema_version,"key":key(r.key),"parent":r.parent.map(key),"action":format!("{:?}",r.action),
        "responder":format!("{:?}",r.responder),"decision":r.decision.map(|d|format!("{d:?}")),"event":event,"handled":handled,"fallback":fallback})
}
