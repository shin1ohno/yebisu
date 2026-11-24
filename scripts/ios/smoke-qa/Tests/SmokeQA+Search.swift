import XCTest

// #138: the sidebar's native search panel, driven for scripts/ios/search-check.py, which judges
// every step after the run from the app's `search-state` records and the wire peer's log. The
// magnifier and the controls behind the panel are pressed by QA identifier; element queries do
// not answer while a presented panel is up in this Simulator (herdr-uikit
// scripts/native-search-qa/Tests/NativeSearch.swift:5-7), so inside the panel this test acts on
// a fixed schedule at points of the panel's layout (herdr-uikit window/native_modal/panel.rs:
// Cancel at x width-96..width-16, y 8..40; the field at y 48..84; the results from y 134, a
// section header then 44 pt rows), measured from the popover's origin. `SEARCH_STEP <name>
// <unix time>` lines split the records by step; marker letters typed into the terminal after a
// close split the peer log.
extension SmokeQA {
    private static let searchEnvironment = [
        "YEBISU_TERMINAL_QA": "1",
        "YEBISU_SMOKE_OBSERVE": "1",
        "YEBISU_QA_AX": "1",
        "YEBISU_INPUT_QA": "1",
    ]
    private static let panelWidth: CGFloat = 360

    private func searchStep(_ name: String) {
        print(String(format: "SEARCH_STEP %@ %.3f", name, Date().timeIntervalSince1970))
    }

    private func searchMark(_ name: String) {
        print(String(format: "SEARCH_MARK %@ %.3f", name, Date().timeIntervalSince1970))
    }

    private func at(_ app: XCUIApplication, _ point: CGPoint) -> XCUICoordinate {
        app.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: point.x, dy: point.y))
    }

    /// Where the popover's content starts: below the magnifier, its left edge clamped to the
    /// window's margin (UIKit keeps an arrowless popover on screen).
    private func panelOrigin(_ magnifier: CGRect, _ app: XCUIApplication) -> CGPoint {
        let x = min(max(magnifier.midX - Self.panelWidth / 2, 24), app.frame.width - Self.panelWidth - 24)
        return CGPoint(x: x, y: magnifier.maxY + 8)
    }

    private func openSearch(_ app: XCUIApplication, _ magnifier: CGRect, _ what: String) {
        at(app, CGPoint(x: magnifier.midX, y: magnifier.midY)).tap()
        print("SEARCH open \(what)")
        sleep(3)
    }

    /// Types into the terminal once the panel is closed and input came back.
    private func marker(_ app: XCUIApplication, _ letter: String) {
        sleep(2)
        app.typeText(letter)
        print("SEARCH marker \(letter)")
        sleep(1)
    }

    func testSidebarSearch() throws {
        continueAfterFailure = true
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        let qa = QAAx(app)
        app.terminate()
        app.launchEnvironment = Self.searchEnvironment
        XCUIDevice.shared.orientation = .landscapeLeft
        app.launch()
        try waitTerminal(qa, "the wire-peer terminal is connected")
        try qa.press(QAIds.terminalInput, effect: 10, what: "the software keyboard is up") { $0.keyboardUp }
        try qa.settledKeyboard("search keyboard up")
        let magnifier = try qa.find(QAIds.sidebarSearch).frame
        let origin = panelOrigin(magnifier, app)
        print("SEARCH_FRAME magnifier \(magnifier) panel origin \(origin) app \(app.frame)")
        save("search-before")

        // T-S1: the magnifier opens the popover; its field takes first responder.
        searchStep("T-S1")
        openSearch(app, magnifier, "T-S1")
        save("search-s1-open")
        // T-S2: typing goes to the field and narrows the filter.
        searchStep("T-S2")
        app.typeText("pro")
        sleep(2)
        save("search-s2-pro")
        // T-S3: Return closes it and keeps the filter; the banner shows; one key reaches the terminal.
        searchStep("T-S3")
        app.typeText("\n")
        sleep(3)
        let banner = qa.element(QAIds.sidebarFilterClear).exists
        print("SEARCH banner exists=\(banner)")
        save("search-s3-closed")
        marker(app, "z")

        // T-S4: Cancel, then a tap outside the popover; both keep the filter.
        searchStep("T-S4")
        openSearch(app, magnifier, "T-S4 cancel")
        at(app, CGPoint(x: origin.x + Self.panelWidth - 56, y: origin.y + 24)).tap()
        sleep(3)
        save("search-s4-cancel")
        openSearch(app, magnifier, "T-S4 outside")
        at(app, CGPoint(x: app.frame.width - 40, y: app.frame.height - 40)).tap()
        sleep(3)
        save("search-s4-outside")
        marker(app, "w")

        // T-S5: Command-V pastes into the field; Command-B and Command-F do nothing while it is up.
        searchStep("T-S5")
        UIPasteboard.general.string = "searchpasteword"
        openSearch(app, magnifier, "T-S5")
        app.typeKey("v", modifierFlags: .command)
        sleep(2)
        app.typeKey("b", modifierFlags: .command)
        sleep(1)
        app.typeKey("f", modifierFlags: .command)
        sleep(2)
        save("search-s5-paste")
        // Clear the field (select all, delete) so the next steps see every row.
        app.typeKey("a", modifierFlags: .command)
        app.typeKey(XCUIKeyboardKey.delete.rawValue, modifierFlags: [])
        sleep(2)
        at(app, CGPoint(x: origin.x + Self.panelWidth - 56, y: origin.y + 24)).tap()
        sleep(3)

        // T-S6: a result row opens its space; the panel closes for it.
        searchStep("T-S6")
        openSearch(app, magnifier, "T-S6")
        save("search-s6-open")
        at(app, CGPoint(x: origin.x + Self.panelWidth / 2, y: origin.y + 134 + 28 + 22)).tap()
        sleep(4)
        save("search-s6-selected")
        marker(app, "x")

        // T-S7: a composition in the terminal (the QA band's mark) is discarded at open.
        searchStep("T-S7")
        try qa.press(QAIds.bandMark)
        openSearch(app, magnifier, "T-S7")
        at(app, CGPoint(x: origin.x + Self.panelWidth - 56, y: origin.y + 24)).tap()
        sleep(3)
        marker(app, "v")

        // T-S8: background and foreground keep the field first responder; the hide key keeps it up.
        searchStep("T-S8")
        openSearch(app, magnifier, "T-S8")
        XCUIDevice.shared.press(.home)
        sleep(3)
        app.activate()
        sleep(4)
        searchMark("foreground")
        save("search-s8-foreground")
        // The software keyboard's dismiss key sits at its bottom-right corner on iPad.
        at(app, CGPoint(x: app.frame.width - 40, y: app.frame.height - 30)).tap()
        sleep(3)
        save("search-s8-hide")
        searchMark("close")
        at(app, CGPoint(x: origin.x + Self.panelWidth - 56, y: origin.y + 24)).tap()
        sleep(3)
        marker(app, "u")
        searchStep("end")
        XCTAssertEqual(app.state, .runningForeground)
    }

    /// T-S9: with YEBISU_QA_SEARCH_COMPACT=1 the panel is a page sheet, whatever the width.
    func testSidebarSearchCompact() throws {
        continueAfterFailure = true
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        let qa = QAAx(app)
        app.terminate()
        app.launchEnvironment = Self.searchEnvironment.merging(["YEBISU_QA_SEARCH_COMPACT": "1"]) { $1 }
        XCUIDevice.shared.orientation = .landscapeLeft
        app.launch()
        try waitTerminal(qa, "the wire-peer terminal is connected")
        let magnifier = try qa.find(QAIds.sidebarSearch).frame
        searchStep("T-S9")
        openSearch(app, magnifier, "T-S9")
        sleep(2)
        save("search-s9-sheet")
        searchStep("end")
        XCTAssertEqual(app.state, .runningForeground)
    }
}
