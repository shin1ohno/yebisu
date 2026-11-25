//! Pick the SSH destination before choosing the daemon on that host.
use super::new_space_choices;
use crate::{
    app::Smoke,
    hosts::HostName,
    setup::Setup,
    shell::{
        HerdrKind,
        sidebar::{model::HostLink, view::host_entries},
        style,
    },
};
use gpui::{AnyElement, Context, Window, div, prelude::*, px, rgb};
use std::collections::BTreeMap;

#[derive(Debug, Eq, PartialEq)]
struct HostChoice {
    name: HostName,
    kinds: Vec<HerdrKind>,
}

#[derive(Debug, Default)]
pub(super) struct Chooser {
    hosts: Vec<HostChoice>,
    choice: Option<(usize, HerdrKind)>,
    selected_host: Option<usize>,
    checking: bool,
    error: Option<String>,
    focus: Option<gpui::FocusHandle>,
}

impl Chooser {
    fn new(hosts: Vec<HostChoice>) -> Self {
        Self {
            hosts,
            ..Self::default()
        }
    }

    fn select_host(&mut self, index: usize) {
        self.error = None;
        self.selected_host = Some(index);
        self.choice = self
            .hosts
            .get(index)
            .and_then(|host| host.kinds.last())
            .map(|kind| (index, *kind));
    }

    fn select_kind(&mut self, kind: HerdrKind) {
        self.error = None;
        if let Some((index, _)) = self.choice
            && self.hosts[index].kinds.contains(&kind)
        {
            self.choice = Some((index, kind));
        }
    }

    fn selection(&self) -> Option<(&str, HerdrKind)> {
        if self.checking {
            return None;
        }
        let (index, kind) = self.choice?;
        Some((self.hosts.get(index)?.name.destination(), kind))
    }
}

fn host_choices(
    setup: &Setup,
    links: impl IntoIterator<Item = (String, HerdrKind)>,
) -> Vec<HostChoice> {
    let mut hosts = BTreeMap::<String, Vec<HerdrKind>>::new();
    for (host, kind) in links {
        hosts.entry(host).or_default().push(kind);
    }
    let mut ordered = Vec::new();
    for host in &setup.hosts {
        ordered.push(make_host(
            setup.host_name(host),
            hosts.remove(host).unwrap_or_default(),
        ));
    }
    ordered.extend(
        hosts
            .into_iter()
            .map(|(host, kinds)| make_host(setup.host_name(&host), kinds)),
    );
    ordered
}

fn make_host(name: HostName, kinds: Vec<HerdrKind>) -> HostChoice {
    HostChoice {
        name,
        kinds: new_space_choices(&kinds),
    }
}

fn kind_label(kind: HerdrKind) -> &'static str {
    match kind {
        HerdrKind::Herdr1 => "Herdr 1",
        HerdrKind::Herdr2 => "Herdr 2",
    }
}

impl Smoke {
    /// New space's entry point for the sidebar and the rail.
    pub(crate) fn open_new_space(&mut self, cx: &mut Context<Self>) {
        if self.input_modal() || self.new_space_open() {
            return;
        }
        self.remember_connected_core();
        let entries = host_entries(
            &self.shell.sidebar,
            &self.terminal,
            self.board.hidden.iter().chain(&self.board.hidden_core),
        );
        let remembered = self
            .shell
            .herdr2
            .selected
            .keys()
            .cloned()
            .map(|host| (host, HerdrKind::Herdr2));
        let hosts = host_choices(
            &self.setup,
            entries
                .into_iter()
                .filter(|entry| entry.link == HostLink::Connected)
                .map(|entry| (entry.host, entry.kind))
                .chain(remembered),
        );
        self.input_transition(|this| {
            if let Some(core) = &this.terminal.core {
                core.invalidate_input();
            }
            this.terminal.input.detach();
            this.terminal.input.fence();
            this.terminal.needs_focus = false;
            this.actions.palette = false;
            this.actions.consumed.clear();
            this.handoff.sync(None);
            this.handoff.cancel();
            this.handoff.select_mode = false;
            let mut chooser = Chooser::new(hosts);
            chooser.focus = Some(cx.focus_handle());
            this.shell.herdr2.new_space = Some(chooser);
        });
        cx.notify();
    }

    pub(crate) fn new_space_open(&self) -> bool {
        self.shell.herdr2.new_space.is_some()
    }

    fn close_new_space(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.shell.herdr2.new_space = None;
        #[cfg(all(target_os = "ios", not(test)))]
        self.shell.herdr2.host_probe.take();
        self.return_keys_to_terminal(window, cx);
    }

    fn create_space(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((host, kind)) = self
            .shell
            .herdr2
            .new_space
            .as_ref()
            .and_then(Chooser::selection)
            .map(|(host, kind)| (host.to_owned(), kind))
        else {
            return;
        };
        if let Err(error) = self.check_space_connection() {
            let reason = error.to_string();
            self.terminal.live.status.clone_from(&reason);
            if let Some(chooser) = self.shell.herdr2.new_space.as_mut() {
                chooser.error = Some(reason);
            }
            cx.notify();
            return;
        }
        self.close_new_space(window, cx);
        match kind {
            HerdrKind::Herdr1 => self.new_herdr1_space(&host, cx),
            HerdrKind::Herdr2 => {
                // #144 decision 10: a space in the `default` core's
                // herdr-app document, never another session. A connection
                // to that core takes the command before its worker starts.
                let connected = self.terminal.core_mode
                    && self.terminal.reconnect.target == host
                    && self
                        .terminal
                        .core
                        .as_ref()
                        .is_some_and(|core| core.name() == super::HERDR2_SESSION);
                let started = connected
                    || self
                        .connect_named_core(host, super::HERDR2_SESSION.into(), None, cx)
                        .is_ok();
                if started {
                    let _ = self.try_core_command(crate::core::Command::NewDocSpace);
                }
            }
        }
        cx.notify();
    }

    fn select_new_space_host(&mut self, index: usize, _cx: &mut Context<Self>) {
        let Some(chooser) = self.shell.herdr2.new_space.as_mut() else {
            return;
        };
        chooser.select_host(index);
        #[cfg(all(target_os = "ios", not(test)))]
        {
            let Some(host) = chooser.hosts.get(index) else {
                return;
            };
            if cfg!(target_abi = "sim") && std::env::var_os("YEBISU_CORE_QA").is_some() {
                return;
            }
            let destination = host.name.destination().to_owned();
            chooser.checking = true;
            let worker = _cx.background_executor().spawn(async move {
                use sheprd_client::ssh::{HostProbe, probe_host};
                let primary = probe_host(&destination, super::HERDR2_SESSION).ok();
                let mut kinds = Vec::new();
                if matches!(primary, Some(HostProbe::Running | HostProbe::Stopped)) {
                    kinds.push(HerdrKind::Herdr1);
                }
                if matches!(
                    primary,
                    Some(HostProbe::CoreAvailable | HostProbe::OutdatedCoreAvailable)
                ) || crate::core::discover_host(&destination, || false)
                    .is_ok_and(|host| host.installed)
                {
                    kinds.push(HerdrKind::Herdr2);
                }
                kinds
            });
            self.shell.herdr2.host_probe = Some(_cx.spawn(async move |weak, cx| {
                let kinds = worker.await;
                let _ = weak.update(cx, |this, cx| {
                    let Some(chooser) = this.shell.herdr2.new_space.as_mut() else {
                        return;
                    };
                    if chooser.selected_host != Some(index) {
                        return;
                    }
                    let Some(host) = chooser.hosts.get_mut(index) else {
                        return;
                    };
                    host.kinds.extend(kinds);
                    host.kinds = new_space_choices(&host.kinds);
                    chooser.checking = false;
                    chooser.select_host(index);
                    cx.notify();
                });
            }));
        }
    }

    fn new_herdr1_space(&mut self, host: &str, cx: &mut Context<Self>) {
        use sheprd_commands::Command::Workspace;
        self.select_host_kind(host, HerdrKind::Herdr1);
        if crate::actions::catalog(&self.terminal)
            .iter()
            .any(|action| action.info.command == Workspace && action.available)
        {
            if let Err(error) = self.run_action(Workspace) {
                self.terminal.live.status = error.to_string();
            }
            return;
        }
        self.start_host_connection(host.to_owned(), cx);
        let host = host.to_owned();
        let selection = self.board.selection;
        cx.spawn(async move |weak, cx| {
            for _ in 0..100 {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(100))
                    .await;
                let finished = weak
                    .update(cx, |this, cx| {
                        if this.terminal.reconnect.target != host
                            || this.board.selection != selection
                            || this.terminal.core_mode
                        {
                            return true;
                        }
                        let ready = crate::actions::catalog(&this.terminal)
                            .iter()
                            .any(|action| action.info.command == Workspace && action.available);
                        if ready {
                            if let Err(error) = this.run_action(Workspace) {
                                this.terminal.live.status = error.to_string();
                            }
                            cx.notify();
                        }
                        ready
                    })
                    .unwrap_or(true);
                if finished {
                    return;
                }
            }
            let _ = weak.update(cx, |this, cx| {
                if this.terminal.reconnect.target == host && this.board.selection == selection {
                    this.terminal.live.status = "Herdr 1 did not connect to create a space".into();
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(crate) fn prepare_new_space(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        #[cfg(all(target_os = "ios", target_abi = "sim", not(test)))]
        self.advance_wave1_fixture(window, cx);
        self.remember_connected_core();
        let Some(focus) = self
            .shell
            .herdr2
            .new_space
            .as_ref()
            .and_then(|chooser| chooser.focus.as_ref())
        else {
            return;
        };
        self.terminal.needs_focus = false;
        if !focus.is_focused(window) {
            focus.focus(window, cx);
            window.dismiss_virtual_keyboard();
            window.notify_text_input_changed(&self.terminal_focus);
        }
    }

    #[cfg(all(target_os = "ios", target_abi = "sim", not(test)))]
    fn advance_wave1_fixture(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if std::env::var_os("YEBISU_CORE_QA").is_none() || !self.terminal.live.connected {
            return;
        }
        let Ok(scenario) = std::env::var("YEBISU_WAVE1_HERDR2_QA") else {
            return;
        };
        if !matches!(scenario.as_str(), "sidebar" | "new-space") {
            return;
        }
        let stage = self.shell.herdr2.wave1_qa_step;
        if stage >= 4 || (stage == 3 && self.core_tabs().len() < 2) {
            return;
        }
        self.shell.herdr2.wave1_qa_step += 1;
        cx.defer_in(window, move |this, window, cx| {
            match stage {
                0 | 1 => {
                    let session = if stage == 0 { "setup" } else { "yebisu" };
                    let _ = this.connect_named_core("qa@pro-dev".into(), session.into(), None, cx);
                }
                2 => {
                    if let Err(error) = this.run_action(sheprd_commands::Command::Tab) {
                        tracing::warn!(%error, "Create Wave 1 fixture tab");
                    }
                }
                _ => {
                    if let Some(core) = &this.terminal.core {
                        let output = br#"["setup",["fixture",[{"address":"t1","state":"live","name":"zsh","status":{"title":"External QA agent","state":"working"}}]]]
["archived",null]
"#;
                        let root = crate::core::workspace_root();
                        if let Err(error) = core.publish_discovery(root.as_deref(), "qa@pro-dev", output) {
                            tracing::warn!(%error, "Publish Wave 1 fixture sessions");
                        }
                    }
                    if scenario != "new-space" {
                        window.dismiss_virtual_keyboard();
                        cx.notify();
                        return;
                    }
                    this.setup.hosts = vec!["qa@pro-dev".into(), "qa@mini".into()];
                    this.open_new_space(cx);
                    if let Some(chooser) = this.shell.herdr2.new_space.as_mut() {
                        chooser.hosts[0].kinds = vec![HerdrKind::Herdr1, HerdrKind::Herdr2];
                        chooser.hosts[1].kinds = vec![HerdrKind::Herdr1];
                        chooser.select_host(0);
                    }
                }
            }
            cx.notify();
        });
    }

    pub(crate) fn render_new_space(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let chooser = self.shell.herdr2.new_space.as_ref()?;
        let focus = chooser.focus.as_ref()?;
        let picked = chooser.choice;
        let selected_host = chooser.selected_host;
        let checking = chooser.checking;
        let no_kind = selected_host.is_some() && !checking && picked.is_none();
        let hosts = chooser.hosts.iter().enumerate().map(|(index, host)| {
            choice_button(
                ("new-space-host", index),
                host.name.to_string(),
                selected_host == Some(index),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_new_space_host(index, cx);
                cx.notify();
            }))
        });
        let kinds = picked
            .into_iter()
            .flat_map(|(index, selected)| {
                chooser.hosts[index]
                    .kinds
                    .iter()
                    .copied()
                    .map(move |kind| (kind, selected))
            })
            .map(|(kind, selected)| {
                choice_button(
                    ("new-space-kind", kind as usize),
                    kind_label(kind).to_owned(),
                    kind == selected,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(chooser) = this.shell.herdr2.new_space.as_mut() {
                        chooser.select_kind(kind);
                    }
                    cx.notify();
                }))
            });
        let mut host_list = div()
            .id("new-space-hosts")
            .flex()
            .flex_col()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(self.shell.indicators.new_space.handle())
            .children(hosts);
        if chooser.hosts.is_empty() {
            host_list = host_list.child("Add a host before creating a space");
        }
        let host_list = host_list.child(self.shell.indicators.new_space.thumb());
        let enabled = chooser.selection().is_some();
        let panel = div()
            .id("new-space-panel")
            .track_focus(focus)
            .flex()
            .flex_col()
            .w_full()
            .max_w(px(style::metric::SETTINGS_LIST))
            .max_h_full()
            .p(px(style::metric::SIDE))
            .gap(px(style::metric::ROW_GAP))
            .rounded(px(style::radius::SHEET))
            .bg(rgb(style::color::SURFACE))
            .text_color(rgb(style::color::TEXT))
            .shadow(vec![style::shadow::SHEET.box_shadow()])
            .child(
                div()
                    .font_weight(style::weight::SEMIBOLD)
                    .text_size(px(style::text::SHEET_TITLE))
                    .child("New space"),
            )
            .child(host_list)
            .when(checking, |panel| panel.child("Checking Herdr versions…"))
            .when(no_kind, |panel| {
                panel.child("No Herdr connection available for this host")
            })
            .when(!checking, |panel| panel.child(div().flex().children(kinds)))
            .children(chooser.error.as_ref().map(|reason| {
                div()
                    .text_color(rgb(style::color::RED))
                    .child(reason.clone())
            }))
            .child(
                div()
                    .flex()
                    .justify_end()
                    .child(
                        choice_button(("new-space-cancel", 0), "Cancel".into(), false).on_click(
                            cx.listener(|this, _, window, cx| this.close_new_space(window, cx)),
                        ),
                    )
                    .child(
                        choice_button(("new-space-create", 0), "Create".into(), enabled)
                            .when(!enabled, |button| {
                                button.text_color(rgb(style::color::DISABLED))
                            })
                            .when(enabled, |button| {
                                button.on_click(
                                    cx.listener(|this, _, window, cx| {
                                        this.create_space(window, cx)
                                    }),
                                )
                            }),
                    ),
            )
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    this.close_new_space(window, cx);
                }
                cx.stop_propagation();
                window.prevent_default();
            }));
        Some(
            div()
                .id("new-space-overlay")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .p(px(style::metric::SIDE))
                .bg(gpui::Rgba::from(style::tint::SHEET_SCRIM))
                .occlude()
                .child(panel)
                .into_any_element(),
        )
    }
}

fn choice_button(
    id: (&'static str, usize),
    label: String,
    selected: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .debug_selector(move || format!("{}-{}", id.0, id.1))
        .min_h(px(style::metric::TOUCH))
        .flex()
        .items_center()
        .px(px(style::metric::SIDE))
        .rounded(px(style::radius::CONTROL))
        .when(selected, |row| {
            row.bg(rgb(style::color::RAISED))
                .text_color(rgb(style::color::ACCENT))
        })
        .child(label)
}

#[cfg(test)]
#[path = "new_space_tests.rs"]
mod tests;
