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

#[test]
fn f24_child_panic_sink_probe() -> TestResult {
    if std::env::var_os("YEBISU_F24_CHILD").is_none() {
        return Ok(());
    }
    let path = PathBuf::from(std::env::var_os("YEBISU_F24_LOG").ok_or("missing test path")?);
    let (send, receive) = sync_channel(4);
    let destination = path.clone();
    let worker = std::thread::spawn(move || write_loop(&destination, receive));
    let (observation, observation_worker) =
        crate::observation::Observation::file_for_test(path.with_extension("observation.jsonl"));
    observation.record(serde_json::json!({"kind":"f24-observation-positive","key_ready":true}));
    drop(observation);
    observation_worker
        .join()
        .map_err(|_| "observation worker panic")??;
    let (queued, messages) = sync_channel(1);
    let observation = crate::observation::Observation::injected(queued);
    observation.record(serde_json::json!({"kind":"queue-positive"}));
    observation.record(serde_json::json!({"raw":"FAKE_PRIVATE_SENTINEL FAKE_PUBLIC_KEY_SENTINEL"}));
    assert!(messages.recv()?.to_string().contains("queue-positive"));
    drop(messages);
    observation.record(serde_json::json!({"raw":"FAKE_PRIVATE_SENTINEL FAKE_PUBLIC_KEY_SENTINEL"}));
    install_panic_hook();
    tracing::subscriber::with_default(Capture(send), || {
        record("startup");
        let _ = std::panic::catch_unwind(|| {
            panic!("FAKE_PRIVATE_SENTINEL RkFLRV9QVUJMSUNfS0VZX1NFTlRJTkVM")
        });
    });
    worker.join().map_err(|_| "writer panic")??;
    println!("f24-child-positive");
    Ok(())
}

#[test]
fn f24_aggregate_actual_process_stderr_and_diagnostic_file() -> TestResult {
    let temporary = Temporary::new()?;
    let path = temporary.0.join("diagnostic.jsonl");
    let child = std::process::Command::new(std::env::current_exe()?)
        .args([
            "--exact",
            "diagnostics::tests::f24_child_panic_sink_probe",
            "--nocapture",
        ])
        .env("YEBISU_F24_CHILD", "1")
        .env("YEBISU_F24_LOG", &path)
        .output()?;
    assert!(child.status.success());
    let stdout = String::from_utf8(child.stdout)?;
    let stderr = String::from_utf8(child.stderr)?;
    let observation = std::fs::read_to_string(path.with_extension("observation.jsonl"))?;
    let diagnostic = std::fs::read_to_string(path)?;
    assert!(stdout.contains("f24-child-positive"));
    assert!(stderr.contains("yebisu panic: payload suppressed"));
    assert!(stderr.contains("smoke observation dropped: full"));
    assert!(stderr.contains("smoke observation dropped: disconnected"));
    assert!(observation.contains("f24-observation-positive"));
    assert!(diagnostic.contains("startup") && diagnostic.contains("panic"));
    let clean = |s: &str| {
        ![
            "FAKE_PRIVATE_SENTINEL",
            "FAKE_PUBLIC_KEY_SENTINEL",
            "RkFLRV9QVUJMSUNfS0VZX1NFTlRJTkVM",
        ]
        .iter()
        .any(|v| s.contains(v))
    };
    let aggregate = format!("{stdout}\n{stderr}\n{diagnostic}\n{observation}");
    assert!(clean(&aggregate));
    for secret in [
        "FAKE_PRIVATE_SENTINEL",
        "FAKE_PUBLIC_KEY_SENTINEL",
        "RkFLRV9QVUJMSUNfS0VZX1NFTlRJTkVM",
    ] {
        assert!(
            !clean(&format!("{aggregate}{secret}")),
            "same detector negative control"
        );
    }
    Ok(())
}
