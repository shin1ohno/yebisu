use super::*;

fn record() -> Record {
    Record {
        selected: Some(Entry {
            host: "qa@two.invalid".into(),
            kind: HerdrKind::Herdr2,
            space: Some("space-a".into()),
        }),
        others: vec![Entry {
            host: "qa@one.invalid".into(),
            kind: HerdrKind::Herdr1,
            space: None,
        }],
    }
}

#[test]
fn a_record_survives_encode_and_decode() {
    let record = record();
    assert_eq!(decode(&encode(&record)).ok(), Some(record));
    let empty = Record::default();
    assert_eq!(decode(&encode(&empty)).ok(), Some(empty));
}

#[test]
fn malformed_records_are_refused() {
    let refused = |json: &str| decode(json.as_bytes()).is_err();
    assert!(refused("not json"));
    assert!(refused(r#"{"version":2,"selected":null,"others":[]}"#));
    assert!(refused(r#"{"version":1,"selected":null}"#));
    // A Herdr 1 entry has no space; a Herdr 2 entry needs one.
    assert!(refused(
        r#"{"version":1,"selected":{"host":"h","kind":"herdr1","space":"s"},"others":[]}"#
    ));
    assert!(refused(
        r#"{"version":1,"selected":{"host":"h","kind":"herdr2","space":null},"others":[]}"#
    ));
    assert!(refused(
        r#"{"version":1,"selected":{"host":"h","kind":"herdr3","space":null},"others":[]}"#
    ));
    // Untrusted text: control characters and padded names.
    assert!(refused(
        r#"{"version":1,"selected":{"host":"h\nx","kind":"herdr1","space":null},"others":[]}"#
    ));
    assert!(refused(
        r#"{"version":1,"selected":{"host":"h","kind":"herdr2","space":" s"},"others":[]}"#
    ));
    assert!(decode(&vec![b' '; MAX_BYTES + 1]).is_err());
}

#[test]
fn the_file_is_saved_atomically_and_a_missing_file_is_no_record() {
    let root = std::env::temp_dir().join(format!("yebisu-launch-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    assert!(matches!(load(&root), Ok(None)));
    save(&root, &record()).unwrap_or_else(|e| panic!("save: {e}"));
    assert_eq!(load(&root).ok().flatten(), Some(record()));
    assert!(
        !root.join(PENDING).exists(),
        "the pending file was not renamed"
    );
    fs::write(root.join(FILE), b"{").unwrap_or_else(|e| panic!("corrupt: {e}"));
    assert!(load(&root).is_err());
    let _ = fs::remove_dir_all(&root);
}
