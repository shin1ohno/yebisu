use super::*;
use crate::hosts::test_support::{MINI, STUDIO, named_setup};

#[gpui::test]
fn full_status_bar_shows_saved_name_and_falls_back_to_derived_name(cx: &mut gpui::TestAppContext) {
    let view = cx.new(Smoke::new);
    view.update(cx, |this, _| {
        this.setup = named_setup();
        let state = state(HerdrKind::Herdr1, true);
        this.terminal.reconnect.target = STUDIO.into();
        let model = this.current_status_bar_model(&state);
        assert_eq!(status_line(&model), "Studio Mac · Herdr 1 · Connected");
        assert_eq!(model.accessibility_label, "Studio Mac, Herdr 1, Connected");
        this.terminal.reconnect.target = MINI.into();
        assert_eq!(this.current_status_bar_model(&state).connection, "mini");
    });
}

fn state(kind: HerdrKind, connected: bool) -> ChromeState {
    ChromeState {
        kind,
        connected,
        ..ChromeState::default()
    }
}

#[test]
fn status_bar_is_single_line() {
    let model = status_bar_model(&state(HerdrKind::Herdr1, true), "pro-dev");
    assert_eq!(status_line(&model), "pro-dev · Herdr 1 · Connected");
    let model = status_bar_model(&state(HerdrKind::Herdr2, false), "pro-dev");
    assert_eq!(status_line(&model), "pro-dev · Herdr 2 · Disconnected");
    // A host name with control characters (untrusted) stays one line.
    let model = status_bar_model(&state(HerdrKind::Herdr1, true), "evil\nhost\r\tname");
    let line = status_line(&model);
    assert!(!line.chars().any(char::is_control), "{line:?}");
    assert!(!model.accessibility_label.chars().any(char::is_control));
    assert_eq!(line.lines().count(), 1);
}

#[test]
fn reconnect_shows_only_when_disconnected() {
    for kind in [HerdrKind::Herdr1, HerdrKind::Herdr2] {
        assert!(!status_bar_model(&state(kind, true), "h").show_reconnect);
        assert!(status_bar_model(&state(kind, false), "h").show_reconnect);
    }
}

#[test]
fn status_bar_full_name_in_accessibility_label() {
    let name = "a-very-long-connection-name-for-pro-dev-x"; // 41 characters
    let model = status_bar_model(&state(HerdrKind::Herdr1, true), name);
    assert_eq!(model.connection, name);
    assert_eq!(
        model.accessibility_label,
        format!("{name}, Herdr 1, Connected")
    );
}

#[test]
fn hidden_sidebar_offers_show_in_status_bar() {
    let shown = state(HerdrKind::Herdr1, true);
    assert!(!status_bar_model(&shown, "h").show_sidebar_hint);
    let hidden = ChromeState {
        sidebar_hidden: true,
        ..shown
    };
    assert!(status_bar_model(&hidden, "h").show_sidebar_hint);
    assert_eq!(
        sidebar_label(&Default::default(), &Default::default()),
        "Expand sidebar ⌘B"
    );
    // Integration 2 (L4 review F1): no Show sidebar while the width forces
    // the rail; pressing it could not show the sidebar.
    let forced = ChromeState {
        sidebar_forced: true,
        ..hidden
    };
    assert!(!status_bar_model(&forced, "h").show_sidebar_hint);
}

#[test]
fn status_bar_carries_terminal_status_text() {
    let model = status_bar_model(&state(HerdrKind::Herdr1, true), "pro-dev");
    assert_eq!(
        status_detail(&model, "Paste queued").as_deref(),
        Some("Paste queued")
    );
    assert_eq!(
        status_detail(&model, "connected"),
        None,
        "repeats the state"
    );
    assert_eq!(status_detail(&model, "  "), None);
    // Untrusted daemon text stays on one line.
    assert_eq!(
        status_detail(&model, "Version mismatch:\nold").as_deref(),
        Some("Version mismatch: old")
    );
}

#[test]
fn status_bar_shows_the_display_name_and_icons() {
    let model = status_bar_model(&state(HerdrKind::Herdr1, true), "dev@pro-dev.example.com");
    assert_eq!(status_line(&model), "pro-dev · Herdr 1 · Connected");
    assert_eq!(model.accessibility_label, "pro-dev, Herdr 1, Connected");
    assert_eq!(
        status_bar_model(&state(HerdrKind::Herdr2, true), "192.0.2.8").connection,
        "192.0.2.8"
    );
    assert_eq!(Icon::Keyboard.size(), (18., 18.));
}

#[test]
fn selection_status_counts_characters_and_names_copy() {
    let model = status_bar_model(
        &ChromeState {
            selecting: true,
            ..state(HerdrKind::Herdr1, true)
        },
        "pro-dev",
    );
    assert_eq!(status_line(&model), "pro-dev · Herdr 1 · Connected");
    assert_eq!(
        crate::shell::edit_bar::selection_status_line(7, usize::MAX),
        "Selected 7 characters · ⌘C copies · Typing is paused until you cancel the selection"
    );
}

#[test]
fn hint_sidebar_uses_the_first_host_key_and_hides_it_without_a_binding() {
    let mut terminal = crate::terminal_view::TerminalView::default();
    terminal.reconnect.target = "sandbox".into();
    terminal.live.connected = true;
    let mut actions = crate::actions::Actions {
        herdr_prefix: true,
        ..Default::default()
    };
    actions.host_keys.set_test_config(
        &terminal,
        "[keys]\nprefix = 'ctrl+a'\ntoggle_sidebar = ['prefix+s', 'alt+b']",
    );
    assert_eq!(
        sidebar_label(&terminal, &actions),
        "Expand sidebar ctrl+a s"
    );

    actions
        .host_keys
        .set_test_config(&terminal, "[keys]\nprefix = 'ctrl+a'\ntoggle_sidebar = []");
    assert_eq!(sidebar_label(&terminal, &actions), "Expand sidebar");
    terminal.live.connected = false;
    assert_eq!(sidebar_label(&terminal, &actions), "Expand sidebar");
}
