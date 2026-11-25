import XCTest

// P10 settings, P11 settings sheet, SH1-151 selection settings.
extension SmokeQA {
    /// Waits until the wire-peer terminal is connected with no sheet open (before S6: a fixed sleep).
    @discardableResult
    func waitTerminal(_ qa: QAAx, _ what: String, timeout: TimeInterval = 60,
                      _ extra: @escaping (QAState) -> Bool = { _ in true }) throws -> QAState {
        try qa.waitState(what, timeout: timeout) {
            $0.string("screen") == "terminal" && $0.bool("connected") == true && $0.isNull("sheet") && extra($0)
        }
    }

    /// The terminal's font size in qa.state (`font_size`), nil before the field is published.
    func fontSize(_ state: QAState) -> Double? {
        (state.raw["font_size"] as? Double) ?? (state.raw["font_size"] as? Int).map(Double.init)
    }

    /// Settings › Display (the sheet opens on it) › Larger, confirmed by the font size growing by
    /// one point. Before S6: OCR found "Settings" and then "Larger" on the sheet's first page.
    @discardableResult
    func pressLarger(_ qa: QAAx) throws -> Double {
        try qa.settleIfMoved("before Settings")
        try openSettingsSheet(qa)
        var state = try qa.waitState("qa.state with the sheet open") { $0.string("sheet") == "settings" }
        if state.string("settings_page") != "fonts" {
            state = try qa.press(QAIds.settingsPageDisplay, what: "the Display page is open") { $0.string("settings_page") == "fonts" }
        }
        let size = try XCTUnwrap(fontSize(state), "qa.state has no font_size: \(state)")
        try qa.press(QAIds.fontLarger, what: "the font size is \(size + 1)") { self.fontSize($0) == size + 1 }
        return size + 1
    }

    func testP11Settings() throws {
        continueAfterFailure = false
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        let qa = QAAx(app)
        app.terminate()
        app.launchEnvironment["YEBISU_TERMINAL_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment["YEBISU_P11_QA"] = "1"
        app.launchEnvironment["YEBISU_P11_KEYS_QA"] = "1"
        app.launchEnvironment["YEBISU_QA_AX"] = "1"
        app.launch()
        try waitTerminal(qa, "the wire-peer terminal is connected")
        let larger = try pressLarger(qa)
        try qa.press(QAIds.p11Probe, what: "the modal probe started") { $0.count("p11_probe_start") == 1 }
        // Before S6: sleep(3). The probe ends one second after it starts (settings_sheet/qa.rs).
        try qa.waitState("the modal probe ended", timeout: 10) { $0.count("p11_probe_end") == 1 }
        save("p11-modal")
        try qa.press(QAIds.settingsClose, what: "the settings sheet is closed") { $0.isNull("sheet") }
        try leaveForeground(app, qa, "P11 app left the foreground after press(.home)")
        app.activate()
        try requireState(app, [.runningForeground], "P11 activate() after press(.home)")
        try waitTerminal(qa, "the terminal after returning, with the larger font") { self.fontSize($0) == larger }
        save("p11-font")
        app.terminate()
        app.launch()
        // Before S6: sleep(4). p11-check.py asserts the reloaded font from the app's receipts.
        try waitTerminal(qa, "the relaunched terminal, with the larger font") { self.fontSize($0) == larger }
        save("p11-relaunch")
        XCTAssertEqual(app.state, .runningForeground)
    }

    /// Settings › Input (UI test redesign S4): the sheet and the page, each press confirmed by
    /// qa.state.
    func openInputPage(_ qa: QAAx) throws -> QAState {
        try qa.settledKeyboard("before Settings")
        try openSettingsSheet(qa)
        return try qa.press(QAIds.settingsPageInput, what: "the Input page is open") { $0.string("settings_page") == "input" }
    }

    func testSH1151SelectionSettings() throws {
        continueAfterFailure = false
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        let qa = QAAx(app, useWindowFrames: true)
        app.terminate()
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment["YEBISU_TERMINAL_QA"] = "1"
        app.launchEnvironment["YEBISU_SELECTION_SETTINGS_QA"] = "1"
        app.launchEnvironment["YEBISU_QA_AX"] = "1"
        app.launchEnvironment["YEBISU_QA_AX_FRAMES"] = "1"
        XCUIDevice.shared.orientation = .portrait
        app.launch()
        // Before S4: sleep(3), then OCR. L2b moved both settings from the handoff band to
        // Settings › Input; the chosen value is the highlighted one, and selection-settings-check.py
        // asserts the loaded values and the saved file from the app's receipts.
        try qa.waitState("the terminal screen is up with its settings loaded", timeout: 60) {
            $0.string("screen") == "terminal" && $0.isNull("sheet") && $0.bool("settings_saved") == true
        }
        let fresh = try openInputPage(qa)
        XCTAssertEqual(fresh.bool("drag_select"), true, "fresh default is Drag: \(fresh)")
        XCTAssertEqual(fresh.bool("copy_on_select"), false, "fresh default is Manual: \(fresh)")
        save("sh1-151-fresh")
        // Shortcut style precedes Hardware keyboard and Selection. The lower
        // rows can be below the fold, even on a full-size iPad.
        let close = try qa.find(QAIds.settingsClose)
        let header = try qa.coordinate(close, id: QAIds.settingsClose, at: CGVector(dx: 0, dy: 0.5))
        let insideContent = header.withOffset(CGVector(dx: -100, dy: 540))
        insideContent.press(forDuration: 0.08, thenDragTo: insideContent.withOffset(CGVector(dx: 0, dy: -230)))
        // Issue #142 stage 3: the first tap within about 1.5 s after this drag reaches the app but is not
        // taken (CI run 45388075978 and a local run: touch began and ended on the control, no press). QAAx no
        // longer sends such a press again, so first prove taps are taken: press the selected Drag option
        // (no change) until GPUI counts it (QAAx.touchReady), not a fixed pause.
        try qa.touchReady(QAIds.startDrag, what: "Settings › Input after the scroll")
        try qa.press(QAIds.startDoubleTap, what: "Start a selection is Double tap") { $0.bool("drag_select") == false }
        try qa.press(QAIds.copyOnSelection, what: "Copy is On selection") { $0.bool("copy_on_select") == true }
        // Before S4: sleep(2). The settings worker wrote both edits (the sheet's "Saved").
        try qa.waitState("both edits saved", timeout: 10) { $0.bool("settings_saved") == true }
        try qa.press(QAIds.settingsClose, what: "the settings sheet is closed") { $0.isNull("sheet") }
        app.terminate()
        app.launch()
        try qa.waitState("the relaunched terminal screen is up", timeout: 60) {
            $0.string("screen") == "terminal" && $0.isNull("sheet") && $0.bool("settings_saved") == true
        }
        let restored = try openInputPage(qa)
        XCTAssertEqual(restored.bool("drag_select"), false, "Double tap restored: \(restored)")
        XCTAssertEqual(restored.bool("copy_on_select"), true, "On selection restored: \(restored)")
        save("sh1-151-restored")
        XCTAssertEqual(app.state, .runningForeground)
    }

    func testP10Settings() throws {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.terminate()
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment["YEBISU_SETTINGS_QA"] = "1"
        app.launch()
        sleep(3)
        save("p10-before")
        XCUIDevice.shared.press(.home)
        sleep(3)
        app.activate()
        sleep(2)
        app.terminate()
        app.launch()
        sleep(3)
        save("p10-restored")
        XCTAssertEqual(app.state, .runningForeground)
    }
}
