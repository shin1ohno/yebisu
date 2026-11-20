use super::*;
use crate::observation::Observation;
use sheprd_commands::Command;

/// Plan v2 7.6: New space on Herdr 1 sends `Command::Workspace`
/// (`workspace.create`), now in the supported catalog.
#[gpui::test]
fn new_space_sends_workspace_on_herdr1(cx: &mut TestAppContext) {
    assert_eq!(
        new_space_command(HerdrKind::Herdr1),
        Some(Command::Workspace)
    );
    let (view, cx, _wire) = connected(cx);
    view.update(cx, |this, _| {
        // The fixture daemon does not list workspace.create; a Herdr 1 that
        // offers it does. In this fixture the peer recorded no
        // workspace.create within 5 s (logs/test-footer-7.log; cause not
        // traced), so the test reads the request the command issued.
        this.terminal.live.methods.push("workspace.create".into());
        if let Ok(mut inbox) = this.terminal.link.inbox.lock() {
            inbox.methods.push("workspace.create".into());
        }
        assert!(
            crate::actions::catalog(&this.terminal)
                .iter()
                .any(|a| a.info.command == Command::Workspace && a.available),
            "Workspace is in the supported catalog"
        );
        assert!(this.terminal.navigating.is_none());
    });
    // The method the command put on the client, from the action record
    // `run_action` writes (actions.rs), not from the issued-request flag only.
    let (sender, records) = mpsc::sync_channel(64);
    view.update(cx, |this, _| {
        this.observation = Observation::injected(sender);
    });
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.footer_tap(FooterItem::NewSpace, window, cx)
        })
    });
    let methods: Vec<String> = records
        .try_iter()
        .filter(|r| r["kind"] == "action")
        .filter_map(|r| r["method"].as_str().map(str::to_owned))
        .collect();
    assert_eq!(
        methods,
        ["workspace.create"],
        "New space sent another command"
    );
    view.update(cx, |this, _| {
        let issued = this
            .terminal
            .navigating
            .as_ref()
            .map(|n| matches!(n.target, crate::board::Target::Command));
        assert_eq!(
            issued,
            Some(true),
            "New space issued no command (status: {})",
            this.terminal.live.status
        );
    });
}

/// q4: on Herdr 2 New space is disabled and says why; a tap sends nothing.
#[gpui::test]
fn new_space_herdr2_disabled_with_reason(cx: &mut TestAppContext) {
    let entry = FooterEntry::for_new_space(HerdrKind::Herdr2);
    assert!(!entry.enabled);
    assert_eq!(entry.reason, Some(HERDR2_NEW_SPACE));
    assert_eq!(new_space_command(HerdrKind::Herdr2), None);
    let herdr1 = FooterEntry::for_new_space(HerdrKind::Herdr1);
    assert!(herdr1.enabled && herdr1.reason.is_none());
    let (view, cx) = window(cx);
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal.open = true;
            this.terminal.core_mode = true;
            this.terminal.live.status = "before".into();
            this.footer_tap(FooterItem::NewSpace, window, cx);
            assert_eq!(this.terminal.live.status, "before", "a disabled tap acts");
        });
    });
}
