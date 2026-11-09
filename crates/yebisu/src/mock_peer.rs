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
        "日本語 かな カナ 漢字",
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
                cells[(row + 1) * width + col + 1].skip = true;
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
        graphics: SurfaceGraphicsScene::default(),
    }
}
fn serve(
    mut server: Stream,
    frames: u64,
    receipts: std::sync::mpsc::Sender<ClientMessage>,
) -> Result<(), Box<dyn std::error::Error>> {
    server.set_read_timeout(Some(Duration::from_secs(300)))?;
    let _: ClientMessage = read_message(&mut server, MAX_FRAME_SIZE)?;
    let mut welcome: serde_json::Value = serde_json::from_str(WELCOME)?;
    welcome["server_version"] = sheprd_client::MIN_HERDR_VERSION.into();
    welcome["methods"] = serde_json::json!(["client_shell.surface.set"]);
    welcome["capabilities"] = serde_json::json!(["surface_interest", "presentation_effects_fence"]);
    let welcome = welcome.to_string();
    for (kind, data) in [
        (ENDPOINT_WELCOME_KIND, welcome.as_str()),
        (ENDPOINT_SNAPSHOT_KIND, SNAPSHOT),
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
    loop {
        // Consume geometry updates from the real client; no external socket or host.
        let message: ClientMessage = read_message(&mut server, MAX_FRAME_SIZE)?;
        let _ = receipts.send(message.clone());
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
        if let ClientMessage::ClientShellResize { surface_size, .. } = message {
            serial += 1;
            let mut frame = surface(7);
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
    let (sender, receiver) = std::sync::mpsc::channel();
    let (local, server) = Stream::pair()?;
    std::thread::Builder::new()
        .name("yebisu-private-peer".into())
        .spawn(move || {
            let _ = serve(server, frames, sender);
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
