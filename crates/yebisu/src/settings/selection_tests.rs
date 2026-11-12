//! SH1-151: real disk and the same handler used by the terminal strip.
use super::{SettingsHost, Values, store, tests::directory};
use crate::app::Smoke;
use gpui::{AppContext as _, TestAppContext};
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};

fn settle(this: &mut Smoke, cx: &mut gpui::Context<Smoke>) {
    let until = Instant::now() + Duration::from_secs(3);
    loop {
        this.poll_settings(cx);
        if this.settings.as_ref().is_some_and(SettingsHost::ready) {
            break;
        }
        assert!(Instant::now() < until, "settings did not settle");
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(this.settings_error.is_none(), "{:?}", this.settings_error);
}
fn open(root: &Path, cx: &mut TestAppContext) -> gpui::Entity<Smoke> {
    let host = SettingsHost::open(root.to_path_buf()).unwrap_or_else(|e| panic!("{e}"));
    let view = cx.new(Smoke::new);
    view.update(cx, |this, _| this.settings = Some(host));
    view
}
#[gpui::test]
fn sh1_151_fresh_store_defaults(cx: &mut TestAppContext) {
    let root = directory("selection-fresh");
    let view = open(&root, cx);
    view.update(cx, |this, cx| {
        assert!(this.handoff.drag_select);
        assert!(!this.handoff.copy_on_select);
        settle(this, cx);
        assert!(this.handoff.drag_select);
        assert!(!this.handoff.copy_on_select);
        // Materialize the fresh values via the real strip handler too.
        this.toggle_selection_setting(false);
        this.toggle_selection_setting(false);
        settle(this, cx);
    });
    let saved = store::load(&root).unwrap_or_else(|e| panic!("{e}"));
    assert!(saved.drag_select);
    assert!(!saved.copy_on_select);
    fs::remove_dir_all(root).unwrap_or_else(|e| panic!("{e}"));
}
#[gpui::test]
fn sh1_151_each_strip_button_survives_reopen(cx: &mut TestAppContext) {
    for copy in [false, true] {
        let root = directory("selection-toggle");
        let view = open(&root, cx);
        view.update(cx, |this, cx| {
            settle(this, cx);
            this.toggle_selection_setting(copy);
            settle(this, cx);
        });
        let reopened = open(&root, cx);
        reopened.update(cx, |this, cx| {
            settle(this, cx);
            assert_eq!(this.handoff.drag_select, copy);
            assert_eq!(this.handoff.copy_on_select, copy);
        });
        // Assert the receiving file itself has both fields, not just load defaults.
        let json: serde_json::Value = serde_json::from_slice(
            &fs::read(root.join("yebisu-settings.json")).unwrap_or_else(|e| panic!("{e}")),
        )
        .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(json["drag_select"], copy);
        assert_eq!(json["copy_on_select"], copy);
        fs::remove_dir_all(root).unwrap_or_else(|e| panic!("{e}"));
    }
}
#[gpui::test]
fn sh1_151_legacy_file_defaults_without_error(cx: &mut TestAppContext) {
    let root = directory("selection-legacy");
    fs::create_dir_all(&root).unwrap_or_else(|e| panic!("{e}"));
    fs::write(root.join("yebisu-settings.json"), br#"{"version":1,"language":"en-US","compact":true,"terminal_priority":true,"board_hidden":true}"#).unwrap_or_else(|e| panic!("{e}"));
    let view = open(&root, cx);
    view.update(cx, |this, cx| {
        settle(this, cx);
        assert!(this.handoff.drag_select);
        assert!(!this.handoff.copy_on_select);
        assert!(
            this.terminal.compact && this.actions.terminal_priority && this.actions.board_hidden
        );
        this.toggle_selection_setting(true);
        settle(this, cx);
    });
    assert!(
        store::load(&root)
            .unwrap_or_else(|e| panic!("{e}"))
            .copy_on_select
    );
    fs::remove_dir_all(root).unwrap_or_else(|e| panic!("{e}"));
}
#[gpui::test]
fn sh1_151_strip_edit_before_load_preserves_other_fields(cx: &mut TestAppContext) {
    for copy in [false, true] {
        let root = directory("selection-load-race");
        let stored = Values {
            compact: true,
            terminal_priority: true,
            board_hidden: true,
            drag_select: !copy,
            copy_on_select: !copy,
            notifications: Default::default(),
        };
        store::save(&root, &stored).unwrap_or_else(|e| panic!("{e}"));
        let expected = Values {
            drag_select: false,
            copy_on_select: true,
            ..stored.clone()
        };
        let view = open(&root, cx);
        view.update(cx, |this, cx| {
            assert!(!this.settings.as_ref().is_some_and(|h| h.loaded));
            this.toggle_selection_setting(copy);
            settle(this, cx);
            assert_eq!(this.settings_values(), expected);
        });
        assert_eq!(
            store::load(&root).unwrap_or_else(|e| panic!("{e}")),
            expected
        );
        fs::remove_dir_all(root).unwrap_or_else(|e| panic!("{e}"));
    }
}
