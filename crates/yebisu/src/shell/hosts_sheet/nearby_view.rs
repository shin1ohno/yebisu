use super::nearby::{Device, Search, Snapshot, Status};
use crate::{
    app::Smoke,
    shell::{
        add_host::{AddHostStep, SheetMode},
        style::{color, metric, radius, text, tint, weight},
    },
};
use gpui::{Context, Rgba, Role, Task, div, prelude::*, px, rgb};
use herdr_ssh::Target;
use std::time::Duration;

// Canvas v18 canvas.json notes.nearby, and the 56 pt row in fork 6c4b06eaa.
const ROW_HEIGHT: f32 = 56.;
const SELECTION_MARK: f32 = text::PAGE_TITLE;
const POLL_INTERVAL: Duration = Duration::from_millis(150);

#[derive(Default)]
pub(super) struct NearbySheet {
    search: Option<Search>,
    poll: Option<Task<()>>,
    snapshot: Snapshot,
    chosen: Option<Device>,
}

impl std::fmt::Debug for NearbySheet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NearbySheet")
            .field("snapshot", &self.snapshot)
            .field("chosen", &self.chosen)
            .finish_non_exhaustive()
    }
}

impl NearbySheet {
    fn already_saved(device: &Device, saved: &[String]) -> bool {
        let Some(candidate) = destination(&device.address) else {
            return true;
        };
        saved
            .iter()
            .filter_map(|target| Target::parse(target).ok())
            .any(|saved| saved.host == candidate.host && saved.port == candidate.port)
    }

    fn visible_devices<'a>(&'a self, saved: &'a [String]) -> impl Iterator<Item = &'a Device> {
        self.snapshot
            .devices
            .iter()
            .filter(|device| !Self::already_saved(device, saved))
    }

    fn chosen_is_current(&self, typed: &str) -> bool {
        self.chosen
            .as_ref()
            .is_some_and(|device| match Target::parse(typed) {
                Ok(current) => destination(&device.address).is_some_and(|chosen| {
                    current.host == chosen.host && current.port == chosen.port
                }),
                Err(_) => typed == device.address,
            })
    }
}

fn destination(address: &str) -> Option<Target> {
    Target::parse(&format!("user@{address}")).ok()
}

fn fill(typed: &str, address: &str) -> String {
    if let Some((user, _)) = typed.trim().split_once('@') {
        let joined = format!("{user}@{address}");
        if Target::parse(&joined).is_ok() {
            return joined;
        }
    }
    address.to_owned()
}

impl Smoke {
    pub(super) fn nearby_needs_username(&self) -> bool {
        let typed = self.setup_input.text.trim();
        self.shell.hosts_sheet.nearby.chosen_is_current(typed) && Target::parse(typed).is_err()
    }

    fn nearby_editable(&self) -> bool {
        !self.setup.busy
            && self.shell.hosts_sheet.add.as_ref().is_some_and(|session| {
                session.mode == SheetMode::Add && session.flow.step != AddHostStep::SignIn
            })
    }

    #[cfg(target_os = "ios")]
    fn browse_nearby(&mut self, restart: bool, cx: &mut Context<Self>) {
        if !cfg!(test)
            && self
                .shell
                .hosts_sheet
                .add
                .as_ref()
                .is_some_and(|session| session.mode == SheetMode::Add)
            && (restart || self.shell.hosts_sheet.nearby.search.is_none())
        {
            self.start_nearby_with(super::nearby::browse, cx);
        }
    }

    #[cfg(not(target_os = "ios"))]
    fn browse_nearby(&mut self, _: bool, _: &mut Context<Self>) {}

    pub(super) fn start_nearby_with(
        &mut self,
        browser: impl FnOnce(&super::nearby::Mailbox) + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        self.shell.hosts_sheet.nearby.search.take();
        self.shell.hosts_sheet.nearby.poll.take();
        self.shell.hosts_sheet.nearby.snapshot = Snapshot::default();
        self.shell.hosts_sheet.nearby.search = Some(Search::start(browser));
        self.shell.hosts_sheet.nearby.poll = Some(cx.spawn(async move |weak, cx| {
            loop {
                cx.background_executor().timer(POLL_INTERVAL).await;
                if weak.update(cx, |this, cx| this.poll_nearby(cx)).is_err() {
                    break;
                }
            }
        }));
        cx.notify();
    }

    pub(super) fn poll_nearby(&mut self, cx: &mut Context<Self>) {
        let nearby = &mut self.shell.hosts_sheet.nearby;
        if let Some(snapshot) = nearby.search.as_ref().and_then(Search::take_latest)
            && snapshot != nearby.snapshot
        {
            nearby.snapshot = snapshot;
            cx.notify();
        }
    }

    fn choose_nearby(&mut self, address: &str, cx: &mut Context<Self>) {
        if !self.nearby_editable() {
            return;
        }
        let nearby = &self.shell.hosts_sheet.nearby;
        let Some(chosen) = nearby
            .visible_devices(&self.setup.hosts)
            .find(|device| device.address == address)
            .cloned()
        else {
            // The row can vanish between paint and pointer up.
            return;
        };
        let typed = fill(&self.setup_input.text, &chosen.address);
        let changed = typed != self.setup_input.text;
        if changed {
            self.setup_input.set_text(typed);
            if Target::parse(&self.setup_input.text).is_err() {
                self.setup_input.selected = 0..0;
            }
            self.hosts_sync_field();
            self.setup.pending = None;
            self.setup.fingerprints.clear();
        }
        let name = chosen.name.clone();
        let sheet = &mut self.shell.hosts_sheet;
        sheet.nearby.chosen = Some(chosen);
        if let Some(field) = &sheet.name {
            field.update(cx, |field, cx| field.suggest(address, name, cx));
        }
        cx.notify();
    }

    pub(super) fn nearby_list(&mut self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        if self.shell.hosts_sheet.add.as_ref()?.mode != SheetMode::Add {
            return None;
        }
        self.browse_nearby(false, cx);
        let nearby = &self.shell.hosts_sheet.nearby;
        // Host tests never trigger native browsing unless they inject a browser.
        nearby.search.as_ref()?;
        let rows: Vec<_> = nearby.visible_devices(&self.setup.hosts).cloned().collect();
        let status = match nearby.snapshot.status {
            Status::Searching => "Searching Bonjour…".into(),
            Status::WaitingForPermission => {
                "Waiting for iPadOS to allow yebisu on your local network…".into()
            }
            Status::Finished => {
                let count = rows.len();
                let noun = if count == 1 { "device" } else { "devices" };
                format!("Search finished. Found {count} {noun}.")
            }
            Status::Unavailable => {
                "Bonjour is unavailable. Check Local Network in Settings.".into()
            }
        };
        let editable = self.nearby_editable();
        let header = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(metric::INLINE_GAP))
            .child(
                div()
                    .flex_none()
                    .text_size(px(text::SMALL))
                    .font_weight(weight::SEMIBOLD)
                    .text_color(rgb(color::MUTED))
                    .child("NEARBY DEVICES"),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_right()
                    .text_color(rgb(color::MUTED))
                    .child(status),
            )
            .when(
                matches!(
                    nearby.snapshot.status,
                    Status::Finished | Status::Unavailable
                ),
                |header| {
                    header.child(
                        super::view::button("nearby-search-again", "Search again", editable, false)
                            .on_click(cx.listener(|this, _, _, cx| {
                                if this.nearby_editable() {
                                    this.browse_nearby(true, cx);
                                }
                            })),
                    )
                },
            );
        let list = div()
            .id("nearby-rows")
            .flex()
            .flex_col()
            .flex_shrink_0()
            .max_h(px(ROW_HEIGHT * 4.5))
            .overflow_y_scroll()
            .children(rows.into_iter().enumerate().map(|(index, device)| {
                let selected = nearby.chosen.as_ref().is_some_and(|chosen| {
                    chosen.address == device.address
                        && nearby.chosen_is_current(&self.setup_input.text)
                });
                let address = device.address;
                let target = fill(&self.setup_input.text, &address);
                div()
                    .id(gpui::SharedString::from(format!("nearby-device-{address}")))
                    .debug_selector(move || format!("nearby-device-{index}"))
                    .role(Role::Button)
                    .aria_label(format!("{}, {target}, Bonjour", device.name))
                    .h(px(ROW_HEIGHT))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap(px(metric::INLINE_GAP))
                    .px(px(metric::INLINE_GAP))
                    .rounded(px(radius::CONTROL))
                    .when(selected, |row| row.bg::<Rgba>(tint::SELECTED.into()))
                    .when(editable, |row| {
                        row.cursor_pointer().hover(|row| row.bg(rgb(color::RAISED)))
                    })
                    .child(div().max_w(px(164.)).truncate().child(device.name))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(rgb(color::MUTED))
                            .child(target),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_color(rgb(color::MUTED))
                            .child("Bonjour"),
                    )
                    .when(selected, |row| {
                        row.child(
                            div()
                                .flex_none()
                                .size(px(SELECTION_MARK))
                                .text_size(px(SELECTION_MARK))
                                .text_color(rgb(color::ACCENT))
                                .child("✓"),
                        )
                    })
                    .on_click(cx.listener(move |this, _, _, cx| this.choose_nearby(&address, cx)))
            }));
        Some(
            div()
                .flex()
                .flex_col()
                .flex_shrink_0()
                .child(header)
                .child(list),
        )
    }
}

#[cfg(all(test, not(target_os = "ios")))]
#[path = "nearby_tests.rs"]
mod tests;
