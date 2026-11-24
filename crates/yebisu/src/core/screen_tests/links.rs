//! L66: each Herdr 2 terminal numbers its OSC 8 hyperlinks into its own
//! table. The composed frame keeps one table, so a press on a pane's link
//! must still read that pane's own target.
use super::support::{cell, saved, screen, size};
use crate::{
    core::screen::{Screen, publish_for},
    terminal_links::{Target, pane_hyperlink_at, pane_link_at},
};

/// Writes `text` at the start of `screen`'s first row as a hyperlink to
/// `target`, the second entry of the screen's own table.
fn link(screen: &mut Screen, text: &str, target: &str) {
    let Some(frame) = screen.frame.as_mut() else {
        panic!("fixture screen has a frame");
    };
    frame.hyperlinks = vec!["https://unused.test/".into(), target.into()];
    for (index, ch) in text.chars().enumerate() {
        let mut linked = cell(&ch.to_string());
        linked.hyperlink = Some(1);
        frame.cells[index] = linked;
    }
}

#[test]
fn each_pane_keeps_its_own_hyperlinks() -> crate::Result<()> {
    let mut screens = vec![screen(" ", 40, 10), screen(" ", 40, 10)];
    link(&mut screens[0], "left docs", "https://example.com/left");
    link(&mut screens[1], "right docs", "https://example.com/right");
    let live = publish_for(&screens, &saved(1, false)?, size(81, 10)?, (1, 1, 0), None);
    let Some(surface) = live.surface.as_ref() else {
        panic!("published a surface");
    };
    assert_eq!(surface.panes.len(), 2, "two panes drawn");
    for (pane, (text, want)) in surface.panes.iter().zip([
        ("left docs", "https://example.com/left"),
        ("right docs", "https://example.com/right"),
    ]) {
        let rect = pane.inner_rect;
        assert!(pane_hyperlink_at(surface, rect.x + 2, rect.y), "{want}");
        let Some(found) = pane_link_at(surface, rect.x + 2, rect.y) else {
            panic!("no link read in pane {}", pane.pane_id);
        };
        assert_eq!(found.target, Target::Web(want.into()));
        let end = rect.x + u16::try_from(text.len()).unwrap_or(u16::MAX);
        assert_eq!(found.columns, rect.x..end, "{want}");
    }
    Ok(())
}
