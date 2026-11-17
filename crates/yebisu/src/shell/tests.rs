//! Wave 0 guard: every skeleton slot is inert, so no screen changes until
//! its stream fills it. Owner: integrator. Streams keep their own tests in
//! their own `*_tests.rs` files and delete the line here that their change
//! makes false, through a patch to the integrator.
use super::*;

#[test]
fn wave0_slots_are_inert() {
    let legacy = entry_move::LegacyBands::current();
    assert!(legacy.toolbar && legacy.input_options && legacy.return_policy);
    assert!(legacy.handoff && legacy.actions && legacy.qa_controls);
    assert!(footer::footer_items(HerdrKind::Herdr1).is_empty());
    assert!(!qa_strip::qa_strip_enabled());
}

#[test]
fn shell_state_defaults() {
    let shell = ShellState::default();
    assert!(format!("{:?}", shell.frame).starts_with("FrameState"));
    assert_eq!(HerdrKind::default(), HerdrKind::Herdr1);
}
