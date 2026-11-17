//! Review B1 and S1 of layout/l1 320adc1: the stored Input options and
//! Return are app settings. Every terminal view carries a copy, so a host
//! switch (board.rs `select_host`), a return from the background and the
//! first load must leave the live values, the draft and the file at the
//! stored values.
use super::support::{directory, open, wait, write_file};
use crate::{app::Smoke, settings::SettingsHost, settings_pages::choices::Choice};
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
    let restarted = open(&root);
    assert!(
        restarted.draft.option_alt && !restarted.draft.reconnect_automatic,
        "the loaded values were not overwritten by defaults"
    );
}
