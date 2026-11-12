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
