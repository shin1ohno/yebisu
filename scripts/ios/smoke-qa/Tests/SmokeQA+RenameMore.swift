import XCTest

// Issue #143 S7, S8 and S10, driven for scripts/ios/rename-check.py (`--suite more`,
// `--suite herdr2`), which judges each step after the run from the app's `rename-state` records
// and the wire peer's log, as for testRenameInPlace (SmokeQA+Rename.swift). Controls are pressed
// by QA identifier and confirmed through qa.state; inside the field the test acts on a fixed
// schedule (the field's own queries may not answer).
extension SmokeQA {
    /// The rename's object in qa.state (shell/rename/qa.rs).
    func renameState(_ state: QAState) -> [String: Any] {
        state.raw["rename"] as? [String: Any] ?? [:]
    }

    /// S7, S8 on the Herdr 1 wire peer. S7: the field is opened over the focused pane's agent row;
    /// the peer then sends a snapshot with an agent above it (`YEBISU_RENAME_QA_INSERT`,
    /// mock_peer.rs), the row moves down and the field must follow it, then the window turns to
    /// portrait and back. S8: at the rail's width, the current space's long press, Rename…, a
    /// name, Return: one workspace.rename.
    func testRenameMore() throws {
        continueAfterFailure = true
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        let qa = QAAx(app)
        app.terminate()
        var environment = Self.renameEnvironment
        environment["YEBISU_RENAME_QA_INSERT"] = "1"
        app.launchEnvironment = environment
        XCUIDevice.shared.orientation = .landscapeLeft
        app.launch()
        try waitTerminal(qa, "the wire-peer terminal is connected")
        try qa.press(QAIds.terminalInput, effect: 10, what: "the software keyboard is up") { $0.keyboardUp }
        try qa.settledKeyboard("rename-more keyboard up")
        save("rename-more-before")

        // S7: the agent row's long press, then its menu's Rename….
        renameStep("S7")
        let agent = try qa.find(QAIds.sidebarCurrentAgent)
        agent.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).press(forDuration: 1.2)
        try qa.press(QAIds.sidebarAgentRename, timeout: 10)
        // The peer inserts the agent once the field is open; the row moves down.
        sleep(8)
        save("rename-s7-moved")
        renameStep("S7-portrait")
        XCUIDevice.shared.orientation = .portrait
        sleep(8)
        save("rename-s7-portrait")
        renameStep("S7-landscape")
        XCUIDevice.shared.orientation = .landscapeLeft
        sleep(8)
        save("rename-s7-landscape")
        outside(app)
        renameMarker(app, "t")

        // S8: the rail (Command-B), the current space's long press, Rename…, "railname", Return.
        renameStep("S8")
        app.typeKey("b", modifierFlags: .command)
        _ = try qa.waitState("the sidebar is the rail") { self.renameState($0)["rail"] as? Bool == true }
        let rail = try qa.find(QAIds.railCurrentSpace)
        rail.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).press(forDuration: 1.2)
        _ = try qa.waitState("the rail item's menu is open") { self.renameState($0)["rail_menu"] as? Bool == true }
        try qa.press(QAIds.railMenuRename)
        sleep(3)
        save("rename-s8-open")
        app.typeText("railname")
        sleep(2)
        app.typeText("\n")
        sleep(3)
        save("rename-s8-done")
        renameMarker(app, "s")
        app.typeKey("b", modifierFlags: .command)
        _ = try qa.waitState("the full sidebar is back") { self.renameState($0)["rail"] as? Bool == false }
        renameStep("end")
        XCTAssertEqual(app.state, .runningForeground)
    }

    /// S10 on the Herdr 2 mock (YEBISU_CORE_QA, as testP9Core): the palette's Rename Space and the
    /// tab's long-press Rename… are listed, disabled, with "Not available on Herdr 2 yet".
    func testRenameHerdr2() throws {
        continueAfterFailure = true
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        let qa = QAAx(app)
        app.terminate()
        app.launchEnvironment = [
            "YEBISU_CORE_QA": "1",
            "YEBISU_SMOKE_OBSERVE": "1",
            "YEBISU_INPUT_QA": "1",
            "YEBISU_QA_AX": "1",
        ]
        XCUIDevice.shared.orientation = .landscapeLeft
        app.launch()
        _ = try qa.waitState("the Herdr 2 terminal is connected and drawn", timeout: 60) {
            $0.string("screen") == "terminal" && $0.bool("connected") == true && !$0.isNull("surface")
        }
        let reason = "Not available on Herdr 2 yet"

        // Herdr 2's ... menu has no Command palette item: the palette opens with Command-Shift-P
        // once the terminal has the keyboard.
        try qa.press(QAIds.terminalInput, effect: 10, what: "the software keyboard is up") { $0.keyboardUp }
        try qa.settledKeyboard("rename-herdr2 keyboard up")
        renameStep("S10-palette")
        app.typeKey("p", modifierFlags: [.command, .shift])
        let palette = try qa.waitState("the palette is open") {
            (self.renameState($0)["palette"] as? [[String: Any]])?.isEmpty == false
        }
        save("rename-s10-palette")
        let rows = renameState(palette)["palette"] as? [[String: Any]] ?? []
        let space = rows.first { $0["label"] as? String == "Rename Space" }
        XCTAssertEqual(space?["enabled"] as? Bool, false, "palette rows: \(rows)")
        XCTAssertEqual(space?["reason"] as? String, reason, "palette rows: \(rows)")
        // Command-Shift-P toggles the palette (XCUITest's Escape did not close it).
        app.typeKey("p", modifierFlags: [.command, .shift])
        _ = try qa.waitState("the palette closed") { self.renameState($0)["palette"] is NSNull }

        renameStep("S10-tab")
        let tab = try qa.find(QAIds.tabsCurrent)
        tab.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).press(forDuration: 1.2)
        let menu = try qa.waitState("the tab's menu is open") {
            (self.renameState($0)["tab_menu"] as? [String: Any])?["core"] as? Bool == true
        }
        save("rename-s10-tab")
        let tabMenu = renameState(menu)["tab_menu"] as? [String: Any]
        XCTAssertEqual(tabMenu?["rename_reason"] as? String, reason, "tab menu: \(String(describing: tabMenu))")
        XCTAssertNoThrow(try qa.find(QAIds.tabMenuRename), "the tab menu lists no Rename…")
        outside(app)
        renameStep("end")
        XCTAssertEqual(app.state, .runningForeground)
    }
}
