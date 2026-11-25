//! The ports a host listens on, for the forward sheet's chips. A reduced
//! form of Sheprd's `listening_ports/scan.rs`: the same `ss` / `lsof` step,
//! but it prints only addresses, and yebisu keeps only their port numbers.
//! Process names, paths and everything else the host prints are dropped,
//! because what a host prints is untrusted data (AGENTS.md); a number from
//! 1 to 65535 is all that can reach the screen.
use std::{
    collections::BTreeSet,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

/// Chips shown at most, lowest first (Sheprd's `MAX_PORTS`).
pub(crate) const MAX_PORTS: usize = 32;
/// Lines read from one answer; the rest are ignored.
const MAX_LINES: usize = 1024;
/// How often the open sheet reads the host's listeners again (Sheprd's
/// interval).
pub(crate) const INTERVAL: Duration = Duration::from_secs(5);
/// How long one scan may take, and how much it may print.
#[cfg(target_os = "ios")]
const TIMEOUT: Duration = Duration::from_secs(3);
#[cfg(target_os = "ios")]
const OUTPUT_LIMIT: u64 = 16_384;
/// How often a waiting scan thread checks whether the sheet closed.
const NAP: Duration = Duration::from_millis(100);

/// Prints `L <address:port>` for each TCP socket this user's processes
/// listen on, or `N` when the host has neither `ss` (Linux) nor `lsof`
/// (macOS). `ss -p` names a process only for this user's sockets, so
/// another user's servers never show; `lsof` lists only this user's. Each
/// `PATH` entry is added only when missing.
pub(crate) const COMMAND: &str = r#"for yebisu_dir in /usr/sbin /sbin; do
    case ":$PATH:" in
        *":$yebisu_dir:"*) ;;
        *) PATH="$PATH:$yebisu_dir" ;;
    esac
done
if command -v ss >/dev/null 2>&1; then
    ss -ltnp 2>/dev/null | awk '/\("[^"]*",pid=[0-9]+/ { print "L", $4 }'
elif command -v lsof >/dev/null 2>&1; then
    lsof -nP -iTCP -sTCP:LISTEN -Fn 2>/dev/null | awk '/^n/ { print "L", substr($0, 2) }'
else
    echo N
fi
true"#;

/// The listening ports in what [`COMMAND`] printed: unique, ascending, at
/// most [`MAX_PORTS`]. `None` when the host has no tool to list them, so the
/// sheet shows no chip row.
pub(crate) fn parse(output: &[u8]) -> Option<Vec<u16>> {
    let mut ports = BTreeSet::new();
    for line in output.split(|byte| *byte == b'\n').take(MAX_LINES) {
        let Ok(line) = std::str::from_utf8(line) else {
            continue;
        };
        let line = line.trim_end_matches('\r');
        if line == "N" {
            return None;
        }
        if let Some(port) = line.strip_prefix("L ").and_then(port) {
            ports.insert(port);
        }
    }
    Some(ports.into_iter().take(MAX_PORTS).collect())
}

/// The port of `127.0.0.1:3000`, `[::1]:3000`, `*:3000` or `0.0.0.0:3000`.
fn port(address: &str) -> Option<u16> {
    let (_, number) = address.trim().rsplit_once(':')?;
    if number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    number.parse().ok().filter(|port| *port > 0)
}

/// Runs [`COMMAND`] on a host; `cancelled` reports the sheet closed. The
/// app's runs it over SSH; tests inject their own.
pub(crate) type Run = Arc<dyn Fn(&str, &dyn Fn() -> bool) -> crate::Result<Vec<u8>> + Send + Sync>;

/// [`COMMAND`] over the in-process SSH client, as Herdr 2 discovery runs its
/// own command (`core.rs`).
#[cfg(target_os = "ios")]
pub(crate) fn ssh() -> Run {
    Arc::new(|host, cancelled| {
        let (exit, output) =
            sheprd_client::run_ssh_command(host, COMMAND, TIMEOUT, OUTPUT_LIMIT, cancelled)
                .map_err(|error| crate::Error::Ssh(error.to_string()))?;
        if !exit.success() {
            return Err(crate::Error::Ssh(String::new()));
        }
        Ok(output)
    })
}

/// A host's listeners, read every [`INTERVAL`] by a thread of its own while
/// the forward sheet is open. Dropping it stops the thread.
pub(crate) struct Scanner {
    latest: Arc<Mutex<Option<Option<Vec<u16>>>>>,
    cancelled: Arc<AtomicBool>,
}

impl Scanner {
    /// Starts scanning `host` at once. `None` when no thread can start; the
    /// sheet then shows no chips.
    pub(crate) fn start(host: String, run: Run) -> Option<Self> {
        let latest = Arc::new(Mutex::new(None));
        let cancelled = Arc::new(AtomicBool::new(false));
        let (mailbox, stop) = (Arc::clone(&latest), Arc::clone(&cancelled));
        thread::Builder::new()
            .name("yebisu-port-scan".into())
            .spawn(move || scan_loop(&host, &run, &mailbox, &stop))
            .ok()?;
        Some(Self { latest, cancelled })
    }

    /// The newest result since the last call: `Some(None)` when the host
    /// has no tool, `Some(Some(ports))` otherwise. A failed scan reports
    /// nothing, so the chips shown stay.
    pub(crate) fn take(&self) -> Option<Option<Vec<u16>>> {
        self.latest
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }
}

impl Drop for Scanner {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

fn scan_loop(
    host: &str,
    run: &Run,
    latest: &Mutex<Option<Option<Vec<u16>>>>,
    cancelled: &AtomicBool,
) {
    let stopped = || cancelled.load(Ordering::Acquire);
    while !stopped() {
        let next = Instant::now() + INTERVAL;
        match run(host, &stopped) {
            Ok(output) => {
                *latest.lock().unwrap_or_else(PoisonError::into_inner) = Some(parse(&output));
            }
            Err(_) => tracing::debug!("listening port scan failed"),
        }
        while !stopped() && Instant::now() < next {
            thread::sleep(NAP);
        }
    }
}
