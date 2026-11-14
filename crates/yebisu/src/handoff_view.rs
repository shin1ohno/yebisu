use crate::{
    app::Smoke,
    handoff::{Selection, Stamp},
    terminal_input::GeneratedInput,
};
use gpui::{AnyElement, ClipboardItem, Context, Window, div, prelude::*, rgb};
impl Smoke {
    pub(crate) fn sync_handoff(&mut self) {
        let stamp = if !self.settings_modal
            && self.terminal.open
            && self.terminal.navigating.is_none()
            && !self.actions.palette
            && !self.terminal.reconnect.suspended
        {
            let Ok(live) = self.terminal.link.inbox.try_lock() else {
                return;
            };
            crate::terminal_input::destination(&live, self.terminal.link.generation).map(
                |destination| Stamp {
                    selection: self.board.selection,
                    destination,
                },
            )
        } else {
            None
        };
        self.handoff.sync(stamp);
        if self.handoff.selection.as_ref().is_some_and(|selection| {
            self.terminal.live.surface.as_ref().is_none_or(|surface| {
                let frame = surface
                    .popup
                    .as_ref()
                    .map(|p| &p.frame)
                    .unwrap_or(&surface.frame);
                frame != selection.frame.as_ref()
            })
        }) {
            self.handoff.selection = None;
        }
    }
    pub(crate) fn copy_selection(&mut self, cx: &mut Context<Self>) {
        self.sync_handoff();
        if let Some(text) = self.handoff.selection.as_ref().and_then(Selection::text) {
            #[cfg(test)]
            {
                self.handoff.clipboard_writes += 1;
            }
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            self.terminal.live.status = "Selection copied".into();
        }
        cx.notify();
    }
    pub(crate) fn start_paste(&mut self, image_only: bool, cx: &mut Context<Self>) {
        self.refresh_terminal_input();
        self.sync_handoff();
        let Some(stamp) = self.handoff.stamp.clone() else {
            return;
        };
        // One preparation at a time: cancelled workers keep their slot until exit.
        if self
            .handoff
            .core_ticket
            .as_ref()
            .is_some_and(|ticket| !ticket.is_finished())
            || self.handoff.workers != 0
            || self
                .handoff
                .cancellation
                .as_ref()
                .is_some_and(|c| !c.is_finished())
            || !self.terminal.input.ready
        {
            self.terminal.live.status = "Paste refused: preparation busy or frame pending".into();
            self.handoff.refused_tail = self.handoff.preparing;
            cx.notify();
            return;
        }
        self.handoff.selection = None;
        self.handoff.select_mode = false;
        self.terminal.input.discard();
        self.terminal.input.fresh_gesture();
        self.flush_terminal();
        if self.terminal.core_mode {
            self.paste_core(stamp.destination, image_only, cx);
            return;
        }
        let target = match &stamp.destination.target {
            crate::terminal::InputTarget::Pane(id) => {
                sheprd_client::protocol::ClientClipboardImageTarget::Pane(id.clone())
            }
            crate::terminal::InputTarget::Popup(id) => {
                sheprd_client::protocol::ClientClipboardImageTarget::Popup(id.clone())
            }
        };
        let Some(handle) = self.terminal.link.handle.as_ref() else {
            return;
        };
        let upload = match handle.reserve_clipboard_input(&stamp.destination.boot, target) {
            Ok(upload) => upload,
            Err(e) => {
                self.terminal.live.status = e.to_string();
                cx.notify();
                return;
            }
        };
        self.handoff.cancellation = Some(upload.cancellation_handle());
        self.handoff.preparing = true;
        self.handoff.workers += 1;
        let serial = self.handoff.serial;
        let inbox = self.terminal.link.inbox.clone();
        let destination = stamp.destination.clone();
        #[cfg(test)]
        {
            self.handoff.clipboard_reads += 1;
        }
        let payload = crate::handoff_clipboard::read(image_only, cx);
        #[cfg(test)]
        let publication_gate = self.handoff.publication_gate.take();
        let work = cx.background_executor().spawn(async move {
            payload.and_then(|payload| {
                crate::handoff::prepare(
                    payload,
                    sheprd_client::protocol::MAX_CLIPBOARD_IMAGE_PAYLOAD,
                )
            })
        });
        #[cfg(test)]
        let completion_gate = self.handoff.completion_gate.take();
        cx.spawn(async move |weak, cx| {
            let prepared = work.await;
            #[cfg(test)]
            if let Some(gate) = completion_gate {
                gate.await;
            }
            let current = weak
                .update(cx, |this, _| {
                    this.sync_handoff();
                    this.handoff.serial == serial && this.handoff.stamp.as_ref() == Some(&stamp)
                })
                .unwrap_or(false);
            let result = if current {
                // Publication may encode a large wire payload: never run on UI.
                cx.background_executor()
                    .spawn(async move {
                        // This seam is after UI stamp admission. The live target
                        // check and SDK cancellation gate still run on release.
                        #[cfg(test)]
                        if let Some(gate) = publication_gate { gate.await; }
                        prepared.and_then(|p| {
                            let live = inbox
                                .lock()
                                .map_err(|_| crate::Error::Ssh("Paste state unavailable".into()))?;
                            if crate::terminal_input::destination(&live, destination.generation)
                                .as_ref()
                                != Some(&destination)
                            {
                                return Err(crate::Error::Ssh("Paste target changed".into()));
                            }
                            p.publish(upload)
                                .map_err(|e| crate::Error::Ssh(e.to_string()))
                        })
                    })
                    .await
            } else {
                drop(upload);
                Err(crate::Error::Ssh("Paste cancelled: target changed".into()))
            };
            let _ = weak.update(cx, |this, cx| {
                this.observation.record(serde_json::json!({"kind":"p8-paste-result","serial":serial,"current":this.handoff.serial==serial,"result":result.as_ref().map(|_|"queued").map_err(|e|e.to_string())}));
                this.handoff.workers = this.handoff.workers.saturating_sub(1);
                // Cancelled workers also release the slot used by native edit availability.
                cx.notify();
                this.sync_handoff();
                if this.handoff.serial != serial {
                    return;
                }
                this.handoff.preparing = false;
                this.handoff.refused_tail = false;
                match result {
                    Ok(()) => {
                        this.terminal.live.status = "Paste queued".into();
                        let held = std::mem::take(&mut this.handoff.held);
                        for event in held {
                            this.send_terminal(event);
                        }
                    }
                    Err(e) => {
                        this.handoff.cancel();
                        this.terminal.live.status = e.to_string();
                    }
                }
            });
        })
        .detach();
        cx.notify();
    }
    pub(crate) fn toggle_selection(&mut self) {
        self.handoff.select_mode ^= true;
        self.handoff.cancel();
        self.terminal.input.detach();
        if !self.handoff.select_mode {
            self.handoff.selection = None;
            self.terminal.input.fresh_gesture();
        }
    }
    pub(crate) fn selection_down(
        &mut self,
        event: &gpui::MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sync_handoff();
        if !self.terminal.geometry.contains(event.position) {
            return;
        }
        self.terminal_focus.focus(window, cx);
        if self.handoff.stamp.is_some()
            && (self.handoff.select_mode || self.handoff.drag_select || event.click_count >= 2)
        {
            self.handoff.cancel();
            self.terminal.input.detach();
            let Some(surface) = &self.terminal.live.surface else {
                return;
            };
            let frame = surface
                .popup
                .as_ref()
                .map(|p| &p.frame)
                .unwrap_or(&surface.frame);
            if let Some(cell) = self.selection_cell(event.position) {
                self.handoff.selection = Some(Selection {
                    frame: std::sync::Arc::new(frame.clone()),
                    anchor: cell,
                    end: cell,
                    dragging: true,
                });
            }
        } else {
            self.handoff.selection = None;
            self.terminal.input.fresh_gesture();
            window.request_virtual_keyboard();
        }
        cx.notify();
    }
    fn selection_cell(&self, point: gpui::Point<gpui::Pixels>) -> Option<(u16, u16)> {
        let surface = self.terminal.live.surface.as_ref()?;
        let frame = surface
            .popup
            .as_ref()
            .map(|p| &p.frame)
            .unwrap_or(&surface.frame);
        if frame.width == 0 || frame.height == 0 {
            return None;
        }
        let geometry = self.terminal.geometry;
        let origin = if surface.popup.is_some() {
            geometry.area(Some(surface)).origin
        } else {
            geometry.grid.origin
        };
        Some((
            ((f32::from(point.x - origin.x) / geometry.cell_width)
                .floor()
                .max(0.) as u16)
                .min(frame.width - 1),
            ((f32::from(point.y - origin.y) / geometry.cell_height)
                .floor()
                .max(0.) as u16)
                .min(frame.height - 1),
        ))
    }
    pub(crate) fn selection_move(&mut self, event: &gpui::MouseMoveEvent, cx: &mut Context<Self>) {
        if event.pressed_button != Some(gpui::MouseButton::Left) {
            return;
        }
        self.sync_handoff();
        let cell = self.selection_cell(event.position);
        if let Some(selection) = &mut self.handoff.selection
            && selection.dragging
            && let Some(cell) = cell
        {
            selection.end = cell;
            cx.notify();
        }
    }
    pub(crate) fn selection_up(&mut self, cx: &mut Context<Self>) {
        let Some(selection) = &mut self.handoff.selection else {
            return;
        };
        if !selection.dragging {
            return;
        }
        selection.dragging = false;
        if self.handoff.copy_on_select {
            self.copy_selection(cx);
        }
        cx.notify();
    }
    pub(crate) fn handoff_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .flex_wrap()
            .gap_2()
            .px_4()
            .children(
                [
                    (
                        "select",
                        if self.handoff.select_mode {
                            "Select: On"
                        } else {
                            "Select"
                        },
                    ),
                    ("copy", "Copy"),
                    ("paste", "Paste"),
                    ("paste-image", "Paste image"),
                    (
                        "drag",
                        if self.handoff.drag_select {
                            "Start: Drag"
                        } else {
                            "Start: Select / double tap"
                        },
                    ),
                    (
                        "auto-copy",
                        if self.handoff.copy_on_select {
                            "Copy: On selection"
                        } else {
                            "Copy: Manual"
                        },
                    ),
                ]
                .into_iter()
                .map(|(id, label)| {
                    div()
                        .id(id)
                        .p_2()
                        .bg(rgb(0x3b4252))
                        .child(label)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            match id {
                                "select" => {
                                    this.toggle_selection();
                                }
                                "copy" => this.copy_selection(cx),
                                "paste" => this.start_paste(false, cx),
                                "paste-image" => this.start_paste(true, cx),
                                "drag" => this.toggle_selection_setting(false),
                                "auto-copy" => this.toggle_selection_setting(true),
                                _ => {}
                            }
                            cx.notify();
                        }))
                }),
            )
            .into_any_element()
    }
    // The strip and adapter tests share this handler, including immediate draft capture.
    pub(crate) fn toggle_selection_setting(&mut self, copy: bool) {
        if copy {
            self.handoff.copy_on_select ^= true;
        } else {
            self.handoff.drag_select ^= true;
        }
        let values = self.settings_values();
        if let Some(host) = &mut self.settings {
            host.edit(values);
            host.flush();
        }
    }

    pub(crate) fn hold_for_paste(&mut self, event: GeneratedInput) -> bool {
        if !self.handoff.hold(event) {
            self.terminal.live.status = "Input refused behind paste".into();
            false
        } else {
            true
        }
    }
}

#[cfg(not(all(target_os = "ios", target_abi = "sim")))]
impl Smoke {
    pub(crate) fn handoff_qa_controls(&self, _: &mut Context<Self>) -> Option<AnyElement> {
        None
    }
    pub(crate) fn edit_menu_qa_controls(&self, _: &mut Context<Self>) -> Option<AnyElement> {
        None
    }
    pub(crate) fn native_edit_qa_controls(&self, _: &mut Context<Self>) -> Option<AnyElement> {
        None
    }
}
