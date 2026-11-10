use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

type TestResult = Result<(), Box<dyn std::error::Error>>;
struct Temporary(PathBuf);
impl Temporary {
    fn new() -> io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/log-tests")
            .join(format!(
                "{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        std::fs::create_dir_all(&path)?;
        Ok(Self(path))
    }
}
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn privacy(secret: &'static str) -> TestResult {
    let temporary = Temporary::new()?;
    let path = temporary.0.join(LOG_LOCATION.trim());
    let (sender, receiver) = sync_channel(QUEUE);
    let destination = path.clone();
    let worker = std::thread::spawn(move || write_loop(&destination, receiver));
    tracing::subscriber::with_default(Capture(sender), || {
        record("startup");
        record("connected");
        let mut input = crate::input::Input::default();
        input.replace(None, secret, false, None);
        assert_eq!(input.text, secret);
        let mut live = crate::live::Live::default();
        let mut surface = crate::mock_peer::surface(1);
        surface.frame.cells[0].symbol = secret.into();
        live.apply(sheprd_client::ClientEvent::Surface(surface.into()));
        live.apply(sheprd_client::ClientEvent::Disconnected {
            reason: secret.into(),
            ssh: None,
        });
        // Test both dependency targets and hostile fields on an accepted target.
        tracing::info!(target: "terminal", terminal_output = secret, typed_key = secret, "{secret}");
        tracing::info!(target: "yebisu::diagnostics", code = "connected", terminal_output = secret, typed_key = secret);
        tracing::error!(target: "yebisu::diagnostics", code = secret, "{secret}");
    });
    worker
        .join()
        .map_err(|_| io::Error::other("writer panicked"))??;
    let content = std::fs::read_to_string(path)?;
    println!("privacy receiving log: {content}");
    assert!(content.contains("startup"));
    assert!(content.contains("connected"));
    assert!(content.contains("disconnected"));
    assert!(!content.contains(secret), "sensitive payload reached file");
    for line in content.lines() {
        let value: serde_json::Value = serde_json::from_str(line)?;
        let keys = value
            .as_object()
            .ok_or_else(|| io::Error::other("not object"))?;
        assert_eq!(keys.len(), 3);
        assert!(value["time"].is_number());
        assert!(value["pid"].is_number());
    }
    Ok(())
}

#[test]
fn terminal_output_never_reaches_log() -> TestResult {
    privacy("TERMINAL_OUTPUT_SECRET")
}
#[test]
fn typed_key_never_reaches_log() -> TestResult {
    privacy("TYPED_KEY_SECRET")
}
#[test]
fn size_cap_and_single_rotation_hold() -> TestResult {
    let temporary = Temporary::new()?;
    let path = temporary.0.join(LOG_LOCATION.trim());
    let mut writer = Writer::open(&path)?;
    for _ in 0..10_000 {
        writer.line("connected")?;
    }
    drop(writer);
    let previous = Writer::previous(&path);
    println!(
        "size receiving current={} previous={:?} cap={MAX_BYTES}",
        path.metadata()?.len(),
        previous.metadata().map(|entry| entry.len()).ok()
    );
    assert!(path.metadata()?.len() <= MAX_BYTES);
    assert!(previous.metadata()?.len() <= MAX_BYTES);
    assert_eq!(
        std::fs::read_dir(path.parent().ok_or_else(|| io::Error::other("no parent"))?)?.count(),
        2
    );
    // Restart with oversized retained files must enforce the same limits.
    std::fs::write(&previous, vec![b'x'; MAX_BYTES as usize + 1])?;
    std::fs::write(&path, vec![b'x'; MAX_BYTES as usize + 1])?;
    let mut writer = Writer::open(&path)?;
    writer.line("startup")?;
    drop(writer);
    assert!(path.metadata()?.len() <= MAX_BYTES);
    assert!(!previous.exists());
    Ok(())
}
#[test]
fn full_queue_drops_without_waiting_and_unknown_codes_are_ignored() {
    let (sender, receiver) = sync_channel(1);
    tracing::subscriber::with_default(Capture(sender), || {
        for _ in 0..10_000 {
            record("startup");
        }
        record("password");
    });
    assert_eq!(receiver.try_iter().collect::<Vec<_>>(), ["startup"]);
}
