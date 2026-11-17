use super::*;
use crate::app::Smoke;
use gpui::AppContext;
use view::SidebarTap;

#[gpui::test]
fn row_tap_goes_through_input_transition(cx: &mut gpui::TestAppContext) {
    let view = cx.new(Smoke::new);
    view.update(cx, |this, _| {
        this.select_host("A");
        // A discarded composition must be retired after any host change.
        this.terminal.input.retain_discard(true);
        this.retirement.reason = None;
        assert!(this.sidebar_tap(SidebarTap::Host("B".into())).is_ok());
        assert_eq!(this.terminal.reconnect.target, "B");
        assert!(this.board.hidden.contains_key("A"));
        assert!(
            this.retirement.reason.is_some(),
            "the tap did not request input retirement"
        );
        // An agent row of a host without a reader is refused, and still
        // passes through the transition.
        this.retirement.reason = None;
        let refused = this.sidebar_tap(SidebarTap::Agent {
            host: "B".into(),
            pane: "w1:p1".into(),
            boot: "boot-v1".into(),
            generation: 0,
        });
        assert!(refused.is_err());
        assert!(this.retirement.reason.is_some());
    });
}
