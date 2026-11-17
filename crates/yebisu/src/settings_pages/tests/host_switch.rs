//! Review B1 and S1 of layout/l1 320adc1: the stored Input options and
//! Return are app settings. Every terminal view carries a copy, so a host
//! switch (board.rs `select_host`), a return from the background and the
//! first load must leave the live values, the draft and the file at the
//! stored values.
use super::support::{directory, open, wait, write_file};
use crate::{
    app::Smoke,
    settings::SettingsHost,
    settings_pages::{choices::Choice, toolbar_sync::ToolbarSetting},
};
use gpui::{AppContext as _, AppLifecyclePhase, TestAppContext};

fn host(this: &mut Smoke) -> &mut SettingsHost {
    this.settings
        .as_mut()
        .unwrap_or_else(|| panic!("settings host"))
}

/// The Input options and Return the selected terminal view holds now.
fn live(this: &Smoke) -> [bool; 4] {
    let options = &this.terminal.input.options;
    [
        options.option_alt,
        options.command_control,
        options.caret_anchor,
        this.terminal.reconnect.automatic,
    ]
}

const CHOSEN: [bool; 4] = [true, true, true, false];

#[gpui::test]
fn host_switch_keeps_stored_input_options_and_return(cx: &mut TestAppContext) {
    let root = directory("host-switch");
    let store = open(&root);
    let view = cx.new(Smoke::new);
    view.update(cx, |this, cx| {
        this.settings = Some(store);
        this.select_host("alpha");
        for choice in [
            Choice::OptionKey,
            Choice::CommandKey,
            Choice::JapaneseCandidates,
        ] {
            this.select_settings_choice(choice, true, cx);
        }
        this.select_settings_choice(Choice::Return, false, cx);
        assert_eq!(live(this), CHOSEN);
        // A host never opened before starts from a default terminal view.
        this.select_host("beta");
        // The key path refreshes input before it encodes a key.
        this.refresh_terminal_input();
        assert_eq!(live(this), CHOSEN, "beta encodes with the stored options");
        // The 33 ms settings poll reads the live values into the draft.
        this.poll_settings(cx);
        this.poll_settings(cx);
        let draft = host(this).draft.clone();
        assert_eq!(
            [
                draft.option_alt,
                draft.command_control,
                draft.caret_anchor,
                draft.reconnect_automatic
            ],
            CHOSEN,
            "the draft keeps the stored values after the switch"
        );
        // Every hidden host reconnects on return only when Return is
        // Automatic (reconnect.rs reads each view's own copy).
        assert!(
            this.board
                .hidden
                .values()
                .all(|terminal| !terminal.reconnect.automatic),
            "hidden hosts follow Return: Tap"
        );
        this.select_host("alpha");
        this.poll_settings(cx);
        assert_eq!(live(this), CHOSEN, "alpha after switching back");
        // A return from the background (the reconnect path) keeps them too.
        this.lifecycle(AppLifecyclePhase::Background, cx);
        this.lifecycle(AppLifecyclePhase::Active, cx);
        this.poll_settings(cx);
        assert_eq!(live(this), CHOSEN, "after a return from the background");
        wait(host(this), SettingsHost::ready);
    });
    let restarted = open(&root);
    let draft = restarted.draft;
    assert_eq!(
        [
            draft.option_alt,
            draft.command_control,
            draft.caret_anchor,
            draft.reconnect_automatic
        ],
        CHOSEN,
        "the file still holds the chosen values"
    );
}

#[gpui::test]
fn stored_input_options_reach_terminal_on_first_load(cx: &mut TestAppContext) {
    let root = directory("first-load");
    write_file(
        &root,
        &serde_json::json!({"version":1,"language":"en-US","compact":false,
            "terminal_priority":false,"board_hidden":false,
            "option_key":"alt","command_key":"ctrl","japanese_candidates":"caret",
            "return_policy":"tap"}),
    );
    let store = SettingsHost::open(root.clone()).unwrap_or_else(|e| panic!("open: {e}"));
    let view = cx.new(Smoke::new);
    view.update(cx, |this, cx| {
        this.settings = Some(store);
        // The app's settings timer: poll until the worker's load arrived.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while live(this) != CHOSEN {
            assert!(std::time::Instant::now() < deadline, "load not adopted");
            this.poll_settings(cx);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        // Later ticks read the live values back; they must not undo the load.
        for _ in 0..3 {
            this.poll_settings(cx);
        }
        assert_eq!(live(this), CHOSEN);
        wait(host(this), SettingsHost::ready);
    });
    let draft = open(&root).draft;
    assert_eq!(
        [
            draft.option_alt,
            draft.command_control,
            draft.caret_anchor,
            draft.reconnect_automatic
        ],
        CHOSEN,
        "the loaded values were not overwritten by defaults"
    );
}

/// A Smoke whose stored values are CHOSEN, selected on host "alpha".
fn chosen(this: &mut Smoke, store: SettingsHost, cx: &mut gpui::Context<Smoke>) {
    this.settings = Some(store);
    this.select_host("alpha");
    for choice in [
        Choice::OptionKey,
        Choice::CommandKey,
        Choice::JapaneseCandidates,
    ] {
        this.select_settings_choice(choice, true, cx);
    }
    this.select_settings_choice(Choice::Return, false, cx);
    this.poll_settings(cx);
    assert_eq!(live(this), CHOSEN);
}

fn draft(this: &mut Smoke) -> [bool; 4] {
    let draft = &host(this).draft;
    [
        draft.option_alt,
        draft.command_control,
        draft.caret_anchor,
        draft.reconnect_automatic,
    ]
}

// Review S3 of 7ffde18: one test per path, each with nothing else in between.

#[gpui::test]
fn poll_alone_gives_new_host_stored_values(cx: &mut TestAppContext) {
    let store = open(&directory("poll-only"));
    let view = cx.new(Smoke::new);
    view.update(cx, |this, cx| {
        chosen(this, store, cx);
        this.select_host("beta");
        // Only the 33 ms timer runs: no key, no lifecycle.
        this.poll_settings(cx);
        assert_eq!(live(this), CHOSEN, "beta's view after the poll");
        assert_eq!(draft(this), CHOSEN, "the poll kept the draft");
    });
}

#[gpui::test]
fn key_path_alone_gives_new_host_stored_values(cx: &mut TestAppContext) {
    let store = open(&directory("key-only"));
    let view = cx.new(Smoke::new);
    view.update(cx, |this, cx| {
        chosen(this, store, cx);
        this.select_host("beta");
        // A key arrives before the next poll.
        this.refresh_terminal_input();
        assert_eq!(live(this), CHOSEN, "beta encodes with the stored values");
    });
}

#[gpui::test]
fn lifecycle_right_after_switch_keeps_draft(cx: &mut TestAppContext) {
    // Review S2: a lifecycle event before the first poll after select_host.
    let root = directory("lifecycle-after-switch");
    let store = open(&root);
    let view = cx.new(Smoke::new);
    view.update(cx, |this, cx| {
        chosen(this, store, cx);
        this.select_host("beta");
        this.settings_lifecycle(AppLifecyclePhase::Background);
        assert_eq!(draft(this), CHOSEN, "the lifecycle save kept the draft");
        wait(host(this), SettingsHost::ready);
    });
    let stored = open(&root).draft;
    assert!(stored.option_alt && !stored.reconnect_automatic);
}

#[gpui::test]
fn sheet_read_right_after_switch_keeps_draft(cx: &mut TestAppContext) {
    // Review S2: any reader of settings_values before the first poll.
    let store = open(&directory("reader-after-switch"));
    let view = cx.new(Smoke::new);
    view.update(cx, |this, cx| {
        chosen(this, store, cx);
        this.select_host("beta");
        let values = this.settings_values();
        assert_eq!(
            [
                values.option_alt,
                values.command_control,
                values.caret_anchor,
                values.reconnect_automatic
            ],
            CHOSEN,
            "settings_values reads the stored values for an unsynced view"
        );
    });
}

#[gpui::test]
fn toolbar_return_toggle_right_after_switch(cx: &mut TestAppContext) {
    let store = open(&directory("toolbar-return"));
    let view = cx.new(Smoke::new);
    view.update(cx, |this, cx| {
        chosen(this, store, cx);
        // Stored Return: Automatic, so the flip below differs from it.
        this.select_settings_choice(Choice::Return, true, cx);
        this.select_host("beta");
        // terminal_view.rs flips the new view's Return, then reports it.
        this.terminal.reconnect.automatic ^= true;
        let flipped = this.terminal.reconnect.automatic;
        this.toolbar_setting_changed(ToolbarSetting::ReturnPolicy, cx);
        let draft = draft(this);
        assert_eq!(draft[3], flipped, "the toolbar's Return flip is kept");
        assert_eq!(draft[..3], CHOSEN[..3], "the other options stay stored");
    });
}

#[gpui::test]
fn hidden_default_view_follows_return_on_lifecycle(cx: &mut TestAppContext) {
    // Review S1: connecting creates hidden views with defaults
    // (terminal_view.rs start_host_connection_inner, `or_default()`).
    let store = open(&directory("hidden-lifecycle"));
    let view = cx.new(Smoke::new);
    view.update(cx, |this, cx| {
        chosen(this, store, cx);
        this.board.hidden.entry("gamma".to_owned()).or_default();
        // reconnect.rs: settings_lifecycle runs first, then each view's
        // Return decides its reconnect on return.
        this.lifecycle(AppLifecyclePhase::Background, cx);
        let gamma = &this.board.hidden["gamma"];
        assert!(!gamma.reconnect.automatic, "gamma follows Return: Tap");
        assert!(gamma.input.options.option_alt, "gamma takes Option: Alt");
    });
}

#[gpui::test]
fn hidden_default_view_follows_stored_values_on_poll(cx: &mut TestAppContext) {
    let store = open(&directory("hidden-poll"));
    let view = cx.new(Smoke::new);
    view.update(cx, |this, cx| {
        chosen(this, store, cx);
        this.board.hidden.entry("delta".to_owned()).or_default();
        this.poll_settings(cx);
        let delta = &this.board.hidden["delta"];
        assert!(!delta.reconnect.automatic && delta.input.options.option_alt);
    });
}
