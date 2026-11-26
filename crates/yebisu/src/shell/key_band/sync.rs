//! Each frame: where the terminal keys show, and the keyboard bar's buttons
//! (#229). The bar's presses arrive on herdr-uikit's stream and press the
//! same keys as the in-app band.

use super::{
    Action,
    bar::{self, Surface},
    keys,
};
use crate::{app::Smoke, layout::Size};
use gpui::{Context, Edges, Modifiers, Pixels, Window};

/// What the last frame put on the keyboard bar, and the press reader.
#[derive(Debug, Default)]
pub(crate) struct BarSync {
    last: Option<bar::Model>,
    #[cfg(target_os = "ios")]
    reader: Option<gpui::Task<()>>,
}

impl Smoke {
    /// The keys' surface for a window of `size` with keyboard insets `ime`.
    pub(in crate::shell) fn key_surface(&self, size: Size, ime: &Edges<Pixels>) -> Surface {
        bar::surface(
            self.shell.fit.keyboard_placement,
            crate::shell::fit::software_keyboard_docked(size, ime),
            self.hardware_keyboard_attached(),
            self.native_field_open(),
        )
    }

    pub(crate) fn key_surface_now(&self, window: &Window) -> Surface {
        let viewport = window.viewport_size();
        let size = Size::new(viewport.width.into(), viewport.height.into());
        self.key_surface(size, &self.platform_insets(window).ime)
    }

    /// The latch the band and the bar draw selected.
    fn drawn_latch(&self) -> Modifiers {
        let destination = self.terminal.input.destination.as_ref();
        let state = &self.shell.frame.key_band;
        Modifiers {
            control: state.pressed(Action::Control, destination),
            alt: state.pressed(Action::Alt, destination),
            platform: state.pressed(Action::Command, destination),
            ..Modifiers::default()
        }
    }

    /// Reads where the keyboard is and puts the keys on its bar. Runs at
    /// the start of every frame; herdr-uikit draws a frame when the
    /// keyboard's placement changes, and the bar only changes with a latch
    /// or a hardware keyboard.
    pub(crate) fn sync_keyboard_keys(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        #[cfg(target_os = "ios")]
        {
            self.shell.fit.keyboard_placement =
                placement(herdr_uikit::keyboard_placement(window.window_handle()));
            if self.shell.frame.key_bar.reader.is_none() {
                self.shell.frame.key_bar.reader = start_reader(window, cx);
            }
        }
        // A hardware keyboard alone shows no bar keys. A native field (the
        // search panel's, the rename field) has a bar of its own.
        let model = bar::model(!self.hardware_keyboard_attached(), self.drawn_latch());
        if self.shell.frame.key_bar.last.as_ref() == Some(&model) {
            return;
        }
        #[cfg(target_os = "ios")]
        herdr_uikit::set_keyboard_bar(window.window_handle(), &bar::uikit_bar(&model));
        #[cfg(not(target_os = "ios"))]
        let _ = (&window, &cx);
        self.shell.frame.key_bar.last = Some(model);
    }

    /// A press of the keyboard bar's item `id`.
    pub(crate) fn press_bar_key(&mut self, id: u32, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(key) = keys::by_id(id) {
            self.press_key_band(key.action, window, cx);
        }
    }

    /// The bar's last buttons (tests and qa.state).
    pub(crate) fn keyboard_bar_model(&self) -> Option<&bar::Model> {
        self.shell.frame.key_bar.last.as_ref()
    }

    /// qa.state's `key_bar`: where the keys show, the keyboard's placement,
    /// the drawn latch and the bar's button count.
    #[cfg(any(all(target_os = "ios", target_abi = "sim"), test))]
    pub(crate) fn qa_key_bar_field(&self, window: &Window) -> serde_json::Value {
        let surface = match self.key_surface_now(window) {
            Surface::None => "none",
            Surface::Bar => "bar",
            Surface::Band => "band",
        };
        let placement = match self.shell.fit.keyboard_placement {
            bar::Placement::Hidden => "hidden",
            bar::Placement::Docked => "docked",
            bar::Placement::Undocked => "undocked",
            bar::Placement::Floating => "floating",
        };
        let drawn = self.drawn_latch();
        let latch: Vec<&str> = [
            (drawn.control, "ctrl"),
            (drawn.alt, "alt"),
            (drawn.platform, "cmd"),
        ]
        .into_iter()
        .filter_map(|(on, name)| on.then_some(name))
        .collect();
        let items = self.keyboard_bar_model().map_or(0, |model| {
            model
                .leading
                .iter()
                .chain(&model.trailing)
                .flatten()
                .count()
                + model.menu.len()
        });
        #[cfg(all(target_os = "ios", target_abi = "sim"))]
        let sample = herdr_uikit::keyboard_placement_sample(window.window_handle());
        #[cfg(not(all(target_os = "ios", target_abi = "sim")))]
        let sample: Option<String> = None;
        serde_json::json!({
            "surface": surface,
            "placement": placement,
            "latch": latch,
            "items": items,
            "sample": sample,
        })
    }
}

#[cfg(target_os = "ios")]
fn placement(placement: herdr_uikit::KeyboardPlacement) -> bar::Placement {
    use herdr_uikit::KeyboardPlacement as P;
    match placement {
        P::Hidden => bar::Placement::Hidden,
        P::Docked => bar::Placement::Docked,
        P::Undocked => bar::Placement::Undocked,
        P::Floating => bar::Placement::Floating,
    }
}

/// Reads the window's bar presses until the window closes.
#[cfg(target_os = "ios")]
fn start_reader(window: &Window, cx: &mut Context<Smoke>) -> Option<gpui::Task<()>> {
    use futures::StreamExt as _;
    let mut presses = herdr_uikit::keyboard_bar_presses(window.window_handle())?;
    Some(cx.spawn_in(window, async move |this, cx| {
        while let Some(id) = presses.next().await {
            if this
                .update_in(cx, |app, window, cx| app.press_bar_key(id, window, cx))
                .is_err()
            {
                break;
            }
        }
    }))
}
