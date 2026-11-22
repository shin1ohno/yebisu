//! Herdr 2: what `enforce_pane_mode` publishes, and when it makes the next
//! paint send the terminal size again (review B1). A detached mailbox keeps
//! these tests away from the process-wide one.
use super::support::measure;
use crate::{
    layout::PaneMode,
    shell::single_pane::{ColumnMetrics, Mailbox},
};
use gpui::TestAppContext;
use sheprd_client::ConnectOptions;

fn column(width: f32) -> ColumnMetrics {
    ColumnMetrics {
        width,
        height: 600.0,
        cell_width: 10.0,
        cell_height: 20.0,
        focus_only: false,
    }
}

#[gpui::test]
fn a_changed_column_is_published_and_forgets_the_sent_size(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| crate::app::Smoke::new(cx));
    let mailbox = Mailbox::detached();
    view.update(cx, |this, _| {
        this.shell.single_pane.set_mailbox(mailbox.clone());
        this.terminal.core_mode = true;
        this.terminal.open = true;
    });
    let frame = |cx: &mut gpui::VisualTestContext, width: f32| {
        cx.update(|window, cx| {
            view.update(cx, |this, cx| {
                // The paint of the previous frame measured `width` and sent
                // the size it had.
                measure(this, width);
                this.terminal.geometry.cell_width = 10.0;
                this.terminal.geometry.cell_height = 20.0;
                this.terminal.size = Some(ConnectOptions::default());
                this.enforce_pane_mode(window, cx);
                this.terminal.size.is_none()
            })
        })
    };
    // First connect: nothing was published, so the first size was sent blind.
    assert!(frame(cx, 500.0), "the first column resends the size");
    assert_eq!(mailbox.latest(), Some(column(500.0)));
    // The same column: the size stays.
    assert!(!frame(cx, 500.0), "an unchanged column sends nothing");
    // Across the line, and in the 560-567 pt band under it.
    for width in [800.0, 567.0, 560.0, 568.0] {
        assert!(frame(cx, width), "{width} pt");
        assert_eq!(mailbox.latest(), Some(column(width)));
        let single = crate::layout::pane_mode(width, 2) == PaneMode::Single;
        assert_eq!(single, width < 568.0);
    }
}

#[gpui::test]
fn only_the_measured_docked_keyboard_publishes_focus_only_at_normal_width(cx: &mut TestAppContext) {
    use gpui::{Edges, WindowInsets, px, size};

    let (view, cx) = cx.add_window_view(|_, cx| crate::app::Smoke::new(cx));
    let mailbox = Mailbox::detached();
    cx.simulate_resize(size(px(1376.0), px(1032.0)));
    view.update(cx, |this, _| {
        this.shell.single_pane.set_mailbox(mailbox.clone());
        this.terminal.core_mode = true;
        this.terminal.open = true;
    });
    for (bottom, left, expected) in [
        (0.0, 0.0, false),
        (71.0, 0.0, false),
        (498.0, 0.0, true),
        (280.0, 300.0, false),
        (20.0, 0.0, false),
    ] {
        view.update_in(cx, |this, window, cx| {
            measure(this, 920.0);
            this.shell.fit.insets = Some(WindowInsets {
                ime: Edges {
                    bottom: px(bottom),
                    left: px(left),
                    ..Edges::default()
                },
                ..WindowInsets::default()
            });
            this.enforce_pane_mode(window, cx);
        });
        assert_eq!(
            mailbox.latest().map(|column| column.focus_only),
            Some(expected)
        );
    }
}
