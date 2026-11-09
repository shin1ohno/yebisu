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
    pub live: Live,
    pub painter: TerminalPainter,
    pub open: bool,
    pub compact: bool,
    pub size: Option<ConnectOptions>,
}
impl Smoke {
    pub(crate) fn connect_terminal(&mut self, cx: &mut Context<Self>) {
        if self.setup.busy {
            return;
        }
        let target = self.input.text.trim().to_owned();
        let generation = self.terminal.link.reset();
        self.terminal.live = Live::default();
        self.terminal.size = None;
        self.terminal.open = true;
        let work = cx.background_executor().spawn(async move {
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
                this.terminal.link.install(generation, result);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(crate) fn poll_terminal(&mut self, cx: &mut Context<Self>) {
        if let Some(mut live) = self.terminal.link.take_update() {
            for text in live.clipboard.drain(..) {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
            }
            self.terminal.live = live;
            cx.notify();
        }
    }
    pub(crate) fn render_terminal(
        &mut self,
        window: &gpui::Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let insets = window.insets().safe_area;
        let entity = cx.entity();
        let generation = self.terminal.link.generation;
        let compact = self.terminal.compact;
        let label = if compact {
            "Compact 12/17"
        } else {
            "Standard 14/20"
        };
        let status = if self.terminal.live.status.is_empty() {
            "Connecting…".to_owned()
        } else {
            self.terminal.live.status.clone()
        };
        div().size_full().pt(insets.top).pb(insets.bottom).pl(insets.left).pr(insets.right).flex().flex_col().bg(rgb(0x101419)).text_color(rgb(0xd8dee9))
            .child(div().flex().gap_4().px_4().py_2()
                .child(div().id("terminal-back").px_4().py_2().bg(rgb(0x3b4252)).child("Hosts").on_click(cx.listener(|this,_,_,cx| {
                    this.terminal.link.reset();this.terminal.open=false;this.terminal.live=Live::default();cx.notify();
                })))
                .child(div().id("terminal-density").px_4().py_2().bg(rgb(0x3b4252)).child(label).on_click(cx.listener(|this,_,_,cx| {
                    this.terminal.compact = !this.terminal.compact;cx.notify();
                })))
                .child(div().child(format!("Read only · {status}"))))
            .child(canvas(|_,_,_| (),move |bounds,_,window,cx| {
                entity.update(cx,|this,cx| {
                    if !this.terminal.open || !this.terminal.link.current(generation) { return; }
                    let (font_size,line_height) = if compact {(12.,17.)}else{(14.,20.)};
                    let face = font("SauceCodePro Nerd Font Mono");
                    let terminal = &mut this.terminal;
                    terminal.painter.set_appearance(font_size,line_height,Theme::default());
                    let cell_width = terminal.painter.cell_width(&face,window,cx);
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
                    if let (Some(snapshot),Some(surface)) = (&terminal.live.snapshot,&terminal.live.surface)
                        && crate::live::coherent(snapshot,surface) {
                        window.paint_layer(bounds,|window| {
                            terminal.painter.paint_frame(&surface.frame,bounds.origin,Some(bounds.size),Ground::Opaque,cell_width,&face,&[],&surface.panes,None,None,window,cx);
                            if let Some(popup) = &surface.popup {
                                let origin = bounds.origin+crate::terminal::popup_origin(&surface.frame,&popup.frame,cell_width,line_height);
                                terminal.painter.paint_frame(&popup.frame,origin,None,Ground::Opaque,cell_width,&face,&[],&[],None,None,window,cx);
                            }
                        });
                        this.observation.record(serde_json::json!({"kind":"terminal","generation":generation,"boot":snapshot.boot_id,"projection":snapshot.revision,"surface":surface.surface_revision,"cols":surface.frame.width,"rows":surface.frame.height,"compact":compact,"viewport":[f32::from(bounds.size.width),f32::from(bounds.size.height)]}));
                    }
                });
            }).size_full()).into_any_element()
    }
}
