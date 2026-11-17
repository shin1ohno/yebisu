//! Wave 0 guard: every skeleton slot is inert, so no screen changes until
//! its stream fills it. Owner: integrator. Streams keep their own tests in
//! their own `*_tests.rs` files and delete the line here that their change
//! makes false, through a patch to the integrator.
use super::*;

#[test]
fn wave0_slots_are_inert() {
    let state = ChromeState::default();
    let legacy = entry_move::LegacyBands::current();
    assert!(legacy.toolbar && legacy.input_options && legacy.return_policy);
    assert!(legacy.handoff && legacy.actions && legacy.qa_controls);
    assert!(entry_points::chrome_entry_points(&state, legacy).is_empty());
    assert!(overflow_menu::overflow_menu_items(&state).is_empty());
    assert!(footer::footer_items(HerdrKind::Herdr1).is_empty());
    assert!(edit_bar::edit_bar_items(true).is_empty());
    assert!(edit_bar::paused_typing_status(true).is_none());
    assert!(mods::mods_item(2).is_none());
    assert!(overlay::toast_slot().is_none());
    assert!(!qa_strip::qa_strip_enabled());
    assert!(fit::status_bar_overflow(0.0).is_empty());
}

#[test]
fn shell_state_defaults() {
    let shell = ShellState::default();
    assert_eq!(format!("{:?}", shell.frame), "FrameState");
    assert_eq!(HerdrKind::default(), HerdrKind::Herdr1);
}
