use super::*;
use crate::terminal_view::TerminalView;
use std::sync::Arc;

#[gpui::test]
fn rail_tab_target_finds_a_disconnected_kept_core_host(cx: &mut gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    view.update(cx, |this, _| {
        this.select_host("alpha");
        this.setup.hosts.push("beta".into());
        let shown: sheprd_client::protocol::ClientShellSnapshot =
            serde_json::from_str(include_str!("../../../fixtures/endpoint-snapshot-v1.json"))
                .unwrap_or_else(|error| panic!("endpoint fixture: {error}"));
        let mut kept = TerminalView::default();
        kept.reconnect.target = "beta".into();
        kept.core_mode = true;
        kept.link.generation = 7;
        kept.live.snapshot = Some(Arc::new(shown));
        this.board.hidden_core.insert("beta".into(), kept);
        let items = rail_items(&this.rail_hosts(), &this.setup);
        let host = items
            .iter()
            .find(|item| item.target == RailTarget::Host("beta".into()))
            .unwrap_or_else(|| panic!("saved host row"));
        assert_eq!(
            this.rail_tab_target(host),
            Some(RailTarget::Space {
                host: "beta".into(),
                kind: HerdrKind::Herdr2,
                workspace_id: "w1".into(),
                boot: "boot-v1".into(),
                generation: 7,
            })
        );
    });
}
