//! The forward sheet (board 42): its sizes and words, the chips from the
//! host's scan, and what Forward does with the field.
use super::*;
use crate::forwards::{
    Forwards, InputError, Reason, State, parse_port,
    worker::{self, Connector, Link},
};
use gpui::{Entity, TestAppContext, VisualTestContext};
use herdr_ssh::forward;
use std::{
    net::TcpStream,
    sync::Arc,
    time::{Duration, Instant},
};

const HOST: &str = "me@pro-dev";

/// SSH that always connects and finds the port open.
struct Open;

impl Connector for Open {
    fn trusted(&self, _: &str) -> crate::Result<()> {
        Ok(())
    }
    fn connect(&self, _: &str) -> Result<Box<dyn Link>, Reason> {
        Ok(Box::new(Open))
    }
}

impl Link for Open {
    fn open(&self, _: u16) -> Result<(), forward::Error> {
        Ok(())
    }
    fn relay(&self, _: u16, _: TcpStream) -> Result<(), forward::Error> {
        Ok(())
    }
    fn wait_end(&self, _: Duration) -> Option<forward::Error> {
        None
    }
    fn stop(&self) {}
}

fn free_port() -> u16 {
    worker::listen(0)
        .and_then(|listener| listener.local_addr())
        .map(|address| address.port())
        .unwrap_or_else(|error| panic!("{error}"))
}

/// A window with `HOST` saved and a scan that answers `output`.
fn sheet_window<'a>(
    cx: &'a mut TestAppContext,
    output: &'static [u8],
) -> (Entity<Smoke>, &'a mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    view.update(cx, |this, _| {
        this.setup.hosts = vec![HOST.into()];
        this.forwards = Forwards::with(Arc::new(Open), worker::listen);
        this.shell.forward_sheet.scan = Some(Arc::new(move |_, _| Ok(output.to_vec())));
    });
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.open_forward_sheet(HOST.into(), window, cx)
        })
    });
    (view, cx)
}

/// Polls the scan until the chips are in (or the scan said "no tool").
fn wait_for_scan(view: &Entity<Smoke>, cx: &mut VisualTestContext) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !view.update(cx, |this, _| this.poll_forward_sheet()) {
        assert!(Instant::now() < deadline, "no scan answer");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn layout_metrics() {
    assert_eq!(sheet_width(1024.), 620.);
    assert_eq!(sheet_width(652.), 620.);
    assert_eq!(sheet_width(320.), 288.);
    assert_eq!(sheet_width(10.), 0.);
    assert_eq!(sheet_top(false, false), 96.);
    assert_eq!(sheet_top(true, false), 16.);
    assert_eq!(sheet_top(false, true), 16.);
    assert_eq!(HEADER_HEIGHT, 60.);
    assert_eq!(FOOTER_HEIGHT, 68.);
    assert_eq!(CLOSE_WIDTH, 88.);
    assert_eq!((field::WIDTH, field::HEIGHT), (200., 48.));
    assert_eq!(field::FONT_SIZE, 15.);
    assert_eq!((CHIP_HEIGHT, CHIP_HIT_HEIGHT), (32., 44.));
    assert_eq!(ROW_MIN_HEIGHT, 44.);
}

#[test]
fn texts() {
    assert_eq!(TITLE, "Forward port");
    assert_eq!(CLOSE, "Close");
    assert_eq!(PORT_LABEL, "Remote port");
    assert_eq!(field::PLACEHOLDER, "3000");
    assert_eq!(SUBMIT, "Forward");
    assert_eq!(LIST_HEADING, "Forwarded ports");
    assert_eq!(
        intro("pro-dev"),
        "Reach a port that a program on pro-dev listens on from this iPad, over SSH. It stays forwarded until you stop it, and pauses while yebisu is off screen."
    );
    assert_eq!(chips_heading("pro-dev"), "Listening on pro-dev");
    assert_eq!(chip_label(3000), ":3000");
    assert_eq!(chip_label(5173), ":5173");
}

#[gpui::test]
fn chip_fills_field_and_forward_starts(cx: &mut TestAppContext) {
    let (view, cx) = sheet_window(
        cx,
        b"L 127.0.0.1:45001\nL *:5173 node EVIL\nL 0.0.0.0:3000\nL [::]:45001\n",
    );
    wait_for_scan(&view, cx);
    view.update(cx, |this, cx| {
        assert_eq!(this.shell.forward_sheet.chips, Some(vec![3000, 45001]));
        this.pick_forward_chip(45001, cx);
        assert_eq!(this.shell.forward_sheet.selected_chip, Some(45001));
        let text = this
            .shell
            .forward_sheet
            .field
            .as_ref()
            .map(|field| field.read(cx).text().to_owned());
        assert_eq!(text.as_deref(), Some("45001"));
    });
    let remote = free_port();
    view.update(cx, |this, cx| this.pick_forward_chip(remote, cx));
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            assert_eq!(this.submit_forward(window, cx), Ok(()));
            assert!(
                !this.forward_sheet_open(),
                "a forward that starts closes the sheet"
            );
            assert_eq!(
                this.forwards.get(HOST, remote).map(|forward| forward.state),
                Some(State::Connecting)
            );
        })
    });
}

#[gpui::test]
fn no_tool_hides_chips(cx: &mut TestAppContext) {
    let asked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = Arc::clone(&asked);
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    view.update(cx, |this, _| {
        this.setup.hosts = vec![HOST.into()];
        this.shell.forward_sheet.scan = Some(Arc::new(move |_, _| {
            counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(b"N\n".to_vec())
        }));
    });
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.open_forward_sheet(HOST.into(), window, cx)
        })
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    while asked.load(std::sync::atomic::Ordering::SeqCst) == 0 {
        assert!(Instant::now() < deadline, "the host was never scanned");
        std::thread::sleep(Duration::from_millis(5));
    }
    std::thread::sleep(Duration::from_millis(50));
    view.update(cx, |this, _| {
        this.poll_forward_sheet();
        assert!(this.forward_sheet_open());
        assert_eq!(
            this.shell.forward_sheet.chips, None,
            "no chip row without ss or lsof"
        );
    });
}

#[gpui::test]
fn forward_refuses_with_the_defined_errors(cx: &mut TestAppContext) {
    let (view, cx) = sheet_window(cx, b"");
    let remote = free_port();
    view.update(cx, |this, _| {
        if let Err(error) = this.forwards.start(
            HOST,
            parse_port(&remote.to_string()).unwrap_or_else(|e| panic!("{e}")),
        ) {
            panic!("{error}");
        }
    });
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            // Nothing typed.
            assert_eq!(this.submit_forward(window, cx), Err(InputError::Port));
            assert!(this.forward_sheet_open());
            // A chip of a port already forwarded from this host.
            this.pick_forward_chip(remote, cx);
            assert_eq!(
                this.submit_forward(window, cx),
                Err(InputError::Duplicate(remote))
            );
            assert!(this.forward_sheet_open(), "an error keeps the sheet");
            assert_eq!(
                this.shell
                    .forward_sheet
                    .error
                    .map(|error| error.to_string()),
                Some(format!(
                    "Port {remote} is already forwarded from this host."
                ))
            );
            assert!(
                this.shell
                    .forward_sheet
                    .field
                    .as_ref()
                    .is_some_and(|field| field.read(cx).invalid),
                "the field's border shows the error"
            );
            this.close_forward_sheet(window, cx);
            assert!(!this.forward_sheet_open());
        })
    });
}

#[gpui::test]
fn another_sheet_or_unsaved_host_opens_nothing(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| Smoke::new(cx));
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.setup.hosts = vec![HOST.into()];
            this.open_forward_sheet("me@unsaved".into(), window, cx);
            assert!(!this.forward_sheet_open());
            this.settings_modal = true;
            this.open_forward_sheet(HOST.into(), window, cx);
            assert!(!this.forward_sheet_open());
            this.settings_modal = false;
            this.open_forward_sheet(HOST.into(), window, cx);
            assert!(this.forward_sheet_open());
            assert!(this.render_forward_sheet(window, cx).is_some());
        })
    });
}
