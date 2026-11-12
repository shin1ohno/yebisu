use crate::{
    app::Smoke,
    link::Link,
    live::Live,
    terminal_painter::{Ground, TerminalPainter},
    theme::Theme,
};
use gpui::{AnyElement, ClipboardItem, Context, canvas, div, font, prelude::*, rgb};
use sheprd_client::{ConnectOptions, ConnectTarget};

#[derive(Default)]
pub(crate) struct TerminalView {
    pub link: Link,
    pub core: Option<crate::core::Session>,
    pub core_mode: bool,
    #[cfg(test)]
    pub core_test_backend: Option<sheprd_core_client::CoreBackend>,
    pub reconnect: crate::reconnect::Reconnect,
    pub live: Live,
    pub painter: TerminalPainter,
    pub open: bool,
    pub compact: bool,
    pub size: Option<ConnectOptions>,
    pub input: crate::terminal_input::TerminalInput,
    pub geometry: crate::input_geometry::InputGeometry,
    pub show_options: bool,
    pub needs_focus: bool,
    pub reconnect_pending: bool,
    #[cfg(all(target_os = "ios", target_abi = "sim"))]
    pub late_reader_release: Option<std::sync::mpsc::Sender<()>>,
    pub navigating: Option<crate::board::Navigation>,
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
        if self.setup.busy || self.terminal.reconnect.suspended {
            return;
        }
        let from_setup = !self.terminal.open;
        let target = if self.terminal.open {
            self.terminal.reconnect.target.clone()
        } else {
            self.setup_input.text.trim().to_owned()
        };
        self.select_host(&target);
        if from_setup {
            self.terminal.core.take();
            self.terminal.core_mode = false;
        }
        self.start_host_connection(target, cx);
        for host in self.setup.hosts.clone() {
            if host != self.terminal.reconnect.target && !self.board.hidden.contains_key(&host) {
                self.start_host_connection(host, cx);
            }
        }
    }
    pub(crate) fn start_host_connection(&mut self, target: String, cx: &mut Context<Self>) {
        if self.terminal.core_mode && self.terminal.reconnect.target == target {
            self.connect_core(cx);
            return;
        }
        if self.board.hidden.get(&target).is_some_and(|t| t.core_mode) {
            return;
        }
        let selected = target == self.terminal.reconnect.target;
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
        let work = cx.background_executor().spawn(async move {
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
                let terminal = if this.terminal.reconnect.target == host {
                    Some(&mut this.terminal)
                } else {
                    this.board.hidden.get_mut(&host)
                };
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
        let mut pending = self
            .board
            .hidden
            .iter()
            .filter(|(_, t)| t.reconnect_pending && !t.reconnect.suspended)
            .map(|(host, _)| host.clone())
            .collect::<Vec<_>>();
        if self.terminal.reconnect_pending && !self.terminal.reconnect.suspended {
            pending.push(self.terminal.reconnect.target.clone());
        }
        for host in pending {
            self.start_host_connection(host, cx);
        }
        self.poll_hidden(cx);
        self.sync_handoff();
        if !self.terminal.open || self.terminal.reconnect.suspended {
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
            let current = entity.update(cx,|this,cx| {
                    if !this.terminal.open || this.board.selection != selection || !this.terminal.link.current(generation) { return false; }
                    let (font_size,line_height) = if compact {(12.,17.)}else{(14.,20.)};
                    let face = font("SauceCodePro Nerd Font Mono");
                    let search_highlights = this.find_highlights();
                    let search_highlight_count = search_highlights.len();
                    let search_receipt = this.find.as_ref().map(|bar|serde_json::json!({"query":bar.search.query,"state":format!("{:?}",bar.search.state),"result_revision":bar.search.result.as_ref().map(|r|r.content_revision),"current":bar.search.result.as_ref().and_then(|r|r.current),"pane":bar.search.scope.pane}));
                    let terminal = &mut this.terminal;
                    terminal.painter.set_appearance(font_size,line_height,Theme::default());
                    let cell_width = terminal.painter.cell_width(&face,window,cx);
                    terminal.geometry = crate::input_geometry::InputGeometry { grid: bounds, cell_width, cell_height: line_height };
                    let options = ConnectOptions {
                        surface_size: crate::terminal::viewport(f32::from(bounds.size.width),f32::from(bounds.size.height),cell_width,line_height),
                        cell_width_px: (cell_width*window.scale_factor()).round() as u32,
                        cell_height_px: (line_height*window.scale_factor()).round() as u32,
                    };
                    terminal.resize_if_ready(options);
                    terminal.input.sync(&terminal.live, generation, terminal.size);
                    if let (Some(snapshot),Some(surface)) = (&terminal.live.snapshot,&terminal.live.surface)
                        && crate::live::coherent(snapshot,surface) {
                        let mut highlights = this.handoff.selection.as_ref().map(|s|s.highlights()).unwrap_or_default();
                        highlights.extend(search_highlights);
                        window.with_content_mask(Some(gpui::ContentMask { bounds }),|window| {
                            terminal.painter.paint_frame(&surface.frame,bounds.origin,Some(bounds.size),Ground::Opaque,cell_width,&face,if surface.popup.is_none() { &highlights } else { &[] },&surface.panes,None,Some(crate::terminal_painter::PlacedImages { placements:&surface.graphics.placements, images:&terminal.live.surface_images, target:crate::terminal_painter::ImageTarget::Main }),window,cx);
                            if let Some(popup) = &surface.popup {
                                let origin = bounds.origin+crate::terminal::popup_origin(&surface.frame,&popup.frame,cell_width,line_height);
                                terminal.painter.paint_frame(&popup.frame,origin,None,Ground::Opaque,cell_width,&face,&highlights,&[],None,Some(crate::terminal_painter::PlacedImages { placements:&surface.graphics.placements, images:&terminal.live.surface_images, target:crate::terminal_painter::ImageTarget::Popup(&popup.terminal_id) }),window,cx);
                            }
                            terminal.painter.paint_composition(&terminal.input.composition.text,
                                terminal.geometry.cursor(Some(surface)).origin, terminal.geometry.area(Some(surface)), &face, window);
                        });
                        this.observation.record(serde_json::json!({"kind":"terminal","p13_search":search_receipt,"p13_highlights":search_highlight_count,"image_assets":terminal.live.surface_images.len(),"image_placements":surface.graphics.placements.len(),"host":terminal.reconnect.target,"pane":snapshot.focused_pane_id,"tab":snapshot.focused_tab_id,"tab_count":snapshot.tabs.iter().filter(|t|Some(&t.workspace_id)==snapshot.focused_workspace_id.as_ref()).count(),"pane_count":snapshot.panes.iter().filter(|p|Some(&p.tab_id)==snapshot.focused_tab_id.as_ref()).count(),"surface_marker":surface.frame.cells.first().map(|c|c.symbol.as_str()),"input_pane":terminal.input.destination.as_ref().and_then(|d|match &d.target { crate::terminal::InputTarget::Pane(id)=>Some(id), _=>None }),"generation":generation,"boot":snapshot.boot_id,"projection":snapshot.revision,"surface":surface.surface_revision,"cols":surface.frame.width,"rows":surface.frame.height,"compact":compact,"marked":terminal.input.composition.text,"selected":[terminal.input.composition.selected.start,terminal.input.composition.selected.end],"viewport":[f32::from(bounds.size.width),f32::from(bounds.size.height)]}));
                    }
                    true
                });
            if !current {
                return;
            }
            let focus = input_entity.read(cx).terminal_focus.clone();
            window.handle_input(
                &focus,
                gpui::ElementInputHandler::new(bounds, input_entity),
                cx,
            );
        }
    }
    pub(crate) fn render_terminal(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.terminal.needs_focus && self.find.is_none() {
            self.terminal_focus.focus(window, cx);
            self.terminal.needs_focus = false;
        }
        // Submit once when an invalidated search hands focus back. Refusals
        // remain fenced; rendering never retries a native retirement request.
        if self.find_handoff.needs_request && self.find.is_none() {
            self.refresh_terminal_input();
            self.request_find_retirement(window, cx);
        }
        let insets = window.insets().safe_area;
        let bottom = window.viewport_size().height - window.fully_visible_bounds().bottom();
        let compact = self.terminal.compact;
        let label = if compact {
            "Compact 12/17"
        } else {
            "Standard 14/20"
        };
        let status = if self.terminal.navigating.is_some() {
            "Navigating… · input paused".to_owned()
        } else if let Some(reason) = self.find_keyboard_status() {
            reason.to_owned()
        } else if self.terminal.live.connected && self.terminal.input.requires_gesture() {
            "Composition discarded · tap terminal to type".to_owned()
        } else if self.terminal.live.status.is_empty() {
            "Connecting…".to_owned()
        } else {
            self.terminal.live.status.clone()
        };
        div()
            .size_full()
            .pt(insets.top)
            .pb(insets.bottom.max(bottom))
            .pl(insets.left)
            .pr(insets.right)
            .flex()
            .flex_col()
            .bg(rgb(0x101419))
            .text_color(rgb(0xd8dee9))
            .track_focus(&self.terminal_focus)
            .on_key_down(cx.listener(Self::terminal_key_down))
            .on_key_up(cx.listener(Self::terminal_key_up))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
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
            .child(
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
                        cx.notify();
                    })),
            )
            .when(
                !self.terminal.core_mode && !self.actions.board_hidden,
                |view| view.child(self.render_board(cx)),
            )
            .when(!self.terminal.core_mode, |view| {
                view.child(self.render_actions(cx))
            })
            .when(self.terminal.core_mode, |view| {
                view.child(self.render_core_controls(cx))
            })
            .when(self.terminal.show_options, |view| {
                view.child(self.input_options(cx))
            })
            .children(self.input_qa_controls(cx))
            .child(self.handoff_controls(cx))
            .child(self.render_find(cx))
            .children(self.p13_qa_controls(cx))
            .children(self.handoff_qa_controls(cx))
            .children(self.native_edit_qa_controls(cx))
            .child(
                div()
                    .id("terminal-input")
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                            if this.find.is_some() {
                                return;
                            }
                            this.selection_down(event, window, cx);
                        }),
                    )
                    .on_mouse_move(cx.listener(|this, event, _, cx| this.selection_move(event, cx)))
                    .on_mouse_up(
                        gpui::MouseButton::Left,
                        cx.listener(|this, _, _, cx| this.selection_up(cx)),
                    )
                    .on_mouse_up_out(
                        gpui::MouseButton::Left,
                        cx.listener(|this, _, _, cx| this.selection_up(cx)),
                    )
                    .child(canvas(|_, _, _| (), self.terminal_paint_callback(cx)).size_full()),
            )
            .into_any_element()
    }
}

#[cfg(test)]
#[path = "terminal_view_tests.rs"]
mod tests;
