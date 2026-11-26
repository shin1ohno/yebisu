import XCTest

// P2 keys and restart through the QA accessibility layer: Add host reads the injected key on
// each launch. Copy is pressed by identifier and confirmed by qa.state.
// The test launches the app itself, both times with the same environment, so p2-check.py
// never touches the app while it runs; it reads the app's files only after terminating it.
extension SmokeQA {
    /// What p2-check.py passes (xcodebuild hands TEST_RUNNER_<name> to the runner as <name>): the
    /// SHA-256 and length of the key line the injected provider holds.
    private struct P2Expected {
        let sha256: String
        let bytes: Int

        init() throws {
            let environment = ProcessInfo.processInfo.environment
            sha256 = try XCTUnwrap(environment["P2_PASTEBOARD_SHA256"],
                                   "P2_PASTEBOARD_SHA256 is not set: run testP2KeysAndRestart through scripts/ios/p2-check.py")
            bytes = try XCTUnwrap(environment["P2_PASTEBOARD_BYTES"].flatMap { Int($0) },
                                  "P2_PASTEBOARD_BYTES is not set: run testP2KeysAndRestart through scripts/ios/p2-check.py")
        }
    }

    private static let p2Environment = [
        "YEBISU_P2_OBSERVE": "1",
        "YEBISU_P11_KEYS_QA": "1",
        "YEBISU_QA_AX": "1",
    ]

    func testP2KeysAndRestart() throws {
        let expected = try P2Expected()
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        let qa = QAAx(app)
        XCUIDevice.shared.orientation = .portrait
        app.launchEnvironment = Self.p2Environment
        app.launch()
        let first = try p2Launch(qa, "P2 first launch")
        save("p2-key")

        // Copy: the app reads its own pasteboard back and publishes the digest and length.
        let copied = try qa.press(QAAxIDs.keysCopy, timeout: 10, what: "copied == 1 with a pasteboard digest") {
            $0.int("copied") == 1 && $0.string("pasteboard_sha256") != nil
        }
        XCTAssertEqual(copied.string("pasteboard_sha256"), expected.sha256,
                       "the pasteboard the app read back is not the injected key line: \(copied)")
        XCTAssertEqual(copied.int("pasteboard_bytes"), expected.bytes, "\(copied)")
        save("p2-copied")

        // Home and return are a lifecycle check; terminate+launch is a restart.
        try leaveForeground(app, qa, "P2 after Home")
        app.activate()
        try qa.waitState("P2 the same process after Home and return", timeout: 30) {
            $0.launch == first.launch && $0.int("copied") == 1
        }
        app.terminate()
        app.launchEnvironment = Self.p2Environment
        app.launch()
        let restarted = try p2Launch(qa, "P2 restart")
        XCTAssertNotEqual(restarted.launch, first.launch, "a restart is a new process")
        save("p2-restarted")
        // Issue #197: once in 60 runs the restarted window was black although the app kept
        // rendering. A second picture once the app has drawn what it had pending (qa.state's seq
        // held still) tells a window that stays black from a late first frame; p2-check.py judges
        // the first one only.
        _ = try? qa.settled("P2 restart, before the second picture")
        save("p2-restarted-later")
        XCTAssertEqual(app.state, .runningForeground)
    }

    /// Add host opens on first run and reads from the injected provider without a press.
    private func p2Launch(_ qa: QAAx, _ what: String) throws -> QAState {
        let state = try qa.waitState("\(what): qa.state", timeout: 30) { $0.launch > 0 }
        try qa.find(QAAxIDs.keysProvider, timeout: 30)
        return try qa.waitState("\(what): key ready after Add host opened", timeout: 10) {
            $0.launch == state.launch && $0.string("sheet") == "add-host" && $0.bool("key_ready") == true
        }
    }
}

/// The key block's identifiers (crates/yebisu/src/qa_ax_ids.rs; qa-ax-ids-check.py ties them).
private enum QAAxIDs {
    static let keysProvider = "qa.keys.provider"
    static let keysCopy = "qa.keys.copy"
}
