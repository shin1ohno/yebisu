//! #265: the pager band above a docked software keyboard gives way when it
//! is what leaves the terminal below its minimum (owner decision
//! 2026-11-27), and every other layout keeps the band and the terminal it had.
use super::{
    PAGER_HEIGHT, bar,
    tests::{install, set_insets, wait_for_terminal},
};
use crate::{
    app::Smoke,
    layout::{Rect, Size, TERMINAL_MIN_HEIGHT},
    mock_peer,
};
use gpui::{Edges, TestAppContext, WindowInsets, px, size};

/// The frame's height outside the terminal: tab row 36, its rule, the
/// status bar 44 and its rule.
const CHROME: f32 = 82.;

#[derive(Clone, Copy, Debug)]
enum Keyboard {
    /// A docked software keyboard this tall, its shortcuts bar included.
    Docked(f32),
    /// A hardware keyboard: only the shortcuts bar, docked, this tall.
    Hardware(f32),
    Floating,
    /// No keyboard at all, as on a Mac.
    None,
}

fn insets(bottom: f32) -> WindowInsets {
    WindowInsets {
        ime: Edges {
            bottom: px(bottom),
            ..Edges::default()
        },
        ..WindowInsets::default()
    }
}

/// `layout_for` (the body of `frame_layout`) with `keyboard` up in a full
/// screen window of `width` × `height`: the key band's rectangle and the
/// terminal's height, `None` for Window too small.
fn layout(
    app: &mut Smoke,
    width: f32,
    height: f32,
    keyboard: Keyboard,
) -> (Option<Rect>, Option<f32>) {
    let (bottom, hardware, placement) = match keyboard {
        Keyboard::Docked(inset) => (inset, false, bar::Placement::Docked),
        Keyboard::Hardware(inset) => (inset, true, bar::Placement::Docked),
        Keyboard::Floating => (0., false, bar::Placement::Floating),
        Keyboard::None => (0., false, bar::Placement::Hidden),
    };
    app.shell.fit.hardware_keyboard_attached = hardware;
    app.shell.fit.keyboard_placement = placement;
    let layout = app.layout_for(
        Size::new(width, height),
        &insets(bottom),
        Default::default(),
        false,
    );
    (
        layout.key_band,
        layout.terminal.rect().map(|rect| rect.height),
    )
}

fn pager(width: f32, height: f32, keyboard: f32) -> Option<Rect> {
    Some(Rect::new(
        0.,
        height - keyboard - PAGER_HEIGHT,
        width,
        PAGER_HEIGHT,
    ))
}

/// The decision in `layout_for`, one window per row: keyboard heights from
/// the #265 Simulator runs (r04 to r14) and the earlier pager band tests.
#[gpui::test]
fn the_pager_band_gives_way_only_to_the_terminal_minimum(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    // (case, width, height, keyboard) -> (band, terminal height)
    let cases: [(&str, f32, f32, Keyboard, Option<Rect>, Option<f32>); 10] = [
        // The issue: 744 - 428 - 52 - 82 = 182 < 206 with the band, so the
        // band goes and the terminal keeps 234.
        (
            "iPad mini (A17 Pro) landscape",
            1133.,
            744.,
            Keyboard::Docked(428.),
            None,
            Some(744. - 428. - CHROME),
        ),
        (
            "iPad mini (A20 Pro) landscape",
            1158.,
            756.,
            Keyboard::Docked(428.),
            None,
            Some(756. - 428. - CHROME),
        ),
        // Room for the band: it stays, the terminal as before.
        (
            "iPad mini (A17 Pro) portrait",
            744.,
            1133.,
            Keyboard::Docked(340.),
            pager(744., 1133., 340.),
            Some(1133. - 340. - PAGER_HEIGHT - CHROME),
        ),
        (
            "iPad mini (5th generation) landscape",
            1024.,
            768.,
            Keyboard::Docked(408.),
            pager(1024., 768., 408.),
            Some(768. - 408. - PAGER_HEIGHT - CHROME),
        ),
        (
            "iPad Pro 13-inch landscape",
            1376.,
            1032.,
            Keyboard::Docked(498.),
            pager(1376., 1032., 498.),
            Some(1032. - 498. - PAGER_HEIGHT - CHROME),
        ),
        (
            "iPad Air 11-inch landscape",
            1210.,
            834.,
            Keyboard::Docked(320.),
            pager(1210., 834., 320.),
            Some(834. - 320. - PAGER_HEIGHT - CHROME),
        ),
        // Too small even without the band: the band stays.
        (
            "short window",
            1133.,
            600.,
            Keyboard::Docked(428.),
            pager(1133., 600., 428.),
            None,
        ),
        // No pager band to begin with: unchanged.
        (
            "iPad mini landscape, hardware keyboard",
            1133.,
            744.,
            Keyboard::Hardware(387.),
            None,
            Some(744. - 387. - CHROME),
        ),
        (
            "iPad mini landscape, floating keyboard",
            1133.,
            744.,
            Keyboard::Floating,
            Some(Rect::new(0., 744. - super::HEIGHT, 1133., super::HEIGHT)),
            Some(744. - super::HEIGHT - CHROME),
        ),
        (
            "Mac",
            1280.,
            800.,
            Keyboard::None,
            None,
            Some(800. - CHROME),
        ),
    ];
    for (case, width, height, keyboard, band, terminal) in cases {
        let got = view.update(cx, |app, _| layout(app, width, height, keyboard));
        assert_eq!(got, (band, terminal), "{case}: {keyboard:?}");
        if let Some(terminal) = terminal {
            assert!(terminal >= TERMINAL_MIN_HEIGHT, "{case}: fixture");
        }
    }
}

/// The same decision on screen: with room (portrait) the pager band shows;
/// at the issue's size (landscape) the terminal draws with the keyboard up
/// and neither the pager band nor Window too small. The frames are read right
/// after the keyboard rises: the private peer's cursor is on row 15
/// (`mock_peer.rs`), below the 13 rows of 234 pt, so it drops the link once
/// it reads the smaller size.
#[gpui::test]
fn ipad_mini_landscape_draws_the_terminal_over_a_docked_keyboard(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    let (client, _) = mock_peer::connect_recorded(1).unwrap_or_else(|error| panic!("{error}"));
    view.update(cx, |app, _| install(app, client));
    cx.simulate_resize(size(px(744.), px(1133.)));
    wait_for_terminal(&view, cx);
    set_insets(&view, cx, 340., false);
    let band = cx
        .debug_bounds("shell-pager-band")
        .unwrap_or_else(|| panic!("portrait lost the pager band"));
    assert_eq!(band.size, size(px(744.), px(PAGER_HEIGHT)));
    assert_eq!(band.bottom(), px(1133. - 340.));

    cx.simulate_resize(size(px(1133.), px(744.)));
    set_insets(&view, cx, 428., false);
    assert!(
        cx.debug_bounds("shell-too-small").is_none(),
        "Window too small over the docked keyboard"
    );
    assert!(cx.debug_bounds("shell-pager-band").is_none());
    let terminal = cx
        .debug_bounds("shell-terminal")
        .unwrap_or_else(|| panic!("terminal"));
    assert_eq!(terminal.size.height, px(744. - 428. - CHROME));
}
