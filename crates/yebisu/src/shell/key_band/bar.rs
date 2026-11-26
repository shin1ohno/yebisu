//! The keys on the software keyboard's own shortcuts bar (#229), through
//! herdr-uikit's `set_keyboard_bar`, and where the keys show in each
//! keyboard state.
//!
//! UIKit shows the bar's items on a docked keyboard and not on a floating
//! one (measured on the iPadOS 27.2 Simulator; herdr-uikit
//! `docs/keyboard-bar.md`). So a floating keyboard gets the in-app band
//! (`render_band`) with the same keys. A hardware keyboard alone shows
//! neither: the bar is emptied, so its minimized bar has no yebisu keys.

use super::keys::{self, Key};
use gpui::Modifiers;

/// Where the software keyboard is: herdr-uikit's `KeyboardPlacement`, as
/// a host type the tests can set.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Placement {
    #[default]
    Hidden,
    Docked,
    Undocked,
    Floating,
}

/// Where the terminal keys show now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Surface {
    /// No software keyboard, a hardware one, or a native field owns the
    /// keyboard: no keys anywhere, and a latch is spent.
    None,
    /// The keyboard's own shortcuts bar.
    Bar,
    /// The in-app band, as the keyboard shows no bar (floating).
    Band,
}

/// The surface for a keyboard at `placement` whose bottom inset says a
/// software keyboard is `docked` (`fit::key_band_height`'s test), with a
/// `hardware` keyboard attached or a `native_field` owning the keyboard.
pub(crate) fn surface(
    placement: Placement,
    docked: bool,
    hardware: bool,
    native_field: bool,
) -> Surface {
    if hardware || native_field {
        return Surface::None;
    }
    match placement {
        Placement::Floating => Surface::Band,
        Placement::Docked | Placement::Undocked => Surface::Bar,
        // The inset can be up before the first frame notification is read.
        Placement::Hidden if docked => Surface::Bar,
        Placement::Hidden => Surface::None,
    }
}

/// One bar button: its key, item id and latch state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Button {
    pub(crate) id: u32,
    pub(crate) key: &'static Key,
    pub(crate) selected: bool,
}

/// The bar's buttons in herdr-uikit's shape: groups of buttons, then the …
/// menu's keys. Empty when the keys must not show.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Model {
    pub(crate) leading: Vec<Vec<Button>>,
    pub(crate) trailing: Vec<Vec<Button>>,
    pub(crate) menu: Vec<Button>,
}

/// The bar for a software keyboard (`show`), with `latch` drawn selected.
pub(crate) fn model(show: bool, latch: Modifiers) -> Model {
    if !show {
        return Model::default();
    }
    let button = |index: usize| {
        keys::ALL.get(index).map(|key| Button {
            id: keys::id(index),
            key,
            selected: match key.action {
                keys::Action::Control => latch.control,
                keys::Action::Alt => latch.alt,
                keys::Action::Command => latch.platform,
                _ => false,
            },
        })
    };
    let groups = |groups: &[&[usize]]| {
        groups
            .iter()
            .map(|group| group.iter().filter_map(|&index| button(index)).collect())
            .collect()
    };
    Model {
        leading: groups(&keys::LEADING),
        trailing: groups(&keys::TRAILING),
        menu: keys::MENU
            .iter()
            .filter_map(|&index| button(index))
            .collect(),
    }
}

/// The bar button's QA identifier.
pub(crate) fn bar_id(key: &Key) -> String {
    format!("key-bar-{}", key.name)
}

#[cfg(target_os = "ios")]
mod uikit {
    use super::{Button, Model, bar_id, keys};
    use herdr_uikit::{KeyboardBar, KeyboardBarItem, KeyboardBarMenuEntry};

    fn item(button: &Button) -> KeyboardBarItem {
        KeyboardBarItem {
            id: button.id,
            title: button.key.label.into(),
            symbol: button.key.symbol.map(Into::into),
            accessibility_label: button.key.accessibility_label.into(),
            identifier: bar_id(button.key),
            selected: button.selected,
            menu: Vec::new(),
        }
    }

    pub(crate) fn bar(model: &Model) -> KeyboardBar {
        let groups = |groups: &[Vec<Button>]| -> Vec<Vec<KeyboardBarItem>> {
            groups
                .iter()
                .map(|group| group.iter().map(item).collect())
                .collect()
        };
        let mut trailing = groups(&model.trailing);
        if !model.menu.is_empty() {
            let more = KeyboardBarItem {
                id: u32::MAX,
                title: "…".into(),
                symbol: Some(keys::MORE_SYMBOL.into()),
                accessibility_label: keys::MORE_LABEL.into(),
                identifier: format!("key-bar-{}", keys::MORE_NAME),
                selected: false,
                menu: model
                    .menu
                    .iter()
                    .map(|button| KeyboardBarMenuEntry {
                        id: button.id,
                        title: button.key.accessibility_label.into(),
                        symbol: button.key.symbol.map(Into::into),
                        identifier: bar_id(button.key),
                    })
                    .collect(),
            };
            trailing.push(vec![more]);
        }
        KeyboardBar {
            leading: groups(&model.leading),
            trailing,
        }
    }
}

#[cfg(target_os = "ios")]
pub(crate) use uikit::bar as uikit_bar;
