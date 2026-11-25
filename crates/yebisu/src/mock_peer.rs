//! Private in-process wire peer. Compiled only in tests and Simulator builds.
use sheprd_client::{
    Client, ConnectOptions, ConnectTarget, Stream,
    protocol::{endpoint::*, *},
};
use std::time::Duration;
pub(crate) mod split;
pub(crate) const SNAPSHOT: &str = include_str!("../fixtures/endpoint-snapshot-v1.json");
const WELCOME: &str = include_str!("../fixtures/endpoint-welcome-v1.json");
pub(crate) fn surface(revision: u64) -> PaneSurfaceFrame {
    let lines = [
        "yebisu P4 · private wire peer",
        if cfg!(target_abi = "sim")
            && std::env::var("YEBISU_SH1_152_CASE").is_ok_and(|c| c.starts_with("E5"))
        {
            "AAA-152"
        } else {
            "日本語 ABC かな カナ 漢字"
        },
        "┌──────────────┐",
        "│ box drawing  │",
        "└──────────────┘",
        "Nerd Font: \u{f120} \u{e0b0} \u{f013}",
        "ANSI palette and truecolour",
        "Type here · keyboard and IME",
    ];
    let width = 80;
    let height = 24;
    let blank = CellData {
        symbol: " ".into(),
        fg: 0,
        bg: 0,
        modifier: 0,
        skip: false,
        hyperlink: None,
    };
    let mut cells = vec![blank.clone(); width * height];
    for (row, line) in lines.iter().enumerate() {
        let mut col = 1;
        for ch in line.chars() {
            let count = unicode_width::UnicodeWidthChar::width(ch)
                .unwrap_or(1)
                .max(1);
            if col + count >= width {
                break;
            }
            cells[(row + 1) * width + col] = CellData {
                symbol: ch.to_string(),
                fg: 0x0200d8de,
                ..blank.clone()
            };
            if count == 2 {
                // Herdr 1 full frames retain blanks; patch rows retain empties.
                // Mix the two on one row, with skip false in both cases.
                if col % 4 == 3 {
                    cells[(row + 1) * width + col + 1].symbol.clear();
                }
            }
            col += count;
        }
    }
    for n in 0..16 {
        for x in 0..4 {
            cells[11 * width + n * 4 + x].bg = 0x01000000 + n as u32;
        }
    }
    let mut surface = PaneSurfaceFrame {
        boot_id: "boot-v1".into(),
        projection_revision: revision,
        surface_revision: 1,
        frame: FrameData {
            width: width as u16,
            height: height as u16,
            cells,
            cursor: Some(CursorState {
                x: 2,
                y: 14,
                visible: true,
                shape: 1,
            }),
            hyperlinks: vec![],
            graphics: vec![],
        },
        panes: vec![],
        splits: vec![],
        popup: None,
        graphics: handoff_graphics(),
    };
    // L66: the link fixture, only with YEBISU_URL_TAP_QA.
    if crate::terminal_links::qa::enabled() {
        crate::terminal_links::qa::herdr1_fixture(&mut surface.frame);
    }
    surface
}
fn serve(
    mut server: Stream,
    frames: u64,
    receipts: std::sync::mpsc::Sender<ClientMessage>,
    board: bool,
    fault: &str,
    mut gate: Option<std::sync::mpsc::Receiver<()>>,
    transport: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    server.set_read_timeout(Some(Duration::from_secs(300)))?;
    let mut held = 0;
    // The size the client last asked for, which its frames have.
    let mut size: Option<(u16, u16)> = None;
    // L66: frames show the link fixture (YEBISU_URL_TAP_QA, or the tests').
    let links = crate::terminal_links::qa::enabled() || fault == "links";
    let _: ClientMessage = read_message(&mut server, MAX_FRAME_SIZE)?;
    let mut welcome: serde_json::Value = serde_json::from_str(WELCOME)?;
    welcome["server_version"] = sheprd_client::MIN_HERDR_VERSION.into();
    welcome["methods"] = serde_json::json!([
        "client_shell.surface.set",
        "pane.focus",
        "pane.link.resolve",
        "workspace.focus",
        "tab.focus",
        "tab.create",
        "pane.split",
        "pane.focus_direction",
        "pane.zoom",
        "pane.close",
        "tab.close",
        "workspace.rename",
        "tab.rename",
        "pane.rename"
    ]);
    welcome["capabilities"] = serde_json::json!(["surface_interest", "presentation_effects_fence"]);
    let welcome = welcome.to_string();
    for (kind, data) in [
        (ENDPOINT_WELCOME_KIND, welcome.as_str()),
        (ENDPOINT_SNAPSHOT_KIND, initial_snapshot()?.as_str()),
    ] {
        write_message(
            &mut server,
            &ServerMessage::EndpointControl {
                kind: kind.into(),
                data: data.into(),
            },
            MAX_FRAME_SIZE,
        )?;
    }
    for n in 1..=frames {
        let mut frame = surface(7);
        frame.surface_revision = n;
        write_message(
            &mut server,
            &ServerMessage::PaneSurface(frame),
            MAX_FRAME_SIZE,
        )?;
    }
    let mut serial = frames;
    let mut board_snapshot: ClientShellSnapshot = serde_json::from_str(&initial_snapshot()?)?;
    if board {
        server.set_read_timeout(Some(Duration::from_millis(500)))?;
    }
    // Issue #143 S7 (Simulator QA): wake up to insert an agent while the
    // rename field is open over an agent row.
    let insert_agent = !board && agent_insert_enabled();
    if insert_agent {
        server.set_read_timeout(Some(Duration::from_millis(250)))?;
    }

    let mut sequence = 0u64;
    loop {
        // Consume geometry updates from the real client; no external socket or host.
        let message: ClientMessage = match read_message(&mut server, MAX_CLIPBOARD_IMAGE_FRAME_SIZE)
        {
            Ok(message) => message,
            Err(e)
                if insert_agent
                    && matches!(
                        e.kind(),
                        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                    ) =>
            {
                if take_agent_insert() {
                    insert_agent_above(&mut board_snapshot);
                    write_message(
                        &mut server,
                        &ServerMessage::EndpointControl {
                            kind: ENDPOINT_SNAPSHOT_KIND.into(),
                            data: serde_json::to_string(&board_snapshot)?,
                        },
                        MAX_FRAME_SIZE,
                    )?;
                    let time = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)?
                        .as_secs_f64();
                    record_peer(serde_json::json!({"kind": "rename-agent-inserted",
                        "revision": board_snapshot.revision, "time": time}))?;
                }
                continue;
            }
            Err(e)
                if board
                    && matches!(
                        e.kind(),
                        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                    ) =>
            {
                board_snapshot.revision += 1;
                board_snapshot.agents[0].agent_status = if board_snapshot.revision.is_multiple_of(2)
                {
                    AgentStatus::Working
                } else {
                    AgentStatus::Blocked
                };
                write_message(
                    &mut server,
                    &ServerMessage::EndpointControl {
                        kind: ENDPOINT_SNAPSHOT_KIND.into(),
                        data: serde_json::to_string(&board_snapshot)?,
                    },
                    MAX_FRAME_SIZE,
                )?;
                write_message(
                    &mut server,
                    &ServerMessage::PaneSurface(focused_surface(&board_snapshot, &mut serial)),
                    MAX_FRAME_SIZE,
                )?;
                continue;
            }
            Err(e) => return Err(e.into()),
        };
        sequence += 1;
        // Ingress is evidence even if parsing, the gate or a response fails.
        record(&message, transport, sequence)?;
        let _ = receipts.send(message.clone());
        // Quiet fixtures must not mask missing UI invalidation with geometry responses.
        if fault == "quiet" {
            continue;
        }
        if let ClientMessage::ClientShellEndpointRequest { request, .. } = &message {
            let request: serde_json::Value = serde_json::from_str(request)?;
            let id = request["id"].as_str().unwrap_or_default().to_owned();
            // L66: where a pressed address ends, as the peer's terminal knows.
            if request["method"] == "pane.link.resolve" {
                let mut probe = serial;
                let mut frame = focused_surface(&board_snapshot, &mut probe).frame;
                // The frame the client shows: the tests' stays at the peer's
                // first size (no resize is answered), with the fixture they
                // add to it.
                if let Some((cols, rows)) = size {
                    resize_frame(&mut frame, cols, rows);
                }
                if links {
                    crate::terminal_links::qa::herdr1_fixture(&mut frame);
                }
                let regions = crate::terminal_links::qa::mock_regions(&frame, &request["params"]);
                write_message(
                    &mut server,
                    &ServerMessage::ClientShellEndpointResponseChunk {
                        boot_id: "boot-v1".into(),
                        request_id: id.clone(),
                        final_chunk: true,
                        data: serde_json::to_vec(&serde_json::json!({"id": id,
                            "result": {"type": "pane_link_resolved", "regions": regions}}))?,
                    },
                    MAX_FRAME_SIZE,
                )?;
                continue;
            }
            if fault != "response-first"
                && let Some(gate) = &gate
            {
                let timeout =
                    if cfg!(target_abi = "sim") && std::env::var_os("YEBISU_P12_QA").is_some() {
                        120
                    } else {
                        5
                    };
                let released = gate.recv_timeout(Duration::from_secs(timeout));
                if p12_observing() {
                    let reason = match released {
                        Ok(()) => "released",
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => "timeout",
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                            "sender_disconnected"
                        }
                    };
                    record_peer(
                        serde_json::json!({"kind":"p12-peer-gate", "transport":transport,
                        "sequence":sequence,"reason":reason}),
                    )?;
                    if reason != "released" {
                        return Err(format!("P12 one-shot gate: {reason}").into());
                    }
                }
            }
            if p12_observing() {
                // Each hold covers one local case, because the client fails the
                // link once a request is 60 s old: Remote then Rearm. A third
                // request remains observable (and fails the oracle); no dead gate wait.
                held += 1;
                if held == P12_HELD_REQUESTS {
                    gate = None;
                }
            }
            // Issue #143: a rename the host refuses, or never answers.
            if crate::mock_commands::is_rename(&request) && fault == "rename-error" {
                write_message(
                    &mut server,
                    &ServerMessage::ClientShellEndpointResponseChunk {
                        boot_id: "boot-v1".into(),
                        request_id: id.clone(),
                        final_chunk: true,
                        data: serde_json::to_vec(
                            &serde_json::json!({"id": id, "error": "workspace not found"}),
                        )?,
                    },
                    MAX_FRAME_SIZE,
                )?;
                continue;
            }
            if crate::mock_commands::is_rename(&request) && fault == "rename-silent" {
                continue;
            }
            if crate::mock_commands::apply(&mut board_snapshot, &request) {
                write_message(
                    &mut server,
                    &ServerMessage::EndpointControl {
                        kind: ENDPOINT_SNAPSHOT_KIND.into(),
                        data: serde_json::to_string(&board_snapshot)?,
                    },
                    MAX_FRAME_SIZE,
                )?;
                write_message(
                    &mut server,
                    &ServerMessage::PaneSurface(focused_surface(&board_snapshot, &mut serial)),
                    MAX_FRAME_SIZE,
                )?;
                write_message(
                    &mut server,
                    &ServerMessage::ClientShellEndpointResponseChunk {
                        boot_id: "boot-v1".into(),
                        request_id: id.clone(),
                        final_chunk: true,
                        data: serde_json::to_vec(&serde_json::json!({"id":id,"result":{}}))?,
                    },
                    MAX_FRAME_SIZE,
                )?;
                continue;
            }
            write_message(
                &mut server,
                &ServerMessage::ClientShellEndpointResponseChunk {
                    boot_id: if fault == "stale" {
                        "stale-boot"
                    } else {
                        "boot-v1"
                    }
                    .into(),
                    request_id: id.clone(),
                    final_chunk: true,
                    data: serde_json::to_vec(&serde_json::json!({"id": id,"result":{}}))?,
                },
                MAX_FRAME_SIZE,
            )?;
            if fault == "response-first"
                && let Some(gate) = &gate
            {
                let _ = gate.recv_timeout(Duration::from_secs(10));
            }
            let method = request["method"].as_str().unwrap_or_default();
            let pane = if method == "workspace.focus" {
                if request["params"]["workspace_id"] == "w2" {
                    "w2:p2"
                } else {
                    "w1:p1"
                }
            } else {
                request["params"]["pane_id"].as_str().unwrap_or("w1:p1")
            };
            let pane = if fault == "ignore" {
                "w1:p1"
            } else if fault == "wrong" {
                "w1:p3"
            } else {
                pane
            };
            if fault == "stale" {
                board_snapshot.boot_id = "stale-boot".into();
            }
            let workspace = if pane == "w2:p2" { "w2" } else { "w1" };
            board_snapshot.focused_workspace_id = Some(workspace.into());
            board_snapshot.focused_tab_id = Some(format!("{workspace}:t1"));
            board_snapshot.focused_pane_id = Some(pane.into());
            for p in &mut board_snapshot.panes {
                p.focused = p.pane_id == pane;
            }
            for w in &mut board_snapshot.workspaces {
                w.focused = w.workspace_id == workspace;
            }
            for t in &mut board_snapshot.tabs {
                t.focused = Some(&t.tab_id) == board_snapshot.focused_tab_id.as_ref();
            }
            for a in &mut board_snapshot.agents {
                a.focused = a.pane_id == pane;
            }
            board_snapshot.revision += 1;
            write_message(
                &mut server,
                &ServerMessage::EndpointControl {
                    kind: ENDPOINT_SNAPSHOT_KIND.into(),
                    data: serde_json::to_string(&board_snapshot)?,
                },
                MAX_FRAME_SIZE,
            )?;
            write_message(
                &mut server,
                &ServerMessage::PaneSurface(focused_surface(&board_snapshot, &mut serial)),
                MAX_FRAME_SIZE,
            )?;
        }
        #[cfg(all(target_os = "ios", target_abi = "sim"))]
        if crate::mock_notifications::respond(&mut server, &message)? {
            return Ok(());
        }
        if let ClientMessage::ClientShellResize { surface_size, .. } = message {
            // The tests' link fixture is the frame they show; a resized
            // frame racing a press would change it under the press.
            if fault == "links" {
                continue;
            }
            serial += 1;
            let mut frame = focused_surface(&board_snapshot, &mut serial);
            frame.surface_revision = serial;
            resize_frame(&mut frame.frame, surface_size.cols, surface_size.rows);
            #[cfg(all(target_os = "ios", target_abi = "sim"))]
            if std::env::var_os(split::ENV).is_some() {
                frame.panes = split::herdr1_frame(&mut frame.frame);
            }
            size = Some((surface_size.cols, surface_size.rows));
            // L66: the link fixture again at the new size, so its wrapped
            // address still reaches the right edge.
            if links {
                crate::terminal_links::qa::herdr1_fixture(&mut frame.frame);
            }
            write_message(
                &mut server,
                &ServerMessage::PaneSurface(frame),
                MAX_FRAME_SIZE,
            )?;
        }
    }
}
/// The frame at `cols` x `rows`, its cells kept where they fit.
fn resize_frame(frame: &mut FrameData, cols: u16, rows: u16) {
    let old = frame.clone();
    frame.width = cols;
    frame.height = rows;
    let blank = CellData {
        symbol: " ".into(),
        fg: 0,
        bg: 0,
        modifier: 0,
        skip: false,
        hyperlink: None,
    };
    frame.cells = vec![blank; usize::from(cols) * usize::from(rows)];
    for y in 0..old.height.min(rows) {
        for x in 0..old.width.min(cols) {
            frame.cells[usize::from(y) * usize::from(cols) + usize::from(x)] =
                old.cells[usize::from(y) * usize::from(old.width) + usize::from(x)].clone();
        }
    }
}
pub(crate) fn connect() -> crate::Result<Client> {
    connect_burst(1)
}
#[cfg(test)]
pub(crate) fn connect_quiet() -> crate::Result<(Client, std::sync::mpsc::Receiver<ClientMessage>)> {
    connect_mode(1, false, "quiet")
}

pub(crate) fn connect_burst(frames: u64) -> crate::Result<Client> {
    connect_recorded(frames).map(|(client, _)| client)
}
pub(crate) fn connect_recorded(
    frames: u64,
) -> crate::Result<(Client, std::sync::mpsc::Receiver<ClientMessage>)> {
    connect_mode(frames, false, "")
}
/// L66: a peer whose frames show the link fixture after a resize, as the
/// Simulator's `YEBISU_URL_TAP_QA` peer does.
#[cfg(test)]
pub(crate) fn connect_links() -> crate::Result<(Client, std::sync::mpsc::Receiver<ClientMessage>)> {
    connect_mode(1, false, "links")
}
pub(crate) fn connect_board() -> crate::Result<Client> {
    connect_mode(1, true, "").map(|(client, _)| client)
}
fn connect_mode(
    frames: u64,
    board: bool,
    fault: &'static str,
) -> crate::Result<(Client, std::sync::mpsc::Receiver<ClientMessage>)> {
    connect_gated(frames, board, fault, None)
}
fn connect_gated(
    frames: u64,
    board: bool,
    fault: &'static str,
    gate: Option<std::sync::mpsc::Receiver<()>>,
) -> crate::Result<(Client, std::sync::mpsc::Receiver<ClientMessage>)> {
    let (sender, receiver) = std::sync::mpsc::channel();
    let (local, server) = Stream::pair()?;
    std::thread::Builder::new()
        .name("yebisu-private-peer".into())
        .spawn(move || {
            static NEXT_TRANSPORT: std::sync::atomic::AtomicU64 =
                std::sync::atomic::AtomicU64::new(1);
            let transport = NEXT_TRANSPORT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if p12_observing()
                && let Err(error) =
                    record_peer(serde_json::json!({"kind":"p12-peer-start", "transport":transport}))
            {
                eprintln!("P12 peer observation failed: {error}");
                return;
            }
            let result = serve(server, frames, sender, board, fault, gate, transport);
            if p12_observing()
                && let Err(error) = record_peer(
                    serde_json::json!({"kind":"p12-peer-exit", "transport":transport,
                    "reason":result.as_ref().err().map(ToString::to_string)}),
                )
            {
                eprintln!("P12 peer exit observation failed: {error}");
            }
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

fn next_surface(revision: u64, serial: &mut u64) -> PaneSurfaceFrame {
    *serial += 1;
    let mut frame = surface(revision);
    frame.surface_revision = *serial;
    frame
}

/// Issue #143 S7 (Simulator QA, `YEBISU_RENAME_QA_INSERT`): 0 until the
/// rename field is open over an agent row (shell/rename/qa.rs asks), 1 once
/// asked, 2 once this peer took it and sent a snapshot with an agent above.
static AGENT_INSERT: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Whether this peer inserts an agent for S7 (Simulator QA only).
fn agent_insert_enabled() -> bool {
    cfg!(all(
        target_os = "ios",
        target_abi = "sim",
        feature = "test-support"
    )) && std::env::var_os("YEBISU_RENAME_QA_INSERT").is_some()
}

/// The rename QA's side: the field is open over an agent row.
#[cfg_attr(
    not(all(target_os = "ios", target_abi = "sim", feature = "test-support")),
    allow(
        dead_code,
        reason = "only the Simulator QA build publishes the rename state"
    )
)]
pub(crate) fn request_agent_insert() {
    if agent_insert_enabled() {
        let _ = AGENT_INSERT.compare_exchange(
            0,
            1,
            std::sync::atomic::Ordering::AcqRel,
            std::sync::atomic::Ordering::Acquire,
        );
    }
}

/// True once, after the field opened over an agent row.
fn take_agent_insert() -> bool {
    AGENT_INSERT
        .compare_exchange(
            1,
            2,
            std::sync::atomic::Ordering::AcqRel,
            std::sync::atomic::Ordering::Acquire,
        )
        .is_ok()
}

/// S7: a new agent on the snapshot's third pane (`w1:p3`), received last, so
/// the sidebar lists it above the focused pane's agent.
fn insert_agent_above(snapshot: &mut ClientShellSnapshot) {
    let Some(mut agent) = snapshot.agents.first().cloned() else {
        return;
    };
    agent.pane_id = "w1:p3".into();
    agent.name = Some("inserted".into());
    agent.state_change_seq += 100;
    agent.focused = false;
    snapshot.agents.insert(0, agent);
    snapshot.revision += 1;
}

fn initial_snapshot() -> Result<String, serde_json::Error> {
    let mut value: serde_json::Value = serde_json::from_str(SNAPSHOT)?;
    let mut second: serde_json::Value = serde_json::from_str(
        &SNAPSHOT
            .replace("w1", "w2")
            .replace("p1", "p2")
            .replace("repo", "second")
            .replace("reviewer", "second-agent"),
    )?;
    second["workspaces"][0]["number"] = 2.into();
    for key in ["workspaces", "tabs", "panes", "agents"] {
        if let (Some(dst), Some(src)) = (value[key].as_array_mut(), second[key].as_array()) {
            dst.extend(src.iter().cloned().map(|mut item| {
                item["focused"] = false.into();
                item
            }));
        }
    }
    let mut third = value["panes"][0].clone();
    third["pane_id"] = "w1:p3".into();
    third["focused"] = false.into();
    if let Some(panes) = value["panes"].as_array_mut() {
        panes.push(third);
    }
    serde_json::to_string(&value)
}
fn focused_surface(snapshot: &ClientShellSnapshot, serial: &mut u64) -> PaneSurfaceFrame {
    let mut frame = next_surface(snapshot.revision, serial);
    frame.boot_id.clone_from(&snapshot.boot_id);
    // A target-specific marker proves the surface changed along with metadata.
    if let Some(cell) = frame.frame.cells.first_mut() {
        cell.symbol = if snapshot.focused_pane_id.as_deref() == Some("w2:p2") {
            "2"
        } else {
            "1"
        }
        .into();
    }
    if cfg!(target_abi = "sim")
        && std::env::var("YEBISU_SH1_152_CASE").is_ok_and(|c| c.starts_with("E5"))
        && snapshot.focused_pane_id.as_deref() != Some("w1:p1")
    {
        for (offset, ch) in "BBB-152".chars().enumerate() {
            if let Some(cell) = frame
                .frame
                .cells
                .get_mut(usize::from(frame.frame.width) * 2 + 1 + offset)
            {
                cell.symbol = ch.to_string();
                cell.skip = false;
            }
        }
    }
    frame
}

/// A recorded peer with `fault` (issue #143: `rename-error`, `rename-silent`).
#[cfg(test)]
pub(crate) fn connect_recorded_fault(
    fault: &'static str,
) -> crate::Result<(Client, std::sync::mpsc::Receiver<ClientMessage>)> {
    connect_mode(1, false, fault)
}

#[cfg(test)]
pub(crate) fn connect_fault(fault: &'static str) -> crate::Result<Client> {
    connect_mode(1, false, fault).map(|(client, _)| client)
}

/// Remote (held for Sidebar) and Rearm (held for Palette).
const P12_HELD_REQUESTS: u32 = 2;
fn p12_observing() -> bool {
    cfg!(all(target_os = "ios", target_abi = "sim")) && std::env::var_os("YEBISU_P12_QA").is_some()
}
/// The variant name of a client message for the P12 trace. The message is
/// untrusted daemon data, so only serde's externally tagged variant name is
/// kept, and only when it is a plain identifier.
fn client_message_kind(message: &ClientMessage) -> String {
    serde_json::to_value(message).map_or_else(|_| UNCLASSIFIED.to_owned(), message_kind)
}
const UNCLASSIFIED: &str = "unclassified";
/// The variant name in a serialized message: a unit variant's string, or the
/// one key of a data variant's object.
fn message_kind(value: serde_json::Value) -> String {
    let name = match value {
        serde_json::Value::String(name) => Some(name),
        serde_json::Value::Object(object) if object.len() == 1 => object.keys().next().cloned(),
        _ => None,
    };
    name.filter(|name| {
        let mut chars = name.chars();
        name.len() <= 64
            && chars
                .next()
                .is_some_and(|first| first.is_ascii_alphabetic())
            && chars.all(|rest| rest.is_ascii_alphanumeric())
    })
    .unwrap_or_else(|| UNCLASSIFIED.to_owned())
}
fn record(
    message: &ClientMessage,
    transport: u64,
    sequence: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    if p12_observing() {
        // No raw keys, pasted text, RPC params or secrets in the P12 trace.
        let safe = match message {
            ClientMessage::ClientShellEndpointRequest { request, .. } => {
                let value: serde_json::Value = serde_json::from_str(request).unwrap_or_default();
                let method = match value["method"].as_str() {
                    Some("pane.split") => "pane.split",
                    _ => "other",
                };
                serde_json::json!({"request_method":method})
            }
            ClientMessage::ClientShellPaneInput { events, .. } => {
                let events = events
                    .iter()
                    .map(|event| match event {
                        ClientPaneInputEvent::Paste(text) if text == "p12-fence" => "fifo_fence",
                        _ => "unexpected_input",
                    })
                    .collect::<Vec<_>>();
                serde_json::json!({"input":events})
            }
            ClientMessage::ClientShellResize { .. } | ClientMessage::ClientShellFocus { .. } => {
                serde_json::json!({"geometry":true})
            }
            // The variant name only (class P of issue #142): no payload.
            other => serde_json::json!({"other_message": true,
                "message_kind": client_message_kind(other)}),
        };
        // qa.state's `peer` (UI test redesign S5): P12 waits for its FIFO
        // fence here, inside the Finish step. Only the fence text is passed,
        // as the trace above keeps no other pasted text.
        #[cfg(any(all(target_os = "ios", target_abi = "sim"), test))]
        if let ClientMessage::ClientShellPaneInput { events, .. } = message {
            crate::qa_ax::peer::input(events.iter().filter_map(|event| match event {
                ClientPaneInputEvent::Paste(text) if text == "p12-fence" => Some(text.as_str()),
                _ => None,
            }));
        }
        return record_peer(
            serde_json::json!({"kind":"p12-peer-ingress", "transport":transport,
            "sequence":sequence,"message":safe}),
        );
    }
    record_peer(serde_json::to_value(message)?)?;
    // qa.state's `peer` (UI test redesign S4): the edit-menu tests wait for
    // a marker or a pasted text here instead of reading p4-peer.jsonl.
    #[cfg(any(all(target_os = "ios", target_abi = "sim"), test))]
    if let ClientMessage::ClientShellPaneInput { events, .. } = message {
        crate::qa_ax::peer::input(events.iter().filter_map(|event| match event {
            ClientPaneInputEvent::Paste(text) => Some(text.as_str()),
            _ => None,
        }));
    }
    #[cfg(all(target_os = "ios", target_abi = "sim", feature = "test-support"))]
    if std::env::var_os("YEBISU_TERMINAL_QA").is_some() {
        observer_received(message);
    }
    Ok(())
}
fn record_peer(mut value: serde_json::Value) -> Result<(), Box<dyn std::error::Error>> {
    if p12_observing() {
        value["time"] = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs_f64()
            .into();
    }
    #[cfg(all(target_os = "ios", target_abi = "sim"))]
    if std::env::var_os("YEBISU_TERMINAL_QA").is_some() {
        let home = std::env::var_os("HOME").ok_or("QA HOME missing")?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(std::path::PathBuf::from(home).join("Documents/p4-peer.jsonl"))?;
        crate::observation::write_jsonl_line(&mut file, &value)?;
    }
    let _ = value;
    Ok(())
}

#[cfg(test)]
pub(crate) fn connect_controlled() -> crate::Result<(
    Client,
    std::sync::mpsc::Receiver<ClientMessage>,
    std::sync::mpsc::Sender<()>,
)> {
    let (release, gate) = std::sync::mpsc::channel();
    connect_gated(1, false, "", Some(gate)).map(|(client, receipts)| (client, receipts, release))
}

#[cfg(test)]
pub(crate) fn connect_response_first() -> crate::Result<(
    Client,
    std::sync::mpsc::Receiver<ClientMessage>,
    std::sync::mpsc::Sender<()>,
)> {
    let (release, gate) = std::sync::mpsc::channel();
    connect_gated(1, false, "response-first", Some(gate))
        .map(|(client, receipts)| (client, receipts, release))
}

fn handoff_graphics() -> SurfaceGraphicsScene {
    #[cfg(all(target_os = "ios", target_abi = "sim"))]
    if std::env::var_os("YEBISU_HANDOFF_QA").is_some() {
        let key = SurfaceGraphicsAssetKey {
            source: SurfaceGraphicsSource::Terminal {
                target: SurfaceGraphicsTarget::Pane {
                    pane_id: "w1:p1".into(),
                },
                image_id: 1,
            },
            image_width: 8,
            image_height: 8,
            format: SurfaceGraphicsFormat::Rgba,
            data_len: 256,
            data_fingerprint: 1,
        };
        return SurfaceGraphicsScene {
            assets: vec![SurfaceGraphicsAsset {
                key: key.clone(),
                data: [255, 0, 170, 255].repeat(64),
            }],
            placements: vec![SurfaceGraphicsPlacement {
                asset: key,
                logical_placement_id: 1,
                x: 30,
                y: 2,
                cols: 8,
                rows: 3,
                source_x: 0,
                source_y: 0,
                source_width: 0,
                source_height: 0,
                x_offset: 0,
                y_offset: 0,
                z: 1,
                scrollback_offset: 0,
            }],
            retained_assets: vec![],
        };
    }
    SurfaceGraphicsScene::default()
}

#[cfg(all(target_os = "ios", target_abi = "sim"))]
pub(crate) fn connect_p12() -> crate::Result<(Client, std::sync::mpsc::SyncSender<()>)> {
    let (release, gate) = std::sync::mpsc::sync_channel(1);
    connect_gated(1, false, "", Some(gate)).map(|(client, _)| (client, release))
}

#[cfg(all(target_os = "ios", target_abi = "sim", feature = "test-support"))]
static OBSERVER_ACK: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
#[cfg(all(target_os = "ios", target_abi = "sim", feature = "test-support"))]
fn observer_received(message: &ClientMessage) {
    if let ClientMessage::ClientShellPaneInput {
        pane_id, events, ..
    } = message
    {
        if pane_id != "w1:p1" {
            return;
        }
        for event in events {
            if let ClientPaneInputEvent::Paste(text) = event
                && let Some(n) = text
                    .strip_prefix("p11-observer-end-")
                    .and_then(|s| s.parse::<u32>().ok())
                    .or_else(|| {
                        text.strip_prefix("native-end-")
                            .and_then(|s| s.parse::<u32>().ok())
                            .and_then(|n| n.checked_add(1))
                    })
            {
                OBSERVER_ACK.store(n, std::sync::atomic::Ordering::Release);
            }
        }
    }
}
#[cfg(all(target_os = "ios", target_abi = "sim", feature = "test-support"))]
pub(crate) fn observer_ack(phase: u32) -> bool {
    OBSERVER_ACK.load(std::sync::atomic::Ordering::Acquire) == phase
}

#[cfg(test)]
#[path = "mock_peer_tests.rs"]
mod tests;
