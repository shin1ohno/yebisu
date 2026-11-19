//! The `peer` field's bookkeeping (UI test redesign S4). The shared slot is
//! written only in a QA build, so these check `Received` and the wire decoder
//! directly.
use super::*;

#[test]
fn a_bracketed_paste_on_the_wire_is_one_text() {
    let mut wire = b"ls\r\x1b[200~sh1-152-clip-E1-core-0\x1b[201~".to_vec();
    wire.extend_from_slice(b"\x1b[200~\xe6\x97\xa5\x1b[201~");
    assert_eq!(bracketed(&wire), ["sh1-152-clip-E1-core-0", "\u{65e5}"]);
    assert!(bracketed(b"plain keys\r").is_empty());
    assert!(
        bracketed(b"\x1b[200~unfinished").is_empty(),
        "a paste without its end is not counted as arrived"
    );
}

#[test]
fn kept_pastes_are_the_newest_and_cut() {
    let mut received = Received::default();
    for index in 0..KEEP + 3 {
        received.paste(&format!("text-{index}"));
    }
    assert_eq!(received.pastes.len(), KEEP);
    assert_eq!(received.pastes.front().map(String::as_str), Some("text-3"));
    assert_eq!(
        received.pastes.back().map(String::as_str),
        Some(format!("text-{}", KEEP + 2).as_str())
    );
    received.paste(&"x".repeat(LONGEST * 2));
    assert_eq!(received.pastes.back().map(String::len), Some(LONGEST));
    received.inputs = 2;
    let field = received.field();
    assert_eq!(field["inputs"], 2);
    assert_eq!(field["pastes"].as_array().map(Vec::len), Some(KEEP));
}
