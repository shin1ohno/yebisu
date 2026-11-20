use crate::app::Smoke;
use gpui::TestAppContext;

#[gpui::test]
fn rail_column_builds_without_a_host(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    view.update(cx, |this, cx| {
        // The frame calls `render_rail` once the sidebar is collapsed; with no
        // host there are no rows, only the fixed footer.
        assert!(this.render_rail(0.0, cx).is_some());
        assert!(this.rail_hosts().is_empty());
        // Drawing twice records the measured widths only once.
        assert!(this.render_rail(0.0, cx).is_some());
        assert!(this.shell.rail.widths_recorded);
    });
}

#[gpui::test]
fn rail_hosts_list_the_selected_host(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    view.update(cx, |this, cx| {
        this.select_host("sandbox");
        let hosts = this.rail_hosts();
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].name, "sandbox");
        assert!(hosts[0].current && !hosts[0].connected);
        assert!(this.render_rail(0.0, cx).is_some());
    });
}
