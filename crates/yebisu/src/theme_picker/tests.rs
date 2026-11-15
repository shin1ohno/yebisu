use super::*;
use serde_json::json;

#[test]
fn only_implemented_palette_is_available_and_default_stays_local() {
    assert_eq!(SUPPORTED, &[Palette::OwnerDefault, Palette::HighContrast]);
    assert_eq!(Palette::OwnerDefault.colors(), None);
    assert_eq!(
        Palette::HighContrast.colors().map(|c| c.foreground),
        Some(0xffffff)
    );
    assert_eq!(Palette::HighContrast.label(), "High contrast");
    assert_eq!(read_local(&json!({})), Ok(Palette::OwnerDefault));
    for palette in SUPPORTED {
        assert_eq!(Palette::from_key(palette.key()), Ok(*palette));
    }
    for value in ["follow-host", "Nord", "unknown", ""] {
        assert_eq!(Palette::from_key(value), Err(Error::UnsupportedPalette));
    }
}

#[test]
fn pure_local_round_trip_preserves_unrelated_and_unknown_fields() {
    let mut document = json!({"version": 1, "language":"en-US", "drag_select":true,
        "copy_on_select":false, "future":{"keep":[1,2,3]}});
    let original = document.clone();
    assert_eq!(patch_local(&mut document, Palette::OwnerDefault), Ok(()));
    let bytes = serde_json::to_vec(&document).unwrap_or_else(|e| panic!("{e}"));
    let mut restored: serde_json::Value =
        serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(read_local(&restored), Ok(Palette::OwnerDefault));
    if let Some(object) = restored.as_object_mut() {
        object.remove("theme");
    }
    assert_eq!(restored, original);
}

#[test]
fn corrupt_or_unsupported_values_are_not_reset() {
    let unsupported = json!({"theme":"future-theme"});
    assert_eq!(read_local(&unsupported), Err(Error::UnsupportedPalette));
    assert_eq!(read_local(&json!({"theme":5})), Err(Error::InvalidSettings));
    let mut invalid = json!(["do not replace"]);
    let before = invalid.clone();
    assert_eq!(
        patch_local(&mut invalid, Palette::OwnerDefault),
        Err(Error::InvalidSettings)
    );
    assert_eq!(invalid, before);
}
