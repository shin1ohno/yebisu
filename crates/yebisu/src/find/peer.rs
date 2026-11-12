//! Controlled receiving peer for P13 adapter and Simulator evidence only.
//! Matches are derived from the same fixture text used to construct the frame.
use sheprd_client::{
    Client, ConnectOptions, ConnectTarget, Stream,
    protocol::{endpoint::*, *},
    scrollback::{CopySearchParams, CopySearchResult, SearchDirection, TextPoint, TextRange},
};
use std::{
    sync::mpsc::{Receiver, sync_channel},
    time::Duration,
};
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
pub(crate) fn connect() -> crate::Result<(Client, Receiver<ClientMessage>)> {
    connect_mode(None, false)
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
    connect_mode(Some(gate), wrong_boot).map(|(client, received)| (client, received, release))
}
fn connect_mode(
    mut gate: Option<Receiver<()>>,
    mut wrong_boot: bool,
) -> crate::Result<(Client, Receiver<ClientMessage>)> {
    let (local, mut server) = Stream::pair()?;
    let (receipts, receiver) = sync_channel(64);
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
                server.set_read_timeout(Some(Duration::from_secs(30)))?;
                let _: ClientMessage = read_message(&mut server, MAX_FRAME_SIZE)?;
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
                write_message(
                    &mut server,
                    &ServerMessage::PaneSurface(surface()),
                    MAX_FRAME_SIZE,
                )?;
                let mut serial = surface().surface_revision;
                let (mut cols, mut rows, mut offset) = (80, 24, 100);
                loop {
                    let message: ClientMessage = read_message(&mut server, MAX_FRAME_SIZE)?;
                    let _ = receipts.try_send(message.clone());
                    #[cfg(all(target_os = "ios", target_abi = "sim"))]
                    record(&message);
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
    use std::io::Write;
    if let Some(home) = std::env::var_os("HOME")
        && let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(std::path::PathBuf::from(home).join("Documents/p13-peer.jsonl"))
    {
        let _ = writeln!(
            file,
            "{}",
            serde_json::to_string(message).unwrap_or_default()
        );
    }
}
