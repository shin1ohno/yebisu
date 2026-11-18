use super::*;
use sheprd_commands::Command;

/// Review ab783fbc2a944abc B1: L2b retired the actions band, which drew the
/// command palette list. The ... menu's "Command palette" still sets
/// `actions.palette`, and terminal input pauses while it is set
/// (`flush_terminal`), so the list must be on screen whenever it is set,
/// with the touch-only commands (Split Down, Zoom, Close pane, Focus, Next /
/// Previous Pane and Tab, Tab 1-9) in it.
#[gpui::test]
fn palette_open_always_draws_its_list(cx: &mut TestAppContext) {
    let (view, cx, _wire) = connected(cx);
    assert!(
        cx.debug_bounds("action-palette").is_none(),
        "closed palette drawn"
    );
    view.update(cx, |this, cx| {
        assert!(!this.legacy_bands().actions);
        assert!(this.run_action(Command::Palette).is_ok());
        assert!(this.actions.palette);
        assert!(this.palette_list_shown());
        assert!(this.legacy_bands().actions, "palette open, no list slot");
        let commands = this.palette_commands();
        for command in [
            Command::SplitDown,
            Command::Zoom,
            Command::ClosePane,
            Command::FocusLeft,
            Command::NextPane,
            Command::NextTab,
            Command::TabNumber(1),
        ] {
            assert!(commands.contains(&command), "{command:?} not in the list");
        }
        cx.notify();
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("action-palette").is_some(),
        "actions.palette is true but no palette list is drawn"
    );
    view.update(cx, |this, cx| {
        assert!(this.run_action(Command::Palette).is_ok());
        assert!(!this.actions.palette);
        assert!(!this.legacy_bands().actions);
        cx.notify();
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("action-palette").is_none());
}
