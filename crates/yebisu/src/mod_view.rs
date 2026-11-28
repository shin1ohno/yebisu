//! Option E (#238), E-1 spike: draw what `yebisu-mod-renderer` adopted into a
//! reserved layout region and route taps and keys back as event target IDs.
//!
//! The iPad never runs mod code. A runner on the host publishes element
//! descriptions; `CopyReceiver` (yebisu-modhost) validates and adopts them;
//! `scene` turns the adopted root view into the closed set of boxes this
//! prototype draws; `draw` makes GPUI elements of it; `events` queues the
//! taps and keys the runner receives back. Decoding runs on a background task
//! and the UI thread only swaps in the latest scene (AGENTS.md mailbox rule).
mod draw;
mod events;
mod fixtures;
mod scene;
mod tables;

pub(crate) use draw::{Hit, Route, slot};
pub(crate) use events::{EventKind, EventQueue, ModEvent};
pub(crate) use fixtures::Fixture;
pub(crate) use scene::{Kind, Scene, Style};
pub(crate) use tables::tables;

use std::sync::Arc;
use yebisu_mod_wire as wire;

/// Why a publication cannot be drawn, or a fixture cannot be built.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub(crate) enum Error {
    #[error("renderer refused the publication: {0}")]
    Renderer(#[from] yebisu_mod_renderer::Error),
    #[error("wire: {0}")]
    Wire(#[from] wire::Error),
    #[error("the publication has no view")]
    NoView,
    #[error("component {0:?} is not drawn by this prototype")]
    UnsupportedComponent(wire::Component),
    #[error("op {0} is not drawn by this prototype")]
    UnsupportedOp(String),
    #[error("style {0} needs exactly one argument of its kind")]
    StyleArgument(String),
    #[error("unknown mod fixture {0}")]
    UnknownFixture(String),
}
pub(crate) type Result<T> = std::result::Result<T, Error>;

/// What the shell keeps for one mod region.
#[derive(Debug, Default)]
pub(crate) struct ModViewState {
    /// The latest adopted scene; a newer one replaces it whole.
    pub(crate) scene: Option<Arc<Scene>>,
    /// Why the latest publication was refused; the region then stays empty.
    pub(crate) error: Option<Error>,
    pub(crate) events: EventQueue,
    /// The node that receives keys: the last tapped node with a key callback.
    pub(crate) focused: Option<usize>,
    /// The line is down: the region is drawn dimmed and takes no input.
    pub(crate) faded: bool,
    /// The QA fixture request was read (once per process).
    pub(crate) requested: bool,
    /// The region's focus: keys reach the focused node only while it holds it.
    pub(crate) focus: Option<gpui::FocusHandle>,
}

impl ModViewState {
    /// Takes the result of a background decode. A refusal clears the scene so
    /// a half-understood publication is never drawn.
    pub(crate) fn install(&mut self, result: Result<Scene>) {
        match result {
            Ok(scene) => {
                // A new generation's node indices mean other nodes.
                if self.scene.as_ref().map(|s| s.generation) != Some(scene.generation) {
                    self.focused = None;
                }
                self.scene = Some(Arc::new(scene));
                self.error = None;
            }
            Err(e) => {
                self.scene = None;
                self.focused = None;
                self.error = Some(e);
            }
        }
    }

    /// Routes one hit from the drawn region. Returns whether an event was
    /// queued for the runner.
    pub(crate) fn hit(&mut self, hit: Hit) -> bool {
        if self.faded {
            return false;
        }
        let Some(scene) = self.scene.clone() else {
            return false;
        };
        match hit {
            Hit::Tap(index) => {
                let Some(node) = scene.node(index) else {
                    return false;
                };
                if node.key.is_some() {
                    self.focused = Some(index);
                }
                let Some(callback) = node.click else {
                    return false;
                };
                self.events.push(ModEvent::new(
                    &scene,
                    index,
                    callback,
                    EventKind::Click,
                    None,
                ))
            }
            Hit::Key(key) => {
                let Some(index) = self.focused else {
                    return false;
                };
                let Some(callback) = scene.node(index).and_then(|n| n.key) else {
                    return false;
                };
                self.events.push(ModEvent::new(
                    &scene,
                    index,
                    callback,
                    EventKind::Key,
                    Some(key),
                ))
            }
        }
    }
}

#[cfg(test)]
#[path = "mod_view/scene_tests.rs"]
mod scene_tests;

#[cfg(test)]
#[path = "mod_view/events_tests.rs"]
mod events_tests;

#[cfg(test)]
#[path = "mod_view/draw_tests.rs"]
mod draw_tests;

#[cfg(test)]
#[path = "mod_view/bench_tests.rs"]
mod bench_tests;
