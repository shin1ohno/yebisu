//! Several real herdr-core servers in one private home, state and runtime
//! directory: one per Herdr 2 space, as a host holds them (`default` and
//! `space-<ms>`), reached the way the iPad reaches one, `hc connect` carrying
//! the protocol over a socket pair. Nothing here touches another user's or the
//! owner's cores: every server is started here and ended only here
//! (`server stop` on its own private session, then the exact child reaped).
use super::vacate::{Stopper, stop_command, stop_result};
use sheprd_core_client::{
    CoreBackend, Endpoint,
    wire::{Run, Terminal, TerminalState},
};
use std::{
    ffi::OsString,
    io::Write as _,
    os::unix::net::{UnixListener, UnixStream},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub(super) const BINARY: &str = "YEBISU_HERDR_CORE_TEST_BINARY";

pub(super) struct Fleet {
    dir: PathBuf,
    binary: OsString,
    cores: Vec<(String, Child)>,
    bridges: Vec<Child>,
    stops: Arc<Mutex<Vec<(String, String)>>>,
    listeners: Vec<UnixListener>,
}

impl Fleet {
    pub(super) fn start() -> Self {
        let binary = std::env::var_os(BINARY).unwrap_or_else(|| panic!("{BINARY} is not set"));
        // A holder socket `<dir>/herdr-core/<space>.holder.<id>.sock` stays
        // within 107 bytes, so the directory name is short: one fleet per
        // process at a time (run these tests with --test-threads=1).
        let dir = std::env::temp_dir().join(format!("{:x}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for name in ["home", "config", "state"] {
            std::fs::create_dir_all(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        }
        // Where the stop command looks for `hc`, as it does on a host.
        let bin = dir.join("home/.local/bin");
        std::fs::create_dir_all(&bin).unwrap_or_else(|e| panic!("bin: {e}"));
        std::os::unix::fs::symlink(&binary, bin.join("herdr-core"))
            .unwrap_or_else(|e| panic!("herdr-core link: {e}"));
        Self {
            dir,
            binary,
            cores: Vec::new(),
            bridges: Vec::new(),
            stops: Arc::default(),
            listeners: Vec::new(),
        }
    }

    fn vars(&self, space: &str) -> Vec<(&'static str, OsString)> {
        let at = |name: &str| self.dir.join(name).into_os_string();
        vec![
            ("HC_SESSION", OsString::from(space)),
            ("HOME", at("home")),
            ("PATH", OsString::from("/usr/bin:/bin")),
            ("SHELL", OsString::from("/bin/sh")),
            ("TERM", OsString::from("xterm-256color")),
            ("XDG_CONFIG_HOME", at("config")),
            ("XDG_RUNTIME_DIR", self.dir.clone().into_os_string()),
            ("XDG_STATE_HOME", at("state")),
        ]
    }

    /// `hc` for the space's own private server.
    pub(super) fn command(&self, space: &str) -> Command {
        let mut command = Command::new(&self.binary);
        command.env_clear().envs(self.vars(space));
        command
    }

    pub(super) fn socket(&self, space: &str) -> PathBuf {
        self.dir.join("herdr-core").join(format!("{space}.sock"))
    }

    /// Starts the space's server and waits until it listens.
    pub(super) fn add(&mut self, space: &str) {
        let child = self
            .command(space)
            .arg("serve")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(
                std::fs::File::create(self.dir.join(format!("{space}.log")))
                    .map_or_else(|_| Stdio::null(), Stdio::from),
            )
            .spawn()
            .unwrap_or_else(|e| panic!("herdr-core serve {space}: {e}"));
        self.cores.push((space.to_owned(), child));
        let deadline = Instant::now() + Duration::from_secs(10);
        while !self.alive(space) {
            assert!(
                Instant::now() < deadline,
                "{space} did not listen: {}",
                std::fs::read_to_string(self.dir.join(format!("{space}.log"))).unwrap_or_default()
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Whether the space's server accepts a connection.
    pub(super) fn alive(&self, space: &str) -> bool {
        UnixStream::connect(self.socket(space)).is_ok()
    }

    /// `hc connect --session <space>` on a socket pair, as SSH carries it.
    pub(super) fn bridge(&mut self, space: &str) -> CoreBackend {
        let (ours, theirs) = UnixStream::pair().unwrap_or_else(|e| panic!("pair: {e}"));
        let input = theirs.try_clone().unwrap_or_else(|e| panic!("clone: {e}"));
        let child = self
            .command(space)
            .args(["connect", "--session", space])
            .stdin(Stdio::from(std::os::fd::OwnedFd::from(input)))
            .stdout(Stdio::from(std::os::fd::OwnedFd::from(theirs)))
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|e| panic!("hc connect {space}: {e}"));
        self.bridges.push(child);
        ours.set_read_timeout(Some(Duration::from_millis(10)))
            .unwrap_or_else(|e| panic!("read timeout: {e}"));
        ours.set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap_or_else(|e| panic!("write timeout: {e}"));
        let core = sheprd_core_client::Core::over(ours).unwrap_or_else(|e| panic!("hello: {e}"));
        CoreBackend::new(core, 10000)
    }

    fn connect(&self, space: &str) -> crate::Result<sheprd_core_client::Core> {
        let vars = self.vars(space);
        let endpoint = Endpoint::resolve(|name| {
            vars.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| value.clone())
        })?;
        Ok(sheprd_core_client::Core::connect(&endpoint)?)
    }

    /// A second, independent client: what `hc ls --all` would show.
    pub(super) fn list(&self, space: &str) -> Vec<Terminal> {
        self.connect(space)
            .and_then(|core| Ok(core.list(true)?))
            .unwrap_or_else(|e| panic!("list {space}: {e}"))
            .terminals
    }

    pub(super) fn live(&self, space: &str) -> usize {
        self.list(space)
            .iter()
            .filter(|t| t.state == TerminalState::Live)
            .count()
    }

    /// Another client starts a program in the space's server, in no tab of
    /// yebisu: `hc run --bg sleep 600`. Returns its address.
    pub(super) fn run_elsewhere(&self, space: &str) -> String {
        let argv = ["sleep".to_owned(), "600".to_owned()];
        self.connect(space)
            .and_then(|core| {
                Ok(core.run(&Run {
                    argv: &argv,
                    ..Run::default()
                })?)
            })
            .unwrap_or_else(|e| panic!("run in {space}: {e}"))
            .address
    }

    /// What the SSH discovery command prints on this host: one line per
    /// space with its server's id and terminals (`discover.py`, run the way
    /// `DISCOVER` runs it, against the private directories).
    pub(super) fn discover(&self) -> Vec<u8> {
        let mut child = Command::new("python3")
            .arg("-I")
            .arg("-")
            .env_clear()
            .envs(
                self.vars("default")
                    .into_iter()
                    .filter(|(k, _)| *k != "HC_SESSION"),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|e| panic!("python3: {e}"));
        child
            .stdin
            .take()
            .unwrap_or_else(|| panic!("python3 stdin"))
            .write_all(include_bytes!("../shell/herdr2_frame/discover.py"))
            .unwrap_or_else(|e| panic!("discover.py: {e}"));
        let output = child
            .wait_with_output()
            .unwrap_or_else(|e| panic!("discover: {e}"));
        assert!(output.status.success(), "discover.py failed");
        output.stdout
    }

    /// The id the space's server answers hello with.
    pub(super) fn core_id(&self, space: &str) -> String {
        self.connect(space)
            .unwrap_or_else(|e| panic!("connect {space}: {e}"))
            .hello()
            .core
            .id
            .clone()
    }

    /// Another client ends one of the space's programs: `hc kill <address>`.
    pub(super) fn kill_elsewhere(&self, space: &str, address: &str) {
        let output = self
            .command(space)
            .args(["kill", address])
            .output()
            .unwrap_or_else(|e| panic!("hc kill: {e}"));
        assert!(output.status.success(), "hc kill {address}");
    }

    /// The stop requests made so far, as `(space, core id)`.
    pub(super) fn stops(&self) -> Vec<(String, String)> {
        self.stops
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// A stopper that runs the production stop command (`stop_command`) here,
    /// where SSH would run it on a host, against this fleet's own directories:
    /// the same script, the same exit codes, the same server check.
    pub(super) fn stopper(&self) -> Stopper {
        let vars = self
            .vars("default")
            .into_iter()
            .filter(|(name, _)| *name != "HC_SESSION")
            .collect::<Vec<_>>();
        let stops = Arc::clone(&self.stops);
        Arc::new(move |space, expect| {
            stops
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push((space.to_owned(), expect.core_id.clone()));
            let command = stop_command(space, expect)?;
            let output = Command::new("/bin/sh")
                .arg("-c")
                .arg(command)
                .env_clear()
                .envs(vars.iter().map(|(name, value)| (name, value)))
                .output()
                .map_err(|e| crate::Error::Ssh(format!("sh: {e}")))?;
            stop_result(output.status.code(), &output.stdout)
        })
    }

    /// The pid of the space's server (`hc server status --json`).
    pub(super) fn pid(&self, space: &str) -> u64 {
        let output = self
            .command(space)
            .args(["server", "status", "--json"])
            .output()
            .unwrap_or_else(|e| panic!("hc server status: {e}"));
        serde_json::from_slice::<serde_json::Value>(&output.stdout)
            .ok()
            .and_then(|status| status["pid"].as_u64())
            .unwrap_or_else(|| panic!("no pid for {space}"))
    }

    /// Ends the space's server and starts it again: the same core id (it is
    /// kept in the state directory), another pid.
    pub(super) fn restart(&mut self, space: &str) {
        self.stop_space(space).unwrap_or_else(|e| panic!("{e}"));
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.alive(space) {
            assert!(Instant::now() < deadline, "{space} did not stop");
            std::thread::sleep(Duration::from_millis(50));
        }
        for (name, child) in &mut self.cores {
            if name == space {
                let _ = child.wait();
            }
        }
        self.cores.retain(|(name, _)| name != space);
        self.add(space);
    }

    /// A socket where a server may be that never answers: a connection is
    /// accepted by the kernel and nothing is read (a busy server, or a
    /// machine that does not reply).
    pub(super) fn silent_socket(&mut self, space: &str) {
        let socket = self.socket(space);
        std::fs::create_dir_all(socket.parent().unwrap_or(&self.dir))
            .unwrap_or_else(|e| panic!("socket dir: {e}"));
        self.listeners
            .push(UnixListener::bind(&socket).unwrap_or_else(|e| panic!("bind {space}: {e}")));
    }

    /// `hc server stop` on one space's private session.
    pub(super) fn stop_space(&self, space: &str) -> crate::Result<()> {
        let output = self
            .command(space)
            .args(["server", "stop"])
            .output()
            .map_err(|e| crate::Error::Ssh(format!("hc server stop: {e}")))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(crate::Error::Ssh(format!(
                "hc server stop: {}",
                String::from_utf8_lossy(&output.stderr)
            )))
        }
    }

    /// Ends every server this fleet started that still runs, then reaps its
    /// exact children.
    pub(super) fn stop(&mut self) {
        let names = self
            .cores
            .iter()
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        for name in names {
            let _ = self.stop_space(&name);
        }
        for (_, child) in &mut self.cores {
            let deadline = Instant::now() + Duration::from_secs(10);
            while child.try_wait().ok().flatten().is_none() {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        self.cores.clear();
    }
}

impl Drop for Fleet {
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
