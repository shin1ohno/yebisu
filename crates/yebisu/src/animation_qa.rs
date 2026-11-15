//! Simulator QA: name the Core Animation animations that keep XCTest from going idle.
//!
//! XCTest waits for the app's animations to finish before every query and gives up
//! after 60 s ("App animations complete notification not received"). One P9 smoke run
//! sat in that wait until the CI job timed out, so the animation's owner was never
//! recorded. While the P9 QA build runs, `sample` is called once a second from the UI
//! thread and writes one `animations-running` record into the observation file when an
//! animation has stayed on any layer of any window for `STUCK_AFTER`.
use std::time::{Duration, Instant};

/// XCTest's own idle waits take 3 s to 11 s in a healthy run; past this the app is stuck.
pub(crate) const STUCK_AFTER: Duration = Duration::from_secs(10);
/// One record per stuck stretch, and never more than this many per process.
const MAX_RECORDS: usize = 8;

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

    /// Feeds one sample. Returns true exactly once for each continuous stretch in which
    /// animations were running for at least `STUCK_AFTER`; a sample without running
    /// animations ends the stretch.
    pub(crate) fn observe(&mut self, now: Instant, running: bool) -> bool {
        if !running {
            self.since = None;
            self.recorded = false;
            return false;
        }
        let since = *self.since.get_or_insert(now);
        if self.recorded
            || self.records >= MAX_RECORDS
            || now.saturating_duration_since(since) < STUCK_AFTER
        {
            return false;
        }
        self.recorded = true;
        self.records += 1;
        true
    }
}

#[cfg(all(target_os = "ios", target_abi = "sim"))]
mod native {
    use super::{STUCK_AFTER, Watch};
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
        if watch.observe(Instant::now(), !layers.is_empty()) {
            observation.record(serde_json::json!({
                "kind": "animations-running",
                "after_seconds": STUCK_AFTER.as_secs(),
                "layers": layers,
            }));
        }
    }

    // Layers that carry at least one animation: their class and animation keys.
    #[allow(
        unsafe_code,
        reason = "Simulator QA reads public CALayer properties on the main thread"
    )]
    fn running_animations() -> Vec<serde_json::Value> {
        // SAFETY: public CALayer methods with their declared signatures, called on the main thread.
        unsafe fn walk(layer: &AnyObject, depth: usize, found: &mut Vec<serde_json::Value>) {
            unsafe {
                let keys: Option<Retained<NSArray<NSString>>> = msg_send![layer, animationKeys];
                if let Some(keys) = keys {
                    let keys: Vec<String> = keys.iter().map(|key| key.to_string()).collect();
                    if !keys.is_empty() && found.len() < MAX_LAYERS {
                        found.push(serde_json::json!({
                            "class": layer.class().name().to_string_lossy(),
                            "keys": keys,
                        }));
                    }
                }
                if depth >= MAX_DEPTH {
                    return;
                }
                let children: Option<Retained<NSArray<AnyObject>>> = msg_send![layer, sublayers];
                if let Some(children) = children {
                    for child in children.iter() {
                        walk(&child, depth + 1, found);
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
                let layer: Retained<AnyObject> = msg_send![&*window, layer];
                walk(&layer, 0, &mut found);
            }
        }
        found
    }
}

#[cfg(all(target_os = "ios", target_abi = "sim"))]
pub(crate) use native::sample;

#[cfg(test)]
mod tests;
