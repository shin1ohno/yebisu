//! SH1-150: real Herdr 1 full/patch cells and the Herdr 2 skip shape.
use crate::{app::Smoke, handoff::Selection, mock_peer};
use gpui::{ClipboardItem, MouseButton, TestAppContext, point, px, size};
use sheprd_client::{
    ClientEvent,
    protocol::{CellData, FrameData},
};
use std::sync::Arc;

fn frame(symbols: &[&str], skip: bool) -> FrameData {
    FrameData {
        width: symbols.len() as u16,
        height: 1,
        cells: symbols
            .iter()
            .map(|s| CellData {
                symbol: (*s).into(),
                fg: 0,
                bg: 0,
                modifier: 0,
                skip: skip && s.is_empty(),
                hyperlink: None,
            })
            .collect(),
        cursor: None,
        hyperlinks: vec![],
        graphics: vec![],
    }
}
fn select(frame: &FrameData, anchor: u16, end: u16) -> Selection {
    Selection {
        frame: Arc::new(frame.clone()),
        anchor: (anchor, 0),
        end: (end, 0),
        dragging: false,
    }
}
#[test]
fn sh1_150_herdr1_full() {
    check_cjk(" ", " ", " ", false);
}
#[test]
fn sh1_150_herdr1_patch() {
    check_cjk("", "", "", false);
}
#[test]
fn sh1_150_herdr1_mixed() {
    check_cjk(" ", "", " ", false);
}
#[test]
fn sh1_150_herdr2_skip() {
    check_cjk("", "", "", true);
}
fn check_cjk(a: &str, b: &str, c: &str, skip: bool) {
    let f = frame(&["日", a, "本", b, "語", c, " ", "A", "B", "C"], skip);
    for (start, end) in [(1, 9), (9, 1)] {
        let s = select(&f, start, end);
        assert_eq!(s.text().as_deref(), Some("日本語 ABC"));
        assert_eq!(s.spans(), vec![(0, 0..10)]);
        assert_eq!(s.highlights()[0].columns, 0..10);
    }
    for (start, end) in [(1, 3), (3, 1)] {
        let s = select(&f, start, end);
        assert_eq!(s.text().as_deref(), Some("日本"));
        assert_eq!(s.spans(), vec![(0, 0..4)]);
    }
    assert_eq!(select(&f, 3, 3).text().as_deref(), Some("本"));
}
#[test]
fn sh1_150_vs16_preserves_real_space_and_covered_text() {
    for spacer in ["", " "] {
        let f = frame(&["日", spacer, "✈️", "", " ", "X"], false);
        assert_eq!(select(&f, 1, 5).text().as_deref(), Some("日✈️ X"));
        assert_eq!(select(&f, 3, 3).text().as_deref(), Some("✈️"));
    }
    // Sequence-only width does not swallow a real space or nonempty cell.
    let f = frame(&["日", "", "✈️", " ", "X"], false);
    assert_eq!(select(&f, 1, 4).text().as_deref(), Some("日✈️ X"));
    let f = frame(&["日", "", "✈️", "X"], false);
    assert_eq!(select(&f, 1, 3).text().as_deref(), Some("日✈️X"));
}

#[gpui::test]
fn sh1_150_real_selection_and_clipboard_adapter(cx: &mut TestAppContext) {
    for spacer in [" ", ""] {
        let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
        cx.simulate_resize(size(px(1200.), px(1000.)));
        view.update(cx, |this, cx| {
            this.terminal.open = true;
            this.terminal.live.apply(ClientEvent::Snapshot(Arc::new(
                serde_json::from_str(mock_peer::SNAPSHOT)
                    .unwrap_or_else(|e| panic!("fixture: {e}")),
            )));
            let mut surface = mock_peer::surface(7);
            // Keep the real surface geometry and populate one known row.
            for (x, s) in ["日", spacer, "本", spacer, "語", spacer, " ", "A", "B", "C"]
                .iter()
                .enumerate()
            {
                surface.frame.cells[160 + x].symbol = (*s).into();
                surface.frame.cells[160 + x].skip = false;
            }
            this.terminal
                .live
                .apply(ClientEvent::Surface(Arc::new(surface)));
            this.terminal.live.connected = true;
            this.terminal.link.inbox = Arc::new(std::sync::Mutex::new(this.terminal.live.clone()));
            this.handoff.select_mode = true;
            cx.notify();
        });
        cx.run_until_parked();
        let g = view.update(cx, |this, _| this.terminal.geometry);
        let at =
            |x: f32| g.grid.origin + point(px((x + 0.5) * g.cell_width), px(2.5 * g.cell_height));
        cx.write_to_clipboard(ClipboardItem::new_string("sentinel".into()));
        cx.simulate_mouse_down(at(1.), MouseButton::Left, Default::default());
        cx.simulate_mouse_move(at(9.), Some(MouseButton::Left), Default::default());
        cx.simulate_mouse_up(at(9.), MouseButton::Left, Default::default());
        view.update(cx, |this, cx| {
            let s = this
                .handoff
                .selection
                .as_ref()
                .unwrap_or_else(|| panic!("selection"));
            assert_eq!(s.highlights()[0].columns, 0..10);
            this.copy_selection(cx);
            assert_eq!(this.handoff.clipboard_writes, 1);
        });
        assert_eq!(
            cx.read_from_clipboard()
                .and_then(|item| item.text())
                .as_deref(),
            Some("日本語 ABC")
        );
    }
}
