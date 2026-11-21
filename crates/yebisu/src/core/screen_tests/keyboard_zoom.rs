use super::support::{column, saved, screen, size};
use crate::core::screen::{pane_sizes, publish};

#[test]
fn keyboard_zoom_gives_the_focused_pane_the_full_grid_and_restores_the_split() -> crate::Result<()>
{
    let size = size(90, 26)?;
    let split_column = column(990.0, 520.0, (10.0, 20.0));
    let mut zoom_column = split_column;
    zoom_column.focus_only = true;
    let screens = [screen("a", 90, 26), screen("b", 90, 26)];
    for active in 0..2 {
        let sizes = pane_sizes(size, 2, false, active, Some(zoom_column))?;
        assert_eq!(sizes[active], Some(size));
        assert_eq!(sizes[1 - active], None);
        let layout = saved(active, false)?;
        let live = publish(&screens, &layout, size, 1, 1, 1, Some(zoom_column));
        let surface = live
            .surface
            .as_ref()
            .unwrap_or_else(|| panic!("no surface"));
        assert_eq!(surface.panes.len(), 1);
        assert_eq!(surface.panes[0].pane_id, layout.ids[active].as_str());
        assert_eq!(surface.panes[0].rect.width, size.cols());
        let live = publish(&screens, &layout, size, 1, 2, 1, Some(split_column));
        let surface = live
            .surface
            .as_ref()
            .unwrap_or_else(|| panic!("no surface"));
        assert_eq!(surface.panes.len(), 2);
        assert_eq!(surface.panes[active].pane_id, layout.ids[active].as_str());
        assert!(surface.panes[active].focused);
    }
    Ok(())
}
