use super::{Page, device_key::KeyView, fonts::Density, general::Document};
use crate::settings::Values;

#[test]
fn pages_are_the_supported_allowlist() {
    assert_eq!(
        Page::ALL.map(Page::id),
        ["fonts", "keyboard", "ipad-key", "general", "notifications"]
    );
}

#[test]
fn font_density_keeps_owner_metrics_and_persisted_field() {
    let mut values = Values::default();
    assert_eq!(Density::from_values(&values), Density::Regular);
    assert_eq!(Density::Regular.metrics().size_px, 14.0);
    assert_eq!(Density::Regular.metrics().line_height_px, 20.0);
    Density::Compact.apply(&mut values);
    assert!(values.compact);
    assert_eq!(Density::from_values(&values), Density::Compact);
    assert_eq!(Density::Compact.metrics().size_px, 12.0);
    assert_eq!(Density::Compact.metrics().line_height_px, 17.0);
}

#[test]
fn device_key_copy_is_the_formatted_public_line_only() {
    let formatted = "restrict ssh-ed25519 YWJjZA== yebisu-M5".to_owned();
    let key = KeyView::from_formatted_worker_line(formatted.clone());
    assert!(matches!(&key, KeyView::Ready(_)));
    assert_eq!(key.copy_text(false), Some(formatted.as_str()));
    assert!(KeyView::Ready(formatted).copy_text(true).is_none());
}

#[test]
fn legal_text_is_bundled_and_nonempty() {
    assert!(!Document::License.text().trim().is_empty());
    assert!(!Document::Notice.text().trim().is_empty());
}
