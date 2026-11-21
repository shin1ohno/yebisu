use super::super::{
    Input, Insets, ModSlot, Regions, Size, TERMINAL_MIN_HEIGHT, TERMINAL_MIN_WIDTH, TerminalFit,
};
use super::{layout, window};

/// Every window from 200×150 to 1376×1032 in 1 pt steps, in the given setup.
fn each_window(setup: Input, mut check: impl FnMut(u16, u16, &TerminalFit)) {
    for width in 200..=1376_u16 {
        for height in 150..=1032_u16 {
            let out = layout(Input {
                window: Size::new(f32::from(width), f32::from(height)),
                ..setup
            });
            check(width, height, &out.terminal);
        }
    }
}

fn setups() -> [Input; 3] {
    [
        window(0.0, 0.0),
        Input {
            regions: Regions::maximum(),
            find_bar_open: true,
            qa_strip_height: 40.0,
            ..window(0.0, 0.0)
        },
        Input {
            regions: Regions::defaults(),
            insets: Insets {
                keyboard: 120.0,
                safe_top: 24.0,
                ..Insets::default()
            },
            ..window(0.0, 0.0)
        },
    ]
}

#[test]
fn terminal_never_below_minimum() {
    for setup in setups() {
        each_window(setup, |width, height, fit| {
            if let TerminalFit::Fits(rect) = fit {
                assert!(
                    rect.width >= TERMINAL_MIN_WIDTH && rect.height >= TERMINAL_MIN_HEIGHT,
                    "{width}×{height}: terminal {rect:?}"
                );
            }
        });
    }
}

#[test]
fn no_nonpositive_size_reaches_terminal() {
    for setup in setups() {
        each_window(setup, |width, height, fit| match fit {
            TerminalFit::Fits(rect) => {
                assert!(
                    rect.width > 0.0 && rect.height > 0.0,
                    "{width}×{height}: {rect:?}"
                );
            }
            TerminalFit::TooSmall(report) => {
                assert!(
                    report.available.width >= 0.0 && report.available.height >= 0.0,
                    "{width}×{height}: {report:?}"
                );
            }
        });
    }
    // Insets larger than the window.
    let out = layout(Input {
        insets: Insets {
            keyboard: 900.0,
            safe_left: 700.0,
            ..Insets::default()
        },
        ..window(600.0, 800.0)
    });
    assert!(matches!(out.terminal, TerminalFit::TooSmall(_)));
}

#[test]
fn too_small_window_reports_instead_of_shrinking() {
    // The old two-column layout cannot fit below the rail cutoff.
    let narrow = layout(window(351.0, 834.0));
    match narrow.terminal {
        TerminalFit::TooSmall(report) => {
            assert_eq!(report.available.width, 278.0);
            assert_eq!(report.minimum.width, 280.0);
            assert_eq!(report.minimum.height, 206.0);
        }
        TerminalFit::Fits(rect) => panic!("shrank instead of reporting: {rect:?}"),
    }
    // Tab row 36 + rule + rule + status bar 44 + 205.
    let short = layout(Input {
        regions: Regions {
            m3: ModSlot::Wants(44.0),
            ..Regions::default()
        },
        ..window(1000.0, 82.0 + 205.0)
    });
    match short.terminal {
        TerminalFit::TooSmall(report) => assert_eq!(report.available.height, 205.0),
        TerminalFit::Fits(rect) => panic!("shrank instead of reporting: {rect:?}"),
    }
    // One more point fits exactly.
    let exact = layout(window(1000.0, 82.0 + 206.0));
    assert!(matches!(exact.terminal, TerminalFit::Fits(rect) if rect.height == 206.0));
}
