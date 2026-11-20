import XCTest

// P11 observer probes.
extension SmokeQA {
    /// The observer's progress in qa.state (settings_sheet/observer_qa.rs and native_observer_qa.rs:
    /// `observer.phase`, 25 once the 24 cases are collected, and `observer.failed`).
    private func observer(_ state: QAState) -> (phase: Int, failed: Bool)? {
        guard let field = state.raw["observer"] as? [String: Any], let phase = field["phase"] as? Int else { return nil }
        return (phase, field["failed"] as? Bool ?? false)
    }

    // SDK11 candidate source harness. UI waits never certify native completion;
    // only the Rust state machine's actual callback/worker/peer receipts advance.
    // UI test redesign S6: the controls are pressed by QA identifier and each press is confirmed by
    // qa.state; completion is the app's own phase, which the label "Observer matrix collected" /
    // "Native matrix collected" is drawn from.
    private func observerProbe(modal: Bool, drawing: Bool = false, disabled: Bool = false) throws {
        continueAfterFailure = false
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        let qa = QAAx(app)
        app.terminate()
        app.launchEnvironment["YEBISU_TERMINAL_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment["YEBISU_P11_KEYS_QA"] = "1"
        app.launchEnvironment["YEBISU_P11_CAPTURE"] = "1"
        app.launchEnvironment["YEBISU_P11_BINDING"] = "P11_BINDING_UNSET"
        app.launchEnvironment["YEBISU_QA_AX"] = "1"
        app.launchEnvironment[modal ? "YEBISU_P11_QA" : "YEBISU_NATIVE_QA"] = "1"
        if drawing { app.launchEnvironment["YEBISU_P11_DRAWING"] = "1" }
        if disabled { app.launchEnvironment["YEBISU_P11_OBSERVER_DISABLED"] = "1" }
        app.launch()
        // Before S6: sleep(3).
        try waitTerminal(qa, "the wire-peer terminal is connected")
        if modal {
            try pressLarger(qa)
        }
        let control = modal ? QAIds.p11Observer : QAIds.nativeNext
        var collected = false
        // A press that is not ready yet (the worker or the peer has not answered) changes nothing
        // but its press count; the loop presses again, as the OCR loop did (at most 160 presses).
        for press in 0..<160 {
            let state = try qa.waitState("qa.state before observer press \(press)") { self.observer($0) != nil }
            let progress = try XCTUnwrap(observer(state))
            if progress.phase > 24 { collected = true; break }
            if progress.failed { print("P11 observer failed at phase \(progress.phase): \(state)"); break }
            try qa.settleIfMoved("before observer press \(press)")
            let after = try qa.press(control, what: "observer press \(press) arrived")
            // Each case focuses the terminal and asks for the keyboard (native_observer_qa.rs).
            if after.keyboard != state.keyboard { qa.keyboardMayMove("observer press \(press) moved the keyboard") }
            Thread.sleep(forTimeInterval: 0.2)
        }
        XCTAssertTrue(collected, "Missing actual callback/worker/receiving completion")
        save(modal ? "p11-observer-modal" : "p11-observer-positive")
        if modal {
            try qa.settleIfMoved("before Close")
            try qa.press(QAIds.settingsClose, what: "the settings sheet is closed") { $0.isNull("sheet") }
            XCUIDevice.shared.press(.home)
            app.activate()
            app.terminate()
            app.launch()
            // Before S6: sleep(3).
            try waitTerminal(qa, "the relaunched terminal is connected")
            save("p11-observer-relaunch")
        }
        app.terminate()
        XCTAssertEqual(app.state, .notRunning)
    }
    func testP11ObserverText() throws { try observerProbe(modal: false) }
    func testP11ObserverDrawing() throws { try observerProbe(modal: false, drawing: true) }
    func testP11ObserverModal() throws { try observerProbe(modal: true) }
    func testP11ObserverDisabled() throws { try observerProbe(modal: true, disabled: true) }
}
