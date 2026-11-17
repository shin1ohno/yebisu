//! S-6, Q7: the toolbar and the sheet edit the same store, and the stored
//! Input options reach the key encoder.
use super::support::{directory, open, wait};
use crate::{
    app::Smoke,
    settings::SettingsHost,
    settings_pages::{choices::Choice, toolbar_sync::ToolbarSetting},
};
use gpui::{AppContext as _, KeyDownEvent, Keystroke, TestAppContext};
use sheprd_client::{ClientEvent, protocol::ClientPaneInputEvent};
use std::sync::Arc;

fn host(this: &mut Smoke) -> &mut SettingsHost {
    this.settings
        .as_mut()
        .unwrap_or_else(|| panic!("settings host"))
}

#[gpui::test]
fn toolbar_change_is_visible_in_sheet_draft(cx: &mut TestAppContext) {
    let root = directory("toolbar");
    let store = open(&root);
    let view = cx.new(Smoke::new);
    view.update(cx, |this, cx| {
        this.settings = Some(store);
        // The legacy toolbar flips its in-memory value, then reports it.
        this.terminal.input.options.option_alt = true;
        this.toolbar_setting_changed(ToolbarSetting::InputOptions, cx);
        this.terminal.reconnect.automatic = false;
        this.toolbar_setting_changed(ToolbarSetting::ReturnPolicy, cx);
        this.terminal.compact = true;
        this.toolbar_setting_changed(ToolbarSetting::Density, cx);
        // No poll_settings in between: the draft took the values at once.
        let draft = &host(this).draft;
        assert!(draft.option_alt, "Option: Alt in the sheet draft");
        assert!(!draft.reconnect_automatic, "Return: Tap in the sheet draft");
        assert!(draft.compact, "Compact in the sheet draft");
        wait(host(this), SettingsHost::ready);
    });
    let restarted = open(&root);
    assert!(restarted.draft.option_alt && !restarted.draft.reconnect_automatic);
}

#[gpui::test]
fn sheet_save_updates_toolbar_toggle(cx: &mut TestAppContext) {
    let root = directory("sheet");
    let store = open(&root);
    let view = cx.new(Smoke::new);
    view.update(cx, |this, cx| {
        this.settings = Some(store);
        for choice in [
            Choice::OptionKey,
            Choice::CommandKey,
            Choice::JapaneseCandidates,
            Choice::ShortcutPriority,
        ] {
            this.select_settings_choice(choice, true, cx);
        }
        this.select_settings_choice(Choice::Return, false, cx);
        // The toolbar labels read these fields (terminal_controls.rs,
        // terminal_view.rs).
        let options = &this.terminal.input.options;
        assert!(options.option_alt, "Option: Alt");
        assert!(options.command_control, "Command: Ctrl");
        assert!(options.caret_anchor, "Candidate: Caret");
        assert!(this.actions.terminal_priority, "Terminal priority");
        assert!(!this.terminal.reconnect.automatic, "Return: Tap reconnect");
        wait(host(this), SettingsHost::ready);
    });
}

fn key(name: &'static str) -> KeyDownEvent {
    KeyDownEvent {
        keystroke: Keystroke::parse(name).unwrap_or_else(|e| panic!("key: {e}")),
        is_held: false,
        prefer_character_input: false,
    }
}

#[gpui::test]
fn option_alt_setting_changes_key_encoding(cx: &mut TestAppContext) {
    let root = directory("encoding");
    let store = open(&root);
    let view = cx.new(Smoke::new);
    view.update(cx, |this, cx| {
        this.settings = Some(store);
        let mut live = crate::live::Live::default();
        live.connected = true;
        let snapshot = serde_json::from_str(crate::mock_peer::SNAPSHOT)
            .unwrap_or_else(|e| panic!("fixture: {e}"));
        live.apply(ClientEvent::Snapshot(Arc::new(snapshot)));
        live.apply(ClientEvent::Surface(Arc::new(crate::mock_peer::surface(7))));
        this.terminal.input.sync(&live, 1, None);
        // Option: Native leaves Option to the system text input.
        this.select_settings_choice(Choice::OptionKey, false, cx);
        assert!(this.terminal.input.key_down(&key("alt-p"), false).is_none());
        this.select_settings_choice(Choice::OptionKey, true, cx);
        assert!(
            this.terminal.input.key_down(&key("alt-p"), false).is_some(),
            "Option: Alt sends Alt to the terminal"
        );
        // Command key: Local keeps ⌘; Ctrl sends ⌘C as Control-C.
        this.select_settings_choice(Choice::CommandKey, false, cx);
        assert!(this.terminal.input.key_down(&key("cmd-c"), false).is_none());
        this.select_settings_choice(Choice::CommandKey, true, cx);
        assert!(matches!(
            this.terminal
                .input
                .key_down(&key("cmd-c"), false)
                .map(|input| input.event),
            Some(ClientPaneInputEvent::Key { modifiers: 2, .. })
        ));
        // Japanese candidates: the anchor the IME geometry reads.
        this.select_settings_choice(Choice::JapaneseCandidates, true, cx);
        assert!(this.terminal.input.options.caret_anchor);
    });
}
