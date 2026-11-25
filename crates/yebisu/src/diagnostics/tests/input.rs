//! The keyboard input diagnostics (Mac hardware-keyboard input loss): the
//! herdr-uikit target and yebisu's text gate reach the log as static codes
//! only, never a key, text, host or terminal payload.
use super::*;

const SECRET: &str = "TYPED_KEY_AND_TERMINAL_SECRET";

fn captured(emit: impl FnOnce()) -> Vec<&'static str> {
    let (sender, receiver) = sync_channel(QUEUE);
    tracing::subscriber::with_default(
        Capture {
            codes: sender,
            receipts: false,
        },
        emit,
    );
    receiver.try_iter().collect()
}

#[test]
fn herdr_uikit_and_text_gate_codes_are_recorded_and_nothing_else() {
    let codes = captured(|| {
        tracing::info!(target: "herdr_uikit::diagnostics", code = "responder_drawing_unfocused");
        // Hostile fields on an accepted target are ignored; the code stays.
        tracing::info!(target: "herdr_uikit::diagnostics", code = "dismissal_application", key = SECRET, "{SECRET}");
        // A payload or an unknown name in the code field is no code.
        tracing::info!(target: "herdr_uikit::diagnostics", code = SECRET);
        tracing::info!(target: "herdr_uikit::diagnostics", code = "responder_drawing_password");
        // Other herdr-uikit targets stay out.
        tracing::info!(target: "herdr_uikit::window", code = "responder_text");
        crate::text_gate::Recorder::default().note(crate::text_gate::Gate::Off(
            crate::text_gate::Reason::Destination,
        ));
    });
    assert_eq!(
        codes,
        [
            "responder_drawing_unfocused",
            "dismissal_application",
            "text_off_destination"
        ]
    );
}

#[test]
fn the_log_file_holds_only_time_pid_and_the_static_code() -> TestResult {
    let temporary = Temporary::new()?;
    let path = temporary.0.join(LOG_LOCATION.trim());
    let (sender, receiver) = sync_channel(QUEUE);
    let destination = path.clone();
    let worker = std::thread::spawn(move || write_loop(&destination, receiver));
    tracing::subscriber::with_default(
        Capture {
            codes: sender,
            receipts: false,
        },
        || {
            tracing::info!(target: "herdr_uikit::diagnostics", code = "press_recovered_text", typed = SECRET, host = SECRET);
            record("text_on");
        },
    );
    worker
        .join()
        .map_err(|_| io::Error::other("writer panicked"))??;
    let content = std::fs::read_to_string(path)?;
    assert!(!content.contains(SECRET), "payload reached the file");
    let events: Vec<String> = content
        .lines()
        .map(|line| {
            let value: serde_json::Value = serde_json::from_str(line)?;
            let keys = value.as_object().map_or(0, serde_json::Map::len);
            assert_eq!(keys, 3, "{line}");
            Ok(value["event"].as_str().unwrap_or_default().to_owned())
        })
        .collect::<Result<_, serde_json::Error>>()?;
    assert_eq!(events, ["press_recovered_text", "text_on"]);
    Ok(())
}

#[test]
fn the_herdr_uikit_target_is_accepted_without_receipts() {
    assert!(accepts(UIKIT_TARGET, false));
    assert_eq!(UIKIT_TARGET, "herdr_uikit::diagnostics");
}

#[test]
fn code_lists_are_static_names_and_never_collide() {
    let mut all: Vec<&str> = CODES
        .iter()
        .chain(UIKIT_CODES)
        .chain(crate::text_gate::CODES)
        .copied()
        .collect();
    for code in &all {
        assert!(
            code.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
            "{code}"
        );
    }
    let count = all.len();
    all.sort_unstable();
    all.dedup();
    assert_eq!(all.len(), count, "a code appears in two lists");
    let mut sorted = UIKIT_CODES.to_vec();
    sorted.sort_unstable();
    assert_eq!(
        sorted, UIKIT_CODES,
        "keep UIKIT_CODES in herdr-uikit's order"
    );
}

#[test]
fn the_ios_list_comparison_tells_lists_apart() {
    assert!(same_codes(UIKIT_CODES, UIKIT_CODES));
    assert!(!same_codes(UIKIT_CODES, &UIKIT_CODES[1..]));
    assert!(!same_codes(&["text_on"], &["text_of"]));
    assert!(same_bytes(b"herdr", b"herdr"));
    assert!(!same_bytes(b"herdr", b"herdx"));
}
