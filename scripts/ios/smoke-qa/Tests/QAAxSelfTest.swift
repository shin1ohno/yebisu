import XCTest

/// S1 self-test of the QA accessibility layer and the QAAx helpers (UI test redesign plan 3.4).
/// It replaces the S0 probe (branch qa/s0-probe, never merged) and checks what S2 will rely on:
///   1. `qa.state` exists from launch on the P0 screen, where no marked control has to be drawn
///      first (S0 found no state there in 3 of 3 runs);
///   2. `qa.p0.tap` is found by identifier and three presses are each confirmed through
///      `qa.state` (`presses` and the app's own `taps` grow by one), never by `isHittable`;
///   3. a wait for a predicate that never holds fails within its bound, naming the wait;
///   4. a relaunch shows a new `launch` and fresh press counts.
/// Each timing is one `QAAX` line. Runs only with QA_AX_SELFTEST set (xcodebuild passes
/// TEST_RUNNER_QA_AX_SELFTEST; scripts/ios/qa-ax-selftest.py), so no existing job runs it.
final class QAAxSelfTest: XCTestCase {
    private static let tap = "qa.p0.tap"

    private func launch(_ app: XCUIApplication) {
        app.launchEnvironment = ["YEBISU_QA_AX": "1", "YEBISU_SMOKE_OBSERVE": "1"]
        let start = Date()
        app.launch()
        print(String(format: "QAAX launch returned after %.3f s", Date().timeIntervalSince(start)))
    }

    func testQAAxSelfTest() throws {
        try XCTSkipUnless(ProcessInfo.processInfo.environment["QA_AX_SELFTEST"] != nil, "S1 self-test only")
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        let qa = QAAx(app)
        app.terminate()
        XCUIDevice.shared.orientation = .portrait
        launch(app)

        // 1. qa.state from launch, on P0, before anything is pressed.
        let first = try qa.waitState("qa.state on the P0 screen after launch", timeout: 30) {
            $0.string("screen") == "p0" && $0.isNull("sheet") && $0.isNull("menu")
        }
        XCTAssertGreaterThan(first.launch, 0, "launch is the app's process id")
        XCTAssertEqual(first.presses(Self.tap), 0)
        XCTAssertEqual(first.duplicates, [])
        let taps = try XCTUnwrap(first.int("taps"), "taps in \(first)")

        // 2. three presses, each confirmed by qa.state.
        try qa.find(Self.tap, timeout: 10)
        for press in 1...3 {
            let state = try qa.press(Self.tap, what: "taps == \(taps + press)") { $0.int("taps") == taps + press }
            XCTAssertEqual(state.presses(Self.tap), press, "press \(press): \(state)")
            XCTAssertEqual(state.launch, first.launch)
        }

        // 3. a bounded wait fails on time and names itself.
        let start = Date()
        do {
            try qa.waitState("a predicate that never holds", timeout: 0.5) { _ in false }
            XCTFail("an impossible wait returned")
        } catch let QAAxError.timeout(what, seconds, reads, last) {
            let elapsed = Date().timeIntervalSince(start)
            print(String(format: "QAAX bounded wait failed as expected after %.3f s (%ld reads)", elapsed, reads))
            XCTAssertEqual(what, "a predicate that never holds")
            XCTAssertGreaterThanOrEqual(seconds, 0.5)
            XCTAssertLessThan(elapsed, 3, "the wait overran its bound")
            XCTAssertNotEqual(last, "nil", "the failure carries the last state")
        }

        // 4. a relaunch is a new launch with fresh counts.
        app.terminate()
        launch(app)
        let relaunched = try qa.waitState("qa.state after a relaunch", timeout: 30) {
            $0.launch != first.launch && $0.string("screen") == "p0"
        }
        XCTAssertEqual(relaunched.presses(Self.tap), 0, "\(relaunched)")
        try qa.press(Self.tap)
        print("QAAX done")
    }

    /// F5 (review of PR 75, SF3 of PR 76): a press on a marked control that an occluding layer
    /// covers is not counted. With the ... menu open, its backdrop covers the "…" button that
    /// opened it; a press there must close the menu without growing presses[qa.tabs.more], and
    /// QAAx.press must say so by name (pressTakenElsewhere, `menu` moved) without pressing again.
    func testQAAxOverlappedPress() throws {
        try XCTSkipUnless(ProcessInfo.processInfo.environment["QA_AX_SELFTEST"] != nil, "S1 self-test only")
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        let qa = QAAx(app)
        app.terminate()
        XCUIDevice.shared.orientation = .portrait
        app.launchEnvironment = ["YEBISU_QA_AX": "1", "YEBISU_SMOKE_OBSERVE": "1", "YEBISU_TERMINAL_QA": "1"]
        app.launch()
        try qa.waitState("the wire-peer terminal is up", timeout: 60) {
            $0.string("screen") == "terminal" && $0.isNull("menu") && $0.isNull("sheet")
        }
        try qa.settledKeyboard("self-test F5")
        let open = try qa.press(QAIds.tabsMore, what: "the ... menu is open") { $0.string("menu") == "overflow" }
        XCTAssertEqual(open.presses(QAIds.tabsMore), 1, "\(open)")
        let start = Date()
        do {
            try qa.press(QAIds.tabsMore, timeout: 3)
            XCTFail("a press under the menu backdrop was counted on qa.tabs.more")
        } catch let QAAxError.pressTakenElsewhere(id, moved, _, after) {
            print(String(format: "QAAX overlapped press refused as expected after %.3f s: moved %@", Date().timeIntervalSince(start), moved.joined(separator: ",")))
            XCTAssertEqual(id, QAIds.tabsMore)
            XCTAssertEqual(moved, ["menu"], "only the menu closed")
            let state = try XCTUnwrap(QAState(after))
            XCTAssertEqual(state.presses(QAIds.tabsMore), 1, "the covered press was not counted: \(state)")
            XCTAssertTrue(state.isNull("menu"), "the backdrop took the press and closed the menu: \(state)")
        }
        // Uncovered again, the same control counts the next press.
        let reopened = try qa.press(QAIds.tabsMore, what: "the ... menu is open again") { $0.string("menu") == "overflow" }
        XCTAssertEqual(reopened.presses(QAIds.tabsMore), 2, "\(reopened)")
        print("QAAX F5 done")
    }
}
