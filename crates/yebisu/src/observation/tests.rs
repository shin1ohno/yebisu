use super::*;
fn clean(output: &str) -> bool {
    !output.contains("FAKE_PUBLIC_KEY_SENTINEL") && !output.contains("FAKE_PRIVATE_SENTINEL")
}
#[test]
fn f24_actual_observation_sinks_and_queue_diagnostics_have_negative_control() {
    let (send, records) = sync_channel(1);
    let (diagnostics, errors) = sync_channel(4);
    let mut observation = Observation::injected(send);
    observation.capture_diagnostics(diagnostics);
    observation.record(serde_json::json!({"kind":"nonsecret-positive-control"}));
    // A dropped payload must not be formatted into the diagnostic.
    observation.record(
        serde_json::json!({"private":"FAKE_PRIVATE_SENTINEL","key":"FAKE_PUBLIC_KEY_SENTINEL"}),
    );
    let record = records
        .recv_timeout(std::time::Duration::from_secs(1))
        .unwrap_or_else(|e| panic!("{e}"));
    let mut jsonl = Vec::new();
    writeln!(&mut jsonl, "{record}").unwrap_or_else(|e| panic!("{e}"));
    drop(records);
    observation.record(
        serde_json::json!({"private":"FAKE_PRIVATE_SENTINEL","key":"FAKE_PUBLIC_KEY_SENTINEL"}),
    );
    let output = String::from_utf8(jsonl).unwrap_or_else(|e| panic!("{e}"));
    assert!(output.contains("nonsecret-positive-control"));
    assert!(clean(&output));
    let diagnostics = errors.try_iter().collect::<Vec<_>>();
    assert_eq!(
        diagnostics,
        vec![
            "smoke observation dropped: full",
            "smoke observation dropped: disconnected"
        ]
    );
    assert!(clean(&diagnostics.join("\n")));
    assert!(!clean("FAKE_PUBLIC_KEY_SENTINEL"));
    assert!(!clean("FAKE_PRIVATE_SENTINEL"));
}

/// Integration 2 (run 44236636053): every record reaches the file in one
/// `write` call, so writers that append to the same file never split a line.
#[test]
fn each_record_is_one_write_of_one_whole_line() {
    struct Calls(Vec<Vec<u8>>);
    impl Write for Calls {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.push(buf.to_vec());
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut calls = Calls(Vec::new());
    let value = serde_json::json!({"kind": "launch", "phase": "P2", "time": 1.5});
    write_line(&mut calls, &value).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(calls.0.len(), 1, "one write per record: {:?}", calls.0);
    let line = String::from_utf8(calls.0.remove(0)).unwrap_or_else(|e| panic!("{e}"));
    assert!(line.ends_with('\n'));
    let parsed: Value = serde_json::from_str(line.trim_end()).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(parsed, value);
}

/// Two writers appending to one file, as two observation sinks of the same
/// QA run do: every complete line still parses.
#[test]
fn concurrent_appenders_never_tear_a_line() {
    let path = std::env::temp_dir().join(format!(
        "yebisu-observation-{}-{:?}.jsonl",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_file(&path);
    let writers: Vec<_> = (0..2)
        .map(|writer| {
            let path = path.clone();
            std::thread::spawn(move || {
                let mut file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)
                    .unwrap_or_else(|e| panic!("{e}"));
                for index in 0..2000 {
                    let value = serde_json::json!({"kind": "launch", "writer": writer, "index": index, "phase": "P2"});
                    write_line(&mut file, &value).unwrap_or_else(|e| panic!("{e}"));
                }
            })
        })
        .collect();
    for writer in writers {
        writer.join().unwrap_or_else(|_| panic!("writer panicked"));
    }
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{e}"));
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 4000);
    for line in lines {
        serde_json::from_str::<Value>(line).unwrap_or_else(|e| panic!("torn line {line:?}: {e}"));
    }
    let _ = std::fs::remove_file(&path);
}
