//! Issue #137 PR 4: wheel scrolling drawn between rows on Herdr 1
//! scrollback (smooth scroll), its gate, and hit testing during a slide.
use super::*;
use crate::input_geometry::InputGeometry;
use gpui::Bounds;
use sheprd_client::protocol::{
    ClientShellPopupSurface, SurfaceGraphicsAssetKey, SurfaceGraphicsFormat,
    SurfaceGraphicsPlacement, SurfaceGraphicsSource, SurfaceGraphicsTarget,
};

/// One pane over the frame with 500 rows of scrollback, at the bottom.
fn scrollback(edit: impl FnOnce(&mut PaneSurfacePane)) -> Vec<PaneSurfacePane> {
    single(|pane| {
        pane.scroll = Some(PaneSurfaceScrollMetrics {
            offset_from_bottom: 0,
            max_offset_from_bottom: 500,
            viewport_rows: 24,
        });
        edit(pane);
    })
}

/// Shows `panes` and paints them: the presentation records the surface a
/// paint draws, as the app paints after every surface it takes.
fn shown(view: &Entity<Smoke>, cx: &mut VisualTestContext, panes: Vec<PaneSurfacePane>) {
    install(view, cx, panes);
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
}

fn moving(view: &Entity<Smoke>, cx: &mut VisualTestContext) -> bool {
    view.read_with(cx, |this, _| {
        this.terminal.presentation.scroll.moving(Instant::now())
    })
}

/// T22: the smooth path asks Herdr for each row the motion enters and
/// keeps the fraction in the slide; the whole-line accumulator must not
/// count that motion again once the pane reads the wheel instead.
#[gpui::test]
fn smooth_scroll_asks_for_entered_rows_and_drops_the_whole_line_remainder(cx: &mut TestAppContext) {
    let (view, cx, wire) = connected(cx);
    shown(&view, cx, scrollback(|_| {}));
    let at = over(&view, cx, FOCUSED);
    let row = cell_height(&view, cx);
    scroll(&view, cx, at, pixels(0., 0.75 * row), TouchPhase::Started);
    assert_eq!(
        wheels(&view, cx, &wire),
        [up(FOCUSED, 1)],
        "the row entered"
    );
    assert!(moving(&view, cx), "the fraction is drawn, not sent");
    scroll(&view, cx, at, pixels(0., 0.1 * row), TouchPhase::Moved);
    assert_eq!(wheels(&view, cx, &wire), [], "still inside the asked row");
    // The program starts reading the mouse: whole steps from here on, and
    // the 0.85 row the slide spent is not counted again.
    shown(&view, cx, scrollback(|pane| pane.mouse_reporting = true));
    scroll(&view, cx, at, pixels(0., 0.5 * row), TouchPhase::Moved);
    assert_eq!(wheels(&view, cx, &wire), [], "0.5 row is no whole step");
}

fn placement() -> SurfaceGraphicsPlacement {
    SurfaceGraphicsPlacement {
        asset: SurfaceGraphicsAssetKey {
            source: SurfaceGraphicsSource::Terminal {
                target: SurfaceGraphicsTarget::Pane {
                    pane_id: FOCUSED.into(),
                },
                image_id: 1,
            },
            image_width: 8,
            image_height: 8,
            format: SurfaceGraphicsFormat::Rgba,
            data_len: 256,
            data_fingerprint: 1,
        },
        logical_placement_id: 1,
        x: 30,
        y: 2,
        cols: 8,
        rows: 3,
        source_x: 0,
        source_y: 0,
        source_width: 0,
        source_height: 0,
        x_offset: 0,
        y_offset: 0,
        z: 1,
        scrollback_offset: 0,
    }
}

/// T23: images and a popup keep whole rows; plain Herdr 1 scrollback slides.
#[gpui::test]
fn slides_need_herdr1_without_images_or_a_popup(cx: &mut TestAppContext) {
    let (view, cx, _wire) = connected(cx);
    install(&view, cx, scrollback(|_| {}));
    view.update(cx, |this, _| {
        let Some(shown) = this.terminal.live.surface.as_ref() else {
            panic!("surface");
        };
        let plain = (**shown).clone();
        assert!(this.slides_allowed(&plain), "plain scrollback slides");
        let mut images = plain.clone();
        images.graphics.placements.push(placement());
        assert!(!this.slides_allowed(&images), "an image never slides");
        let mut popup = plain.clone();
        popup.popup = Some(Box::new(ClientShellPopupSurface {
            terminal_id: "popup".into(),
            title: "popup".into(),
            width: None,
            height: None,
            frame: plain.frame.clone(),
            mouse_reporting: false,
            sgr_pixel_mouse: false,
            pixel_width: 0,
            pixel_height: 0,
        }));
        assert!(!this.slides_allowed(&popup), "a popup stays on its rows");
    });
}

/// T23: Herdr 2 panes never slide; yebisu cannot see their position.
#[gpui::test]
fn herdr2_panes_never_slide(cx: &mut TestAppContext) {
    let (backend, _wire) =
        crate::core_mock::recorded(false).unwrap_or_else(|e| panic!("mock core: {e}"));
    let view = cx.new(Smoke::new);
    let cx = cx.add_empty_window();
    view.update(cx, |this, cx| {
        this.select_host("issue-137-smooth-core");
        this.connect_core_using(cx, move |_| Ok(backend));
    });
    let deadline = Instant::now() + Duration::from_secs(30);
    while !view.update(cx, |this, cx| {
        this.poll_terminal(cx);
        this.terminal.live.surface.is_some()
    }) {
        assert!(Instant::now() < deadline, "no Herdr 2 surface");
        std::thread::sleep(Duration::from_millis(5));
    }
    view.update(cx, |this, _| {
        let Some(shown) = this.terminal.live.surface.as_ref() else {
            panic!("surface");
        };
        assert!(!this.slides_allowed(shown));
    });
}

/// T24: while a pane is drawn between rows, a press hits the row drawn
/// under it, not the row at its grid position.
#[test]
fn hit_testing_follows_the_slide() {
    let mut surface = mock_peer::surface(7);
    surface.panes = single(|_| {});
    let rect = surface.panes[0].inner_rect;
    let mut geometry = InputGeometry {
        grid: Bounds::new(point(px(0.), px(0.)), size(px(800.), px(480.))),
        cell_width: 10.,
        cell_height: 20.,
        slide: None,
    };
    // The middle of row 5 at rest.
    let at = point(px(105.), px(110.));
    assert_eq!(geometry.cell_at(&surface, at, None), Some((10, 5)));
    // Content drawn 0.75 rows down: row 4's lower part is under the press.
    geometry.slide = Some((rect, 0.75));
    assert_eq!(geometry.cell_at(&surface, at, None), Some((10, 4)));
    // Drawn 0.75 rows up: row 6's upper part is.
    geometry.slide = Some((rect, -0.75));
    assert_eq!(geometry.cell_at(&surface, at, None), Some((10, 6)));
    // A slide of another pane leaves this one at rest.
    geometry.slide = Some((
        SurfaceRect {
            x: 0,
            y: 0,
            width: 4,
            height: 4,
        },
        0.75,
    ));
    assert_eq!(geometry.cell_at(&surface, at, None), Some((10, 5)));
}
