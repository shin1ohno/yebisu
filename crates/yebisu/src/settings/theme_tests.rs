use super::{SettingsHost, Values, store, tests::wait};
use crate::{theme::Theme, theme_picker::Palette};
use std::fs;

fn directory(name: &str) -> std::path::PathBuf {
    std::env::var_os("P15_FIXTURE_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/p15-tests")
        })
        .join(format!("{}-{name}", std::process::id()))
}

#[test]
fn explicit_default_before_load_overrides_stored_theme_only() {
    let root = directory("p15-default-race");
    let stored = Values {
        theme: Palette::HighContrast,
        compact: true,
        ..Values::default()
    };
    store::save(&root, &stored).unwrap_or_else(|e| panic!("{e}"));
    let mut host = SettingsHost::open(root.clone()).unwrap_or_else(|e| panic!("{e}"));
    host.choose_theme(Palette::OwnerDefault);
    wait(&mut host, SettingsHost::ready);
    assert_eq!(host.draft.theme, Palette::OwnerDefault);
    assert!(host.draft.compact);
    assert_eq!(
        store::load(&root).unwrap_or_else(|e| panic!("{e}")),
        host.draft
    );
    fs::remove_dir_all(root).unwrap_or_else(|e| panic!("{e}"));
}

#[gpui::test]
fn theme_handler_applies_immediately_and_survives_background_relaunch(
    cx: &mut gpui::TestAppContext,
) {
    use gpui::AppContext as _;
    let root = directory("p15-theme-adapter");
    let initial = Values {
        compact: true,
        terminal_priority: true,
        ..Values::default()
    };
    store::save(&root, &initial).unwrap_or_else(|e| panic!("{e}"));
    let file = root.join("yebisu-settings.json");
    let mut json: serde_json::Value =
        serde_json::from_slice(&fs::read(&file).unwrap_or_else(|e| panic!("{e}")))
            .unwrap_or_else(|e| panic!("{e}"));
    json["future"] = serde_json::json!({"preserve": 42});
    // P14 c639876 preferences.rs encode shape, retained before its integration.
    let notifications = serde_json::json!({"route":"toast","sound":false,
        "volume":37,"muted":true,"quiet":[60,120]});
    json["notifications"] = notifications.clone();
    fs::write(&file, json.to_string()).unwrap_or_else(|e| panic!("{e}"));
    let view = cx.new(crate::app::Smoke::new);
    view.update(cx, |this, cx| {
        this.settings = Some(SettingsHost::open(root.clone()).unwrap_or_else(|e| panic!("{e}")));
        // Exercise the same handler as the mounted palette buttons before initial load.
        this.choose_local_theme(Palette::HighContrast, cx);
        assert_eq!(Theme::from_palette(this.local_theme).background, 0);
        assert_eq!(Theme::from_palette(this.local_theme).foreground, 0xffffff);
        this.settings_lifecycle(gpui::AppLifecyclePhase::Background);
        wait(
            this.settings.as_mut().unwrap_or_else(|| panic!("host")),
            SettingsHost::ready,
        );
        let _content = this.render_p15_settings(cx);
    });
    let saved = store::load(&root).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        saved,
        Values {
            theme: Palette::HighContrast,
            ..initial
        }
    );
    assert!(saved.drag_select);
    assert!(!saved.copy_on_select);
    let json: serde_json::Value =
        serde_json::from_slice(&fs::read(&file).unwrap_or_else(|e| panic!("{e}")))
            .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(json["future"]["preserve"], 42);
    assert_eq!(json["notifications"], notifications);
    assert_eq!(json["theme"], "high-contrast");
    let reopened = cx.new(crate::app::Smoke::new);
    reopened.update(cx, |this, cx| {
        this.settings = Some(SettingsHost::open(root.clone()).unwrap_or_else(|e| panic!("{e}")));
        let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            this.poll_settings(cx);
            if this.settings.as_ref().is_some_and(SettingsHost::ready) {
                break;
            }
            assert!(std::time::Instant::now() < until);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert_eq!(this.settings_values(), saved);
        this.choose_local_theme(Palette::OwnerDefault, cx);
        assert!(Theme::from_palette(this.local_theme) == Theme::default());
        wait(
            this.settings.as_mut().unwrap_or_else(|| panic!("host")),
            SettingsHost::ready,
        );
    });
    fs::remove_dir_all(root).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn unsupported_persisted_theme_blocks_overwrite() {
    let root = directory("p15-theme-future");
    store::save(&root, &Values::default()).unwrap_or_else(|e| panic!("{e}"));
    let path = root.join("yebisu-settings.json");
    let mut json: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap_or_else(|e| panic!("{e}")))
            .unwrap_or_else(|e| panic!("{e}"));
    json["theme"] = serde_json::json!("future-theme");
    let bytes = json.to_string();
    fs::write(&path, &bytes).unwrap_or_else(|e| panic!("{e}"));
    let mut host = SettingsHost::open(root.clone()).unwrap_or_else(|e| panic!("{e}"));
    wait(&mut host, |host| host.loaded);
    host.choose_theme(Palette::HighContrast);
    host.flush();
    assert!(host.blocked);
    assert!(!host.ready());
    assert_eq!(
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("{e}")),
        bytes
    );
    fs::remove_dir_all(root).unwrap_or_else(|e| panic!("{e}"));
}
