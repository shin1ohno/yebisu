//! The terminal keys over the software keyboard (#229). The keyboard's own
//! shortcuts bar shows them (`bar`); a keyboard without one (a floating
//! keyboard) gets the in-app band drawn here instead (canvas v20, R33).

use crate::{
    app::Smoke,
    shell::style::{color, weight},
    terminal::HeldKeys,
    terminal_input::{Destination, GeneratedInput},
};
use gpui::{
    AnyElement, AvailableSpace, Context, Div, KeyDownEvent, Keystroke, Modifiers, MouseButton,
    PathBuilder, Role, Stateful, Window, canvas, div, point, prelude::*, px, rgb,
};

pub(crate) mod bar;
#[cfg(not(test))]
mod hardware;
pub(crate) mod keys;
pub(crate) mod sync;
#[cfg(not(test))]
pub(super) use hardware::hardware_keyboard_attached;
pub(crate) use keys::Action;
use keys::Key;

// KeyBand.dc.html:188–206. Compact.dc.html's key row uses the same controls.
pub(super) const HEIGHT: f32 = 76.0;
/// The pager-only band above a docked keyboard: the pager's 44 pt touch
/// row and 4 pt above and below.
pub(super) const PAGER_HEIGHT: f32 = 52.0;
pub(crate) const ROW_HEIGHT: f32 = 56.0;
const KEY_HEIGHT: f32 = 44.0;
const WIDE_KEY: f32 = 52.0;
const ARROW_KEY: f32 = 48.0;
const GROUP_RADIUS: f32 = 14.0;
const KEY_RADIUS: f32 = 10.0;
const KEY_TEXT: f32 = 12.0;
const ROW_GAP: f32 = 8.0;
const PAGER_MIN_WIDTH: f32 = 160.0;

#[derive(Debug, Default)]
pub(crate) struct State {
    destination: Option<Destination>,
    modifiers: Modifiers,
}

impl State {
    #[cfg(any(target_os = "ios", test))]
    pub(super) fn is_armed(&self) -> bool {
        self.modifiers.control || self.modifiers.alt || self.modifiers.platform
    }

    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }

    fn toggle(&mut self, action: Action, destination: Destination) {
        if self.destination.as_ref() != Some(&destination) {
            self.clear();
        }
        self.destination = Some(destination);
        match action {
            Action::Control => self.modifiers.control = !self.modifiers.control,
            Action::Alt => self.modifiers.alt = !self.modifiers.alt,
            Action::Command => self.modifiers.platform = !self.modifiers.platform,
            _ => {}
        }
    }

    fn pressed(&self, action: Action, destination: Option<&Destination>) -> bool {
        self.destination.as_ref() == destination
            && match action {
                Action::Control => self.modifiers.control,
                Action::Alt => self.modifiers.alt,
                Action::Command => self.modifiers.platform,
                _ => false,
            }
    }

    fn latched(&self, destination: &Destination) -> Modifiers {
        if self.destination.as_ref() == Some(destination) {
            self.modifiers
        } else {
            Modifiers::default()
        }
    }

    fn take(&mut self, destination: &Destination) -> Modifiers {
        let previous = std::mem::take(self);
        if previous.destination.as_ref() == Some(destination) {
            previous.modifiers
        } else {
            Modifiers::default()
        }
    }
}

/// Compact layout embeds the same keys in one horizontally scrollable row.
pub(crate) fn render_key_row(
    app: &Smoke,
    pager: Option<AnyElement>,
    cx: &mut Context<Smoke>,
) -> AnyElement {
    let middle = pager.map(|pager| div().flex_1().min_w(px(PAGER_MIN_WIDTH)).child(pager));
    div()
        .id("shell-key-band-row")
        .w_full()
        .min_w_0()
        .h(px(ROW_HEIGHT))
        .flex_none()
        .flex()
        .items_center()
        .gap(px(ROW_GAP))
        .px(px(ROW_GAP))
        .overflow_x_scroll()
        // A vertical swipe does not move the band sideways.
        .restrict_scroll_to_axis()
        .child(group(app, keys::band_order().take(BAR_KEYS), cx))
        .child(middle.unwrap_or_else(|| div().flex_1().min_w_0()))
        .child(group(app, keys::band_order().skip(BAR_KEYS), cx))
        .into_any_element()
}

/// The key band, for a keyboard without a shortcuts bar (floating). No
/// pager: one pane at a time is only for a docked keyboard.
pub(super) fn render_band(app: &Smoke, height: f32, cx: &mut Context<Smoke>) -> AnyElement {
    band_frame("shell-key-band", height)
        .child(render_key_row(app, Some(div().into_any_element()), cx))
        .into_any_element()
}

/// The thin band above a docked keyboard: the pane pager only, as the
/// keyboard's own bar has the keys (#229, owner 2026-11-26).
pub(super) fn render_pager_band(height: f32, pager: Option<AnyElement>) -> AnyElement {
    band_frame("shell-pager-band", height)
        .justify_center()
        .px(px(ROW_GAP))
        .children(pager.map(|pager| div().flex_1().min_w(px(PAGER_MIN_WIDTH)).child(pager)))
        .into_any_element()
}

fn band_frame(id: &'static str, height: f32) -> Stateful<Div> {
    div()
        .id(id)
        .debug_selector(move || id.into())
        .h(px(height))
        .flex_none()
        .w_full()
        .flex()
        .items_center()
        .overflow_hidden()
        .border_t_1()
        .border_color(rgb(color::RAISED))
        .bg(rgb(color::SURFACE))
}

/// The band's first group holds the bar's keys; the second, the … menu's.
const BAR_KEYS: usize = keys::ALL.len() - keys::MENU.len();

fn group(app: &Smoke, keys: impl Iterator<Item = &'static Key>, cx: &mut Context<Smoke>) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(2.0))
        .p(px(4.0))
        .rounded(px(GROUP_RADIUS))
        .bg(rgb(color::RAISED))
        .children(keys.map(|key| button(app, key, cx)))
}

fn button(app: &Smoke, key: &'static Key, cx: &mut Context<Smoke>) -> AnyElement {
    let pressed = app
        .shell
        .frame
        .key_band
        .pressed(key.action, app.terminal.input.destination.as_ref());
    let action = key.action;
    let face = div()
        .flex()
        .flex_none()
        .w(px(width(key)))
        .h(px(KEY_HEIGHT))
        .rounded(px(KEY_RADIUS))
        .items_center()
        .justify_center()
        .text_size(px(KEY_TEXT))
        .text_color(rgb(color::TEXT))
        .when(pressed, |button| {
            button
                .bg(rgb(color::ACCENT))
                .text_color(rgb(color::BACKGROUND))
                .font_weight(weight::SEMIBOLD)
        })
        .when_else(
            action == Action::Hide,
            |button| button.child(hide_icon()),
            |button| button.child(key.label),
        );
    let face = if action == Action::Control {
        face.relative()
            .children(crate::qa_ax::mark(crate::qa_ax::ids::KEY_BAND_CTRL, "ctrl"))
    } else {
        face
    };
    if action == Action::Hide && app.input_modal() {
        let target = bind_button(div().occlude(), key, cx);
        face.relative()
            .child(hide_above_sheet(target))
            .into_any_element()
    } else {
        bind_button(face, key, cx).into_any_element()
    }
}

/// A word gets a wide key, an arrow a slightly narrower one, a symbol a
/// square one.
fn width(key: &Key) -> f32 {
    match key.action {
        Action::Key("left" | "up" | "down" | "right") => ARROW_KEY,
        _ if key.label.chars().count() == 1 => KEY_HEIGHT,
        _ => WIDE_KEY,
    }
}

/// The band key's element and QA identifier.
pub(crate) fn band_id(key: &Key) -> String {
    format!("key-band-{}", key.name)
}

fn bind_button(button: Div, key: &'static Key, cx: &mut Context<Smoke>) -> Stateful<Div> {
    let action = key.action;
    let id = band_id(key);
    button
        .id(gpui::SharedString::from(id.clone()))
        .debug_selector(move || id.clone())
        .role(Role::Button)
        .aria_label(key.accessibility_label)
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, |_, window, cx| {
            window.prevent_default();
            cx.stop_propagation();
        })
        .on_click(cx.listener(move |app, _, window, cx| {
            cx.stop_propagation();
            app.press_key_band(action, window, cx);
        }))
}

fn hide_above_sheet(target: Stateful<Div>) -> impl IntoElement {
    canvas(
        move |bounds, window, cx| {
            // Raise only the hit target over the scrim. Respect the key row's
            // scroll clip so an offscreen Hide cannot intercept sheet input.
            let visible = bounds.intersect(&window.content_mask().bounds);
            if visible.is_empty() {
                return;
            }
            let mut target = target
                .w(visible.size.width)
                .h(visible.size.height)
                .into_any_element();
            target.layout_as_root(visible.size.map(AvailableSpace::Definite), window, cx);
            window.defer_draw(target, visible.origin, 0, None);
        },
        |_, (), _, _| {},
    )
    .absolute()
    .inset_0()
}

fn hide_icon() -> impl IntoElement {
    canvas(
        |_, _, _| (),
        |bounds, _, window, _| {
            let mut path = PathBuilder::stroke(px(1.5));
            let segments: &[&[(f32, f32)]] = &[
                &[
                    (3.5, 2.5),
                    (16.5, 2.5),
                    (18., 4.),
                    (18., 10.),
                    (16.5, 11.5),
                    (3.5, 11.5),
                    (2., 10.),
                    (2., 4.),
                    (3.5, 2.5),
                ],
                &[(5., 5.5), (6., 5.5)],
                &[(8., 5.5), (9., 5.5)],
                &[(11., 5.5), (12., 5.5)],
                &[(14., 5.5), (15., 5.5)],
                &[(6., 8.5), (14., 8.5)],
                &[(7., 14.), (10., 16.5), (13., 14.)],
            ];
            for segment in segments {
                if let Some((&(x, y), rest)) = segment.split_first() {
                    path.move_to(bounds.origin + point(px(x), px(y)));
                    for &(x, y) in rest {
                        path.line_to(bounds.origin + point(px(x), px(y)));
                    }
                }
            }
            if let Ok(path) = path.build() {
                window.paint_path(path, rgb(color::TEXT));
            }
        },
    )
    .size(px(20.0))
}

fn stroke(key: &str, modifiers: Modifiers) -> KeyDownEvent {
    KeyDownEvent {
        keystroke: Keystroke {
            key: key.into(),
            modifiers,
            key_char: None,
        },
        is_held: false,
        prefer_character_input: false,
    }
}

/// Convert an accepted UIKit commit only after the existing retirement and IME
/// handling has vetted it. A multiscalar commit stays intact.
pub(crate) fn apply_latch(
    state: &mut State,
    input: GeneratedInput,
    report_all: bool,
) -> [Option<GeneratedInput>; 2] {
    use sheprd_client::protocol::ClientPaneInputEvent::TextCommit;
    let TextCommit(text) = &input.event else {
        return [Some(input), None];
    };
    let mut chars = text.chars();
    let Some(ch) = chars.next() else {
        return [Some(input), None];
    };
    if chars.next().is_some() {
        return [Some(input), None];
    }
    let modifiers = state.take(&input.destination);
    // A committed control character spends the latch and keeps its original bytes.
    if ch.is_control() || (!modifiers.alt && !modifiers.control) {
        return [Some(input), None];
    }
    let key = if ch == ' ' { "space" } else { text.as_str() };
    // An explicit Alt latch represents intent even if hardware Option types text.
    let Some(event) = crate::terminal::key_input(&stroke(key, modifiers), true) else {
        return [Some(input), None];
    };
    let mut held = HeldKeys::default();
    let press = held.press(key, event, report_all);
    let release = held.release(key).map(|event| GeneratedInput {
        destination: input.destination.clone(),
        event,
    });
    [
        Some(GeneratedInput {
            event: press,
            ..input
        }),
        release,
    ]
}

/// The key a hardware press of one committed scalar reports (herdr-uikit
/// keymap.rs), with the band's `latch`: a capital is its letter with Shift,
/// and a C0 control letter, as from Control-A, is Control and the letter.
/// None for text that no single key types.
fn typed_keystroke(text: &str, latch: Modifiers) -> Option<Keystroke> {
    let mut chars = text.chars();
    let (Some(ch), None) = (chars.next(), chars.next()) else {
        return None;
    };
    let mut modifiers = latch;
    let key = match ch {
        ' ' => "space".into(),
        '\n' | '\r' => "enter".into(),
        '\t' => "tab".into(),
        '\u{1b}' => "escape".into(),
        _ if control_letter(ch) => {
            modifiers.control = true;
            char::from_u32(u32::from(ch) + 0x60)?.to_string()
        }
        _ if ch.is_control() => return None,
        _ if ch.is_ascii_uppercase() => {
            modifiers.shift = true;
            ch.to_ascii_lowercase().to_string()
        }
        _ => ch.to_string(),
    };
    Some(Keystroke {
        key,
        modifiers,
        key_char: None,
    })
}

/// U+0001 to U+001A without the ones with keys of their own (Backspace,
/// Tab, Return).
fn control_letter(ch: char) -> bool {
    matches!(ch, '\u{1}'..='\u{1a}') && !matches!(ch, '\u{8}' | '\t' | '\n' | '\r')
}

impl Smoke {
    /// Herdr prefix style for a commit without a KeyDown of its own (the
    /// software keyboard, an IME, the band's text keys). A commit is a key
    /// under the band's latch, as a control letter, or after an armed
    /// prefix; other text stays typing, because a hardware press took the
    /// prefix route before UIKit typed it. Returns whether the app took the
    /// commit; the latch is then spent.
    pub(crate) fn route_typed(
        &mut self,
        input: &GeneratedInput,
        latched: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        use sheprd_client::protocol::ClientPaneInputEvent::TextCommit;
        let TextCommit(text) = &input.event else {
            return false;
        };
        let latch = if latched {
            self.shell.frame.key_band.latched(&input.destination)
        } else {
            Modifiers::default()
        };
        if latch.platform {
            // ⌘ and the letter act as a hardware ⌘ chord would. A commit no
            // single key types spends the latch and types nothing.
            self.shell.frame.key_band.clear();
            if let Some(keystroke) = typed_keystroke(text, latch) {
                self.dispatch_chord(keystroke, window, cx);
            }
            return true;
        }
        let armed = self.actions.host_keys.armed();
        if !(armed || latch.control || latch.alt || text.chars().all(control_letter)) {
            return false;
        }
        let Some(keystroke) = typed_keystroke(text, latch) else {
            // A commit ends a pending prefix as any hardware key would.
            self.actions.host_keys.clear_press();
            return false;
        };
        if !self.route_generated_key(keystroke, window, cx) {
            return false;
        }
        self.shell.frame.key_band.clear();
        true
    }

    /// Replays `keystroke` through GPUI's key dispatch, as the hardware
    /// keyboard's press of it: key bindings, Shortcut priority and the
    /// terminal decide as they do for that press. Deferred, as this runs
    /// inside an input callback or a band tap.
    fn dispatch_chord(
        &mut self,
        keystroke: Keystroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.defer(cx, move |window, cx| {
            window.dispatch_keystroke(keystroke, cx);
        });
    }

    pub(crate) fn press_key_band(
        &mut self,
        action: Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // A tap that began on the band before the search panel took the
        // keyboard: Hide must not become a keyboard dismissal, nor any key input.
        if self.native_field_open() {
            return;
        }
        // Any band key ends an earlier key binding's failure reason (L57).
        self.actions.key_status = None;
        if action == Action::Hide {
            self.shell.frame.key_band.clear();
            let dismiss = Window::dismiss_virtual_keyboard;
            #[cfg(test)]
            let dismiss = tests::observe_dismissal(dismiss, cx);
            dismiss(window);
            cx.notify();
            return;
        }
        if self.input_modal()
            || self.find.is_some()
            || self.find_handoff.paused
            || self.actions.palette
            || self.shell.shortcuts.open
        {
            return;
        }
        if !self.terminal_focus.is_focused(window) || self.shell.fit.too_small.is_some() {
            return;
        }
        self.input_transition(|app| {
            app.refresh_terminal_input();
            app.sync_handoff();
            let Some(destination) = app.terminal.input.destination.clone() else {
                return;
            };
            match action {
                Action::Control | Action::Alt | Action::Command => {
                    app.shell.frame.key_band.toggle(action, destination)
                }
                Action::Prefix => {
                    app.shell.frame.key_band.clear();
                    if let Some(prefix) = app.actions.host_keys.prefix_keystroke(&app.terminal) {
                        app.dispatch_chord(prefix, window, cx);
                    }
                }
                Action::Paste => {
                    app.shell.frame.key_band.clear();
                    app.start_paste(false, cx);
                }
                Action::Text(text) => {
                    if app.terminal.input.composition.marked.is_none()
                        && let Some(input) = app.terminal.input.commit(text)
                    {
                        app.handoff.resume_typing();
                        if !app.route_typed(&input, true, window, cx) {
                            let events = apply_latch(
                                &mut app.shell.frame.key_band,
                                input,
                                app.terminal.input.report_all,
                            );
                            for event in events.into_iter().flatten() {
                                app.send_terminal(event);
                            }
                        }
                    }
                }
                Action::Key(key) => {
                    let modifiers = app.shell.frame.key_band.take(&destination);
                    let keystroke = Keystroke {
                        key: key.into(),
                        modifiers,
                        key_char: None,
                    };
                    if modifiers.platform {
                        app.dispatch_chord(keystroke, window, cx);
                        window.notify_text_input_changed(&app.terminal_focus);
                        cx.notify();
                        return;
                    }
                    let routed =
                        (modifiers.control || modifiers.alt || app.actions.host_keys.armed())
                            && app.route_generated_key(keystroke, window, cx);
                    let report_all = app.terminal.input.report_all;
                    if !routed
                        && let Some(press) = app
                            .terminal
                            .input
                            .key_down(&stroke(key, modifiers), report_all)
                    {
                        app.handoff.resume_typing();
                        app.send_terminal(press);
                        if let Some(release) = app.terminal.input.key_up(key) {
                            app.send_terminal(release);
                        }
                    }
                }
                Action::Hide => {}
            }
            window.notify_text_input_changed(&app.terminal_focus);
            cx.notify();
        });
    }
}

#[cfg(test)]
mod bar_tests;
#[cfg(test)]
mod pager_room_tests;
#[cfg(test)]
mod tests;
