//! Simulator-only wire fixture. Commands come only from the opt-in P14 controls.
use sheprd_client::{Stream, protocol::*};
use std::io::Write;

pub(crate) fn respond(
    server: &mut Stream,
    message: &ClientMessage,
) -> Result<bool, Box<dyn std::error::Error>> {
    if std::env::var_os("YEBISU_P14_QA").is_none() {
        return Ok(false);
    }
    let ClientMessage::ClientShellPaneInput { events, .. } = message else {
        return Ok(false);
    };
    for event in events {
        let ClientPaneInputEvent::Paste(command) = event else {
            continue;
        };
        let pane = match command.as_str() {
            "p14-live" => "w1:p3",
            "p14-quiet" | "p14-disconnect" => "w1:p1",
            "p14-invalid" => "missing-pane",
            _ => continue,
        };
        let notification = ServerMessage::SemanticNotification(SemanticNotification {
            kind: SemanticNotificationKind::NeedsAttention,
            title: "P14 fixture title must not be displayed".into(),
            body: Some("P14 fixture body must not be displayed".into()),
            sound: None,
            agent: None,
            workspace_id: None,
            tab_id: None,
            pane_id: Some(pane.into()),
            position: None,
        });
        write_message(server, &notification, MAX_FRAME_SIZE)?;
        // Primary sending peer receipt; not an app-produced delivery claim.
        let home = std::env::var_os("HOME").ok_or("missing fixture HOME")?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(std::path::PathBuf::from(home).join("Documents/p14-sent.jsonl"))?;
        writeln!(
            file,
            "{}",
            serde_json::json!({"command":command,"message":notification,"disconnect":command=="p14-disconnect"})
        )?;
        file.flush()?;
        if command == "p14-disconnect" {
            return Ok(true);
        }
    }
    Ok(false)
}
