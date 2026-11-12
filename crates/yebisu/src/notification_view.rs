//! App adapter: consumes each connected effect once, never reconnects from a tap.
use crate::{
    app::Smoke,
    notifications::{self, Context as TargetContext, Event, Failure, Policy, Route},
};
use chrono::Timelike;
use gpui::{AnyElement, Context, SystemNotification, div, prelude::*, rgb};
use std::{
    collections::VecDeque,
    sync::mpsc,
    time::{Duration, Instant},
};

struct Posted {
    tag: String,
    target: TargetContext,
    received: Instant,
}
pub(crate) struct Toast {
    tag: String,
    title: &'static str,
    expires: Instant,
}
pub(crate) struct State {
    #[cfg(all(target_os = "ios", target_abi = "sim"))]
    qa_step: u8,
    #[cfg(all(target_os = "ios", target_abi = "sim"))]
    qa_last_tag: Option<String>,
    pub authorization: notifications::Authorization,
    permission_busy: bool,
    permission_sender: mpsc::SyncSender<Result<notifications::Authorization, Failure>>,
    permissions: mpsc::Receiver<Result<notifications::Authorization, Failure>>,
    pub policy: Policy,
    pub active: bool,
    pub status: String,
    pub toast: Option<Toast>,
    posted: VecDeque<Posted>,
    waiting: VecDeque<(Event, TargetContext)>,
    sound: Option<notifications::sound::Service>,
    sound_context: Option<TargetContext>,
    session: String,
    sequence: u64,
    epoch: u64,
    results: mpsc::Receiver<(u64, Result<(), Failure>)>,
    sender: mpsc::SyncSender<(u64, Result<(), Failure>)>,
}
impl Default for State {
    fn default() -> Self {
        let (sender, results) = mpsc::sync_channel(8);
        let (permission_sender, permissions) = mpsc::sync_channel(1);
        let session = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
            .to_string();
        Self {
            #[cfg(all(target_os = "ios", target_abi = "sim"))]
            qa_step: 0,
            #[cfg(all(target_os = "ios", target_abi = "sim"))]
            qa_last_tag: None,
            authorization: notifications::Authorization::Unknown,
            permission_busy: false,
            permission_sender,
            permissions,
            policy: Policy::default(),
            active: true,
            status: String::new(),
            toast: None,
            posted: VecDeque::new(),
            waiting: VecDeque::new(),
            sound: None,
            sound_context: None,
            session,
            sequence: 0,
            epoch: 0,
            results,
            sender,
        }
    }
}
impl State {
    pub fn pause_foreground(&mut self) {
        self.toast = None;
        self.waiting.clear();
        if let Some(sound) = &self.sound {
            sound.cancel();
        }
        self.sound_context = None;
    }
    pub fn permission(&mut self, request: bool) {
        if self.permission_busy {
            self.fail(Failure::Busy);
            return;
        }
        self.permission_busy = true;
        let sender = self.permission_sender.clone();
        #[cfg(target_os = "ios")]
        if request {
            herdr_uikit::request_notification_authorization(move |result| {
                if let Err(herdr_uikit::NotificationError::Native) = result {
                    let _ = sender.try_send(Err(Failure::Native));
                    return;
                }
                let sender = sender.clone();
                herdr_uikit::notification_authorization(move |state| {
                    let _ = sender.try_send(Ok(state.into()));
                });
            });
        } else {
            herdr_uikit::notification_authorization(move |state| {
                let _ = sender.try_send(Ok(state.into()));
            });
        }
        #[cfg(not(target_os = "ios"))]
        {
            let _ = request;
            let _ = sender.try_send(Err(Failure::Native));
        }
    }

    pub fn set_policy(&mut self, policy: Policy) {
        if self.policy != policy {
            self.invalidate();
            self.policy = policy;
        }
    }
    pub fn invalidate(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
        self.toast = None;
        self.sound_context = None;
        self.waiting.clear();
        if let Some(sound) = &self.sound {
            sound.cancel();
        }
        #[cfg(target_os = "ios")]
        herdr_uikit::remove_notifications(
            &self
                .posted
                .iter()
                .map(|p| p.tag.clone())
                .collect::<Vec<_>>(),
        );
        self.posted.clear();
    }
    fn fail(&mut self, error: Failure) {
        self.status = format!("Notifications: {error:?}");
    }
    fn submit(
        &mut self,
        event: Event,
        target: TargetContext,
        active: std::sync::Arc<std::sync::atomic::AtomicBool>,
        inbox: std::sync::Arc<std::sync::Mutex<crate::live::Live>>,
    ) {
        let now = Instant::now();
        if now.saturating_duration_since(event.received) >= notifications::MAX_AGE {
            self.fail(Failure::Stale);
            return;
        }
        let local = chrono::Local::now();
        let minute = (local.hour() * 60 + local.minute()) as u16;
        if let Some(error) = self.policy.suppression(minute) {
            self.fail(error);
            return;
        }
        let validation = target.clone();
        let policy = self.policy.clone();
        let received = event.received;
        let current: std::sync::Arc<dyn Fn() -> bool + Send + Sync> =
            std::sync::Arc::new(move || {
                let local = chrono::Local::now();
                active.load(std::sync::atomic::Ordering::Acquire)
                    && policy
                        .suppression((local.hour() * 60 + local.minute()) as u16)
                        .is_none()
                    && received.elapsed() < notifications::MAX_AGE
                    && inbox
                        .try_lock()
                        .map(|live| {
                            validation
                                .validate(&live, &validation.host, validation.generation)
                                .is_ok()
                        })
                        .unwrap_or(false)
            });
        if self.active
            && self.policy.sound
            && let Some(sound) = event.sound
        {
            if self.sound.is_none() {
                match notifications::sound::Service::start() {
                    Ok(service) => self.sound = Some(service),
                    Err(error) => self.fail(error),
                }
            }
            if let Some(service) = &self.sound
                && let Err(error) = service.submit(sound, self.policy.volume, current.clone())
            {
                self.fail(error);
            }
            self.sound_context = Some(target.clone());
        }
        match self.policy.route {
            Route::Off => {
                self.fail(Failure::Disabled);
                return;
            }
            Route::Toast if !self.active => {
                self.fail(Failure::Inactive);
                return;
            }
            _ => {}
        }
        let Some(sequence) = self.sequence.checked_add(1) else {
            self.fail(Failure::Overflow);
            return;
        };
        self.sequence = sequence;
        let tag = format!("yebisu-{}-{sequence}", self.session);
        if self.posted.len() >= 64 {
            if let Some(old) = self.posted.pop_front() {
                #[cfg(target_os = "ios")]
                herdr_uikit::remove_notifications(&[old.tag]);
                #[cfg(not(target_os = "ios"))]
                let _ = old;
            }
        }
        self.posted.push_back(Posted {
            tag: tag.clone(),
            target,
            received: event.received,
        });
        if self.policy.route == Route::Toast {
            self.toast = Some(Toast {
                tag,
                title: event.title(),
                expires: now + Duration::from_secs(5),
            });
            self.status = "Notifications: foreground toast".into();
        } else {
            let notification = SystemNotification {
                tag: tag.into(),
                title: event.title().into(),
                body: "Open current status in yebisu".into(),
                actions: vec![],
            };
            let sender = self.sender.clone();
            let epoch = self.epoch;
            #[cfg(target_os = "ios")]
            herdr_uikit::post_notification_when(
                notification,
                move || current(),
                move |result| {
                    let result = result.map_err(|e| match e {
                        herdr_uikit::NotificationError::Permission(state) => {
                            Failure::Permission(state.into())
                        }
                        herdr_uikit::NotificationError::Busy => Failure::Busy,
                        herdr_uikit::NotificationError::Cancelled => Failure::Stale,
                        _ => Failure::Native,
                    });
                    let _ = sender.try_send((epoch, result));
                },
            );
            #[cfg(not(target_os = "ios"))]
            {
                let _ = notification;
                let _ = current;
                let _ = sender.try_send((epoch, Err(Failure::Native)));
            }
            self.status = "Notifications: OS submission pending".into();
        }
    }
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Ok(result) = self.permissions.try_recv() {
            self.permission_busy = false;
            changed = true;
            match result {
                Ok(state) => {
                    self.authorization = state;
                    self.status = format!("Notification permission: {state:?}");
                }
                Err(error) => self.fail(error),
            }
        }
        while let Ok((epoch, result)) = self.results.try_recv() {
            if epoch != self.epoch {
                continue;
            }
            changed = true;
            match result {
                Ok(()) => {
                    self.status = "Notifications: OS accepted request; display not confirmed".into()
                }
                Err(error) => self.fail(error),
            }
        }
        if let Some(sound) = &self.sound
            && let Some(Err(error)) = sound.result()
        {
            self.fail(error);
            changed = true;
        }
        if self
            .toast
            .as_ref()
            .is_some_and(|t| t.expires <= Instant::now())
        {
            self.toast = None;
            changed = true;
        }
        changed
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.invalidate();
    }
}

impl Smoke {
    pub(crate) fn collect_notifications(&mut self, cx: &mut Context<Self>) {
        let local = chrono::Local::now();
        if let Some(error) = self
            .notifications
            .policy
            .suppression((local.hour() * 60 + local.minute()) as u16)
            && (self.notifications.sound_context.is_some()
                || !self.notifications.posted.is_empty()
                || !self.notifications.waiting.is_empty())
        {
            self.notifications.invalidate();
            self.notifications.fail(error);
            cx.notify();
        }
        let invalidated = self
            .notifications
            .sound_context
            .iter()
            .chain(self.notifications.posted.iter().map(|p| &p.target))
            .any(|target| {
                let terminal = if target.host == self.terminal.reconnect.target {
                    Some(&self.terminal)
                } else {
                    self.board.hidden.get(&target.host)
                };
                terminal.is_none_or(|terminal| {
                    terminal.reconnect.suspended
                        || terminal
                            .link
                            .inbox
                            .try_lock()
                            .map(|live| {
                                target
                                    .validate(
                                        &live,
                                        &terminal.reconnect.target,
                                        terminal.link.generation,
                                    )
                                    .is_err()
                            })
                            .unwrap_or(true)
                })
            });
        if invalidated {
            self.notifications.invalidate();
            self.notifications.fail(Failure::Stale);
            cx.notify();
        }
        let mut events = Vec::new();
        for terminal in std::iter::once(&mut self.terminal).chain(self.board.hidden.values_mut()) {
            if let Some(error) = terminal.live.notifications.failure.take() {
                self.notifications.fail(error);
            }
            let host = terminal.reconnect.target.clone();
            let generation = terminal.link.generation;
            for event in terminal.live.notifications.pending.drain(..) {
                let target = event.context(host.clone(), generation);
                events.push((event, target));
            }
        }
        let mut changed = !events.is_empty();
        for pair in events {
            if self.notifications.waiting.len() >= notifications::LIMIT {
                self.notifications.waiting.pop_front();
                self.notifications.fail(Failure::Overflow);
            }
            self.notifications.waiting.push_back(pair);
        }
        while self
            .notifications
            .waiting
            .front()
            .is_some_and(|(event, _)| event.received.elapsed() >= Duration::from_secs(1))
        {
            let Some((event, target)) = self.notifications.waiting.pop_front() else {
                break;
            };
            changed = true;
            let terminal = if target.host == self.terminal.reconnect.target {
                Some(&self.terminal)
            } else {
                self.board.hidden.get(&target.host)
            };
            let valid = terminal.ok_or(Failure::Disconnected).and_then(|t| {
                if t.reconnect.suspended {
                    return Err(Failure::Disconnected);
                }
                let live = t.link.inbox.try_lock().map_err(|_| Failure::Busy)?;
                target.validate(&live, &t.reconnect.target, t.link.generation)?;
                Ok((t.link.notification_active.clone(), t.link.inbox.clone()))
            });
            match valid {
                Ok((active, inbox)) => self.notifications.submit(event, target, active, inbox),
                Err(error) => self.notifications.fail(error),
            }
        }
        changed |= self.notifications.poll();
        if changed {
            cx.notify();
        }
    }
    pub(crate) fn open_notification(&mut self, tag: &str, action: bool, cx: &mut Context<Self>) {
        let local = chrono::Local::now();
        self.open_notification_at(
            tag,
            action,
            Instant::now(),
            (local.hour() * 60 + local.minute()) as u16,
            cx,
        );
    }
    fn open_notification_at(
        &mut self,
        tag: &str,
        action: bool,
        now: Instant,
        minute: u16,
        cx: &mut Context<Self>,
    ) {
        #[cfg(all(target_os = "ios", target_abi = "sim"))]
        if std::env::var_os("YEBISU_P14_QA").is_some() {
            self.notifications.qa_last_tag = Some(tag.to_owned());
        }
        let posted = self
            .notifications
            .posted
            .iter()
            .position(|p| p.tag == tag)
            .and_then(|i| self.notifications.posted.remove(i));
        self.notifications.toast = None;
        if self.settings_modal {
            self.notifications.fail(Failure::Busy);
            cx.notify();
            return;
        }
        let result = (|| {
            if action {
                return Err(Failure::InvalidTarget);
            }
            if let Some(error) = self.notifications.policy.suppression(minute) {
                return Err(error);
            }
            let posted = posted.ok_or(Failure::Stale)?;
            if now.saturating_duration_since(posted.received) > Duration::from_secs(300) {
                return Err(Failure::Stale);
            }
            let target = posted.target;
            let terminal = if target.host == self.terminal.reconnect.target {
                Some(&self.terminal)
            } else {
                self.board.hidden.get(&target.host)
            }
            .ok_or(Failure::Disconnected)?;
            if terminal.reconnect.suspended
                || !terminal
                    .link
                    .notification_active
                    .load(std::sync::atomic::Ordering::Acquire)
            {
                return Err(Failure::Disconnected);
            }
            let mut live = terminal.link.inbox.try_lock().map_err(|_| Failure::Busy)?;
            target.validate(&live, &terminal.reconnect.target, terminal.link.generation)?;
            let pane = target.pane.as_ref().ok_or(Failure::InvalidTarget)?;
            if terminal.navigating.is_some() {
                return Err(Failure::Busy);
            }
            // Validation and command enqueue share the current reader lock; no select_host redial.
            let handle = terminal.link.handle.as_ref().ok_or(Failure::Disconnected)?;
            let request = handle
                .focus_pane(&target.boot, pane)
                .map_err(|_| Failure::Native)?;
            let revision = live
                .snapshot
                .as_ref()
                .map(|s| s.revision)
                .ok_or(Failure::Incoherent)?;
            let navigation = crate::board::Navigation {
                boot: target.boot.clone(),
                revision,
                target: crate::board::Target::Agent(pane.clone()),
                request,
            };
            live.begin_navigation(Some(navigation.clone()));
            drop(live);
            self.select_host(&target.host);
            self.terminal.input.detach();
            self.terminal.navigating = Some(navigation);
            Ok(())
        })();
        if let Err(error) = result {
            self.notifications.fail(error);
            self.terminal.open = false;
        }
        cx.notify();
    }
    pub(crate) fn render_notification(&self, cx: &Context<Self>) -> AnyElement {
        let mut panel = div()
            .absolute()
            .bottom_0()
            .right_0()
            .max_w(gpui::px(384.))
            .p_2()
            .bg(rgb(0x3b4252))
            .text_color(rgb(0xeceff4));
        if let Some(toast) = &self.notifications.toast {
            let tag = toast.tag.clone();
            panel = panel.child(div().id("notification-toast").child(toast.title).on_click(
                cx.listener(move |this, _, _, cx| this.open_notification(&tag, false, cx)),
            ));
        }
        if !self.notifications.status.is_empty() {
            panel = panel.child(self.notifications.status.clone());
        }
        #[cfg(all(target_os = "ios", target_abi = "sim"))]
        {
            panel = panel
                .children(self.p14_controls(cx))
                .children(self.p14_paint(cx));
        }
        panel.into_any_element()
    }
}

#[cfg(test)]
mod tests;

#[cfg(all(test, target_os = "ios"))]
mod response_tests;

#[cfg(all(target_os = "ios", target_abi = "sim"))]
mod qa;
