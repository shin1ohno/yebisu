//! Settings › Sidebar's density is stored with the other two-way choices:
//! Dense is the default and is not written, Roomy survives a restart, and a
//! file without the key or with a name this build does not know loads Dense.
use super::*;
use crate::settings::{
    SettingsHost, Values, store,
    tests::{directory, wait},
};
use std::{fs, path::Path};

fn stored(root: &Path) -> serde_json::Value {
    let bytes = fs::read(root.join("yebisu-settings.json"))
        .unwrap_or_else(|e| panic!("read settings: {e}"));
    serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("parse settings: {e}"))
}

fn write(root: &Path, document: &serde_json::Value) {
    fs::create_dir_all(root).unwrap_or_else(|e| panic!("create {root:?}: {e}"));
    fs::write(root.join("yebisu-settings.json"), document.to_string())
        .unwrap_or_else(|e| panic!("write settings: {e}"));
}

/// The fields every settings file carries (`store::load`).
fn file(extra: &[(&str, serde_json::Value)]) -> serde_json::Value {
    let mut document = serde_json::json!({"version":1,"language":"en-US","compact":false,
        "terminal_priority":false,"board_hidden":false});
    for (key, value) in extra {
        document[*key] = value.clone();
    }
    document
}

fn open_loaded(root: &Path) -> SettingsHost {
    let mut host = SettingsHost::open(root.to_path_buf()).unwrap_or_else(|e| panic!("open: {e}"));
    wait(&mut host, SettingsHost::ready);
    host
}

#[test]
fn sidebar_density_defaults_to_dense_and_is_not_written() {
    assert_eq!(Values::default().sidebar_density, SidebarDensity::Dense);
    let root = directory("density-default");
    store::save(&root, &Values::default()).unwrap_or_else(|e| panic!("save: {e}"));
    assert_eq!(stored(&root).get("sidebar_density"), None);
    let loaded = store::load(&root).unwrap_or_else(|e| panic!("load: {e}"));
    assert_eq!(loaded.sidebar_density, SidebarDensity::Dense);
}

#[test]
fn sidebar_density_roomy_survives_a_restart() {
    let root = directory("density-roomy");
    let mut host = open_loaded(&root);
    let mut values = host.draft.clone();
    values.sidebar_density = SidebarDensity::Roomy;
    host.edit(values);
    wait(&mut host, SettingsHost::ready);
    drop(host);
    assert_eq!(stored(&root)["sidebar_density"], "roomy");
    let restarted = open_loaded(&root);
    assert_eq!(restarted.error, None);
    assert_eq!(restarted.draft.sidebar_density, SidebarDensity::Roomy);
    // Back to Dense removes the key, as for every default choice.
    let mut restarted = restarted;
    let mut values = restarted.draft.clone();
    values.sidebar_density = SidebarDensity::Dense;
    restarted.edit(values);
    wait(&mut restarted, SettingsHost::ready);
    assert_eq!(stored(&root).get("sidebar_density"), None);
}

#[test]
fn sidebar_density_missing_or_unknown_loads_dense() {
    for (name, extra) in [
        ("density-old", vec![]),
        ("density-unknown", vec![("sidebar_density", "cosy".into())]),
        ("density-type", vec![("sidebar_density", 3.into())]),
        (
            "density-roomy-file",
            vec![("sidebar_density", "roomy".into())],
        ),
    ] {
        let root = directory(name);
        write(&root, &file(&extra));
        let loaded = store::load(&root).unwrap_or_else(|e| panic!("{name}: {e}"));
        let expected = if name == "density-roomy-file" {
            SidebarDensity::Roomy
        } else {
            SidebarDensity::Dense
        };
        assert_eq!(loaded.sidebar_density, expected, "{name}");
    }
}

#[test]
fn sidebar_density_chosen_before_load_wins_over_the_file() {
    let root = directory("density-before-load");
    write(&root, &file(&[]));
    let mut host = SettingsHost::open(root.clone()).unwrap_or_else(|e| panic!("open: {e}"));
    let mut values = host.draft.clone();
    values.sidebar_density = SidebarDensity::Roomy;
    host.edit(values);
    wait(&mut host, SettingsHost::ready);
    assert_eq!(host.draft.sidebar_density, SidebarDensity::Roomy);
    assert_eq!(stored(&root)["sidebar_density"], "roomy");
}

fn settle(this: &mut crate::app::Smoke, cx: &mut gpui::Context<crate::app::Smoke>) {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        this.poll_settings(cx);
        if this.settings.as_ref().is_some_and(SettingsHost::ready) {
            break;
        }
        assert!(std::time::Instant::now() < until, "settings did not settle");
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(this.settings_error.is_none(), "{:?}", this.settings_error);
}

fn smoke(root: &Path, cx: &mut gpui::TestAppContext) -> gpui::Entity<crate::app::Smoke> {
    use gpui::AppContext as _;
    let host = SettingsHost::open(root.to_path_buf()).unwrap_or_else(|e| panic!("open: {e}"));
    let view = cx.new(crate::app::Smoke::new);
    view.update(cx, |this, _| this.settings = Some(host));
    view
}

#[gpui::test]
fn sidebar_density_reaches_the_sidebar_and_survives_the_poll(cx: &mut gpui::TestAppContext) {
    let root = directory("density-smoke");
    let view = smoke(&root, cx);
    view.update(cx, |this, cx| {
        settle(this, cx);
        assert_eq!(this.shell.sidebar.density, SidebarDensity::Dense);
        let mut values = this.settings_values();
        values.sidebar_density = SidebarDensity::Roomy;
        this.apply_settings(values, cx);
        // The next polls push the live values; Roomy must stay.
        settle(this, cx);
        assert_eq!(this.shell.sidebar.density, SidebarDensity::Roomy);
        assert_eq!(
            this.settings_values().sidebar_density,
            SidebarDensity::Roomy
        );
    });
    assert_eq!(stored(&root)["sidebar_density"], "roomy");
    let reopened = smoke(&root, cx);
    reopened.update(cx, |this, cx| {
        settle(this, cx);
        assert_eq!(this.shell.sidebar.density, SidebarDensity::Roomy);
    });
}
