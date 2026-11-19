import XCTest

// P2 keys and restart, through the QA accessibility layer (UI test redesign S2): the key block's
// controls are found by identifier and every press is confirmed by qa.state, not by reading the
// screen. The test launches the app itself, both times with the same environment, so p2-check.py
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
        save("p2-initial")
        try readDeviceKey(qa, "P2 first launch")
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
        XCUIDevice.shared.press(.home)
        try requireState(app, [.runningBackground, .runningBackgroundSuspended], "P2 after Home")
        app.activate()
        try qa.waitState("P2 the same process after Home and return", timeout: 30) {
            $0.launch == first.launch && $0.int("copied") == 1
        }
        app.terminate()
        app.launchEnvironment = Self.p2Environment
        app.launch()
        let restarted = try p2Launch(qa, "P2 restart")
        XCTAssertNotEqual(restarted.launch, first.launch, "a restart is a new process")
        try readDeviceKey(qa, "P2 restart")
        save("p2-restarted")
        XCTAssertEqual(app.state, .runningForeground)
    }

    /// After a launch: the layer is up, the key block shows the injected provider's line (drawn
    /// only after the provider's view preflight passed), and no key has been read yet.
    @discardableResult
    private func p2Launch(_ qa: QAAx, _ what: String) throws -> QAState {
        let state = try qa.waitState("\(what): qa.state", timeout: 30) { $0.launch > 0 }
        try qa.find(QAAxIDs.keysProvider, timeout: 30)
        return try qa.waitState("\(what): key block drawn without a key", timeout: 10) {
            $0.launch == state.launch && $0.bool("key_ready") == false
        }
    }

    /// Presses Read or initialize device key until the app reports `key_ready`. A press the app
    /// never received is repeated once by QAAx.press (Mac mini 6f2d709 lost the first one).
    private func readDeviceKey(_ qa: QAAx, _ what: String) throws {
        do {
            try qa.press(QAAxIDs.keysRead, timeout: 10, what: "\(what): key_ready") { $0.bool("key_ready") == true }
        } catch {
            save("p2-no-key")
            throw error
        }
    }
}

/// The key block's identifiers (crates/yebisu/src/qa_ax_ids.rs; qa-ax-ids-check.py ties them).
private enum QAAxIDs {
    static let keysProvider = "qa.keys.provider"
    static let keysRead = "qa.keys.read"
    static let keysCopy = "qa.keys.copy"
}
