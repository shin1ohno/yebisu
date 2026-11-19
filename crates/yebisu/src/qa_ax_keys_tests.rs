use super::{copied_fields, sha256_hex};
use serde_json::{Value, json};

#[test]
fn sha256_hex_matches_the_standard_vectors() {
    // FIPS 180-2 "abc" and the empty input, as hashlib.sha256().hexdigest() prints them.
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn copied_fields_carry_the_digest_and_length_never_the_text() {
    let line = b"restrict ssh-ed25519 RkFLRV9QMl9RQQ== yebisu-injected-qa";
    let fields = copied_fields(2, Some(line));
    let names: Vec<&str> = fields.iter().map(|(key, _)| *key).collect();
    assert_eq!(names, ["copied", "pasteboard_sha256", "pasteboard_bytes"]);
    assert_eq!(fields[0].1, json!(2));
    assert_eq!(fields[1].1, json!(sha256_hex(line)));
    assert_eq!(fields[2].1, json!(line.len()));
    let published = Value::Object(
        fields
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
    .to_string();
    assert!(!published.contains("RkFLRV9QMl9RQQ"), "{published}");
    for (key, _) in copied_fields(1, None) {
        assert!(
            !crate::qa_ax::layer::RESERVED.contains(&key),
            "{key} is the layer's own"
        );
    }
}

#[test]
fn an_empty_pasteboard_is_null_not_a_digest_of_nothing() {
    let fields = copied_fields(1, None);
    assert!(fields[1].1.is_null() && fields[2].1.is_null());
}
