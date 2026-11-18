use super::{filter_rows, fleet};

fn names(query: &str) -> Vec<String> {
    let rows = fleet();
    filter_rows(&rows, query)
        .into_iter()
        .map(|row| format!("{}:{}", row.identity.host, row.name))
        .collect()
}

#[test]
fn empty_query_keeps_every_row_in_order() {
    assert_eq!(
        names(""),
        ["pro-dev:claude", "pro-dev:Codex", "mini:claude"]
    );
    assert_eq!(names("   "), names(""));
}

#[test]
fn query_matches_name_or_host_ignoring_case() {
    assert_eq!(names("CLAUDE"), ["pro-dev:claude", "mini:claude"]);
    assert_eq!(names("codex"), ["pro-dev:Codex"]);
    assert_eq!(names("mini"), ["mini:claude"]);
    assert_eq!(names("pro-dev"), ["pro-dev:claude", "pro-dev:Codex"]);
}

#[test]
fn every_word_must_match() {
    assert_eq!(names("pro-dev claude"), ["pro-dev:claude"]);
    assert!(names("mini codex").is_empty());
}
