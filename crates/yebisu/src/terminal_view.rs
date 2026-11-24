mod pane_names;
mod surface;

use crate::{
    app::Smoke, link::Link, live::Live, settings_pages::toolbar_sync::ToolbarSetting,
    terminal_painter::TerminalPainter, theme::Theme,
};
use gpui::{AnyElement, ClipboardItem, Context, div, font, prelude::*, rgb};
use sheprd_client::{ConnectOptions, ConnectTarget};

/// The accessibility label of the terminal input area's QA mark.
const TERMINAL_INPUT_LABEL: &str = "Terminal";

#[derive(Default)]
pub(crate) struct TerminalView {
    pub link: Link,
    pub core: Option<crate::core::Session>,
    pub core_mode: bool,
    #[cfg(test)]
    pub core_test_backend: Option<sheprd_core_client::CoreBackend>,
    #[cfg(test)]
    pub test_client: Option<sheprd_client::Client>,
    pub reconnect: crate::reconnect::Reconnect,
    pub live: Live,
    pub painter: TerminalPainter,
    pub open: bool,
    pub font: crate::settings::font::TerminalFont,
    pub compact: bool,
    pub size: Option<ConnectOptions>,
    /// Last focus report, scoped to the connection generation and server boot.
    pub(crate) surface_focus: Option<(u64, String, bool)>,
    pub(crate) surface_reclaim: Option<surface::Reclaim>,
    pub input: crate::terminal_input::TerminalInput,
    pub geometry: crate::input_geometry::InputGeometry,
    pub(crate) canvas_bounds: Option<gpui::Bounds<gpui::Pixels>>,
    pub show_options: bool,
    pub needs_focus: bool,
    pub reconnect_pending: bool,
    pub navigating: Option<crate::board::Navigation>,
    /// L66: the pointer's rest over the terminal, for the Command underline.
    pub(crate) link_hover: crate::terminal_links::Hover,
    /// Wheel and trackpad motion not yet a whole line (issue #137).
    pub(crate) wheel: crate::upstream::wheel::WheelAccumulator,
}
impl TerminalView {
    pub(crate) fn resize_if_ready(&mut self, options: ConnectOptions) {
        if self.navigating.is_some() || self.size == Some(options) {
            return;
        }
        if let Some(core) = &self.core {
            if let Ok(size) =
                sheprd_backend::Size::new(options.surface_size.cols, options.surface_size.rows)
            {
                match core.send(crate::core::Command::Resize(size)) {
                    Ok(()) => self.size = Some(options),
                    Err(e) => self.live.status = e.to_string(),
                }
            }
            return;
        }
        if let (Some(handle), Some(snapshot)) = (&self.link.handle, &self.live.snapshot) {
            match handle.resize(&snapshot.boot_id, options) {
                Ok(()) => self.size = Some(options),
                Err(e) => self.live.status = e.to_string(),
            }
        }
    }
    pub(crate) fn restore_stop_reason(&mut self) {
        self.live.version_mismatch = self.link.stop_reason.clone();
        if let Some(reason) = self.link.stop_reason.get() {
            self.live.status = format!("Version mismatch: {reason:?}");
        }
    }
}
impl Smoke {
    pub(crate) fn connect_terminal(&mut self, cx: &mut Context<Self>) {
        self.input_transition(|this| this.connect_terminal_inner(cx))
    }
    fn connect_terminal_inner(&mut self, cx: &mut Context<Self>) {
        if self.setup.busy || self.terminal.reconnect.suspended {
            return;
        }
        let from_setup = !self.terminal.open;
        let target = if self.terminal.open {
            self.terminal.reconnect.target.clone()
        } else {
            self.setup_input.text.trim().to_owned()
        };
        if from_setup {
            self.select_host_kind(&target, crate::shell::HerdrKind::Herdr1);
        } else {
            self.select_host(&target);
        }
        self.start_host_connection(target, cx);
        for host in self.setup.hosts.clone() {
            if host != self.terminal.reconnect.target
                && !self.board.hidden.contains_key(&host)
                && !self.board.hidden_core.contains_key(&host)
            {
                self.start_herdr1_connection(host, cx);
            }
        }
    }
    pub(crate) fn start_host_connection(&mut self, target: String, cx: &mut Context<Self>) {
        if self.terminal.core_mode && self.terminal.reconnect.target == target {
            self.connect_core(cx);
        } else {
            self.start_herdr1_connection(target, cx);
        }
    }
    pub(crate) fn start_herdr1_connection(&mut self, target: String, cx: &mut Context<Self>) {
        self.input_transition(|this| this.start_herdr1_connection_inner(target, cx))
    }
    fn start_herdr1_connection_inner(&mut self, target: String, cx: &mut Context<Self>) {
        let selected = self.terminal.reconnect.target == target && !self.terminal.core_mode;
        let terminal = if selected {
            &mut self.terminal
        } else {
            self.board.hidden.entry(target.clone()).or_default()
        };
        terminal.restore_stop_reason();
        if terminal.live.version_mismatch.get().is_some() {
            terminal.reconnect_pending = false;
            return;
        }
        let Some(generation) = terminal.link.prepare_connection() else {
            terminal.reconnect_pending = terminal.link.stop_reason.get().is_none();
            terminal.restore_stop_reason();
            return;
        };
        terminal.reconnect_pending = false;
        let host_generation = terminal.reconnect.begin(target.clone());
        let host = target.clone();
        terminal.input.discard();
        terminal
            .input
            .sync(&Live::default(), terminal.link.generation, None);
        self.observation.record(serde_json::json!({"kind":"reconnect","host":host,"selected":selected,"generation":generation,"automatic":terminal.reconnect.automatic}));
        terminal.live = Live::default();
        terminal.navigating = None;
        terminal.size = None;
        terminal.open = true;
        terminal.needs_focus = true;
        #[cfg(test)]
        let test_client = terminal.test_client.take();
        let work = cx.background_executor().spawn(async move {
            #[cfg(test)]
            if let Some(client) = test_client {
                return Ok(client);
            }
            #[cfg(all(target_os = "ios", target_abi = "sim"))]
            if std::env::var_os("YEBISU_TERMINAL_QA").is_some() {
                return if std::env::var_os("YEBISU_BOARD_QA").is_some() {
                    crate::mock_peer::connect_board()
                } else {
                    crate::mock_peer::connect()
                };
            }
            // Same P2 trust gate. The SSH bridge independently verifies pins again.
            let scan = sheprd_client::ssh::scan_host_key(&target)
                .map_err(|e| crate::Error::Ssh(e.to_string()))?;
            crate::hosts::authentication_gate(&scan)?;
            sheprd_client::connect(
                ConnectTarget::Ssh {
                    target,
                    session: "default".into(),
                },
                ConnectOptions::default(),
            )
            .map_err(|e| crate::Error::Ssh(e.to_string()))
        });
        cx.spawn(async move |weak, cx| {
            let result = work.await;
            let _ = weak.update(cx, |this, cx| {
                let terminal = this.herdr1_terminal_mut(&host);
                if let Some(terminal) =
                    terminal.filter(|t| t.reconnect.current(&host, host_generation))
                {
                    terminal.link.install(generation, result);
                } else if let Ok(client) = result {
                    client.handle.disconnect();
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(crate) fn poll_terminal(&mut self, cx: &mut Context<Self>) {
        // herdr-uikit's input receipts (Simulator QA only), before the
        // refresh so a moved counter is published by this poll.
        #[cfg(all(target_os = "ios", target_abi = "sim"))]
        crate::qa_ax::input::drain(&self.observation);
        // A qa.state counter or the mock peer moved: publish it (QA only).
        crate::qa_ax::refresh_if_wanted(cx);
        // L66: Herdr's answer to where a pressed address ends.
        self.poll_link_resolve(cx);
        let mut pending = self
            .board
            .hidden
            .iter()
            .filter(|(_, t)| t.reconnect_pending && !t.reconnect.suspended)
            .map(|(host, _)| host.clone())
            .collect::<Vec<_>>();
        if self.terminal.reconnect_pending
            && !self.terminal.reconnect.suspended
            && !self.terminal.core_mode
        {
            pending.push(self.terminal.reconnect.target.clone());
        }
        for host in pending {
            self.start_herdr1_connection(host, cx);
        }
        self.poll_hidden(cx);
        self.poll_removed_space(cx);
        for hidden in self.board.hidden.values_mut() {
            hidden.report_surface_focus(false);
        }
        self.sync_handoff();
        if self.terminal.reconnect.suspended {
            return;
        }
        if !self.terminal.open {
            self.terminal.report_surface_focus(false);
            // The Hosts list does not disconnect the current host. Keep draining its
            // reader exactly as a hidden host is, so a notification that arrives
            // while the terminal is closed is delivered now instead of aging out.
            if let Some(mut live) = self.terminal.link.take_update() {
                live.clipboard.clear();
                self.terminal.live = live;
                crate::board::finish_navigation(&mut self.terminal);
                self.record_board(&self.terminal.reconnect.target, &self.terminal.live);
                cx.notify();
            }
            return;
        }
        if let Some(mut live) = self.terminal.link.take_update() {
            for text in live.clipboard.drain(..) {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
            }
            self.terminal
                .input
                .sync(&live, self.terminal.link.generation, self.terminal.size);
            self.terminal.live = live;
            crate::board::finish_navigation(&mut self.terminal);
            self.record_board(&self.terminal.reconnect.target, &self.terminal.live);
            self.flush_terminal();
            cx.notify();
        }
    }
    pub(crate) fn terminal_paint_callback(
        &self,
        cx: &Context<Self>,
    ) -> impl FnOnce(gpui::Bounds<gpui::Pixels>, (), &mut gpui::Window, &mut gpui::App) + 'static + use<>
    {
        let input_entity = cx.entity();
        let entity = cx.entity();
        let generation = self.terminal.link.generation;
        let compact = self.terminal.compact;
        let selection = self.board.selection;
        move |bounds, _, window, cx| {
            // The canvas can extend past an ancestor's clip. Use the same
            // visible rectangle for the wire size, painting and input.
            let bounds = bounds.intersect(&window.content_mask().bounds);
            let current = entity.update(cx,|this,cx| {
                    if !this.terminal.open || this.board.selection != selection || !this.terminal.link.current(generation) { return false; }
                    let (font_size,line_height) = this.terminal_font_metrics(window);
                    let face = font(this.terminal.font.family.clone());
                    let search_highlights = this.find_highlights();
                    let search_highlight_count = search_highlights.len();
                    let search_receipt = this.find.as_ref().map(|bar|serde_json::json!({"query":bar.search.query,"state":format!("{:?}",bar.search.state),"result_revision":bar.search.result.as_ref().map(|r|r.content_revision),"current":bar.search.result.as_ref().and_then(|r|r.current),"pane":bar.search.scope.pane}));
                    let terminal = &mut this.terminal;
                    terminal.painter.set_appearance(font_size,line_height,Theme::from_palette(this.local_theme));
                    let cell_width = terminal.painter.cell_width(&face,window,cx);
                    terminal.geometry = crate::input_geometry::InputGeometry::from_canvas(bounds, cell_width, line_height);
                    terminal.canvas_bounds = Some(bounds);
                    let shown = terminal.coherent_frame();
                    let panes = shown.as_ref().map_or_else(Vec::new, |(_, surface)| {
                        terminal.geometry.framed_panes(surface)
                    });
                    let viewport = terminal.viewport_size(&panes);
                    let options = ConnectOptions {
                        surface_size: crate::terminal::viewport(f32::from(viewport.width),f32::from(viewport.height),cell_width,line_height),
                        cell_width_px: (cell_width*window.scale_factor()).round() as u32,
                        cell_height_px: (line_height*window.scale_factor()).round() as u32,
                    };
                    if viewport.width >= gpui::px(cell_width)
                        && viewport.height >= gpui::px(line_height)
                    {
                        let resized = terminal.size != Some(options);
                        terminal.resize_if_ready(options);
                        if let Some((snapshot, surface)) = shown.as_ref()
                            && terminal.size == Some(options)
                        {
                            terminal.update_surface_focus(window.is_window_active(), resized, options, snapshot, surface);
                        }
                    }
                    terminal.input.sync(&terminal.live, generation, terminal.size);
                    if let Some((snapshot, surface)) = shown.as_ref() {
                        let mut highlights = this.handoff.selection.as_ref()
                            .map(|selection| selection.surface_highlights(surface))
                            .unwrap_or_default();
                        highlights.extend(search_highlights);
                        terminal.paint_surface((snapshot, surface), &panes, &highlights, &face, window, cx);
                        terminal.paint_link_hover(surface, &panes, window);
                        this.observation.record(serde_json::json!({"kind":"terminal","p13_search":search_receipt,"p13_highlights":search_highlight_count,"settings_modal":this.settings_modal,"theme":this.local_theme.key(),"image_assets":terminal.live.surface_images.len(),"image_placements":surface.graphics.placements.len(),"host":terminal.reconnect.target,"pane":snapshot.focused_pane_id,"tab":snapshot.focused_tab_id,"tab_count":snapshot.tabs.iter().filter(|t|Some(&t.workspace_id)==snapshot.focused_workspace_id.as_ref()).count(),"pane_count":snapshot.panes.iter().filter(|p|Some(&p.tab_id)==snapshot.focused_tab_id.as_ref()).count(),"surface_marker":surface.frame.cells.first().map(|c|c.symbol.as_str()),"input_pane":terminal.input.destination.as_ref().and_then(|d|match &d.target { crate::terminal::InputTarget::Pane(id)=>Some(id), _=>None }),"generation":generation,"select_mode":this.handoff.select_mode,"selection_blocks_input":this.handoff.blocks_input(),"text_input_paused":terminal.input.requires_gesture(),"input_ready":terminal.input.ready,"boot":snapshot.boot_id,"projection":snapshot.revision,"surface":surface.surface_revision,"cols":surface.frame.width,"rows":surface.frame.height,"compact":compact,"cell_size":[options.cell_width_px,options.cell_height_px],"font_size":font_size,"line_height":line_height,"font_family":terminal.font.family,"marked":terminal.input.composition.text,"selected":[terminal.input.composition.selected.start,terminal.input.composition.selected.end],"viewport":[f32::from(bounds.size.width),f32::from(bounds.size.height)]}));
                        #[cfg(all(target_os = "ios", target_abi = "sim"))]
                        this.sh1_152_advance_e1_text(window, cx);
                    }
                    true
                });
            if !current {
                return;
            }
            let focus = input_entity.read(cx).terminal_focus.clone();
            window.handle_input(
                &focus,
                crate::actions::herdr_keys::TerminalInputHandler::new(bounds, input_entity),
                cx,
            );
        }
    }
    pub(crate) fn render_terminal(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.terminal.needs_focus && self.find.is_none() && !self.input_modal() {
            self.terminal_focus.focus(window, cx);
            self.terminal.needs_focus = false;
        }
        // Submit once when an invalidated search hands focus back, again after
        // a NativeCallbackInFlight refusal, and again while the owner is busy
        // or moved (begin/complete_find_retirement); other refusals stay fenced.
        if self.find_handoff.needs_request && self.find.is_none() {
            self.refresh_terminal_input();
            self.request_find_retirement(window, cx);
        }
        self.enforce_pane_mode(window, cx);
        let too_small = self.render_too_small(window, cx);
        if !window.is_window_active() || too_small.is_some() {
            self.terminal.report_surface_focus(false);
        }
        if let Some(too_small) = too_small {
            if self.compact_window_at(window) {
                return self.shell_frame(too_small, window, cx);
            }
            return too_small;
        }
        let legacy = self.legacy_bands();
        let board_in_band = self.board_in_band();
        let core_band = self.core_controls_in_band();
        let compact = self.terminal.compact;
        let (font_size, line_height) = self.terminal_font_metrics(window);
        let label = format!(
            "{} {font_size}/{line_height}",
            if compact { "Compact" } else { "Standard" }
        );
        let status = self.terminal_status();
        // L66: after another app was in front, a held Command may be stale.
        self.terminal
            .link_hover
            .observe_active(window.is_window_active());
        #[cfg(any(test, all(target_os = "ios", target_abi = "sim")))]
        self.publish_link_targets();
        let body = div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(crate::shell::style::color::BACKGROUND))
            .text_color(rgb(crate::shell::style::color::TEXT))
            .track_focus(&self.terminal_focus)
            .on_key_down(cx.listener(Self::terminal_key_down))
            .on_key_up(cx.listener(Self::terminal_key_up))
            // L66: on the focused terminal, where a hardware Command arrives.
            .on_modifiers_changed(
                cx.listener(|this, event, _, cx| this.link_hover_modifiers(event, cx)),
            )
            .when(legacy.toolbar, |view| {
                view.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_2()
                        .child(self.settings_button(cx))
                        .px_4()
                        .py_2()
                        .child(
                            div()
                                .id("terminal-back")
                                .px_4()
                                .py_2()
                                .bg(rgb(0x3b4252))
                                .child("Hosts")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    window.blur(cx);
                                    this.terminal.input.detach();
                                    this.handoff.cancel();
                                    this.board.selection = this.board.selection.wrapping_add(1);
                                    this.terminal.open = false;
                                    this.smoke_mode = false;
                                    cx.notify();
                                })),
                        )
                        .child(
                            div()
                                .id("terminal-density")
                                .px_4()
                                .py_2()
                                .bg(rgb(0x3b4252))
                                .child(label)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.terminal.compact = !this.terminal.compact;
                                    this.terminal.font.size = None;
                                    this.toolbar_setting_changed(ToolbarSetting::Density, cx);
                                    cx.notify();
                                })),
                        )
                        .child(
                            div()
                                .id("input-options")
                                .px_2()
                                .py_2()
                                .bg(rgb(0x3b4252))
                                .child("Input options")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.terminal.show_options ^= true;
                                    cx.notify();
                                })),
                        )
                        .child(
                            div()
                                .id("reconnect")
                                .px_2()
                                .py_2()
                                .bg(rgb(0x3b4252))
                                .child("Reconnect")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if !this.terminal.live.connected
                                        && this.terminal.live.version_mismatch.get().is_none()
                                    {
                                        this.connect_terminal(cx);
                                    }
                                })),
                        )
                        .child(div().child(format!("P9 · {status}"))),
                )
            })
            .when(legacy.return_policy, |view| {
                view.child(
                    div()
                        .id("reconnect-policy")
                        .px_4()
                        .py_2()
                        .child(if self.terminal.reconnect.automatic {
                            "Return: Automatic"
                        } else {
                            "Return: Tap reconnect"
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.terminal.reconnect.automatic ^= true;
                            this.toolbar_setting_changed(ToolbarSetting::ReturnPolicy, cx);
                            cx.notify();
                        })),
                )
            })
            .when(
                board_in_band && !self.terminal.core_mode && !self.actions.board_hidden,
                |view| view.child(self.render_board(cx)),
            )
            .when(legacy.actions && !self.terminal.core_mode, |view| {
                view.child(self.render_actions(cx))
            })
            .when(core_band && self.terminal.core_mode, |view| {
                view.child(self.render_core_controls(cx))
            })
            .when(legacy.input_options && self.terminal.show_options, |view| {
                view.child(self.input_options(cx))
            })
            .when(legacy.qa_controls, |view| {
                view.children(self.input_qa_controls(cx))
            })
            .when(legacy.handoff, |view| view.child(self.handoff_controls(cx)))
            .when(legacy.qa_controls, |view| {
                view.children(self.p13_qa_controls(cx))
                    .children(self.handoff_qa_controls(cx))
                    .children(self.native_edit_qa_controls(cx))
                    .children(self.edit_menu_qa_controls(cx))
                    .children(self.sh1_156_qa_controls(cx))
                    .children(self.sh1_152_controls(cx))
            })
            .child(
                div()
                    .id("terminal-input")
                    .debug_selector(|| "terminal-input".into())
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .overflow_hidden()
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                            if this.find.is_some() {
                                return;
                            }
                            // iPadOS treats a Control-click as a secondary click.
                            if crate::edit_menu_gestures::secondary_press(
                                event.button,
                                &event.modifiers,
                            ) {
                                this.secondary_click(event, window, cx);
                                return;
                            }
                            if !this.front_layers.covers(event.position, window) {
                                // L66: Command + press on a link opens it and
                                // reaches nothing else.
                                if this.command_press(event, window, cx) {
                                    return;
                                }
                                this.selection_down(event, window, cx);
                            }
                        }),
                    )
                    .on_mouse_down(
                        gpui::MouseButton::Right,
                        cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                            this.secondary_click(event, window, cx);
                        }),
                    )
                    .on_mouse_move(cx.listener(|this, event, _, cx| {
                        this.link_hover_move(event, cx);
                        this.selection_move(event, cx)
                    }))
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                        if !hovered {
                            this.link_hover_left(cx);
                        }
                    }))
                    .on_mouse_up(
                        gpui::MouseButton::Left,
                        cx.listener(|this, _, window, cx| this.selection_up(window, cx)),
                    )
                    .on_mouse_up_out(
                        gpui::MouseButton::Left,
                        cx.listener(|this, _, window, cx| this.selection_up(window, cx)),
                    )
                    .child(self.edit_menu_canvas(cx))
                    .child(self.terminal_wheel_canvas(cx))
                    .children(self.jump_to_bottom_buttons(cx))
                    // Last, so no other child's hitbox lies over it.
                    .children(crate::qa_ax::mark(
                        crate::qa_ax::ids::TERMINAL_INPUT,
                        TERMINAL_INPUT_LABEL,
                    )),
            )
            .into_any_element();
        self.shell_frame(body, window, cx)
    }
}

#[cfg(test)]
#[path = "terminal_view_tests.rs"]
mod tests;

#[cfg(not(all(target_os = "ios", target_abi = "sim")))]
impl Smoke {
    fn sh1_152_controls(&self, _: &mut Context<Self>) -> Option<AnyElement> {
        None
    }
}

#[cfg(test)]
#[path = "terminal_view_tests/design.rs"]
mod design_tests;
