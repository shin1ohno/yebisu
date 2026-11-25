import XCTest

// #144 D2: the official app's herdr-app document read from the mock core (YEBISU_CORE_DOC_QA
// names a fixture of crates/yebisu/fixtures/herdr-app/). Controls are pressed by QA identifier
// and every result is read from qa.state (herdr_doc); p9-check validate_doc counts the doc.set
// receipts the mock core wrote, which must be none while the document is only read.
extension QAState {
    var herdrDoc: [String: Any]? { raw["herdr_doc"] as? [String: Any] }
    var herdrDocState: String? { herdrDoc?["state"] as? String }
    var herdrDocLocalTabs: Int? { herdrDoc?["local_tabs"] as? Int }
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
        /// Launches the app on the mock core with the document `fixture`.
        func launchHerdrDoc(_ fixture: String) -> (XCUIApplication, QAAx) {
            let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
            app.terminate()
            app.launchEnvironment["YEBISU_CORE_QA"] = "1"
            app.launchEnvironment["YEBISU_CORE_DOC_QA"] = fixture
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
}
