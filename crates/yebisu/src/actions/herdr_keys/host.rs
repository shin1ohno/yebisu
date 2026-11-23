//! Read the selected connection's Herdr config using the existing pinned SSH transport.
use super::{DEFAULTS, Fallback, MAX_BYTES, Prefix, Profile, Route, matches, ui_key};
use crate::{
    app::Smoke, live::Live, settings_pages::Page, setup::Setup, terminal_view::TerminalView,
};
use gpui::{Context, KeyDownEvent};
use sheprd_commands::Command;
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

// Only the remote shell expands its environment. No data from the config,
// target, or UI is interpolated into the SSH command. A regular-file check
// avoids opening an existing FIFO or device. The extra byte detects overflow.
#[cfg(any(target_os = "ios", test))]
const READ_CONFIG: &str = r#"exec /bin/sh -c '
case "$XDG_CONFIG_HOME" in
    /*) config_home="$XDG_CONFIG_HOME" ;;
    *)
        case "$HOME" in /*) ;; *) exit 46 ;; esac
        config_home="$HOME/.config"
        ;;
esac
path="$config_home/herdr/config.toml"
if [ ! -e "$path" ]; then exit 44; fi
if [ ! -f "$path" ]; then exit 45; fi
if [ ! -r "$path" ]; then exit 46; fi
exec /usr/bin/head -c 65537 "$path"
'"#;

const TIMEOUT: Duration = Duration::from_secs(5);

struct Pending {
    deadline: Instant,
    stop: Arc<AtomicBool>,
    receive: mpsc::Receiver<Result<Profile, Fallback>>,
}

impl Pending {
    fn cancel(&self) {
        self.stop.store(true, Ordering::Release);
    }
}

impl Drop for Pending {
    fn drop(&mut self) {
        self.cancel();
    }
}

struct Connection {
    inbox: Weak<Mutex<Live>>,
    target: String,
}

impl Connection {
    fn matches(&self, terminal: &TerminalView) -> bool {
        connected(terminal)
            && self.target == terminal.reconnect.target
            && self.inbox.ptr_eq(&Arc::downgrade(&terminal.link.inbox))
    }
}

fn connected(terminal: &TerminalView) -> bool {
    terminal.live.connected
        && !terminal.reconnect.suspended
        && !terminal.reconnect.target.is_empty()
}

#[derive(Default)]
pub(crate) struct HostKeys {
    connection: Option<Connection>,
    pending: Option<Pending>,
    result: Option<Result<Profile, Fallback>>,
    prefix: Prefix,
    press_owner: Option<(u64, u64, u64)>,
}

impl HostKeys {
    fn current(&self, terminal: &TerminalView) -> bool {
        self.connection
            .as_ref()
            .is_some_and(|c| c.matches(terminal))
    }

    fn profile(&self, terminal: &TerminalView) -> &Profile {
        if self.current(terminal)
            && let Some(Ok(profile)) = &self.result
        {
            profile
        } else {
            &DEFAULTS
        }
    }

    pub(crate) fn prefix_row(&self, terminal: &TerminalView, setup: &Setup) -> (String, String) {
        let profile = self.profile(terminal);
        let key = profile
            .prefixes
            .iter()
            .map(|(_, label)| label.as_str())
            .collect::<Vec<_>>()
            .join(" / ");
        let current = self.current(terminal);
        let reason = match (current, &self.result) {
            (false, _) if connected(terminal) => Some(Fallback::Loading),
            (false, _) => Some(Fallback::Disconnected),
            (true, None) => Some(Fallback::Loading),
            (true, Some(Err(reason))) => Some(*reason),
            (true, Some(Ok(profile))) => profile.fallback,
        };
        let host = setup.host_name(&terminal.reconnect.target);
        let detail = if let Some(reason) = reason {
            if connected(terminal) {
                format!(
                    "{}. Using Herdr's default. {}",
                    host.display(),
                    reason.reason()
                )
            } else {
                format!("Using Herdr's default. {}", reason.reason())
            }
        } else {
            format!(
                "From {}’s Herdr config ([keys].prefix). To change it, edit that file on the host.",
                host.display()
            )
        };
        (key, detail)
    }

    pub(crate) fn shortcut_notice(
        &self,
        terminal: &TerminalView,
        server: bool,
    ) -> Option<&'static str> {
        if !connected(terminal) {
            return Some("No connected host");
        }
        match self.result.as_ref().filter(|_| self.current(terminal)) {
            None => Some("Reading host keybindings · Using default keys"),
            Some(Err(_)) => Some("Host keybindings unavailable · Using default keys"),
            Some(Ok(profile)) if profile.fallback.is_some() => Some("Using default prefix ctrl+b"),
            Some(Ok(_)) if !server => Some("Using default chords"),
            Some(Ok(_)) => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn set_test_config(&mut self, terminal: &TerminalView, config: &str) {
        self.connection = Some(Connection {
            inbox: Arc::downgrade(&terminal.link.inbox),
            target: terminal.reconnect.target.clone(),
        });
        self.result = Some(Profile::from_file(Ok(config.as_bytes())));
    }

    pub(crate) fn labels(
        &self,
        terminal: &TerminalView,
        server: bool,
        command: Command,
    ) -> Vec<String> {
        if !connected(terminal) {
            return Vec::new();
        }
        let prefixes = self.profile(terminal);
        let profile = if server { prefixes } else { &DEFAULTS };
        let mut labels = Vec::new();
        for binding in profile
            .bindings
            .iter()
            .filter(|binding| binding.command == command)
        {
            let key = ui_key(&binding.key);
            if profile.command(&key, binding.prefixed) != Some(command) {
                continue;
            }
            if binding.prefixed {
                for (prefix, label) in &prefixes.prefixes {
                    let prefix = ui_key(prefix);
                    if !matches(&prefix, &key) {
                        labels.push(format!("{label} {}", binding.label));
                    }
                }
            } else if !prefixes.prefix(&key) {
                labels.push(binding.label.clone());
            }
        }
        labels
    }

    pub(crate) fn clear_press(&mut self) {
        self.prefix.clear();
        self.press_owner = None;
    }

    /// A prefix waits for its second key.
    pub(crate) fn armed(&self) -> bool {
        self.prefix.armed.is_some()
    }

    pub(crate) fn route(
        &mut self,
        terminal: &TerminalView,
        selection: u64,
        enabled: bool,
        server: bool,
        event: &KeyDownEvent,
    ) -> Route {
        if !enabled || !connected(terminal) {
            self.clear_press();
            return Route::Native;
        }
        let current = self.current(terminal);
        let owner = (
            selection,
            terminal.link.generation,
            terminal.input.retirement_revision(),
        );
        if self.press_owner != Some(owner) {
            self.prefix.clear();
        }
        self.press_owner = Some(owner);
        let prefixes = if current && let Some(Ok(profile)) = &self.result {
            profile
        } else {
            &DEFAULTS
        };
        let profile = if server { prefixes } else { &DEFAULTS };
        self.prefix
            .route(profile, prefixes, &event.keystroke, event.is_held)
    }

    fn poll(&mut self, terminal: &TerminalView, needed: bool, cx: &mut Context<Smoke>) {
        if !self.current(terminal) && self.connection.take().is_some() {
            if let Some(pending) = &self.pending {
                pending.cancel();
            }
            self.result = None;
            self.clear_press();
            cx.notify();
        }
        if let Some(pending) = &self.pending {
            let result = match pending.receive.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => Some(Err(Fallback::ReadFailed)),
            };
            let timed_out = Instant::now() >= pending.deadline;
            if timed_out {
                pending.cancel();
            }
            // A cancelled read still occupies the only slot until its thread
            // returns, including when SSH is waiting on the local Keychain.
            if result.is_some() {
                self.pending = None;
            }
            if self.connection.is_some() && self.result.is_none() {
                self.result = if timed_out {
                    Some(Err(Fallback::Timeout))
                } else {
                    result
                };
                if self.result.is_some() {
                    self.clear_press();
                    cx.notify();
                }
            }
        }
        if needed && connected(terminal) && self.connection.is_none() && self.pending.is_none() {
            let target = terminal.reconnect.target.clone();
            self.pending = Some(start(target.clone(), cx));
            self.connection = Some(Connection {
                inbox: Arc::downgrade(&terminal.link.inbox),
                target,
            });
            cx.notify();
        }
    }
}

impl Smoke {
    pub(crate) fn poll_host_keys(&mut self, cx: &mut Context<Self>) {
        let needed = self.actions.herdr_prefix
            || self.settings_modal && matches!(self.settings_page, Page::Input | Page::Keyboard);
        self.actions.host_keys.poll(&self.terminal, needed, cx);
    }
}

fn start(target: String, cx: &mut Context<Smoke>) -> Pending {
    let deadline = Instant::now() + TIMEOUT;
    let stop = Arc::new(AtomicBool::new(false));
    let cancelled = stop.clone();
    let (send, receive) = mpsc::sync_channel(1);
    cx.background_executor()
        .spawn(async move {
            let result = read(&target, &cancelled);
            let _ = send.try_send(result);
        })
        .detach();
    Pending {
        deadline,
        stop,
        receive,
    }
}

#[cfg(target_os = "ios")]
fn read(target: &str, cancelled: &AtomicBool) -> Result<Profile, Fallback> {
    use sheprd_client::{Error, ssh};
    #[cfg(target_abi = "sim")]
    if std::env::var_os("YEBISU_TERMINAL_QA").is_some()
        || std::env::var_os("YEBISU_CORE_QA").is_some()
    {
        return match std::env::var("YEBISU_HERDR_KEYS_QA") {
            Ok(config) => Profile::from_file(Ok(config.as_bytes())),
            Err(_) => Err(Fallback::Unavailable),
        };
    }
    let (status, output) =
        sheprd_client::run_ssh_command(target, READ_CONFIG, TIMEOUT, MAX_BYTES as u64 + 1, || {
            cancelled.load(Ordering::Acquire)
        })
        .map_err(|error| match error {
            Error::Ssh(ssh::Error::Timeout) => Fallback::Timeout,
            Error::Ssh(ssh::Error::OutputLimit) => Fallback::TooLarge,
            _ => Fallback::ReadFailed,
        })?;
    Profile::from_file(match status.code() {
        Some(0) => Ok(&output),
        Some(44) => Err(Fallback::MissingFile),
        Some(45) => Err(Fallback::NotRegular),
        Some(46) => Err(Fallback::PermissionDenied),
        _ => Err(Fallback::ReadFailed),
    })
}

#[cfg(not(target_os = "ios"))]
fn read(_: &str, _: &AtomicBool) -> Result<Profile, Fallback> {
    Err(Fallback::Unavailable)
}

#[cfg(test)]
#[path = "host_tests.rs"]
mod tests;
