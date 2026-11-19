import XCTest
import Vision

// P10 settings, P11 settings sheet, SH1-151 selection settings.
extension SmokeQA {
    func testP11Settings() throws {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.terminate()
        app.launchEnvironment["YEBISU_TERMINAL_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment["YEBISU_P11_QA"] = "1"
        app.launchEnvironment["YEBISU_P11_KEYS_QA"] = "1"
        app.launch()
        sleep(4)
        try tapText("Settings", in: app)
        sleep(1)
        try tapText("Larger", in: app)
        try tapText("Probe modal callbacks", in: app)
        sleep(3)
        save("p11-modal")
        try tapText("Close", in: app)
        sleep(2)
        XCUIDevice.shared.press(.home)
        sleep(1)
        app.activate()
        save("p11-font")
        app.terminate()
        app.launch()
        sleep(4)
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
        let qa = QAAx(app)
        app.terminate()
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment["YEBISU_TERMINAL_QA"] = "1"
        app.launchEnvironment["YEBISU_SELECTION_SETTINGS_QA"] = "1"
        app.launchEnvironment["YEBISU_QA_AX"] = "1"
        XCUIDevice.shared.orientation = .portrait
        app.launch()
        // Before S4: sleep(3), then OCR. L2b moved both settings from the handoff band to
        // Settings › Input; the chosen value is the highlighted one, and selection-settings-check.py
        // asserts the loaded values and the saved file from the app's receipts.
        try qa.waitState("the terminal screen is up with its settings loaded", timeout: 60) {
            $0.string("screen") == "terminal" && $0.isNull("sheet") && $0.bool("settings_saved") == true
        }
        let fresh = try openInputPage(qa)
        // Both choices are drawn (before S4: OCR found "Double tap" and "On selection").
        try qa.find(QAIds.startDoubleTap)
        try qa.find(QAIds.copyOnSelection)
        XCTAssertEqual(fresh.bool("drag_select"), true, "fresh default is Drag: \(fresh)")
        XCTAssertEqual(fresh.bool("copy_on_select"), false, "fresh default is Manual: \(fresh)")
        save("sh1-151-fresh")
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
