//! Controlled receiving peer for P13 adapter and Simulator evidence only.
//! Matches are derived from the same fixture text used to construct the frame.
use sheprd_client::{
    Client, ConnectOptions, ConnectTarget, Stream,
    protocol::{endpoint::*, *},
    scrollback::{CopySearchParams, CopySearchResult, SearchDirection, TextPoint, TextRange},
};
use std::{
    net::Shutdown,
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, sync_channel},
    },
    time::{Duration, Instant},
};
/// The peer drops the link after this long without a client message.
const IDLE: Duration = Duration::from_secs(30);

/// Idle bound of one controlled peer link. It starts at `arm`, not at launch:
/// before its first tap XCUITest may leave the app idle for longer than the
/// bound (34.5 s after launch in P13 attempt 8), and a launch-anchored read
/// timeout dropped the link before the fixture began.
pub(crate) struct Idle {
    limit: Duration,
    state: Mutex<IdleState>,
}
#[derive(Default)]
struct IdleState {
    armed: Option<Instant>,
    heard: Option<Instant>,
    closed: bool,
}
impl Idle {
    fn new(limit: Duration, armed: bool) -> Arc<Self> {
        Arc::new(Self {
            limit,
            state: Mutex::new(IdleState {
                armed: armed.then(Instant::now),
                ..IdleState::default()
            }),
        })
    }
    /// Starts the bound. Later calls keep the first start.
    pub(crate) fn arm(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.armed.get_or_insert_with(Instant::now);
        }
    }
    fn heard(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.heard = Some(Instant::now());
        }
    }
    fn close(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.closed = true;
        }
    }
    /// `None` once the link ended; `Some(true)` when an armed bound expired.
    fn expired(&self, now: Instant) -> Option<bool> {
        let state = self.state.lock().ok()?;
        if state.closed {
            return None;
        }
        Some(state.armed.is_some_and(|armed| {
            let since = state.heard.map_or(armed, |heard| heard.max(armed));
            now.saturating_duration_since(since) >= self.limit
        }))
    }
}
/// Ends a link idle past its armed bound by shutting the peer's socket; the
/// blocked read then fails and the peer thread exits.
fn watch(idle: Arc<Idle>, stream: Stream) {
    let tick = (idle.limit / 10).clamp(Duration::from_millis(5), Duration::from_secs(1));
    loop {
        std::thread::sleep(tick);
        match idle.expired(Instant::now()) {
            None => return,
            Some(false) => {}
            Some(true) => {
                let _ = stream.shutdown(Shutdown::Both);
                return;
            }
        }
    }
}
#[cfg(all(target_os = "ios", target_abi = "sim"))]
static FIXTURE: Mutex<Option<Arc<Idle>>> = Mutex::new(None);
/// The Simulator fixture's link. Its idle bound waits for `arm_fixture`.
#[cfg(all(target_os = "ios", target_abi = "sim"))]
pub(crate) fn connect_fixture() -> crate::Result<Client> {
    let idle = Idle::new(IDLE, false);
    if let Ok(mut slot) = FIXTURE.lock() {
        *slot = Some(idle.clone());
    }
    connect_mode(None, false, idle, None).map(|(client, _)| client)
}
/// Called by every fixture tap; only the first one starts the bound.
#[cfg(all(target_os = "ios", target_abi = "sim"))]
pub(crate) fn arm_fixture() {
    if let Ok(slot) = FIXTURE.lock()
        && let Some(idle) = slot.as_ref()
    {
        idle.arm();
    }
}
const LINES: [(u32, &str); 3] = [
    (0, "ASCII needle needle"),
    (1, "日本語 日本語"),
    (100, "beyond"),
];
pub(crate) fn surface() -> PaneSurfaceFrame {
    resized(80, 24, 1, 100)
}
fn resized(cols: u16, rows: u16, serial: u64, offset: u64) -> PaneSurfaceFrame {
    let mut frame = crate::mock_peer::surface(7);
    frame.graphics.placements.clear();
    let mut blank = frame.frame.cells[79].clone();
    blank.symbol = " ".into();
    blank.skip = false;
    frame.frame.cells = vec![blank; usize::from(cols) * usize::from(rows)];
    frame.frame.width = cols;
    frame.frame.height = rows;
    frame.frame.cursor = None;
    frame.surface_revision = serial;
    let top = 100u64.saturating_sub(offset);
    for (absolute, line) in LINES {
        let row = u64::from(absolute);
        if row < top || row >= top.saturating_add(u64::from(rows)) {
            continue;
        }
        let row = (row - top) as usize;
        let mut col = 0;
        for ch in line.chars() {
            let width = unicode_width::UnicodeWidthChar::width(ch)
                .unwrap_or(1)
                .max(1);
            if col + width > usize::from(cols) {
                break;
            }
            frame.frame.cells[row * usize::from(cols) + col].symbol = ch.to_string();
            if width == 2 {
                frame.frame.cells[row * usize::from(cols) + col + 1]
                    .symbol
                    .clear();
            }
            col += width;
        }
    }
    let rect = SurfaceRect {
        x: 0,
        y: 0,
        width: cols,
        height: rows,
    };
    frame.panes = vec![PaneSurfacePane {
        pane_id: "w1:p1".into(),
        content_revision: 4,
        rect,
        inner_rect: rect,
        scrollbar_rect: None,
        scroll: Some(PaneSurfaceScrollMetrics {
            offset_from_bottom: offset,
            max_offset_from_bottom: 100,
            viewport_rows: u64::from(rows),
        }),
        focused: true,
        mouse_reporting: false,
        sgr_pixel_mouse: false,
        alternate_screen_active: false,
        pixel_width: 0,
        pixel_height: 0,
    }];
    frame
}

fn search(params: &CopySearchParams) -> CopySearchResult {
    let mut matches = Vec::new();
    if !params.query.is_empty() {
        for (row, line) in LINES {
            for (byte, text) in line.match_indices(params.query.as_str()) {
                let col = unicode_width::UnicodeWidthStr::width(&line[..byte]) as u16;
                let width = unicode_width::UnicodeWidthStr::width(text) as u16;
                matches.push(TextRange {
                    start: TextPoint { row, col },
                    end: TextPoint {
                        row,
                        col: col.saturating_add(width.saturating_sub(1)),
                    },
                });
            }
        }
    }
    let current = if matches.is_empty() {
        None
    } else {
        let previous = params
            .previous
            .and_then(|p| matches.iter().position(|m| *m == p));
        Some(match (params.direction, previous) {
            (SearchDirection::Forward, Some(i)) => (i + 1) % matches.len(),
            (SearchDirection::Backward, Some(i)) => (i + matches.len() - 1) % matches.len(),
            (SearchDirection::Forward, None) => 0,
            (SearchDirection::Backward, None) => matches.len() - 1,
        } as u32)
    };
    CopySearchResult {
        pane_id: params.pane_id.clone(),
        content_revision: params.content_revision,
        total: matches.len() as u64,
        matches,
        current,
        current_global: current.map(u64::from),
    }
}
/// Host fixtures: the bound starts at connect.
#[cfg(test)]
pub(crate) fn connect() -> crate::Result<(Client, Receiver<ClientMessage>)> {
    connect_mode(None, false, Idle::new(IDLE, true), None)
}
/// Publishes the initial state, then records client messages without replying.
#[cfg(test)]
pub(crate) fn connect_quiet(report_all: bool) -> crate::Result<(Client, Receiver<ClientMessage>)> {
    connect_mode(None, false, Idle::new(IDLE, true), Some(report_all))
}
/// An unarmed link with a short bound, for the bound's own host test.
#[cfg(test)]
pub(crate) fn connect_idle(
    limit: Duration,
) -> crate::Result<(Client, Receiver<ClientMessage>, Arc<Idle>)> {
    let idle = Idle::new(limit, false);
    connect_mode(None, false, idle.clone(), None).map(|(client, received)| (client, received, idle))
}
#[cfg(test)]
pub(crate) fn connect_delayed(
    wrong_boot: bool,
) -> crate::Result<(
    Client,
    Receiver<ClientMessage>,
    std::sync::mpsc::SyncSender<()>,
)> {
    let (release, gate) = sync_channel(1);
    connect_mode(Some(gate), wrong_boot, Idle::new(IDLE, true), None)
        .map(|(client, received)| (client, received, release))
}
fn connect_mode(
    mut gate: Option<Receiver<()>>,
    mut wrong_boot: bool,
    idle: Arc<Idle>,
    key_receipts_report_all: Option<bool>,
) -> crate::Result<(Client, Receiver<ClientMessage>)> {
    let (local, mut server) = Stream::pair()?;
    let (receipts, receiver) = sync_channel(64);
    let watched = server.try_clone()?;
    let watcher = idle.clone();
    std::thread::Builder::new()
        .name("yebisu-p13-peer-idle".into())
        .spawn(move || watch(watcher, watched))?;
    std::thread::Builder::new()
        .name("yebisu-p13-peer".into())
        .spawn(move || {
            let mut serve = || -> Result<(), Box<dyn std::error::Error>> {
                #[cfg(all(target_os = "ios", target_abi = "sim"))]
                if let Some(home) = std::env::var_os("HOME") {
                    std::fs::write(
                        std::path::PathBuf::from(home).join("Documents/p13-peer.jsonl"),
                        "",
                    )?;
                }
                let _: ClientMessage = read_message(&mut server, MAX_FRAME_SIZE)?;
                idle.heard();
                let mut welcome: serde_json::Value =
                    serde_json::from_str(include_str!("../../fixtures/endpoint-welcome-v1.json"))?;
                welcome["server_version"] = sheprd_client::MIN_HERDR_VERSION.into();
                welcome["methods"] = serde_json::json!([
                    "client_shell.surface.set",
                    "pane.copy_search",
                    "pane.scroll"
                ]);
                welcome["capabilities"] =
                    serde_json::json!(["surface_interest", "presentation_effects_fence"]);
                for (kind, data) in [
                    (ENDPOINT_WELCOME_KIND, welcome.to_string()),
                    (
                        ENDPOINT_SNAPSHOT_KIND,
                        crate::mock_peer::SNAPSHOT.to_owned(),
                    ),
                ] {
                    write_message(
                        &mut server,
                        &ServerMessage::EndpointControl {
                            kind: kind.into(),
                            data,
                        },
                        MAX_FRAME_SIZE,
                    )?;
                }
                if let Some(enabled) = key_receipts_report_all {
                    // The first surface makes the fixture ready. Publish the
                    // mode before it so the reader has applied both by then.
                    write_message(
                        &mut server,
                        &ServerMessage::ClientShellKeyboardReportAll { enabled },
                        MAX_FRAME_SIZE,
                    )?;
                }
                write_message(
                    &mut server,
                    &ServerMessage::PaneSurface(surface()),
                    MAX_FRAME_SIZE,
                )?;
                let mut serial = surface().surface_revision;
                let (mut cols, mut rows, mut offset) = (80, 24, 100);
                loop {
                    let message: ClientMessage = read_message(&mut server, MAX_FRAME_SIZE)?;
                    idle.heard();
                    let _ = receipts.try_send(message.clone());
                    #[cfg(all(target_os = "ios", target_abi = "sim"))]
                    record(&message);
                    // Resize and scroll replies would let the reader hold the
                    // inbox while a test needs a stable owner for retirement.
                    if key_receipts_report_all.is_some() {
                        continue;
                    }
                    if let ClientMessage::ClientShellResize { surface_size, .. } = &message {
                        serial += 1;
                        cols = surface_size.cols;
                        rows = surface_size.rows;
                        write_message(
                            &mut server,
                            &ServerMessage::PaneSurface(resized(
                                surface_size.cols,
                                surface_size.rows,
                                serial,
                                offset,
                            )),
                            MAX_FRAME_SIZE,
                        )?;
                    }
                    if let ClientMessage::ClientShellEndpointRequest { boot_id, request } = message
                    {
                        let request: serde_json::Value = serde_json::from_str(&request)?;
                        if request["method"] == "pane.copy_search"
                            && let Some(wait) = gate.take()
                        {
                            wait.recv_timeout(Duration::from_secs(10))?;
                        }
                        if request["method"] == "pane.scroll" {
                            offset = request["params"]["offset_from_bottom"]
                                .as_u64()
                                .unwrap_or(100)
                                .min(100);
                            serial += 1;
                            write_message(
                                &mut server,
                                &ServerMessage::PaneSurface(resized(cols, rows, serial, offset)),
                                MAX_FRAME_SIZE,
                            )?;
                        }
                        let boot_id = if wrong_boot {
                            wrong_boot = false;
                            "wrong-boot".into()
                        } else {
                            boot_id
                        };
                        let result = if request["method"] == "pane.copy_search" {
                            let params: CopySearchParams =
                                serde_json::from_value(request["params"].clone())?;
                            let mut value = serde_json::to_value(search(&params))?;
                            value["type"] = "pane_copy_search".into();
                            value
                        } else {
                            serde_json::json!({})
                        };
                        let id = request["id"].as_str().unwrap_or_default().to_owned();
                        write_message(
                            &mut server,
                            &ServerMessage::ClientShellEndpointResponseChunk {
                                boot_id,
                                request_id: id.clone(),
                                final_chunk: true,
                                data: serde_json::to_vec(
                                    &serde_json::json!({"id":id,"result":result}),
                                )?,
                            },
                            MAX_FRAME_SIZE,
                        )?;
                    }
                }
            };
            let _ = serve();
            idle.close();
        })?;
    sheprd_client::connect_with_connector(
        ConnectTarget::Local,
        ConnectOptions::default(),
        true,
        move |_, _| Ok(local),
    )
    .map(|client| (client, receiver))
    .map_err(|e| crate::Error::Ssh(e.to_string()))
}
#[cfg(all(target_os = "ios", target_abi = "sim"))]
fn record(message: &ClientMessage) {
    if let Some(home) = std::env::var_os("HOME")
        && let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(std::path::PathBuf::from(home).join("Documents/p13-peer.jsonl"))
    {
        let _ = crate::observation::write_jsonl_line(
            &mut file,
            &serde_json::to_string(message).unwrap_or_default(),
        );
    }
}
