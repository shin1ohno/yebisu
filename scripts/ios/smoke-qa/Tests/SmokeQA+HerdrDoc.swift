import XCTest

// #144: the official app's herdr-app document on the mock core (YEBISU_CORE_DOC_QA names a
// fixture of crates/yebisu/fixtures/herdr-app/; YEBISU_CORE_DOC_QA_CONFLICTS makes the first
// writes stale). Controls are pressed by QA identifier and every result is read from qa.state
// (herdr_doc); p9-check validate_doc checks the doc.set and labelled terminal.run receipts the
// mock core wrote: S5's write, S3's refused write and its write again, and none for S1 or S4;
// then testP9HerdrDocRename's (#144 D3): S2's rename, and S3's refused rename and its write again.
extension QAState {
    var herdrDoc: [String: Any]? { raw["herdr_doc"] as? [String: Any] }
    var herdrDocState: String? { herdrDoc?["state"] as? String }
    var herdrDocLocalTabs: Int? { herdrDoc?["local_tabs"] as? Int }
    var herdrDocWrites: Int? { herdrDoc?["writes"] as? Int }
    var herdrDocConflicts: Int? { herdrDoc?["conflicts"] as? Int }
    var herdrDocLast: String? { herdrDoc?["last"] as? String }
    /// Each document space's row name and tab count, in the document's order.
    var herdrDocSpaces: [String] {
        ((herdrDoc?["spaces"] as? [[String: Any]]) ?? []).map {
            "\($0["name"] as? String ?? "?"):\($0["tabs"] as? Int ?? -1)"
        }
    }
}

extension SmokeQA {
    func testP9HerdrDoc() throws {
        continueAfterFailure = false
        /// Launches the app on the mock core with the document `fixture`; the first `conflicts`
        /// writes find that another app wrote first.
        func launchHerdrDoc(_ fixture: String, conflicts: Int = 0) -> (XCUIApplication, QAAx) {
            let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
            app.terminate()
            app.launchEnvironment["YEBISU_CORE_QA"] = "1"
            app.launchEnvironment["YEBISU_CORE_DOC_QA"] = fixture
            app.launchEnvironment["YEBISU_CORE_DOC_QA_CONFLICTS"] = String(conflicts)
            app.launchEnvironment["YEBISU_QA_AX"] = "1"
            app.launch()
            return (app, QAAx(app))
        }
        // S1: two document spaces, in the document's order, with their tab counts. The unnamed
        // one shows its folder's last component (no host is asked in QA). With no iPad-only tabs
        // the first space is drawn: its first tab's terminal.
        var (app, qa) = launchHerdrDoc("two-spaces")
        let read = try qa.waitState("the two document spaces are drawn", timeout: 60) {
            $0.herdrDocState == "readable" && $0.herdrDocSpaces == ["Build:2", "beta:1"]
                && $0.bool("connected") == true && $0.coreTerminal(0) == "t1"
        }
        XCTAssertEqual(read.herdrDocLocalTabs, 0, "\(read)")
        XCTAssertEqual((read.herdrDoc?["selected"] as? String)?.hasPrefix("sp"), true, "\(read)")
        try qa.hideKeyboard("herdr doc S1")
        // S5 (D4): New tab in the drawn document space writes the document once; the new tab
        // is the space's third. p9-check finds the labels on its terminal.run receipt.
        let saved = try menuItem(qa, QAIds.menuNewTab, what: "the new tab in the document") {
            $0.herdrDocWrites == 1 && $0.herdrDocLast == "saved" && $0.herdrDocSpaces == ["Build:3", "beta:1"]
        }
        XCTAssertEqual(saved.herdrDocConflicts, 0, "\(saved)")
        try qa.hideKeyboard("herdr doc S5")
        app.terminate()

        // S3 (D4): the first write finds a newer document; the intent is applied to it and sent
        // again over the newer version (decision 5).
        (app, qa) = launchHerdrDoc("two-spaces", conflicts: 1)
        try qa.waitState("the document is drawn again", timeout: 60) {
            $0.herdrDocState == "readable" && $0.bool("connected") == true && $0.coreTerminal(0) == "t1"
        }
        try qa.hideKeyboard("herdr doc S3")
        try menuItem(qa, QAIds.menuNewTab, what: "the new tab after one conflict") {
            $0.herdrDocWrites == 1 && $0.herdrDocConflicts == 1 && $0.herdrDocLast == "saved"
                && $0.herdrDocSpaces == ["Build:3", "beta:1"]
        }
        try qa.hideKeyboard("herdr doc S3 end")
        app.terminate()

        // S4: a document the official app cannot read: one iPad-only row, the session's own tabs,
        // and New tab adds an iPad-only tab without writing the document.
        (app, qa) = launchHerdrDoc("unreadable-root")
        let unreadable = try qa.waitState("the unreadable document is shown as such", timeout: 60) {
            $0.herdrDocState == "unreadable" && $0.bool("connected") == true && $0.herdrDocLocalTabs == 1
        }
        XCTAssertEqual(unreadable.herdrDocSpaces, [], "\(unreadable)")
        try qa.hideKeyboard("herdr doc S4")
        try menuItem(qa, QAIds.menuNewTab, what: "a second iPad-only tab") { $0.herdrDocLocalTabs == 2 }
        try qa.hideKeyboard("herdr doc end")
        // p9-check requires this attachment, so a filtered-out test cannot pass unseen.
        save("p9-herdr-doc")
        XCTAssertEqual(app.state, .runningForeground)
    }

    /// #144 D3: Rename Space on Herdr 2 writes the document. S2: a double tap on the current
    /// space's name, a name, Return: one write over the version read, and the row shows the name.
    /// S3: the first write finds a newer document; the rename is applied to it and sent again.
    func testP9HerdrDocRename() throws {
        continueAfterFailure = false
        func launch(conflicts: Int) throws -> (XCUIApplication, QAAx) {
            let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
            app.terminate()
            app.launchEnvironment["YEBISU_CORE_QA"] = "1"
            app.launchEnvironment["YEBISU_CORE_DOC_QA"] = "two-spaces"
            app.launchEnvironment["YEBISU_CORE_DOC_QA_CONFLICTS"] = String(conflicts)
            app.launchEnvironment["YEBISU_QA_AX"] = "1"
            app.launch()
            let qa = QAAx(app)
            try qa.waitState("the two document spaces are drawn", timeout: 60) {
                $0.herdrDocState == "readable" && $0.herdrDocSpaces == ["Build:2", "beta:1"]
                    && $0.bool("connected") == true && $0.coreTerminal(0) == "t1"
            }
            try qa.hideKeyboard("herdr doc rename launch")
            return (app, qa)
        }
        /// A double tap on the current space's name opens the native field (as #165's R2b;
        /// a Herdr 2 row has no long-press menu, `sidebar/view.rs`), and the name and Return
        /// commit it. QAAx places the taps from the mark's painted frame.
        func renameCurrentSpace(_ app: XCUIApplication, _ qa: QAAx, to name: String) throws {
            let row = try qa.find(QAIds.sidebarCurrentSpace)
            try qa.coordinate(row, id: QAIds.sidebarCurrentSpace, at: CGVector(dx: 0.5, dy: 0.5))
                .doubleTap()
            let field = app.textFields.firstMatch
            XCTAssertTrue(field.waitForExistence(timeout: 15), "the rename field opened")
            app.typeText(name + "\n")
        }
        // S2
        var (app, qa) = try launch(conflicts: 0)
        try renameCurrentSpace(app, qa, to: "ship")
        let saved = try qa.waitState("the renamed space is written and read back", timeout: 30) {
            $0.herdrDocWrites == 1 && $0.herdrDocLast == "saved" && $0.herdrDocSpaces == ["ship:2", "beta:1"]
        }
        XCTAssertEqual(saved.herdrDocConflicts, 0, "\(saved)")
        app.terminate()

        // S3
        (app, qa) = try launch(conflicts: 1)
        try renameCurrentSpace(app, qa, to: "ship")
        try qa.waitState("the rename is saved after one conflict", timeout: 30) {
            $0.herdrDocWrites == 1 && $0.herdrDocConflicts == 1 && $0.herdrDocLast == "saved"
                && $0.herdrDocSpaces == ["ship:2", "beta:1"]
        }
        // p9-check requires this attachment, so a filtered-out test cannot pass unseen.
        save("p9-herdr-doc-rename")
        XCTAssertEqual(app.state, .runningForeground)
    }
}
