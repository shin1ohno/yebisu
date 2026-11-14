//! Private in-process wire peer. Compiled only in tests and Simulator builds.
use sheprd_client::{
    Client, ConnectOptions, ConnectTarget, Stream,
    protocol::{endpoint::*, *},
};
use std::time::Duration;
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
    PaneSurfaceFrame {
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
    }
}
fn serve(
    mut server: Stream,
    frames: u64,
    receipts: std::sync::mpsc::Sender<ClientMessage>,
    board: bool,
    fault: &str,
    gate: Option<std::sync::mpsc::Receiver<()>>,
) -> Result<(), Box<dyn std::error::Error>> {
    server.set_read_timeout(Some(Duration::from_secs(300)))?;
    let _: ClientMessage = read_message(&mut server, MAX_FRAME_SIZE)?;
    let mut welcome: serde_json::Value = serde_json::from_str(WELCOME)?;
    welcome["server_version"] = sheprd_client::MIN_HERDR_VERSION.into();
    welcome["methods"] = serde_json::json!([
        "client_shell.surface.set",
        "pane.focus",
        "workspace.focus",
        "tab.focus",
        "tab.create",
        "pane.split",
        "pane.focus_direction",
        "pane.zoom",
        "pane.close",
        "tab.close"
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

    loop {
        // Consume geometry updates from the real client; no external socket or host.
        let message: ClientMessage = match read_message(&mut server, MAX_CLIPBOARD_IMAGE_FRAME_SIZE)
        {
            Ok(message) => message,
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
        let _ = receipts.send(message.clone());
        // Regression fixture: after the initial surface, geometry requests must
        // not produce updates that could hide a missing UI invalidation.
        if fault == "quiet" {
            record(&message)?;
            continue;
        }
        if let ClientMessage::ClientShellEndpointRequest { request, .. } = &message {
            let request: serde_json::Value = serde_json::from_str(request)?;
            let id = request["id"].as_str().unwrap_or_default().to_owned();
            if fault != "response-first"
                && let Some(gate) = &gate
            {
                let _ = gate.recv_timeout(Duration::from_secs(5));
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
                record(&message)?;
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
        record(&message)?;
        if let ClientMessage::ClientShellResize { surface_size, .. } = message {
            serial += 1;
            let mut frame = focused_surface(&board_snapshot, &mut serial);
            let old = frame.frame.clone();
            frame.surface_revision = serial;
            frame.frame.width = surface_size.cols;
            frame.frame.height = surface_size.rows;
            let blank = CellData {
                symbol: " ".into(),
                fg: 0,
                bg: 0,
                modifier: 0,
                skip: false,
                hyperlink: None,
            };
            frame.frame.cells =
                vec![blank; usize::from(surface_size.cols) * usize::from(surface_size.rows)];
            for y in 0..old.height.min(surface_size.rows) {
                for x in 0..old.width.min(surface_size.cols) {
                    frame.frame.cells
                        [usize::from(y) * usize::from(surface_size.cols) + usize::from(x)] =
                        old.cells[usize::from(y) * usize::from(old.width) + usize::from(x)].clone();
                }
            }
            write_message(
                &mut server,
                &ServerMessage::PaneSurface(frame),
                MAX_FRAME_SIZE,
            )?;
        }
    }
}
pub(crate) fn connect() -> crate::Result<Client> {
    connect_burst(1)
}
pub(crate) fn connect_burst(frames: u64) -> crate::Result<Client> {
    connect_recorded(frames).map(|(client, _)| client)
}
pub(crate) fn connect_recorded(
    frames: u64,
) -> crate::Result<(Client, std::sync::mpsc::Receiver<ClientMessage>)> {
    connect_mode(frames, false, "")
}
#[cfg(test)]
pub(crate) fn connect_quiet() -> crate::Result<(Client, std::sync::mpsc::Receiver<ClientMessage>)> {
    connect_mode(1, false, "quiet")
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
            let _ = serve(server, frames, sender, board, fault, gate);
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

#[cfg(test)]
pub(crate) fn connect_fault(fault: &'static str) -> crate::Result<Client> {
    connect_mode(1, false, fault).map(|(client, _)| client)
}

fn record(message: &ClientMessage) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(all(target_os = "ios", target_abi = "sim"))]
    if std::env::var_os("YEBISU_TERMINAL_QA").is_some() {
        use std::io::Write;
        if let Some(home) = std::env::var_os("HOME")
            && let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(std::path::PathBuf::from(home).join("Documents/p4-peer.jsonl"))
        {
            let _ = writeln!(file, "{}", serde_json::to_string(&message)?);
        }
    }
    let _ = message;
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
