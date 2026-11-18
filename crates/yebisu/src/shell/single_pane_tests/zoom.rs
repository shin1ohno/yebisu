//! Rule: a narrow column with a two-pane tab sends Zoom, once.
use super::support::{HOST, connected, measure, ready, show_zoomed, zoom_requests};
use crate::shell::single_pane::{SinglePaneState, ZoomRequest, herdr1_zoom_request};
use gpui::TestAppContext;

#[gpui::test]
fn herdr1_narrow_column_requests_zoom_once(cx: &mut TestAppContext) {
    // The rule: 567 pt is narrow, 568 pt keeps two panes.
    let idle = SinglePaneState::default();
    assert_eq!(
        herdr1_zoom_request(&idle, HOST, 1, 567.0, 2, false),
        Some(ZoomRequest::Zoom)
    );
    assert_eq!(herdr1_zoom_request(&idle, HOST, 1, 568.0, 2, false), None);

    // On the real screen path: the first narrow frame sends one pane.zoom
    // for the focused pane; later frames, also after the host answered and
    // before its snapshot shows the zoom, send nothing more.
    let peer = connected(cx);
    peer.view.update(cx, |this, _| {
        measure(this, 400.0);
        this.enforce_herdr1(400.0);
        this.enforce_herdr1(400.0);
    });
    ready(&peer.view, cx);
    // The host answered, but the snapshot in hand does not show the zoom yet.
    peer.view.update(cx, |this, _| {
        show_zoomed(this, false);
        this.enforce_herdr1(400.0);
    });
    let sent = zoom_requests(&peer);
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert_eq!(sent[0]["params"]["pane_id"], "w1:p1");
    assert_eq!(sent[0]["params"]["mode"], "toggle");
}

#[gpui::test]
fn herdr1_zoom_waits_while_input_would_be_discarded(cx: &mut TestAppContext) {
    let peer = connected(cx);
    // run_action would close the palette and cancel a selection: wait.
    peer.view.update(cx, |this, _| {
        measure(this, 400.0);
        this.actions.palette = true;
        this.enforce_herdr1(400.0);
        this.actions.palette = false;
        this.handoff.select_mode = true;
        this.enforce_herdr1(400.0);
        this.handoff.select_mode = false;
        this.terminal.input.mark("かな", None);
        this.enforce_herdr1(400.0);
        // The Hosts sheet over the terminal takes the typing (review S1).
        this.terminal.input.discard();
        this.shell.hosts_sheet.open_list();
        assert!(this.hosts_sheet_modal());
        this.enforce_herdr1(400.0);
        this.shell.hosts_sheet.close();
    });
    assert!(zoom_requests(&peer).is_empty(), "nothing is sent meanwhile");
    // Keys typed while the surface is still resizing wait in the input queue.
    peer.view.update(cx, |this, _| {
        this.terminal.input.discard();
        this.terminal.input.ready = false;
        this.enforce_herdr1(400.0);
    });
    assert!(zoom_requests(&peer).is_empty(), "held input is not dropped");
    // The first free frame sends it.
    peer.view.update(cx, |this, _| {
        this.terminal.input.discard();
        this.terminal.input.ready = true;
        this.enforce_herdr1(400.0);
    });
    assert_eq!(zoom_requests(&peer).len(), 1);
}
