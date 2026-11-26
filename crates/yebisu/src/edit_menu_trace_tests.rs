use super::*;

#[test]
fn windows_are_named_by_kind_not_by_class() {
    assert_eq!(window_kind("UIWindow"), "app");
    assert_eq!(window_kind("UITextEffectsWindow"), "text_effects");
    assert_eq!(window_kind("UIRemoteKeyboardWindow"), "other");
}

#[test]
fn only_uikit_edit_menu_views_are_counted() {
    for class in [
        "_UIEditMenuContainerView",
        "_UIEditMenuListView",
        "_UIEditMenuPageButton",
    ] {
        assert!(is_edit_menu_view(class), "{class}");
    }
    for class in ["UIView", "_UIContextMenuView", "UIEditMenuInteraction"] {
        assert!(!is_edit_menu_view(class), "{class}");
    }
}

#[test]
fn the_first_responder_is_named_by_kind() {
    assert_eq!(responder_kind(None), "none");
    assert_eq!(
        responder_kind(Some(
            "herdr_uikit::window::text_input::view_class::GPUITextInputView0.1.0"
        )),
        "text_input"
    );
    assert_eq!(responder_kind(Some("GPUIView")), "drawing");
    assert_eq!(responder_kind(Some("UITextField")), "other");
}

#[test]
fn a_window_record_carries_codes_and_counts_only() {
    let record = window_record("UIWindow", true, 5, Some("GPUIView"));
    assert_eq!(
        record,
        serde_json::json!({"window": "app", "key": true, "menu_views": 5, "responder": "drawing"})
    );
    // A class name never reaches the record.
    let other = window_record("SecretPanelWindow", false, 0, Some("SecretFieldEditor"));
    let text = other.to_string();
    assert!(!text.contains("Secret"), "{text}");
}

#[test]
fn the_trace_is_taken_at_fixed_delays() {
    assert_eq!(DELAYS_MS, [30, 300, 1000, 3000]);
}
