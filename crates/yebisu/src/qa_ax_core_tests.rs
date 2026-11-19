//! The core QA fields stay off outside a Simulator QA build, and the layout
//! value has the shape the Swift tests read (`QAState.core`).
use super::*;

#[test]
fn core_fields_are_off_in_a_host_build() {
    count("pasted_images");
    assert!(COUNTS.with(|counts| counts.borrow().is_empty()));
    let ids = [sheprd_backend::TerminalId::new("t1").map_err(|_| "id")];
    let ids: Vec<_> = ids.into_iter().flatten().collect();
    layout(1, 0, false, &ids);
    assert_eq!(
        layout_field(),
        Value::Null,
        "the worker's layout is not kept"
    );
}

#[test]
fn layout_value_names_panes_active_and_terminals() {
    let value = layout_value(2, 1, true, &["t1", "t2"]);
    assert_eq!(
        value,
        json!({"panes": 2, "active": 1, "vertical": true, "terminals": ["t1", "t2"]})
    );
}
