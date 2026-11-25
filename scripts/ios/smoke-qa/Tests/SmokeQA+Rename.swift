import XCTest

// Issue #143: rename in place, driven for scripts/ios/rename-check.py, which judges every step
// after the run from the app's `rename-state` records (shell/rename/qa.rs, herdr-uikit's
// `native_inline_field_state`) and the wire peer's log. The current space's row and the current
// tab's label are pressed by QA identifier; once the native field is up, element queries may not
// answer (herdr-uikit scripts/native-search-qa/Tests/NativeSearch.swift:5-7), so inside the field
// this test acts on a fixed schedule. `RENAME_STEP <name> <unix time>` lines split the records by
// step; a marker letter typed into the terminal after each close splits the peer log. Escape is
// not typed: XCUITest's Escape never reached a presented field (herdr-uikit
// docs/native-inline-field.md); the device checks D2, D3 cover it.
extension SmokeQA {
    private static let renameEnvironment = [
        "YEBISU_TERMINAL_QA": "1",
        "YEBISU_SMOKE_OBSERVE": "1",
        "YEBISU_QA_AX": "1",
        "YEBISU_INPUT_QA": "1",
    ]

    private func renameStep(_ name: String) {
        print(String(format: "RENAME_STEP %@ %.3f", name, Date().timeIntervalSince1970))
    }

    /// Types into the terminal once the field is closed and input came back.
    private func renameMarker(_ app: XCUIApplication, _ letter: String) {
        sleep(3)
        app.typeText(letter)
        print("RENAME marker \(letter)")
        sleep(1)
    }

    /// Long press of the current space's row, then its menu's Rename…. A long press reaches the
    /// row's press layer, not the mark's tap count, so it is confirmed by the menu it opens.
    private func renameSpace(_ qa: QAAx) throws {
        let row = try qa.find(QAIds.sidebarCurrentSpace)
        row.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).press(forDuration: 1.2)
        _ = try qa.waitState("the space row's menu is open") { $0.string("row_menu") == "space" }
        try qa.press(QAIds.sidebarSpaceRename)
        sleep(3)
    }

    /// A tap far from the field, on the terminal above the software keyboard.
    private func outside(_ app: XCUIApplication) {
        app.coordinate(withNormalizedOffset: .zero)
            .withOffset(CGVector(dx: app.frame.width - 160, dy: 160)).tap()
        sleep(3)
    }

    func testRenameInPlace() throws {
        continueAfterFailure = true
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        let qa = QAAx(app)
        app.terminate()
        app.launchEnvironment = Self.renameEnvironment
        XCUIDevice.shared.orientation = .landscapeLeft
        app.launch()
        try waitTerminal(qa, "the wire-peer terminal is connected")
        try qa.press(QAIds.terminalInput, effect: 10, what: "the software keyboard is up") { $0.keyboardUp }
        try qa.settledKeyboard("rename keyboard up")
        save("rename-before")

        // R1 (S1): long press the space row, Rename…, a new name, Return: one workspace.rename.
        renameStep("R1")
        try renameSpace(qa)
        save("rename-r1-open")
        app.typeText("ship")
        sleep(2)
        save("rename-r1-typed")
        app.typeText("\n")
        sleep(3)
        save("rename-r1-done")
        renameMarker(app, "z")

        // R2 (S2): a double tap on the tab's label (the first tap is the tab it is on already),
        // "build", Return: one tab.rename.
        renameStep("R2")
        let tab = try qa.find(QAIds.tabsCurrent)
        tab.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).doubleTap()
        sleep(3)
        save("rename-r2-open")
        app.typeText("build")
        sleep(2)
        app.typeText("\n")
        sleep(3)
        save("rename-r2-done")
        renameMarker(app, "y")

        // R2b (S2, space): a double tap on the current space's name, "space2", Return: one
        // workspace.rename, and the row moved at most once.
        renameStep("R2b")
        let row = try qa.find(QAIds.sidebarCurrentSpace)
        row.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).doubleTap()
        sleep(3)
        save("rename-r2b-open")
        app.typeText("space2")
        sleep(2)
        app.typeText("\n")
        sleep(3)
        renameMarker(app, "u")

        // R3 (S5): a tap outside the field cancels: no request, the tap does nothing else.
        renameStep("R3")
        try renameSpace(qa)
        app.typeText("never")
        sleep(1)
        outside(app)
        save("rename-r3-outside")
        renameMarker(app, "x")

        // R4 (S6): nothing reaches the terminal while the field is up: keys, Command-V of a
        // marker word, Command-B; then an outside tap closes it.
        renameStep("R4")
        UIPasteboard.general.string = "renamepasteword"
        try renameSpace(qa)
        app.typeText("q")
        sleep(1)
        app.typeKey("v", modifierFlags: .command)
        sleep(1)
        app.typeKey("b", modifierFlags: .command)
        sleep(2)
        save("rename-r4-typed")
        outside(app)
        renameMarker(app, "w")

        // R5 (S1, Japanese): Control-Space to the Japanese romaji source, "puro", Return twice.
        // Whether the Simulator composes is recorded (`marked`), not required.
        renameStep("R5")
        try renameSpace(qa)
        app.typeKey(" ", modifierFlags: .control)
        sleep(2)
        app.typeText("puro")
        sleep(2)
        save("rename-r5-typed")
        app.typeText("\n")
        sleep(2)
        app.typeText("\n")
        sleep(3)
        app.typeKey(" ", modifierFlags: .control)
        sleep(1)
        save("rename-r5-done")
        renameMarker(app, "v")
        renameStep("end")
        XCTAssertEqual(app.state, .runningForeground)
    }
}
