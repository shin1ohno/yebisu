import XCTest

// P9 core (UI test redesign S3): every control is pressed by its QA identifier and every press
// is confirmed through qa.state; nothing is read off the screen.
extension SmokeQA {
    /// Opens the tab row's ... menu by its "…" button and waits until qa.state shows it open.
    func openMenu(_ qa: QAAx) throws {
        try qa.press(QAIds.tabsMore, what: "the ... menu is open") { $0.string("menu") == "overflow" }
    }

    /// Opens the ... menu and presses its item `id`; `until` is the item's effect. The menu
    /// closes as the item runs, so `menu` must be null again too.
    @discardableResult
    func menuItem(_ qa: QAAx, _ id: String, what: String, until: @escaping (QAState) -> Bool) throws -> QAState {
        try openMenu(qa)
        return try qa.press(id, timeout: 10, what: what) { $0.isNull("menu") && until($0) }
    }

    /// Taps the terminal so it takes the keyboard, then types `text` into it.
    func typeIntoTerminal(_ app: XCUIApplication, _ qa: QAAx, _ text: String) throws {
        try qa.press(QAIds.terminalInput)
        app.typeText(text)
    }

    func testP9Core() throws {
        continueAfterFailure = false
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        let qa = QAAx(app)
        app.terminate()
        app.launchEnvironment["YEBISU_CORE_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment["YEBISU_INPUT_QA"] = "1"
        app.launchEnvironment["YEBISU_QA_AX"] = "1"
        // Drag selection consumes the tap that must open native input here. The start mode is
        // set at launch (S3); before S3 the test opened Settings › Input › Double tap › Close.
        app.launchEnvironment["YEBISU_QA_START_SELECTION"] = "double-tap"
        app.launch()
        let start = try qa.waitState("the Herdr 2 terminal is connected and drawn", timeout: 60) {
            $0.string("screen") == "terminal" && $0.bool("connected") == true && !$0.isNull("surface")
                && $0.corePanes == 1 && $0.coreTerminal(0) != nil && $0.string("pane") == $0.coreTerminal(0)
        }
        XCTAssertEqual(start.bool("drag_select"), false, "the launch variable did not set Double tap: \(start)")
        let first = try XCTUnwrap(start.coreTerminal(0))
        // The keyboard the terminal's focus raises lifts the QA band; press only once it stopped.
        try qa.settledKeyboard("P9 start")
        try typeIntoTerminal(app, qa, "P9-native\n")
        try qa.settledKeyboard("P9 native input")
        try qa.press(QAIds.bandMark)
        try qa.press(QAIds.bandDelete)
        try qa.press(QAIds.bandCommit)
        // L8: the Herdr 2 band moved into the tab row's ... menu (patch 07).
        let split = try menuItem(qa, QAIds.menuSplit, what: "two panes") { $0.corePanes == 2 }
        let vertical = try XCTUnwrap(split.coreVertical, "\(split)")
        try menuItem(qa, QAIds.menuSplitDirection, what: "the split direction flipped") {
            $0.corePanes == 2 && $0.coreVertical == !vertical
        }
        let pane2 = try menuItem(qa, QAIds.menuPane2, what: "pane 2 focused") {
            $0.coreActive == 1 && $0.coreTerminal(1) != nil && $0.string("pane") == $0.coreTerminal(1)
        }
        XCTAssertNotEqual(pane2.string("pane"), first, "pane 2 is a second terminal: \(pane2)")
        try typeIntoTerminal(app, qa, "P9-pane2\n")
        save("p9-split")
        try menuItem(qa, QAIds.menuDetach, what: "the core detached") { $0.bool("connected") == false }
        // The status bar shows Reconnect while the core is detached (L2a).
        try qa.press(QAIds.statusReconnect, timeout: 30, what: "the core reattached with both panes") {
            $0.bool("connected") == true && $0.corePanes == 2 && $0.coreTerminal(0) == first
        }
        try menuItem(qa, QAIds.menuPane1, what: "pane 1 focused") {
            $0.bool("connected") == true && $0.coreActive == 0 && $0.string("pane") == first
        }
        try typeIntoTerminal(app, qa, "P9-pane1\n")
        save("p9-reattached")
        XCTAssertEqual(app.state, .runningForeground)
        // Leave no software keyboard for the next test in this xcodebuild (testSH154CoreImage).
        try qa.hideKeyboard("P9 end")
    }
}
