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
    pub navigating: Option<crate::board::Navigation>,
}
impl Smoke {
    pub(crate) fn connect_terminal(&mut self, cx: &mut Context<Self>) {
        if self.setup.busy || self.terminal.reconnect.suspended {
            return;
        }
        let target = if self.terminal.open {
            self.terminal.reconnect.target.clone()
        } else {
            self.setup_input.text.trim().to_owned()
        };
        self.select_host(&target);
        self.start_host_connection(target, cx);
        for host in self.setup.hosts.clone() {
            if host != self.terminal.reconnect.target && !self.board.hidden.contains_key(&host) {
                self.start_host_connection(host, cx);
            }
        }
    }
    pub(crate) fn start_host_connection(&mut self, target: String, cx: &mut Context<Self>) {
        let selected = target == self.terminal.reconnect.target;
        let terminal = if selected {
            &mut self.terminal
        } else {
            self.board.hidden.entry(target.clone()).or_default()
        };
        let host_generation = terminal.reconnect.begin(target.clone());
        let host = target.clone();
        terminal.input.discard();
        terminal
            .input
            .sync(&Live::default(), terminal.link.generation, None);
        let generation = terminal.link.reset();
        self.observation.record(serde_json::json!({"kind":"reconnect","generation":generation,"automatic":terminal.reconnect.automatic}));
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
        self.poll_hidden(cx);
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
                    let terminal = &mut this.terminal;
                    terminal.painter.set_appearance(font_size,line_height,Theme::default());
                    let cell_width = terminal.painter.cell_width(&face,window,cx);
                    terminal.geometry = crate::input_geometry::InputGeometry { grid: bounds, cell_width, cell_height: line_height };
                    let options = ConnectOptions {
                        surface_size: crate::terminal::viewport(f32::from(bounds.size.width),f32::from(bounds.size.height),cell_width,line_height),
                        cell_width_px: (cell_width*window.scale_factor()).round() as u32,
                        cell_height_px: (line_height*window.scale_factor()).round() as u32,
                    };
                    if terminal.size != Some(options)
                        && let (Some(handle),Some(snapshot)) = (&terminal.link.handle,&terminal.live.snapshot) {
                        match handle.resize(&snapshot.boot_id,options) {
                            Ok(()) => terminal.size=Some(options),
                            Err(e) => terminal.live.status=e.to_string(),
                        }
                    }
                    terminal.input.sync(&terminal.live, generation, terminal.size);
                    if let (Some(snapshot),Some(surface)) = (&terminal.live.snapshot,&terminal.live.surface)
                        && crate::live::coherent(snapshot,surface) {
                        window.paint_layer(bounds,|window| {
                            terminal.painter.paint_frame(&surface.frame,bounds.origin,Some(bounds.size),Ground::Opaque,cell_width,&face,&[],&surface.panes,None,None,window,cx);
                            if let Some(popup) = &surface.popup {
                                let origin = bounds.origin+crate::terminal::popup_origin(&surface.frame,&popup.frame,cell_width,line_height);
                                terminal.painter.paint_frame(&popup.frame,origin,None,Ground::Opaque,cell_width,&face,&[],&[],None,None,window,cx);
                            }
                            terminal.painter.paint_composition(&terminal.input.composition.text,
                                terminal.geometry.cursor(Some(surface)).origin, terminal.geometry.area(Some(surface)), &face, window);
                        });
                        this.observation.record(serde_json::json!({"kind":"terminal","generation":generation,"boot":snapshot.boot_id,"projection":snapshot.revision,"surface":surface.surface_revision,"cols":surface.frame.width,"rows":surface.frame.height,"compact":compact,"marked":terminal.input.composition.text,"selected":[terminal.input.composition.selected.start,terminal.input.composition.selected.end],"viewport":[f32::from(bounds.size.width),f32::from(bounds.size.height)]}));
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
        if self.terminal.needs_focus {
            self.terminal_focus.focus(window, cx);
            self.terminal.needs_focus = false;
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
                                    && this.terminal.live.version_mismatch.is_none()
                                {
                                    this.connect_terminal(cx);
                                }
                            })),
                    )
                    .child(div().child(format!("P6 · {status}"))),
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
            .child(self.render_board(cx))
            .when(self.terminal.show_options, |view| {
                view.child(self.input_options(cx))
            })
            .children(self.input_qa_controls(cx))
            .child(
                div()
                    .id("terminal-input")
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                            if this.terminal.geometry.contains(event.position) {
                                this.terminal_focus.focus(window, cx);
                                this.terminal.input.fresh_gesture();
                                window.request_virtual_keyboard();
                            }
                        }),
                    )
                    .child(canvas(|_, _, _| (), self.terminal_paint_callback(cx)).size_full()),
            )
            .into_any_element()
    }
}

#[cfg(test)]
#[path = "terminal_view_tests.rs"]
mod tests;
