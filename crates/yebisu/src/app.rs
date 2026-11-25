use crate::{input::Input, observation::Observation};
use gpui::{
    AnyElement, Context, Entity, FocusHandle, RenderImage, Window, canvas, div, img, prelude::*,
    px, rgb,
};
use std::{sync::Arc, time::Duration};

const TITLE: &str = "yebisu · GPUI 0.3.12 · P0";
/// The P0 tap control's text, also its QA accessibility label.
const P0_TAP_LABEL: &str = "Tap / type";

pub(crate) struct Smoke {
    pub(crate) find: Option<crate::find_view::FindBar>,
    pub(crate) find_handoff: crate::find::retirement::Handoff,
    pub(crate) forwards: crate::forwards::Forwards,
    pub(crate) notifications: crate::notification_view::State,
    #[cfg(all(target_os = "ios", target_abi = "sim", feature = "test-support"))]
    pub(crate) observer: crate::p11_observer::Harness,
    pub(crate) settings: Option<crate::settings::SettingsHost>,
    pub(crate) settings_modal: bool,
    pub(crate) settings_focus: FocusHandle,
    pub(crate) settings_shortcut: Option<gpui::Subscription>,
    pub(crate) settings_page: crate::settings_pages::Page,
    pub(crate) settings_fonts: Vec<String>,
    pub(crate) settings_fonts_loading: bool,
    pub(crate) key_ticket: Option<crate::settings::key_ops::Ticket>,
    pub(crate) local_theme: crate::theme_picker::Palette,
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
    pub(crate) edit_menu: crate::edit_menu::EditMenu,
    #[cfg(all(target_os = "ios", target_abi = "sim"))]
    pub(crate) menu_qa: crate::edit_menu_qa_sh1_152::MenuQa,
    pub(crate) front_layers: crate::front_layer::FrontLayers,
    pub(crate) keep_awake: crate::keep_awake::KeepAwake,
    pub(crate) actions: crate::actions::Actions,
    pub(crate) native_menus: crate::native_menus::NativeMenus,
    #[cfg(all(target_os = "ios", target_abi = "sim"))]
    pub(crate) p12_qa: crate::native_menus_qa::MenuQa,
    pub(crate) retirement: crate::input_retirement::Retirement,
    /// The last `accepts_text_input` answer the diagnostics recorded.
    pub(crate) text_gate: crate::text_gate::Recorder,
    // SH1-157 per-stream state; the streams read it from Wave 1 on.
    #[allow(dead_code)]
    pub(crate) shell: crate::shell::ShellState,
    ticks: u64,
    pub(crate) taps: u64,
    image: Arc<RenderImage>,
    pub(crate) observation: Observation,
    /// Cold-launch restore of what was open, and its record.
    pub(crate) launch: crate::launch_restore::LaunchRestore,
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
                        #[cfg(all(target_os = "ios", target_abi = "sim"))]
                        crate::animation_qa::sample(&this.observation);
                        // Simulator QA: a stalled main thread leaves a gap.
                        #[cfg(all(target_os = "ios", target_abi = "sim"))]
                        crate::qa_ax::input::heartbeat(&this.observation, this.ticks);
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
                        #[cfg(all(target_os = "ios", target_abi = "sim"))]
                        if this.p12_qa.deadline.is_some() {
                            cx.notify();
                        }
                        this.poll_settings(cx);
                        this.poll_find(cx);
                        this.poll_terminal(cx);
                        this.poll_forwards(cx);
                        this.collect_notifications(cx);
                        // After the polls, so it sees each host's latest status.
                        this.sync_keep_awake(cx);
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        Self {
            find: None,
            find_handoff: Default::default(),
            forwards: crate::forwards::Forwards::new(),
            notifications: Default::default(),
            #[cfg(all(target_os = "ios", target_abi = "sim", feature = "test-support"))]
            observer: Default::default(),
            settings: None,
            settings_modal: false,
            settings_focus: cx.focus_handle(),
            settings_shortcut: None,
            settings_page: crate::settings_pages::Page::Fonts,
            settings_fonts: Vec::new(),
            settings_fonts_loading: false,
            key_ticket: None,
            settings_error: None,
            local_theme: crate::theme_picker::Palette::default(),
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
            native_menus: Default::default(),
            retirement: Default::default(),
            text_gate: Default::default(),
            shell: Default::default(),
            handoff: crate::handoff::Handoff {
                drag_select: crate::qa_ax::launch_drag_select(),
                ..Default::default()
            },
            edit_menu: Default::default(),
            #[cfg(all(target_os = "ios", target_abi = "sim"))]
            menu_qa: Default::default(),
            #[cfg(all(target_os = "ios", target_abi = "sim"))]
            p12_qa: Default::default(),
            front_layers: Default::default(),
            keep_awake: Default::default(),
            ticks: 0,
            taps: 0,
            image: Arc::new(RenderImage::new(vec![image::Frame::new(pixels)])),
            observation: Observation::new(),
            launch: Default::default(),
        }
    }
}

impl Render for Smoke {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.install_settings_shortcut(window, cx);
        #[cfg(target_os = "ios")]
        self.bind_text_retirement(window, cx);
        // A draw has begun: a retired search field is no longer GPUI's input
        // handler, so it stops forwarding (find/field.rs).
        self.find_handoff.frames = self.find_handoff.frames.wrapping_add(1);
        #[cfg(all(target_os = "ios", target_abi = "sim"))]
        self.p12_tick(window, cx);
        self.front_layers.clear();
        if self.edit_menu.activation.is_none() {
            self.edit_menu.activation =
                Some(cx.observe_window_activation(window, |this, window, _| {
                    if !window.is_window_active() {
                        this.drop_closing_press();
                        this.dismiss_edit_menu(window);
                    }
                }));
        }
        self.prepare_new_space(window, cx);
        let rendered = self.render_screen(window, cx);
        self.publish_native_edits(window);
        let screen = div()
            .relative()
            .size_full()
            .child(rendered)
            .child(self.render_notification(window, cx))
            .children(self.render_shortcuts_popover(window, cx))
            .children(self.render_hosts_sheet(window, cx))
            .children(self.render_forward_sheet(window, cx))
            .children(self.render_mods_overlay(window, cx))
            .children(self.render_new_space(cx))
            .into_any_element();
        #[cfg(all(target_os = "ios", target_abi = "sim"))]
        let screen = div()
            .relative()
            .size_full()
            .child(screen)
            .children(self.p12_qa_controls(cx))
            .into_any_element();
        self.install_native_menus(window, cx);
        // Every root below draws unsized text at the canvas body size
        // (shell/style.rs), not GPUI's 16; sized text keeps its own size.
        let root_text = px(crate::shell::style::text::BASE);
        let screen = div()
            .size_full()
            .text_size(root_text)
            .on_action(cx.listener(Self::native_command_action))
            .child(screen)
            // QA accessibility layer: qa.state from the first frame (qa_ax.rs).
            .children(self.qa_ax_root(cx))
            .into_any_element();
        if self.settings_modal {
            let sheet = self.render_settings_sheet(window, cx);
            return div()
                .relative()
                .size_full()
                .text_size(root_text)
                .child(screen)
                .child(sheet)
                .into_any_element();
        }
        let Some(error) = self.settings_error.as_ref() else {
            return screen;
        };
        div()
            .size_full()
            .relative()
            .text_size(root_text)
            .child(screen)
            .child(
                div()
                    .id("settings-error")
                    .occlude()
                    .debug_selector(|| "settings-error".into())
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .child(crate::front_layer::front_layer(
                        div()
                            .p_3()
                            .bg(rgb(0x3b4252))
                            .text_color(rgb(0xeceff4))
                            .child(format!("Settings: {error}"))
                            .into_any_element(),
                        self.front_layers.clone(),
                    )),
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
            // The first-run frame draws whenever no terminal is open (SH1-157 L7);
            // the full-page Keys and hosts screen is gone.
            return self
                .render_first_run(window, cx)
                .unwrap_or_else(|| div().into_any_element());
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
                    .child(format!("{P0_TAP_LABEL} · {}", self.taps))
                    .children(crate::qa_ax::mark(crate::qa_ax::ids::P0_TAP, P0_TAP_LABEL))
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
    #[cfg(all(target_os = "ios", target_abi = "sim", feature = "test-support"))]
    if crate::p11_observer::capture::start().is_err() {
        return; // A requested capture that failed cannot run QA.
    }
    #[cfg(all(target_os = "ios", target_abi = "sim"))]
    if let Err(error) = crate::qa_stderr::start() {
        // The checks fail on a missing app-stderr.log, so this is not silent.
        eprintln!("yebisu QA stderr capture failed: {error}");
    }
    if let Err(error) = crate::diagnostics::init() {
        eprintln!("yebisu diagnostics initialization failed: {error}");
    }
    herdr_uikit::application().run(|cx: &mut App| {
        // Once per process: every call adds a global listener (not in render).
        #[cfg(all(target_os = "ios", target_abi = "sim"))]
        crate::qa_ax::input::register_global(cx);
        match cx.open_window(WindowOptions::default(), |window, cx| {
            let entity = cx.new(Smoke::new);
            let notification_owner = entity.downgrade();
            cx.on_system_notification_response(move |response, cx| {
                let _ = notification_owner.update(cx, |this, cx| {
                    this.open_notification(&response.tag, response.action_id.is_some(), cx)
                });
            });
            let weak = entity.downgrade();
            cx.on_app_lifecycle(move |phase, cx| {
                let _ = weak.update(cx, |this, cx| this.lifecycle(phase, cx));
            })
            .detach();
            entity.update(cx, |this, cx| {
                #[cfg(target_abi = "sim")]
                if std::env::var_os("YEBISU_P12_QA").is_some() {
                    // This fixture never loads saved hosts/settings or accesses keys.
                    return;
                }
                this.start_settings(cx);
                this.query_notification_permission();
                this.start_job(crate::setup::Job::Load, cx);
                this.start_launch_restore(cx);
            });
            #[cfg(target_abi = "sim")]
            if std::env::var_os("YEBISU_TERMINAL_QA").is_some() {
                entity.update(cx, |this, cx| {
                    this.terminal.reconnect.begin("qa@private-peer".into());
                    let generation = this.terminal.link.reset();
                    if std::env::var_os("YEBISU_P12_QA").is_some() {
                        match crate::mock_peer::connect_p12() {
                            Ok((client, gate)) => {
                                this.p12_qa.gate = Some(gate);
                                this.terminal.link.install(generation, Ok(client));
                            }
                            Err(error) => this.terminal.link.install(generation, Err(error)),
                        }
                    } else if std::env::var_os("YEBISU_P13_QA").is_some() {
                        this.terminal
                            .link
                            .install(generation, crate::find::peer::connect_fixture());
                        this.actions.board_hidden = true;
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
