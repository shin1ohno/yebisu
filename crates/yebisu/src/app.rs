use crate::{input::Input, observation::Observation};
use gpui::{
    AnyElement, Context, Entity, FocusHandle, RenderImage, Window, canvas, div, img, prelude::*,
    rgb,
};
use std::{sync::Arc, time::Duration};

const TITLE: &str = "yebisu · GPUI 0.3.12 · P0";

pub(crate) struct Smoke {
    pub(crate) settings: Option<crate::settings::SettingsHost>,
    pub(crate) settings_error: Option<String>,
    pub(crate) focus: FocusHandle,
    pub(crate) terminal_focus: FocusHandle,
    pub(crate) input: Input,
    pub(crate) setup_input: Input,
    pub(crate) key: String,
    pub(crate) setup: crate::setup::Setup,
    pub(crate) smoke_mode: bool,
    pub(crate) terminal: crate::terminal_view::TerminalView,
    pub(crate) board: crate::board::Board,
    pub(crate) handoff: crate::handoff::Handoff,
    pub(crate) actions: crate::actions::Actions,
    pub(crate) retirement: crate::input_retirement::Retirement,
    ticks: u64,
    taps: u64,
    image: Arc<RenderImage>,
    pub(crate) observation: Observation,
}

impl Smoke {
    pub(crate) fn new(cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        let mut pixels = image::RgbaImage::from_fn(32, 32, |x, y| {
            if (x / 8 + y / 8) % 2 == 0 {
                image::Rgba([235, 203, 139, 255])
            } else {
                image::Rgba([180, 142, 173, 255])
            }
        });
        // RenderImage uploads BGRA bytes; the source constants above are RGBA.
        for pixel in pixels.pixels_mut() {
            pixel.0.swap(0, 2);
        }
        cx.spawn(async |weak, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                if weak
                    .update(cx, |this, cx| {
                        this.ticks += 1;
                        if this.smoke_mode {
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        cx.spawn(async |weak, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(33))
                    .await;
                if weak
                    .update(cx, |this, cx| {
                        this.poll_settings(cx);
                        this.poll_terminal(cx)
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        Self {
            settings: None,
            settings_error: None,
            focus,
            terminal_focus: cx.focus_handle(),
            input: Input::default(),
            setup_input: Input::default(),
            key: String::new(),
            setup: crate::setup::Setup::default(),
            smoke_mode: std::env::var_os("YEBISU_SMOKE_OBSERVE").is_some(),
            terminal: Default::default(),
            board: Default::default(),
            actions: Default::default(),
            retirement: Default::default(),
            handoff: crate::handoff::Handoff {
                drag_select: true,
                ..Default::default()
            },
            ticks: 0,
            taps: 0,
            image: Arc::new(RenderImage::new(vec![image::Frame::new(pixels)])),
            observation: Observation::new(),
        }
    }
}

impl Render for Smoke {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        #[cfg(target_os = "ios")]
        self.bind_text_retirement(window, cx);
        let screen = self.render_screen(window, cx);
        #[cfg(target_os = "ios")]
        self.publish_native_edits(window);
        let Some(error) = self.settings_error.as_ref() else {
            return screen;
        };
        div()
            .size_full()
            .relative()
            .child(screen)
            .child(
                div()
                    .id("settings-error")
                    .debug_selector(|| "settings-error".into())
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .p_3()
                    .bg(rgb(0x3b4252))
                    .text_color(rgb(0xeceff4))
                    .child(format!("Settings: {error}")),
            )
            .into_any_element()
    }
}
impl Smoke {
    fn render_screen(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.terminal.open {
            return self.render_terminal(window, cx);
        }
        if !self.smoke_mode {
            return self.render_setup(cx);
        }
        let entity = cx.entity();
        let input_entity = entity.clone();
        div()
            .id("smoke")
            .size_full()
            .flex()
            .flex_col()
            .justify_center()
            .items_center()
            .gap_4()
            .bg(rgb(0x2e3440))
            .text_color(rgb(0xeceff4))
            .text_xl()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                this.key = format!("{:?}", event.keystroke);
                cx.notify();
            }))
            .child(region(
                "text",
                div().text_3xl().child(TITLE).into_any_element(),
                entity.clone(),
            ))
            .child(
                div()
                    .font_family("SauceCodePro Nerd Font Mono")
                    .child("Nerd Font: \u{f120}  日本語  🦀"),
            )
            .child(region(
                "quad",
                div()
                    .id("quad")
                    .w_64()
                    .h_16()
                    .flex()
                    .justify_center()
                    .items_center()
                    .bg(rgb(0x88c0d0))
                    .text_color(rgb(0x2e3440))
                    .border_4()
                    .border_color(rgb(0xa3be8c))
                    .rounded_xl()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.taps += 1;
                        this.focus.focus(window, cx);
                        window.request_virtual_keyboard();
                        eprintln!("yebisu input click taps={}", this.taps);
                        cx.notify();
                    }))
                    .child(format!("Tap / type · {}", self.taps))
                    .into_any_element(),
                entity.clone(),
            ))
            .child(
                div()
                    .underline()
                    .child(format!("Async counter: {}", self.ticks)),
            )
            .child(region(
                "image",
                img(self.image.clone()).w_24().h_24().into_any_element(),
                entity.clone(),
            ))
            .child(region(
                "input",
                div()
                    .w_96()
                    .h_12()
                    .relative()
                    .bg(rgb(0x3b4252))
                    .child(self.input.text.clone())
                    .child(
                        canvas(
                            |_, _, _| (),
                            move |bounds, _, window, cx| {
                                let focus = input_entity.read(cx).focus.clone();
                                window.handle_input(
                                    &focus,
                                    gpui::ElementInputHandler::new(bounds, input_entity),
                                    cx,
                                );
                            },
                        )
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full(),
                    )
                    .into_any_element(),
                entity.clone(),
            ))
            .child(format!("Marked: {:?}", self.input.marked))
            .child(format!("KeyDown: {}", self.key))
            .child(
                div()
                    .id("setup-back")
                    .child("Keys and hosts")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.smoke_mode = false;
                        cx.notify();
                    })),
            )
            .into_any_element()
    }
}

// Paint receipts describe the same region that the pixel oracle samples.
fn region(name: &'static str, content: AnyElement, view: Entity<Smoke>) -> impl IntoElement {
    div().relative().child(content).child(canvas(|_, _, _| (), move |bounds, _, window, cx| {
        let state = view.read(cx);
        let viewport = window.viewport_size();
        state.observation.record(serde_json::json!({
            "kind": "paint", "region": name,
            "bounds": [f32::from(bounds.origin.x), f32::from(bounds.origin.y), f32::from(bounds.size.width), f32::from(bounds.size.height)],
            "viewport": [f32::from(viewport.width), f32::from(viewport.height)],
            "ticks": state.ticks, "input": state.input.text, "marked": state.input.marked,
            "title": TITLE
        }));
    }).absolute().top_0().left_0().size_full())
}

#[cfg(target_os = "ios")]
pub(crate) fn run() {
    use gpui::{App, WindowOptions};
    if let Err(error) = crate::diagnostics::init() {
        eprintln!("yebisu diagnostics initialization failed: {error}");
    }
    herdr_uikit::application().run(|cx: &mut App| {
        match cx.open_window(WindowOptions::default(), |window, cx| {
            let entity = cx.new(Smoke::new);
            let weak = entity.downgrade();
            cx.on_app_lifecycle(move |phase, cx| {
                let _ = weak.update(cx, |this, cx| this.lifecycle(phase, cx));
            })
            .detach();
            entity.update(cx, |this, cx| {
                this.start_settings(cx);
                this.start_job(crate::setup::Job::Load, cx);
            });
            #[cfg(target_abi = "sim")]
            if std::env::var_os("YEBISU_TERMINAL_QA").is_some() {
                entity.update(cx, |this, cx| {
                    this.terminal.reconnect.begin("qa@private-peer".into());
                    let generation = this.terminal.link.reset();
                    if std::env::var_os("YEBISU_R5_QA").is_some() {
                        let (release, wait) = std::sync::mpsc::channel();
                        let old = this.terminal.link.inbox.clone();
                        this.terminal.late_reader_release = Some(release);
                        this.terminal.link.reader = Some(std::thread::spawn(move || {
                            if wait.recv_timeout(Duration::from_secs(60)).is_ok()
                                && let Ok(mut live) = old.lock()
                            {
                                live.apply(sheprd_client::ClientEvent::VersionMismatch(
                                    sheprd_client::VersionMismatch::DaemonOutdated {
                                        server_version: Some("R5-late-reader".into()),
                                    },
                                ));
                            }
                        }));
                    } else if std::env::var_os("YEBISU_R3_QA").is_some() {
                        // Inject only into the reader mailbox, before any UI poll.
                        if let Ok(mut live) = this.terminal.link.inbox.try_lock() {
                            live.apply(sheprd_client::ClientEvent::VersionMismatch(
                                sheprd_client::VersionMismatch::DaemonOutdated {
                                    server_version: Some("0.8".into()),
                                },
                            ));
                        }
                    } else {
                        this.terminal
                            .link
                            .install(generation, crate::mock_peer::connect());
                    }
                    this.terminal.open = true;
                    this.terminal.needs_focus = true;
                    if std::env::var_os("YEBISU_BOARD_QA").is_some() {
                        this.terminal.link.reset();
                        this.terminal.reconnect.begin("qa-alpha".into());
                        let generation = this.terminal.link.generation;
                        this.terminal
                            .link
                            .install(generation, crate::mock_peer::connect_board());
                        this.start_host_connection("qa-beta".into(), cx);
                    }
                    cx.notify();
                });
            }
            #[cfg(target_abi = "sim")]
            if std::env::var_os("YEBISU_CORE_QA").is_some() {
                entity.update(cx, |this, cx| {
                    this.setup.busy = false;
                    this.setup_input.set_text("qa@private-core".into());
                    this.connect_core(cx);
                });
            }
            #[cfg(all(target_abi = "sim", feature = "retirement-probe"))]
            if std::env::var_os("YEBISU_RETIREMENT_QA").is_some() {
                crate::retirement_qa::start(&entity, window, cx);
            }
            let focus = entity.read(cx).focus.clone();
            focus.focus(window, cx);
            entity
        }) {
            Ok(_) => {
                crate::diagnostics::record("window_opened");
                eprintln!("yebisu GPUI window opened");
            }
            Err(error) => {
                crate::diagnostics::record("window_error");
                eprintln!("yebisu window error: {error:#}");
                cx.quit();
            }
        }
    });
}
