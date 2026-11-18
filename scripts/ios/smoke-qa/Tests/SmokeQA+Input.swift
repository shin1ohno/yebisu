import XCTest
import Vision

// SH1-156 pointer click after a discarded composition.
extension SmokeQA {
    // SH1-156: the owner's device failure, driven with the pointer. After a
    // discarded composition, a trackpad click on the terminal must let letters
    // reach the terminal, and so must Select on then off, and closing Settings
    // without touching the terminal (R7). sh1-156-check.py
    // asserts the private peer's receipts; this test only drives UIKit.
    func testSH1156PointerClickAfterDiscard() throws {
        continueAfterFailure = false
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.terminate()
        app.launchEnvironment["YEBISU_TERMINAL_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment["YEBISU_SH1_156_QA"] = "1"
        XCUIDevice.shared.orientation = .portrait
        app.launch()
        sleep(3)
        try tapText("Discard composition", in: app)
        sleep(1)
        save("sh1-156-discarded")
        // A pointer click (not a touch) on the terminal's input row.
        let row = try tapText(terminalInputRow, in: app, tap: false)
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 1 - row.box.midY)).click()
        sleep(1)
        save("sh1-156-clicked")
        app.typeText("clicked")
        sleep(1)
        // L2b: Select is the ... menu's Select text; while it is on, the status bar
        // says typing is paused (shell/edit_bar.rs PAUSED_TYPING_STATUS).
        let more = try openOverflowMenu(in: app)
        try tapText("Select text", in: app)
        sleep(1)
        try tapText("Typing is paused", in: app, tap: false)
        save("sh1-156-select-on")
        try openOverflowMenu(in: app, at: more)
        try tapText("Select text", in: app)
        sleep(1)
        save("sh1-156-select-off")
        app.typeText("released")
        sleep(2)
        save("sh1-156-typed")
        XCTAssertFalse(textVisible("Typing is paused"), "Select stayed on")
        // R7: after Settings closes, typing without touching the terminal.
        try tapText("Settings", in: app)
        sleep(2)
        try tapText("Close", in: app)
        sleep(2)
        app.typeText("settings")
        sleep(2)
        save("sh1-156-after-settings")
        XCTAssertEqual(app.state, .runningForeground)
    }
}
