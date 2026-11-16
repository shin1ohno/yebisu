//! Real SettingsHost persistence with the frozen P11 font/default/save contract.
use super::{
    SettingsHost, Values, store,
    tests::{directory, wait},
};
use crate::notifications::{Policy, Route};
#[test]
fn preload_notification_edit_preserves_loaded_font_and_other_notification_fields() {
    let root = directory("p14-preload");
    let mut stored = Values::default();
    stored.font.size = Some(18.);
    stored.notifications = Policy {
        route: Route::System,
        volume: 25,
        quiet: Some((1320, 480)),
        ..Policy::default()
    };
    store::save(&root, &stored).unwrap_or_else(|e| panic!("fixture save: {e}"));
    let mut owner = SettingsHost::open(root.clone()).unwrap_or_else(|e| panic!("open: {e}"));
    let mut early = Values::default();
    early.notifications.sound = false;
    owner.edit(early);
    wait(&mut owner, SettingsHost::ready);
    let mut expected = stored;
    expected.notifications.sound = false;
    assert_eq!(owner.draft, expected);
    assert_eq!(
        store::load(&root).unwrap_or_else(|e| panic!("read: {e}")),
        expected
    );
    assert!(owner.draft.drag_select);
    assert!(!owner.draft.copy_on_select);
    std::fs::remove_dir_all(root).unwrap_or_else(|e| panic!("cleanup: {e}"));
}

#[test]
fn native_oracle_fixture_matches_real_writer_defaults() -> crate::Result<()> {
    let root = directory("writer-oracle");
    store::save(&root, &Values::default())?;
    let actual: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("yebisu-settings.json"))?)
            .map_err(|e| crate::Error::Settings(e.to_string()))?;
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../scripts/ios/fixtures/settings-writer-default.json"
    ))
    .map_err(|e| crate::Error::Settings(e.to_string()))?;
    assert_eq!(actual, fixture);
    std::fs::remove_dir_all(root)?;
    Ok(())
}
