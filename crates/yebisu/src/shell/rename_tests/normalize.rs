//! `normalize_name`: one line, trimmed, at most 160 characters.
use crate::shell::rename::normalize::{MAX_CHARS, normalize_name};

#[test]
fn trims_and_flattens() {
    assert_eq!(normalize_name("  build  "), "build");
    assert_eq!(normalize_name("a\nb\tc\u{7}d"), "a b c d");
    assert_eq!(normalize_name("\n\n"), "");
    assert_eq!(normalize_name("repo"), "repo");
}

#[test]
fn cuts_to_160_characters() {
    assert_eq!(MAX_CHARS, 160);
    let long = "x".repeat(161);
    assert_eq!(normalize_name(&long).chars().count(), 160);
    // Japanese with combining marks: each `char` counts.
    let marks = "か\u{3099}".repeat(81);
    let cut = normalize_name(&marks);
    assert_eq!(cut.chars().count(), 160);
    assert!(cut.starts_with("か\u{3099}"));
}
