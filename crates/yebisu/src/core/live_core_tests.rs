//! Against a real herdr-core (Herdr 2) in a private home, state and socket,
//! reached the way the iPad reaches it: `hc connect` speaking the protocol
//! over its standard input and output. Opt in with
//! `YEBISU_HERDR_CORE_TEST_BINARY=/path/to/herdr-core cargo test -p yebisu
//! live_core -- --ignored`. Each test starts its own core and ends only that
//! core (`server stop` on its private socket) and the children it started.
use super::*;
use crate::{app::Smoke, live::Live};
use gpui::{AppContext, Entity, KeyDownEvent, KeyUpEvent, Keystroke, Modifiers, TestAppContext};
use std::{
    ffi::OsString,
    os::unix::net::UnixStream,
    path::PathBuf,
    process::{Child, Stdio},
    time::{Duration, Instant},
};

const BINARY: &str = "YEBISU_HERDR_CORE_TEST_BINARY";

struct Sandbox {
    dir: PathBuf,
    binary: OsString,
    child: Child,
    bridges: Vec<Child>,
    vars: Vec<(&'static str, OsString)>,
}

impl Sandbox {
    fn start() -> Self {
        let binary = std::env::var_os(BINARY).unwrap_or_else(|| panic!("{BINARY} is not set"));
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        // Unix socket paths are short; keep the directory name short too.
        let dir = std::env::temp_dir().join(format!(
            "hc{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("sandbox dir: {e}"));
        let at = |name: &str| dir.join(name).into_os_string();
        for name in ["home", "config", "state", "run"] {
            std::fs::create_dir_all(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        }
        let vars = vec![
            ("HC_SESSION", OsString::from("live")),
            ("HC_SOCKET", at("c.sock")),
            ("HOME", at("home")),
            ("PATH", OsString::from("/usr/bin:/bin")),
            ("SHELL", OsString::from("/bin/sh")),
            ("TERM", OsString::from("xterm-256color")),
            ("XDG_CONFIG_HOME", at("config")),
            ("XDG_RUNTIME_DIR", at("run")),
            ("XDG_STATE_HOME", at("state")),
        ];
        let child = Self::command(&binary, &vars)
            .arg("serve")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|e| panic!("herdr-core serve: {e}"));
        let sandbox = Self {
            dir,
            binary,
            child,
            bridges: Vec::new(),
            vars,
        };
        sandbox.wait_ready();
        sandbox
    }

    fn command(binary: &OsString, vars: &[(&'static str, OsString)]) -> std::process::Command {
        let mut command = std::process::Command::new(binary);
        command
            .env_clear()
            .envs(vars.iter().map(|(name, value)| (name, value)));
        command
    }

    fn socket(&self) -> PathBuf {
        self.dir.join("c.sock")
    }

    fn wait_ready(&self) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while UnixStream::connect(self.socket()).is_err() {
            assert!(Instant::now() < deadline, "sandboxed core did not listen");
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// `hc connect --session live` on a socket pair, as SSH carries it for
    /// the iPad: the protocol over the bridge's standard input and output.
    fn bridge(&mut self) -> sheprd_core_client::CoreBackend {
        let (ours, theirs) = UnixStream::pair().unwrap_or_else(|e| panic!("pair: {e}"));
        let input = theirs.try_clone().unwrap_or_else(|e| panic!("clone: {e}"));
        let child = Self::command(&self.binary, &self.vars)
            .args(["connect", "--session", "live"])
            .stdin(Stdio::from(std::os::fd::OwnedFd::from(input)))
            .stdout(Stdio::from(std::os::fd::OwnedFd::from(theirs)))
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|e| panic!("hc connect: {e}"));
        self.bridges.push(child);
        ours.set_read_timeout(Some(Duration::from_millis(10)))
            .unwrap_or_else(|e| panic!("read timeout: {e}"));
        ours.set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap_or_else(|e| panic!("write timeout: {e}"));
        let core = sheprd_core_client::Core::over(ours).unwrap_or_else(|e| panic!("hello: {e}"));
        sheprd_core_client::CoreBackend::new(core, 10000)
    }

    /// The network drops: the bridges this test started end; the core and
    /// its programs keep running.
    fn cut(&mut self) {
        for bridge in &mut self.bridges {
            let _ = bridge.kill();
            let _ = bridge.wait();
        }
    }

    /// A second, independent client: what `hc ls` would show.
    fn list(&self) -> Vec<sheprd_core_client::wire::Terminal> {
        let endpoint = sheprd_core_client::Endpoint::resolve(|name| {
            self.vars
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| value.clone())
        })
        .unwrap_or_else(|e| panic!("endpoint: {e}"));
        assert_eq!(endpoint.socket, self.socket());
        sheprd_core_client::Core::connect(&endpoint)
            .and_then(|core| core.list(true))
            .unwrap_or_else(|e| panic!("list: {e}"))
            .terminals
    }

    /// Ends this sandbox's own core through its private socket, then reaps
    /// the exact children this test started.
    fn stop(&mut self) {
        let _ = Self::command(&self.binary, &self.vars)
            .args(["server", "stop"])
            .output();
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.child.try_wait().ok().flatten().is_none() {
            if Instant::now() >= deadline {
                let _ = self.child.kill();
                let _ = self.child.wait();
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        self.stop();
        for bridge in &mut self.bridges {
            let deadline = Instant::now() + Duration::from_secs(5);
            while bridge.try_wait().ok().flatten().is_none() {
                if Instant::now() >= deadline {
                    let _ = bridge.kill();
                    let _ = bridge.wait();
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn text(live: &Live) -> String {
    live.surface
        .as_ref()
        .map(|s| s.frame.cells.iter().map(|c| c.symbol.as_str()).collect())
        .unwrap_or_default()
}

/// What the view shows now, for the failure message and the report.
fn seen(view: &Entity<Smoke>, cx: &mut gpui::VisualTestContext) -> String {
    view.update(cx, |this, _| {
        let live = &this.terminal.live;
        let panes = live.surface.as_ref().map_or(0, |s| s.panes.len());
        format!(
            "connected={} panes={panes} core={} status={:?}",
            live.connected,
            this.terminal.core.is_some(),
            live.status
        )
    })
}

#[track_caller]
fn wait(
    view: &Entity<Smoke>,
    cx: &mut gpui::VisualTestContext,
    what: &str,
    predicate: impl Fn(&Live) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut last = String::new();
    loop {
        if view.update(cx, |this, cx| {
            this.poll_terminal(cx);
            predicate(&this.terminal.live)
        }) {
            return;
        }
        let now = seen(view, cx);
        if now != last {
            eprintln!("live-core: {now}");
            last = now;
        }
        assert!(Instant::now() < deadline, "{what}: {last}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn command(view: &Entity<Smoke>, cx: &mut gpui::VisualTestContext, command: Command) {
    view.update(cx, |this, _| {
        this.terminal
            .core
            .as_ref()
            .unwrap_or_else(|| panic!("no Herdr 2 session"))
            .send(command)
            .unwrap_or_else(|e| panic!("command: {e}"));
    });
}

fn type_text(view: &Entity<Smoke>, cx: &mut gpui::VisualTestContext, text: &str) {
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal_focus.focus(window, cx);
            this.refresh_terminal_input();
            this.terminal.input.fresh_gesture();
            gpui::EntityInputHandler::replace_text_in_range(this, None, text, window, cx);
        })
    });
}

/// Control-D through the key path a hardware keyboard takes.
fn control_d(view: &Entity<Smoke>, cx: &mut gpui::VisualTestContext) {
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.terminal_focus.focus(window, cx);
            this.refresh_terminal_input();
            this.terminal.input.fresh_gesture();
            let keystroke = Keystroke {
                key: "d".into(),
                key_char: Some("d".into()),
                modifiers: Modifiers {
                    control: true,
                    ..Modifiers::default()
                },
            };
            this.terminal_key_down(
                &KeyDownEvent {
                    keystroke: keystroke.clone(),
                    is_held: false,
                    prefer_character_input: false,
                },
                window,
                cx,
            );
            this.terminal_key_up(&KeyUpEvent { keystroke }, window, cx);
        })
    });
}

fn panes(live: &Live) -> usize {
    live.surface.as_ref().map_or(0, |s| s.panes.len())
}

/// Connects a fresh view to the sandbox and splits its tab: two shells.
fn split_session<'a>(
    sandbox: &mut Sandbox,
    cx: &'a mut TestAppContext,
) -> (Entity<Smoke>, &'a mut gpui::VisualTestContext) {
    let view = cx.new(Smoke::new);
    let cx = cx.add_empty_window();
    let backend = sandbox.bridge();
    view.update(cx, |this, cx| {
        this.select_host("private-core");
        this.connect_core_using(cx, move |_| Ok(backend));
    });
    wait(&view, cx, "first pane", |live| {
        live.connected && panes(live) == 1
    });
    command(&view, cx, Command::Split);
    wait(&view, cx, "split", |live| {
        live.connected && panes(live) == 2 && live.status.contains("active 2")
    });
    assert_eq!(
        sandbox
            .list()
            .iter()
            .filter(|t| t.state == sheprd_core_client::wire::TerminalState::Live)
            .count(),
        2,
        "the core runs both shells"
    );
    (view, cx)
}

/// The shell that keeps running still runs a command typed into it.
fn survivor_runs(view: &Entity<Smoke>, cx: &mut gpui::VisualTestContext) {
    type_text(view, cx, "echo SURV$((40+2))IVOR\r");
    wait(view, cx, "survivor output", |live| {
        live.connected && text(live).contains("SURV42IVOR")
    });
}

#[gpui::test]
#[ignore = "needs YEBISU_HERDR_CORE_TEST_BINARY"]
fn live_core_control_d_in_one_pane_keeps_the_session(cx: &mut TestAppContext) {
    let mut sandbox = Sandbox::start();
    let (view, cx) = split_session(&mut sandbox, cx);
    // Pane 2 is focused after the split; its shell ends on Control-D.
    control_d(&view, cx);
    wait(&view, cx, "pane 2 closed, pane 1 kept", |live| {
        live.connected && panes(live) == 1 && !live.status.contains("ended")
    });
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(2) {
        view.update(cx, |this, cx| this.poll_terminal(cx));
        let now = view.update(cx, |this, _| {
            (this.terminal.live.connected, this.terminal.core.is_some())
        });
        assert_eq!(now, (true, true), "session dropped: {}", seen(&view, cx));
        std::thread::sleep(Duration::from_millis(20));
    }
    survivor_runs(&view, cx);
    view.update(cx, |this, _| {
        this.terminal.core.take();
    });
}

/// The tabs the worker last listed for the view's own session.
fn tabs(view: &Entity<Smoke>, cx: &mut gpui::VisualTestContext) -> Vec<TabInfo> {
    view.update(cx, |this, _| {
        let epoch = this.terminal.live.input_epoch;
        let core = this
            .terminal
            .core
            .as_ref()
            .unwrap_or_else(|| panic!("no Herdr 2 session"));
        let name = core.name().to_owned();
        core.sessions(epoch)
            .into_iter()
            .find(|session| session.name == name)
            .map(|session| session.tabs)
            .unwrap_or_default()
    })
}

/// Waits until the worker lists `id` as the tab in use.
fn wait_tab(view: &Entity<Smoke>, cx: &mut gpui::VisualTestContext, id: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !tabs(view, cx).iter().any(|tab| tab.id == id && tab.active) {
        view.update(cx, |this, cx| this.poll_terminal(cx));
        assert!(
            Instant::now() < deadline,
            "tab {id} not in use: {}",
            seen(view, cx)
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[gpui::test]
#[ignore = "needs YEBISU_HERDR_CORE_TEST_BINARY"]
fn live_core_exit_in_a_one_pane_tab_keeps_the_other_tabs(cx: &mut TestAppContext) {
    let mut sandbox = Sandbox::start();
    let view = cx.new(Smoke::new);
    let cx = cx.add_empty_window();
    let backend = sandbox.bridge();
    view.update(cx, |this, cx| {
        this.select_host("private-core");
        this.connect_core_using(cx, move |_| Ok(backend));
    });
    wait(&view, cx, "first tab", |live| {
        live.connected && panes(live) == 1
    });
    let first = tabs(&view, cx);
    command(&view, cx, Command::NewTab);
    wait(&view, cx, "second tab", |live| {
        live.connected && panes(live) == 1
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    while tabs(&view, cx).len() != 2 {
        view.update(cx, |this, cx| this.poll_terminal(cx));
        assert!(Instant::now() < deadline, "second tab not listed");
        std::thread::sleep(Duration::from_millis(10));
    }
    type_text(&view, cx, "exit\r");
    wait(&view, cx, "the one-pane tab's program ended", |live| {
        live.status.contains("ended")
    });
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(2) {
        view.update(cx, |this, cx| this.poll_terminal(cx));
        let now = view.update(cx, |this, _| {
            (this.terminal.live.connected, this.terminal.core.is_some())
        });
        assert_eq!(now, (true, true), "session dropped: {}", seen(&view, cx));
        std::thread::sleep(Duration::from_millis(20));
    }
    // The other tab is still there and its shell still runs commands.
    let first = first
        .first()
        .unwrap_or_else(|| panic!("first tab not listed"))
        .id
        .clone();
    assert!(tabs(&view, cx).iter().any(|tab| tab.id == first));
    command(&view, cx, Command::SelectTab(first.clone()));
    wait_tab(&view, cx, &first);
    wait(&view, cx, "first tab selected", |live| {
        live.connected && panes(live) == 1 && !live.status.contains("ended")
    });
    survivor_runs(&view, cx);
    view.update(cx, |this, _| {
        this.terminal.core.take();
    });
}

#[gpui::test]
#[ignore = "needs YEBISU_HERDR_CORE_TEST_BINARY"]
fn live_core_last_tab_closed_keeps_the_session_and_a_stopped_core_disconnects(
    cx: &mut TestAppContext,
) {
    let mut sandbox = Sandbox::start();
    let (view, cx) = split_session(&mut sandbox, cx);
    let tab = tabs(&view, cx)
        .first()
        .unwrap_or_else(|| panic!("tab not listed"))
        .id
        .clone();
    command(&view, cx, Command::CloseTab(tab));
    wait(&view, cx, "last tab closed", |live| {
        live.connected && live.status.contains("No tabs")
    });
    command(&view, cx, Command::NewTab);
    wait(&view, cx, "a new tab after the last one closed", |live| {
        live.connected && panes(live) == 1
    });
    survivor_runs(&view, cx);
    // A real loss still reaches the view: the core stops.
    sandbox.stop();
    wait(&view, cx, "core stop reported", |live| !live.connected);
    view.update(cx, |this, _| {
        assert!(
            !this.terminal.live.status.is_empty(),
            "the loss has a reason"
        );
        this.terminal.core.take();
    });
}

#[gpui::test]
#[ignore = "needs YEBISU_HERDR_CORE_TEST_BINARY"]
fn live_core_tab_switch_after_a_pane_exit_keeps_the_session(cx: &mut TestAppContext) {
    let mut sandbox = Sandbox::start();
    let (view, cx) = split_session(&mut sandbox, cx);
    let first = tabs(&view, cx)
        .first()
        .unwrap_or_else(|| panic!("tab not listed"))
        .id
        .clone();
    control_d(&view, cx);
    wait(&view, cx, "pane 2's program ended", |live| {
        live.status.contains("ended") || panes(live) == 1
    });
    // Leave the tab and come back: the split is attached again.
    command(&view, cx, Command::NewTab);
    let deadline = Instant::now() + Duration::from_secs(10);
    while tabs(&view, cx).len() != 2 {
        view.update(cx, |this, cx| this.poll_terminal(cx));
        assert!(
            Instant::now() < deadline,
            "second tab not listed: {}",
            seen(&view, cx)
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    command(&view, cx, Command::SelectTab(first.clone()));
    wait_tab(&view, cx, &first);
    wait(&view, cx, "first tab selected again", |live| {
        live.connected && panes(live) == 1 && !live.status.contains("ended")
    });
    view.update(cx, |this, _| assert!(this.terminal.core.is_some()));
    survivor_runs(&view, cx);
    view.update(cx, |this, _| {
        this.terminal.core.take();
    });
}

#[gpui::test]
#[ignore = "needs YEBISU_HERDR_CORE_TEST_BINARY"]
fn live_core_link_lost_after_the_last_pane_ended_disconnects(cx: &mut TestAppContext) {
    let mut sandbox = Sandbox::start();
    let view = cx.new(Smoke::new);
    let cx = cx.add_empty_window();
    let backend = sandbox.bridge();
    view.update(cx, |this, cx| {
        this.select_host("private-core");
        this.connect_core_using(cx, move |_| Ok(backend));
    });
    wait(&view, cx, "first pane", |live| {
        live.connected && panes(live) == 1
    });
    control_d(&view, cx);
    // The last pane of the tab keeps its final screen; the session stays.
    wait(&view, cx, "the only pane's program ended", |live| {
        live.connected && panes(live) == 1 && live.status.contains("ended: 1")
    });
    // Nothing reads a stream now, yet losing the link is still reported.
    sandbox.cut();
    wait(&view, cx, "link loss reported", |live| !live.connected);
    view.update(cx, |this, _| {
        assert!(
            this.terminal.live.status.contains("lost"),
            "the loss has its reason: {}",
            this.terminal.live.status
        );
        this.terminal.core.take();
    });
}
