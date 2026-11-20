import XCTest

// P0 input and lifecycle receipts, P3 terminal.
extension SmokeQA {
    /// Waits until the P0 input field holds `text` (qa.state `p0_input`, what the field draws).
    /// Before S6 a Vision pass looked for one word of it on the screen.
    func waitP0Input(_ qa: QAAx, _ text: String, what: String) throws {
        try qa.waitState("\(what): p0_input is '\(text)'", timeout: 20) { $0.string("p0_input") == text }
    }

    /// Holds still for `seconds` in 10 s slices, reading qa.state once after each slice (a read is
    /// no input, so the app stays idle for smoke_oracle.py's 60 s interval). A read that takes long
    /// shows when XCTest started to wait for the app to idle (S5 follow-up S6-2: run 44547820393
    /// attempt 1 waited 80 s at the first query after the 60 s, then its screenshot timed out).
    func holdIdle(_ qa: QAAx, seconds: Int) {
        var slow: [String] = []
        for slice in stride(from: 10, through: seconds, by: 10) {
            sleep(10)
            let start = Date()
            let state = qa.read()
            let took = Date().timeIntervalSince(start)
            print(String(format: "P0 idle t=%ld s: qa.state read took %.2f s (seq %ld)", slice, took, state?.seq ?? -1))
            if took > 5 { slow.append(String(format: "t=%ld s %.1f s", slice, took)) }
        }
        if !slow.isEmpty { print("P0 idle: slow qa.state reads while idle (XCTest waited for the app to idle): \(slow)") }
    }

    func testInputAndLifecycleReceipts() throws {
        // A failed state wait below must stop the test at once under its own name,
        // not turn into three 30 s snapshot timeouts on whatever comes next.
        continueAfterFailure = false
        // smoke-check.py launched the app with YEBISU_SMOKE_OBSERVE and, since S6, YEBISU_QA_AX.
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        let qa = QAAx(app)
        XCUIDevice.shared.orientation = .portrait
        app.activate()
        try requireState(app, [.runningForeground], "initial activate()")
        // Before S6: OCR read "Async counter". The tap control is marked when the P0 screen is drawn.
        try qa.waitState("first paint after launch: the P0 screen", timeout: 20) { $0.string("screen") == "p0" }
        try qa.find(QAIds.p0Tap, timeout: 20)
        // Before S6: a tap at the window point (0.5, 0.425), on the same "Tap / type" control.
        try qa.press(QAIds.p0Tap, what: "the P0 control counted the tap") { ($0.int("taps") ?? 0) >= 1 }
        app.typeText("P0-input")
        try waitP0Input(qa, "P0-input", what: "typed 'P0-input' received")
        save("initial")
        XCUIDevice.shared.press(.home)
        // activate() issued while the Home press is still being handled leaves the app
        // suspended in the background (class C of the L17 diagnosis).
        try requireState(app, [.runningBackground, .runningBackgroundSuspended], "app left the foreground after press(.home)")
        app.activate()
        try requireState(app, [.runningForeground], "activate() after press(.home)")
        try waitP0Input(qa, "P0-input", what: "repaint of 'P0-input' after returning")
        app.typeText("-returned")
        try waitP0Input(qa, "P0-input-returned", what: "typed '-returned' received")
        save("returned")
        XCUIDevice.shared.orientation = .landscapeLeft
        sleep(2)
        // The native keyboard covers the input field in landscape, so no screen text
        // shows this value; qa.state does, and smoke_oracle.py still requires its receipt.
        app.typeText("-rotated")
        try waitP0Input(qa, "P0-input-returned-rotated", what: "typed '-rotated' received")
        save("rotated")
        XCUIDevice.shared.orientation = .portrait
        sleep(2)
        save("idle-before")
        holdIdle(qa, seconds: 60)
        // typeText waits for the app to idle first (C-anim); the time is printed by name so a
        // long wait is told from a lost input.
        let typing = Date()
        app.typeText("-idle")
        let typed = Date().timeIntervalSince(typing)
        print(String(format: "P0 idle: typeText('-idle') took %.2f s%@", typed,
                     typed > 20 ? " (XCTest waited for the app to idle, class C-anim; see the animations-running records)" : ""))
        try waitP0Input(qa, "P0-input-returned-rotated-idle", what: "typed '-idle' received")
        save("idle")
        // Before S6: a tap at (0.97, 0.96), the iPad keyboard's hide key; now that key as a UIKit
        // element, confirmed by qa.state.
        try qa.hideKeyboard("P0 image-clear")
        save("image-clear")
        XCTAssertEqual(app.state, .runningForeground)
        // This liveness assertion is auxiliary. smoke_oracle.py asserts rendered
        // regions and app-written receipt state for every saved stage.
    }

    /// Opens the settings sheet from the sidebar footer's Settings (the rail's when the sidebar
    /// is a rail) and waits until qa.state shows the sheet.
    func openSettingsSheet(_ qa: QAAx) throws {
        let footer = qa.element(QAIds.footerSettings).exists ? QAIds.footerSettings : QAIds.railSettings
        try qa.press(footer, timeout: 10, what: "the settings sheet is open") { $0.string("sheet") == "settings" }
    }

    /// Settings › Display › `density` › Close, each press confirmed by qa.state; returns the
    /// state after Close.
    @discardableResult
    func setDensity(_ qa: QAAx, compact: Bool) throws -> QAState {
        try openSettingsSheet(qa)
        try qa.press(QAIds.settingsPageDisplay, what: "the Display page is open") { $0.string("settings_page") == "fonts" }
        let density = compact ? QAIds.densityCompact : QAIds.densityRegular
        try qa.press(density, what: "compact == \(compact)") { $0.bool("compact") == compact }
        return try qa.press(QAIds.settingsClose, what: "the settings sheet is closed") {
            $0.isNull("sheet") && $0.bool("compact") == compact
        }
    }

    /// Rotates the device and waits up to 10 s for the app's viewport to follow. A Simulator
    /// whose viewport does not change is left as it is: p3-check.py reports rotation as
    /// UNVERIFIED unless --require-rotation, as before S3.
    func rotate(_ qa: QAAx, to orientation: UIDeviceOrientation, landscape: Bool) throws {
        XCUIDevice.shared.orientation = orientation
        func wide(_ state: QAState) -> Bool? {
            guard let size = state.raw["viewport"] as? [Double], size.count == 2 else { return nil }
            return size[0] > size[1]
        }
        do {
            try qa.waitState("the viewport is \(landscape ? "landscape" : "portrait")", timeout: 10) { wide($0) == landscape }
        } catch QAAxError.timeout(_, _, _, let last) {
            print("P3 the viewport did not follow the orientation request (\(orientation.rawValue)); last qa.state=\(last)")
        }
        try qa.settledKeyboard("P3 after rotation")
    }

    func testP3Terminal() throws {
        continueAfterFailure = false
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        let qa = QAAx(app)
        app.terminate()
        app.launchEnvironment["YEBISU_TERMINAL_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment["YEBISU_QA_AX"] = "1"
        XCUIDevice.shared.orientation = .portrait
        app.launch()
        let start = try qa.waitState("the wire-peer terminal is connected and drawn", timeout: 60) {
            $0.string("screen") == "terminal" && $0.bool("connected") == true && !$0.isNull("surface") && $0.isNull("sheet")
        }
        XCTAssertEqual(start.bool("compact"), false, "the terminal starts at the regular density: \(start)")
        try qa.settledKeyboard("P3 start")
        save("p3-standard")
        // L2b moved the density from the toolbar to Settings › Display (L1).
        try setDensity(qa, compact: true)
        try qa.settledKeyboard("P3 compact")
        save("p3-compact")
        try setDensity(qa, compact: false)
        try rotate(qa, to: .landscapeLeft, landscape: true)
        save("p3-landscape")
        try rotate(qa, to: .portrait, landscape: false)
        save("p3-returned")
        XCTAssertEqual(app.state, .runningForeground)
    }
}
