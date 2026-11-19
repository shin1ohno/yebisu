import XCTest
import Vision

// P0 input and lifecycle receipts, P3 terminal.
extension SmokeQA {
    func testInputAndLifecycleReceipts() throws {
        // A failed state wait below must stop the test at once under its own name,
        // not turn into three 30 s snapshot timeouts on whatever comes next.
        continueAfterFailure = false
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        XCUIDevice.shared.orientation = .portrait
        app.activate()
        try requireState(app, [.runningForeground], "initial activate()")
        try waitForRenderedText("Async counter", what: "first paint after launch")
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.425)).tap()
        app.typeText("P0-input")
        try waitForRenderedText("input", what: "typed 'P0-input' received")
        save("initial")
        XCUIDevice.shared.press(.home)
        // activate() issued while the Home press is still being handled leaves the app
        // suspended in the background (class C of the L17 diagnosis).
        try requireState(app, [.runningBackground, .runningBackgroundSuspended], "app left the foreground after press(.home)")
        app.activate()
        try requireState(app, [.runningForeground], "activate() after press(.home)")
        try waitForRenderedText("input", what: "repaint of 'P0-input' after returning")
        app.typeText("-returned")
        try waitForRenderedText("returned", what: "typed '-returned' received")
        save("returned")
        XCUIDevice.shared.orientation = .landscapeLeft
        sleep(2)
        // The native keyboard covers the input field in landscape, so no screen text
        // shows this value. smoke_oracle.py still requires its receipt in app.jsonl.
        app.typeText("-rotated")
        sleep(2)
        save("rotated")
        XCUIDevice.shared.orientation = .portrait
        sleep(2)
        save("idle-before")
        sleep(60)
        app.typeText("-idle")
        try waitForRenderedText("idle", what: "typed '-idle' received")
        save("idle")
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.97, dy: 0.96)).tap()
        sleep(2)
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
