//! Wave 0 guard: every skeleton slot is inert, so no screen changes until
//! its stream fills it. Owner: integrator. Streams keep their own tests in
//! their own `*_tests.rs` files and delete the line here that their change
//! makes false, through a patch to the integrator.
use super::*;

#[test]
fn wave0_slots_are_inert() {
    assert!(!qa_strip::qa_strip_enabled());
}

#[test]
fn shell_state_defaults() {
    let shell = ShellState::default();
    assert!(format!("{:?}", shell.frame).starts_with("FrameState"));
    assert_eq!(HerdrKind::default(), HerdrKind::Herdr1);
}
