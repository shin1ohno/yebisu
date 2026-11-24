//! The P12 trace keeps a client message's variant name, never its payload.
use super::*;

#[test]
fn an_unclassified_message_is_named_by_its_variant_only() {
    assert_eq!(client_message_kind(&ClientMessage::Detach), "Detach");
    let secret = b"secret-typed-text".to_vec();
    let kind = client_message_kind(&ClientMessage::Input { data: secret });
    assert_eq!(kind, "Input");
    assert!(!kind.contains("secret"));
}

#[test]
fn a_name_that_is_not_a_plain_identifier_is_unclassified() {
    for value in [
        serde_json::json!("1Detach"),
        serde_json::json!("De tach"),
        serde_json::json!("Detach\n"),
        serde_json::json!("x".repeat(65)),
        serde_json::json!({"bad-name": 1}),
        serde_json::json!({"A": 1, "B": 2}),
        serde_json::json!([1]),
        serde_json::json!(null),
    ] {
        assert_eq!(message_kind(value.clone()), "unclassified", "{value}");
    }
    assert_eq!(
        message_kind(serde_json::json!("x".repeat(64))),
        "x".repeat(64)
    );
    assert_eq!(
        message_kind(serde_json::json!({"Resize": {"cols": 80}})),
        "Resize"
    );
}
