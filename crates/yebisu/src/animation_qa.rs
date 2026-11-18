//! Simulator QA: name the Core Animation animations that keep XCTest from going idle.
//!
//! XCTest waits for the app's animations to finish before every query and gives up
//! after 60 s ("App animations complete notification not received"). One P9 smoke run
//! sat in that wait until the CI job timed out, so the animation's owner was never
//! recorded. While the P9 QA build runs, `sample` is called once a second from the UI
//! thread and writes one `animations-running` record into the observation file when an
//! animation has stayed on any layer of any window for `STUCK_AFTER`: each layer's
//! window, owning view and animations. When that stretch ends it writes
//! `animations-ended` with how long it lasted, so a record without one means the
//! animation never stopped on its own while the process ran.
use std::time::{Duration, Instant};

/// XCTest's own idle waits take 3 s to 11 s in a healthy run; past this the app is stuck.
pub(crate) const STUCK_AFTER: Duration = Duration::from_secs(10);
/// One record per stuck stretch, and never more than this many per process.
const MAX_RECORDS: usize = 8;

/// What one sample means for the record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Event {
    /// Nothing to write.
    Quiet,
    /// Animations have run for `STUCK_AFTER`: write `animations-running`.
    Stuck,
    /// A stretch that was written ended after this long: write `animations-ended`.
    Ended(Duration),
}

pub(crate) struct Watch {
    since: Option<Instant>,
    recorded: bool,
    records: usize,
}

impl Watch {
    pub(crate) const fn new() -> Self {
        Self {
            since: None,
            recorded: false,
            records: 0,
        }
    }

    /// Feeds one sample. Returns `Stuck` exactly once for each continuous stretch in
    /// which animations were running for at least `STUCK_AFTER`, and `Ended` with the
    /// stretch's length at the first sample without running animations after it; a
    /// stretch that was not recorded ends quietly.
    pub(crate) fn observe(&mut self, now: Instant, running: bool) -> Event {
        if !running {
            let ended = match (self.recorded, self.since) {
                (true, Some(since)) => Event::Ended(now.saturating_duration_since(since)),
                _ => Event::Quiet,
            };
            self.since = None;
            self.recorded = false;
            return ended;
        }
        let since = *self.since.get_or_insert(now);
        if self.recorded
            || self.records >= MAX_RECORDS
            || now.saturating_duration_since(since) < STUCK_AFTER
        {
            return Event::Quiet;
        }
        self.recorded = true;
        self.records += 1;
        Event::Stuck
    }
}

#[cfg(all(target_os = "ios", target_abi = "sim"))]
mod native {
    use super::{Event, STUCK_AFTER, Watch};
    use crate::observation::Observation;
    use objc2::{class, msg_send, rc::Retained, runtime::AnyObject};
    use objc2_foundation::{NSArray, NSString};
    use std::{sync::Mutex, time::Instant};

    static WATCH: Mutex<Watch> = Mutex::new(Watch::new());
    const MAX_DEPTH: usize = 24;
    const MAX_LAYERS: usize = 32;

    /// Call from the UI thread. Does nothing outside the P9 QA build.
    pub(crate) fn sample(observation: &Observation) {
        if std::env::var_os("YEBISU_CORE_QA").is_none() {
            return;
        }
        let layers = running_animations();
        let Ok(mut watch) = WATCH.lock() else {
            return;
        };
        match watch.observe(Instant::now(), !layers.is_empty()) {
            Event::Quiet => {}
            Event::Stuck => observation.record(serde_json::json!({
                "kind": "animations-running",
                "after_seconds": STUCK_AFTER.as_secs(),
                "layers": layers,
            })),
            Event::Ended(lasted) => observation.record(serde_json::json!({
                "kind": "animations-ended",
                "seconds": lasted.as_secs_f64(),
            })),
        }
    }

    // The class name of an object, or "nil".
    #[allow(
        unsafe_code,
        reason = "Simulator QA reads the class of a UIKit object on the main thread"
    )]
    fn class_name(object: *mut AnyObject) -> String {
        // SAFETY: a non-null Objective-C object pointer returned by UIKit.
        unsafe { object.as_ref() }.map_or_else(
            || "nil".to_owned(),
            |object| object.class().name().to_string_lossy().into_owned(),
        )
    }

    // Layers that carry at least one animation: their window, owning view (the layer's
    // delegate), class, animation keys, and each animation's class, duration and repeat
    // count (an infinite repeat count, or a duration far past STUCK_AFTER, is a loop).
    #[allow(
        unsafe_code,
        reason = "Simulator QA reads public CALayer properties on the main thread"
    )]
    fn running_animations() -> Vec<serde_json::Value> {
        // SAFETY: public CALayer methods with their declared signatures, called on the main thread.
        unsafe fn walk(
            window: &str,
            layer: &AnyObject,
            depth: usize,
            found: &mut Vec<serde_json::Value>,
        ) {
            unsafe {
                let keys: Option<Retained<NSArray<NSString>>> = msg_send![layer, animationKeys];
                if let Some(keys) = keys
                    && keys.count() > 0
                    && found.len() < MAX_LAYERS
                {
                    let mut animations = Vec::new();
                    for key in keys.iter() {
                        let animation: *mut AnyObject = msg_send![layer, animationForKey: &*key];
                        let Some(animation) = animation.as_ref() else {
                            continue;
                        };
                        let duration: f64 = msg_send![animation, duration];
                        let repeat: f32 = msg_send![animation, repeatCount];
                        animations.push(serde_json::json!({
                            "key": key.to_string(),
                            "class": animation.class().name().to_string_lossy(),
                            "duration": duration,
                            "repeat_count": if repeat.is_finite() { serde_json::json!(repeat) } else { serde_json::json!("inf") },
                        }));
                    }
                    let delegate: *mut AnyObject = msg_send![layer, delegate];
                    found.push(serde_json::json!({
                        "window": window,
                        "view": class_name(delegate),
                        "class": layer.class().name().to_string_lossy(),
                        "keys": keys.iter().map(|key| key.to_string()).collect::<Vec<_>>(),
                        "animations": animations,
                    }));
                }
                if depth >= MAX_DEPTH {
                    return;
                }
                let children: Option<Retained<NSArray<AnyObject>>> = msg_send![layer, sublayers];
                if let Some(children) = children {
                    for child in children.iter() {
                        walk(window, &child, depth + 1, found);
                    }
                }
            }
        }
        let mut found = Vec::new();
        // SAFETY: public UIKit methods with their declared signatures, called from a UI callback.
        unsafe {
            let app: *mut AnyObject = msg_send![class!(UIApplication), sharedApplication];
            let windows: Retained<NSArray<AnyObject>> = msg_send![app, windows];
            for window in windows.iter() {
                let name = window.class().name().to_string_lossy().into_owned();
                let layer: Retained<AnyObject> = msg_send![&*window, layer];
                walk(&name, &layer, 0, &mut found);
            }
        }
        found
    }
}

#[cfg(all(target_os = "ios", target_abi = "sim"))]
pub(crate) use native::sample;

#[cfg(test)]
mod tests;
