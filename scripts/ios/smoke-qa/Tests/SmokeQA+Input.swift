import XCTest

// SH1-156 pointer click after a discarded composition.
extension SmokeQA {
    // SH1-156: the owner's device failure, driven with the pointer. After a discarded composition,
    // a trackpad click on the terminal must let letters reach the terminal, and so must Select on
    // then off, and closing Settings without touching the terminal (R7). sh1-156-check.py asserts
    // the private peer's receipts; this test only drives UIKit. UI test redesign S4: controls by QA
    // identifier, every step confirmed by qa.state, no OCR and no fixed sleep.
    func testSH1156PointerClickAfterDiscard() throws {
        continueAfterFailure = false
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        let qa = QAAx(app)
        try launchPeerTerminal(app, qa, "YEBISU_SH1_156_QA")
        let start = try qa.waitState("qa.state before Discard composition", timeout: 5) { _ in true }
        // The pointer click below waits for the keyboard on its own.
        try pressSettled(qa, QAIds.sh156Discard, movesKeyboard: false, what: "the composition was discarded") {
            $0.count("sh156_discarded") > start.count("sh156_discarded")
        }
        save("sh1-156-discarded")
        // A pointer click (not a touch) on the terminal's input area, where "Type here" is drawn.
        // GPUI counts it on qa.terminal.input like a tap (mouse down and up inside it).
        try qa.settledKeyboard("before the pointer click")
        let terminal = try qa.find(QAIds.terminalInput)
        let before = try qa.waitState("qa.state before the pointer click", timeout: 5) { _ in true }
        terminal.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).click()
        try qa.waitState("the pointer click reached the terminal", timeout: 5) {
            $0.presses(QAIds.terminalInput) > before.presses(QAIds.terminalInput)
        }
        save("sh1-156-clicked")
        typed(app, qa, "clicked")
        // L2b: Select is the ... menu's Select text; while it is on, the status bar says typing is
        // paused (shell/edit_bar.rs PAUSED_TYPING_STATUS), which qa.state's `selecting` mirrors.
        try menuItem(qa, QAIds.menuSelectText, what: "typing is paused (Select on)") { $0.bool("selecting") == true }
        save("sh1-156-select-on")
        try menuItem(qa, QAIds.menuSelectText, what: "typing resumed (Select off)") { $0.bool("selecting") == false }
        save("sh1-156-select-off")
        typed(app, qa, "released")
        save("sh1-156-typed")
        XCTAssertEqual(qa.read()?.bool("selecting"), false, "Select stayed on")
        // R7: after Settings closes, typing without touching the terminal.
        try openSettingsSheet(qa)
        try qa.press(QAIds.settingsClose, what: "the settings sheet is closed") { $0.isNull("sheet") }
        typed(app, qa, "settings")
        save("sh1-156-after-settings")
        XCTAssertEqual(app.state, .runningForeground)
    }

    /// Types `text` and waits until the private peer received input since (before S4: sleep(1) or
    /// sleep(2)); sh1-156-check.py decides from the peer's receipts whether the letters arrived.
    func typed(_ app: XCUIApplication, _ qa: QAAx, _ text: String) {
        let inputs = qa.read()?.peerInputs ?? 0
        app.typeText(text)
        logWait(qa, "typed '\(text)' reached the peer", seconds: 5) { $0.peerInputs > inputs }
    }
}
