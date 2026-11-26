import XCTest

// #231: a sidebar space row's menu opens on a long press and on a pointer's secondary click, on
// Herdr 1 and Herdr 2 rows, and lists Rename… and Close Space. Close asks first only when
// something could be lost (Herdr 1: always while the space has a pane, its snapshot names no
// program; Herdr 2: an agent's status or a program other than a shell). Every step is read from
// qa.state: `row_menu` / `row_menu_space` (the open menu and its row), `space_close` (the open
// confirmation) and the counters `row_menu_secondary`, `space_menu_close`, `space_menu_rename`,
// `space_close_asked`, `space_close_sent`. scripts/ios/space-menu-check.py also counts what the
// receiving side got: Herdr 1's workspace.close and workspace.rename in the wire peer's log, and
// Herdr 2's doc.set receipts in the mock core's.
extension SmokeQA {
    /// A point in GPUI window coordinates, as QAAx turns painted bounds into a touch.
    private func spaceMenuPoint(_ app: XCUIApplication, _ point: CGPoint) -> XCUICoordinate {
        app.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: point.x, dy: point.y))
    }

    /// The current space's name in its row, and the next row's (a space row is 56 pt, Dense,
    /// crates/yebisu/src/shell/sidebar.rs SPACE_ROW_HEIGHT).
    private func spaceRows(_ qa: QAAx) throws -> (current: CGPoint, next: CGPoint) {
        let state = try qa.waitState("the current space's frame") { $0.windowFrame(QAIds.sidebarCurrentSpace) != nil }
        let name = try XCTUnwrap(state.windowFrame(QAIds.sidebarCurrentSpace))
        print("SPACE MENU current-space frame \(name)")
        return (CGPoint(x: name.midX, y: name.midY), CGPoint(x: name.midX, y: name.midY + 56))
    }

    /// Opens `row`'s menu at `point` with a long press or a secondary click and checks it is that
    /// row's (and, for a click, that the click was a secondary one). The first pointer event after
    /// a launch can be lost before GPUI sees it (EditMenuTargetsQA.swift, PR 132 run 45142793709),
    /// so a click the app never counted (`row_menu_secondary` unchanged) is sent once more.
    private func openSpaceMenu(_ app: XCUIApplication, _ qa: QAAx, at point: CGPoint, row: String,
                               secondary: Bool) throws {
        let before = try qa.waitState("qa.state before the menu") { _ in true }
        let opened = { (state: QAState) -> Bool in
            state.string("row_menu") == "space" && state.string("row_menu_space") == row
                && (!secondary || state.count("row_menu_secondary") == before.count("row_menu_secondary") + 1)
        }
        guard secondary else {
            spaceMenuPoint(app, point).press(forDuration: 1.2)
            try qa.waitState("the menu of \(row) is open", timeout: 10, opened)
            return
        }
        spaceMenuPoint(app, point).rightClick()
        if (try? qa.waitState("the menu of \(row) is open", timeout: 5, opened)) != nil {
            return
        }
        let seen = try qa.waitState("qa.state after the click") { _ in true }
        guard seen.count("row_menu_secondary") == before.count("row_menu_secondary") else {
            XCTFail("SPACE MENU the click reached the app but opened no menu of \(row): \(seen)")
            return
        }
        print("SPACE MENU the secondary click on \(row) was not seen; clicking once more")
        spaceMenuPoint(app, point).rightClick()
        try qa.waitState("the menu of \(row) is open", timeout: 10, opened)
    }

    private func launchSpaceMenu(_ environment: [String: String], _ ready: String,
                                 _ predicate: @escaping (QAState) -> Bool) throws -> (XCUIApplication, QAAx) {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.terminate()
        app.launchEnvironment = environment
        app.launchEnvironment["YEBISU_QA_AX"] = "1"
        app.launchEnvironment["YEBISU_QA_AX_FRAMES"] = "1"
        XCUIDevice.shared.orientation = .landscapeLeft
        app.launch()
        let qa = QAAx(app, useWindowFrames: true)
        try qa.waitState(ready, timeout: 60, predicate)
        try qa.hideKeyboard("space menu launch")
        return (app, qa)
    }

    func testSpaceMenuHerdr1() throws {
        continueAfterFailure = false
        // Each XCTest action can wait 60 s for an idle app on a loaded host.
        executionTimeAllowance = 900
        // The rename suite's peer (no YEBISU_BOARD_QA: that board's agents keep changing, so the
        // app never idles and every XCTest action waits its 60 s, run sim-space-menu-2).
        let (app, qa) = try launchSpaceMenu(Self.renameEnvironment, "the wire-peer terminal is connected") {
            $0.string("screen") == "terminal" && $0.bool("connected") == true && $0.isNull("sheet")
                && $0.active && $0.windowFrame(QAIds.sidebarCurrentSpace) != nil
        }
        let rows = try spaceRows(qa)
        // H1-a: a secondary click on w2's row, Close Space: it asks (w2 has a pane), Cancel sends nothing.
        try openSpaceMenu(app, qa, at: rows.next, row: "w2", secondary: true)
        save("space-menu-h1-secondary")
        try qa.press(QAIds.sidebarSpaceClose, what: "the confirmation") {
            ($0.raw["space_close"] as? [String: Any])?["space"] as? String == "w2"
                && $0.count("space_menu_close") == 1 && $0.count("space_close_asked") == 1
        }
        save("space-menu-h1-asked")
        try qa.press(QAIds.sidebarSpaceCloseCancel, what: "the confirmation is gone") {
            $0.isNull("space_close") && $0.count("space_close_sent") == 0
        }
        // H1-b: a long press on w2's row, Close Space, Close Space in the confirmation: one close.
        try openSpaceMenu(app, qa, at: rows.next, row: "w2", secondary: false)
        try qa.press(QAIds.sidebarSpaceClose, what: "the confirmation again") {
            !$0.isNull("space_close") && $0.count("space_menu_close") == 2 && $0.count("space_close_asked") == 2
        }
        try qa.press(QAIds.sidebarSpaceCloseConfirm, what: "the close is sent once") {
            $0.isNull("space_close") && $0.count("space_close_sent") == 1
        }
        // The current space (w1) is still listed.
        try qa.waitState("w1 stays", timeout: 10) { $0.windowFrame(QAIds.sidebarCurrentSpace) != nil }
        save("space-menu-h1-closed")
        // H1-c: a long press on the current space's row, Rename…: the field, a name, Return.
        try openSpaceMenu(app, qa, at: rows.current, row: "w1", secondary: false)
        try qa.press(QAIds.sidebarSpaceRename, what: "the rename starts") { $0.count("space_menu_rename") == 1 }
        let field = app.textFields.firstMatch
        XCTAssertTrue(field.waitForExistence(timeout: 15), "the rename field opened")
        app.typeText("ship\n")
        sleep(3)
        let end = try qa.waitState("each item ran once") { _ in true }
        XCTAssertEqual(end.count("space_close_sent"), 1, "\(end)")
        XCTAssertEqual(end.count("space_menu_rename"), 1, "\(end)")
        // space-menu-check.py requires this attachment, so a filtered-out test cannot pass unseen.
        save("space-menu-h1")
        XCTAssertEqual(app.state, .runningForeground)
        app.terminate()
    }

    func testSpaceMenuHerdr2() throws {
        continueAfterFailure = false
        executionTimeAllowance = 900
        let (app, qa) = try launchSpaceMenu(
            ["YEBISU_CORE_QA": "1", "YEBISU_CORE_DOC_QA": "two-spaces"],
            "the two document spaces are drawn") {
            $0.herdrDocState == "readable" && $0.herdrDocSpaces == ["Build:2", "beta:1"]
                && $0.bool("connected") == true && $0.coreTerminal(0) == "t1"
                && $0.windowFrame(QAIds.sidebarCurrentSpace) != nil
        }
        let rows = try spaceRows(qa)
        let build = "default\u{0}sp00000000000000000000000000000015"
        let beta = "default\u{0}sp00000000000000000000000000000016"
        // H2-a: a long press on Build (t2 reports a status): Close asks; Cancel writes nothing.
        try openSpaceMenu(app, qa, at: rows.current, row: build, secondary: false)
        try qa.press(QAIds.sidebarSpaceClose, what: "the confirmation") {
            ($0.raw["space_close"] as? [String: Any])?["tabs"] as? Int == 2 && $0.count("space_close_asked") == 1
        }
        save("space-menu-h2-asked")
        try qa.press(QAIds.sidebarSpaceCloseCancel, what: "the confirmation is gone") {
            $0.isNull("space_close") && $0.count("space_close_sent") == 0
        }
        // H2-b: a secondary click on beta (shells only): Close Space closes at once; Build and its
        // two tabs stay.
        try openSpaceMenu(app, qa, at: rows.next, row: beta, secondary: true)
        try qa.press(QAIds.sidebarSpaceClose, what: "beta leaves the document") {
            $0.count("space_close_sent") == 1 && $0.count("space_close_asked") == 1
                && $0.herdrDocSpaces == ["Build:2"] && $0.herdrDocWrites == 1 && $0.herdrDocLast == "saved"
        }
        save("space-menu-h2-closed")
        // H2-c: a secondary click on Build, Rename…, a name, Return: one write.
        try openSpaceMenu(app, qa, at: rows.current, row: build, secondary: true)
        try qa.press(QAIds.sidebarSpaceRename, what: "the rename starts") { $0.count("space_menu_rename") == 1 }
        let field = app.textFields.firstMatch
        XCTAssertTrue(field.waitForExistence(timeout: 15), "the rename field opened")
        app.typeText("ship\n")
        try qa.waitState("the renamed space is written and read back", timeout: 30) {
            $0.herdrDocSpaces == ["ship:2"] && $0.herdrDocWrites == 2 && $0.herdrDocLast == "saved"
        }
        // H2-d: a long press on it, Close Space, Close Space in the confirmation: one more write.
        try openSpaceMenu(app, qa, at: rows.current, row: build, secondary: false)
        try qa.press(QAIds.sidebarSpaceClose, what: "the confirmation again") {
            !$0.isNull("space_close") && $0.count("space_close_asked") == 2
        }
        try qa.press(QAIds.sidebarSpaceCloseConfirm, what: "ship leaves the document") {
            $0.isNull("space_close") && $0.count("space_close_sent") == 2
                && $0.herdrDocSpaces == [] && $0.herdrDocWrites == 3 && $0.herdrDocLast == "saved"
        }
        let end = try qa.waitState("each item ran once per pick") { _ in true }
        XCTAssertEqual(end.count("space_menu_close"), 3, "\(end)")
        XCTAssertEqual(end.count("space_menu_rename"), 1, "\(end)")
        save("space-menu-h2")
        XCTAssertEqual(app.state, .runningForeground)
        app.terminate()
    }
}
