//! #238 E-1 spike: the adopted mod scene in M2, the right dock, at the
//! rectangle L0 computed for it. Owner: the E-1 stream.
//!
//! Decoding and adoption run on a background task; the UI thread installs
//! the latest scene and draws it. Taps and keys go to `ModViewState::hit`,
//! which queues the runner's events.
use super::disconnect::DisconnectFade;
use crate::{
    app::Smoke,
    layout::{M2_RANGE, ModSlot, Rect},
    mod_view::{self, Fixture},
};
use gpui::{AnyElement, Context, div, prelude::*, px};
use std::rc::Rc;

/// QA: the fixture publication a running app draws in M2 (`counter`,
/// `status`, `form`, `nested`). There is no line to a runner yet.
pub(crate) const FIXTURE_ENV: &str = "YEBISU_MOD_FIXTURE";

impl Smoke {
    /// Reads the QA fixture request once per process.
    pub(crate) fn mod_view_tick(&mut self, cx: &mut Context<Self>) {
        if self.shell.mod_view.requested {
            return;
        }
        self.shell.mod_view.requested = true;
        if let Ok(name) = std::env::var(FIXTURE_ENV) {
            self.load_mod_fixture(name, cx);
        }
    }

    /// Adopts fixture `name` off the UI thread, then installs it.
    pub(crate) fn load_mod_fixture(&mut self, name: String, cx: &mut Context<Self>) {
        let adopted =
            cx.background_spawn(async move { Fixture::from_name(&name).and_then(Fixture::adopt) });
        cx.spawn(async move |weak, cx| {
            let result = adopted.await;
            // The view may be gone; nothing to draw then.
            weak.update(cx, |this, cx| {
                this.install_mod_scene(result);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// The latest scene replaces the previous one. While a scene is drawn the
    /// region asks for M2 at its default width; a refusal leaves it empty.
    pub(crate) fn install_mod_scene(&mut self, result: mod_view::Result<mod_view::Scene>) {
        let drawn = result.is_ok();
        self.shell.mod_view.install(result);
        self.shell.fit.mod_regions.m2 = if drawn {
            ModSlot::Wants(M2_RANGE.default)
        } else {
            ModSlot::Empty
        };
    }

    /// The scene at `rect` (L0's M2), or `None` without a region or scene.
    pub(crate) fn render_mod_dock(
        &mut self,
        rect: Option<Rect>,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let rect = rect?;
        let scene = self.shell.mod_view.scene.clone()?;
        let faded = self.shell.fit.disconnect.fade(self.fit_now()) != DisconnectFade::Visible;
        self.shell.mod_view.faded = faded;
        let focus = self
            .shell
            .mod_view
            .focus
            .get_or_insert_with(|| cx.focus_handle())
            .clone();
        let weak = cx.entity().downgrade();
        let route: mod_view::Route = Rc::new(move |hit, _, cx| {
            weak.update(cx, |this, cx| {
                if this.shell.mod_view.hit(hit) {
                    cx.notify();
                }
            })
            .ok();
        });
        Some(
            div()
                .id("shell-mod-dock")
                .debug_selector(|| "shell-mod-dock".into())
                .absolute()
                .left(px(rect.x))
                .top(px(rect.y))
                .w(px(rect.width))
                .h(px(rect.height))
                .child(mod_view::slot(&scene, faded, &focus, route))
                .into_any_element(),
        )
    }
}

#[cfg(test)]
#[path = "mod_dock_tests.rs"]
mod tests;
