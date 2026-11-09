// Modified from shin1ohno/herdr-gpui crates/sheprd-gpui/src/osc52.rs at 398c132a06723a0cc1494842a34666d173ec0200
use super::*;

#[test]
fn decodes_padded_unpadded_and_multibyte_payloads() {
    assert_eq!(decode("dGVzdA==").as_deref(), Some("test"));
    assert_eq!(decode("dGVzdA").as_deref(), Some("test"));
    assert_eq!(decode("aGVsbG8gd29ybGQ=").as_deref(), Some("hello world"));
    assert_eq!(decode("5Lmf5LiN").as_deref(), Some("\u{4e5f}\u{4e0d}"));
    // An empty payload is a deliberate clipboard clear, not an error.
    assert_eq!(decode("").as_deref(), Some(""));
}

#[test]
fn rejects_malformed_and_noncanonical_payloads() {
    for data in [
        "dGVzdA=",   // wrong padding for the symbol count
        "dGVzdA===", // more padding than a quantum allows
        "dGVzd!",    // outside the alphabet
        "dGVzdA==x", // data after padding
        "=",         // no symbols at all
        "A",         // a lone symbol carries no byte
        "AB",        // a two-symbol quantum with non-zero trailing bits
    ] {
        assert_eq!(decode(data), None, "{data}");
    }
}

#[test]
fn rejects_non_utf8_and_oversized_payloads() {
    // 0xff is a byte, not valid UTF-8 text.
    assert_eq!(decode("/w=="), None);
    // Four symbols carry three bytes, so this is the largest payload that
    // still fits the limit; one more quantum spills past it.
    let allowed = "AAAA".repeat(MAX_TEXT_BYTES / 3);
    assert!(decode(&allowed).is_some());
    let too_long = format!("{allowed}AAAA");
    assert_eq!(decode(&too_long), None);
}
