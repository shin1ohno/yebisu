use super::{
    SettingsHost, Values,
    font::TerminalFont,
    store,
    tests::{directory, wait},
};
use std::fs;

#[test]
fn defaults_keep_owner_metrics_and_legacy_file() {
    let value = serde_json::json!({"version":1,"language":"en-US","compact":false,"terminal_priority":false,"board_hidden":false});
    let font = TerminalFont::from_json(&value).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(font.metrics(false), (14., 20.));
    assert_eq!(font.metrics(true), (12., 17.));
    assert_eq!(font.family, "SauceCodePro Nerd Font Mono");
}

#[test]
fn size_edit_before_load_preserves_stored_family_and_durable_newest_draft() {
    let root = directory("font-load");
    let stored = Values {
        font: TerminalFont {
            size: Some(18.),
            family: "Menlo".into(),
        },
        ..Values::default()
    };
    store::save(&root, &stored).unwrap_or_else(|e| panic!("{e}"));
    let mut host = SettingsHost::open(root.clone()).unwrap_or_else(|e| panic!("{e}"));
    let mut edited = Values::default();
    edited.font.size = Some(22.);
    host.edit(edited);
    wait(&mut host, SettingsHost::ready);
    assert_eq!(host.draft.font.family, "Menlo");
    assert_eq!(host.draft.font.size, Some(22.));
    assert_eq!(
        store::load(&root).unwrap_or_else(|e| panic!("{e}")),
        host.draft
    );
    let bytes = fs::read(root.join("yebisu-settings.json")).unwrap_or_else(|e| panic!("{e}"));
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(json["terminal_font_size"], 22.);
    assert_eq!(json["terminal_font_family"], "Menlo");
    fs::remove_dir_all(root).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn invalid_font_file_is_preserved() {
    let root = directory("font-invalid");
    fs::create_dir_all(&root).unwrap_or_else(|e| panic!("{e}"));
    for invalid in [
        serde_json::json!(7),
        serde_json::json!(49),
        serde_json::json!("18"),
    ] {
        let bytes = serde_json::json!({"version":1,"language":"en-US","compact":false,"terminal_priority":false,"board_hidden":false,"terminal_font_size":invalid}).to_string();
        fs::write(root.join("yebisu-settings.json"), &bytes).unwrap_or_else(|e| panic!("{e}"));
        let mut host = SettingsHost::open(root.clone()).unwrap_or_else(|e| panic!("{e}"));
        wait(&mut host, |h| h.loaded);
        assert!(host.error.is_some());
        let mut values = Values::default();
        values.font.size = Some(20.);
        host.edit(values);
        host.flush();
        let (ack, handled) = std::sync::mpsc::sync_channel(1);
        host.sender
            .send(super::worker::Request::Barrier(ack))
            .unwrap_or_else(|e| panic!("{e}"));
        handled
            .recv_timeout(std::time::Duration::from_secs(3))
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(!host.ready());
        assert_eq!(
            fs::read_to_string(root.join("yebisu-settings.json")).unwrap_or_else(|e| panic!("{e}")),
            bytes
        );
    }
    fs::remove_dir_all(root).unwrap_or_else(|e| panic!("{e}"));
}

#[gpui::test]
fn font_consumer_adapter_flushes_background_and_reloads(cx: &mut gpui::TestAppContext) {
    use gpui::AppContext as _;
    let root = directory("font-consumer");
    let host = SettingsHost::open(root.clone()).unwrap_or_else(|e| panic!("{e}"));
    let view = cx.new(crate::app::Smoke::new);
    view.update(cx, |this, cx| {
        this.settings = Some(host);
        this.terminal.font.size = Some(24.);
        this.terminal.font.family = "Menlo".into();
        this.terminal.compact = true;
        // UIKit interrupts before the periodic SettingsHost poll sees this edit.
        this.lifecycle(gpui::AppLifecyclePhase::Background, cx);
        wait(
            this.settings.as_mut().unwrap_or_else(|| panic!("host")),
            SettingsHost::ready,
        );
    });
    let stored = store::load(&root).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(stored.font.size, Some(24.));
    assert_eq!(stored.font.family, "Menlo");
    assert_eq!(stored.font.metrics(stored.compact), (24., 34.));
    let reopened = cx.new(crate::app::Smoke::new);
    reopened.update(cx, |this, cx| {
        this.settings = Some(SettingsHost::open(root.clone()).unwrap_or_else(|e| panic!("{e}")));
        let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            this.poll_settings(cx);
            if this.settings.as_ref().is_some_and(SettingsHost::ready) {
                break;
            }
            assert!(std::time::Instant::now() < until, "adapter load timeout");
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert_eq!(this.settings_values(), stored);
        this.select_host("font-other-host");
        this.poll_settings(cx);
        assert_eq!(this.settings_values(), stored);
        this.select_host("font-original-host");
        this.poll_settings(cx);
        assert_eq!(this.settings_values(), stored);
        assert_eq!(
            this.terminal.font.metrics(this.terminal.compact),
            (24., 34.)
        );
    });
    fs::remove_dir_all(root).unwrap_or_else(|e| panic!("{e}"));
}

#[gpui::test]
fn regular_button_before_load_preserves_reset_intent_and_other_fields(
    cx: &mut gpui::TestAppContext,
) {
    use gpui::{AppContext as _, px, size};
    for prior_edit in [false, true] {
        let root = directory(if prior_edit {
            "regular-prior"
        } else {
            "regular"
        });
        let stored = Values {
            compact: true,
            font: TerminalFont {
                size: Some(18.),
                family: "Menlo".into(),
            },
            copy_on_select: true,
            ..Values::default()
        };
        store::save(&root, &stored).unwrap_or_else(|e| panic!("{e}"));
        let (view, cx) = cx.add_window_view(|_, cx| crate::app::Smoke::new(cx));
        cx.simulate_resize(size(px(1100.), px(900.)));
        cx.update(|window, cx| {
            view.update(cx, |this, cx| {
                this.settings_fonts = vec!["Menlo".into()];
                this.begin_settings_modal(window, cx);
            })
        });
        // Render the actual button before installing the pending load. No fake
        // UI handler or reducer-only call substitutes for the click below.
        cx.run_until_parked();
        let regular = cx
            .debug_bounds("density-Regular")
            .unwrap_or_else(|| panic!("Regular"));
        view.update(cx, |this, cx| {
            this.settings =
                Some(SettingsHost::open(root.clone()).unwrap_or_else(|e| panic!("{e}")));
            if prior_edit {
                let mut values = this.settings_values();
                values.board_hidden = true;
                this.apply_settings(values, cx);
            }
            assert!(
                !this
                    .settings
                    .as_ref()
                    .unwrap_or_else(|| panic!("host"))
                    .loaded
            );
            assert!(!this.terminal.compact);
            assert_eq!(this.terminal.font.size, None);
            cx.notify();
        });
        // Repaint the enabled production control without advancing the timer.
        cx.run_until_parked();
        cx.simulate_click(regular.center(), Default::default());
        view.update(cx, |this, cx| {
            let host = this.settings.as_ref().unwrap_or_else(|| panic!("host"));
            assert!(!host.loaded, "test must click before Loaded");
            assert!(host.edited_before_load.compact && host.edited_before_load.font_size);
            assert!(host.revision > 0);
            this.select_host("regular-other-host");
            this.settings_lifecycle(gpui::AppLifecyclePhase::Background);
            let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
            loop {
                this.poll_settings(cx);
                if this.settings.as_ref().is_some_and(SettingsHost::ready) {
                    break;
                }
                assert!(std::time::Instant::now() < until, "durable reset timeout");
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            assert!(!this.terminal.compact);
            assert_eq!(this.terminal.font.size, None);
            assert_eq!(this.terminal.font.family, "Menlo");
            assert!(this.handoff.copy_on_select);
            assert_eq!(this.actions.board_hidden, prior_edit);
        });
        let durable = store::load(&root).unwrap_or_else(|e| panic!("{e}"));
        assert!(!durable.compact);
        assert_eq!(durable.font.size, None);
        assert_eq!(durable.font.family, stored.font.family);
        assert!(durable.copy_on_select);
        assert_eq!(durable.board_hidden, prior_edit);
        cx.update(|window, cx| {
            view.update(cx, |this, cx| {
                this.end_settings_modal(window, cx);
                this.begin_settings_modal(window, cx);
                assert_eq!(this.settings_values(), durable);
            })
        });
        let reopened = cx.new(crate::app::Smoke::new);
        reopened.update(cx, |this, cx| {
            this.settings =
                Some(SettingsHost::open(root.clone()).unwrap_or_else(|e| panic!("{e}")));
            let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
            loop {
                this.poll_settings(cx);
                if this.settings.as_ref().is_some_and(SettingsHost::ready) {
                    break;
                }
                assert!(std::time::Instant::now() < until, "reload timeout");
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            this.select_host("regular-reopened-host");
            this.poll_settings(cx);
            assert_eq!(this.settings_values(), durable);
        });
        fs::remove_dir_all(root).unwrap_or_else(|e| panic!("{e}"));
    }
}
