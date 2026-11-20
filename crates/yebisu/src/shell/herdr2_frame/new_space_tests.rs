use super::*;

#[gpui::test]
fn new_space_asks_for_the_host_and_the_herdr(cx: &mut gpui::TestAppContext) {
    let hosts = host_choices(
        &["me@pro-dev.home.local".to_owned(), "mini".to_owned()],
        [
            ("mini".into(), HerdrKind::Herdr1),
            ("me@pro-dev.home.local".into(), HerdrKind::Herdr1),
            ("me@pro-dev.home.local".into(), HerdrKind::Herdr2),
        ],
    );
    let mut chooser = Chooser::new(hosts);
    assert_eq!(
        chooser.selection(),
        None,
        "the host must be chosen on every invocation"
    );
    chooser.select_host(0);
    assert_eq!(chooser.hosts[0].name.display(), "pro-dev");
    assert_eq!(
        chooser.selection(),
        Some(("me@pro-dev.home.local", HerdrKind::Herdr2))
    );
    chooser.select_kind(HerdrKind::Herdr1);
    assert_eq!(
        chooser.selection(),
        Some(("me@pro-dev.home.local", HerdrKind::Herdr1))
    );
    chooser.select_host(1);
    chooser.select_kind(HerdrKind::Herdr2);
    assert_eq!(chooser.selection(), Some(("mini", HerdrKind::Herdr1)));
    chooser.select_host(0);
    assert_eq!(
        chooser.selection(),
        Some(("me@pro-dev.home.local", HerdrKind::Herdr2))
    );

    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    view.update(cx, |this, cx| {
        this.setup.hosts = vec!["me@pro-dev.home.local".into()];
        this.terminal.reconnect.target = "me@pro-dev.home.local".into();
        this.terminal.open = true;
        this.terminal.live.connected = true;
        this.shell
            .herdr2
            .select_session("me@pro-dev.home.local".into(), "default".into());
        this.open_new_space(cx);
    });
    cx.run_until_parked();
    let row = cx
        .debug_bounds("new-space-host-0")
        .unwrap_or_else(|| panic!("host chooser must be drawn"));
    cx.simulate_click(row.center(), gpui::Modifiers::none());
    view.read_with(cx, |this, _| {
        assert_eq!(
            this.shell
                .herdr2
                .new_space
                .as_ref()
                .and_then(Chooser::selection),
            Some(("me@pro-dev.home.local", HerdrKind::Herdr2))
        );
    });
}
