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
