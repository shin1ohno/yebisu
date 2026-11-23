import XCTest

// L61 (owner report 2026-11-23, TestFlight build 6): a pointer's secondary click on the sidebar's
// AGENTS rows showed the native edit menu with Paste. The menu belongs where Paste applies, the
// terminal's content area; the sidebar (host, space and AGENTS rows), the rail, the tab row and
// the status bar show none. Targets are found from the GPUI frames qa.state reports
// (YEBISU_QA_AX_FRAMES), not by reading the screen. Every click is printed as one
// `L61 TARGET {json}` line and saved as a screenshot; scripts/ios/sh1_152_targets.py judges the
// lines after the run and marks each click on its screenshot.
final class EditMenuTargetsQA: XCTestCase {
    private struct Target {
        let name: String
        let point: CGPoint
        let paste: Bool
    }

    private var app: XCUIApplication!
    private var qa: QAAx!

    func testL61SecondaryClickTargets() throws {
        continueAfterFailure = true
        try launch()
        let state = try XCTUnwrap(qa.read())
        let list = try frame(state, QAIds.sidebarList)
        let host = try frame(state, QAIds.sidebarHostAlpha)
        let terminal = try frame(state, QAIds.terminalInput)
        let more = try frame(state, QAIds.tabsMore)
        print("L61 FRAMES list=\(list) host=\(host) terminal=\(terminal) more=\(more) app=\(app.frame)")
        // The first space row of qa-alpha lies right under its host row (rows: host 64 pt, space 56 pt,
        // agent 44 pt, crates/yebisu/src/shell/sidebar.rs); the AGENTS section ends the list.
        let targets = [
            Target(name: "terminal", point: CGPoint(x: terminal.midX, y: terminal.minY + terminal.height * 0.6), paste: true),
            Target(name: "agents-row", point: CGPoint(x: list.minX + list.width * 0.3, y: list.maxY - 22), paste: false),
            Target(name: "space-row", point: CGPoint(x: host.midX, y: host.maxY + 28), paste: false),
            Target(name: "host-row", point: CGPoint(x: host.midX, y: host.midY), paste: false),
            Target(name: "tab-row", point: CGPoint(x: terminal.midX, y: more.midY), paste: false),
            Target(name: "status-bar", point: CGPoint(x: terminal.midX, y: terminal.maxY + 20), paste: false),
            // Again after the others: the terminal's own menu still opens.
            Target(name: "terminal-again", point: CGPoint(x: terminal.midX, y: terminal.minY + terminal.height * 0.4), paste: true),
        ]
        for target in targets { try secondaryClick(target) }

        // A long press on a sidebar row keeps opening that row's menu, without an edit menu.
        coordinate(CGPoint(x: host.midX, y: host.midY)).press(forDuration: 1.2)
        let opened = (try? qa.waitState("the host row's long-press menu", timeout: 5) { $0.string("row_menu") == "host" }) != nil
        let longPressPaste = app.menuItems["Paste"].waitForExistence(timeout: 1)
        report(["name": "host-row-long-press", "x": host.midX, "y": host.midY, "row_menu": opened, "paste": longPressPaste])
        save("l61-host-row-long-press")
        XCTAssertTrue(opened, "L61 the host row's long-press menu did not open")
        XCTAssertFalse(longPressPaste, "L61 a long press on the host row showed the edit menu")
        // A tap beside the menu closes it; the terminal takes no input from that tap.
        coordinate(CGPoint(x: terminal.midX, y: terminal.minY + terminal.height * 0.8)).tap()
        _ = try? qa.waitState("the host row's menu closed", timeout: 5) { $0.isNull("row_menu") }

        // The rail: Command-B collapses the sidebar to it.
        app.typeKey("b", modifierFlags: .command)
        guard let railState = try? qa.waitState("the rail is shown", timeout: 10, { $0.windowFrame(QAIds.railSettings) != nil }),
              let rail = railState.windowFrame(QAIds.railSettings) else {
            save("l61-rail-missing")
            XCTFail("L61 the rail did not appear after Command-B")
            return
        }
        print("L61 FRAMES rail-settings=\(rail)")
        try secondaryClick(Target(name: "rail", point: CGPoint(x: rail.midX, y: rail.minY - 160), paste: false))
        try secondaryClick(Target(name: "rail-item", point: CGPoint(x: rail.midX, y: host.midY), paste: false))
        coordinate(CGPoint(x: rail.midX, y: host.midY)).press(forDuration: 1.2)
        let railPaste = app.menuItems["Paste"].waitForExistence(timeout: 1)
        report(["name": "rail-item-long-press", "x": rail.midX, "y": host.midY, "paste": railPaste])
        save("l61-rail-item-long-press")
        XCTAssertFalse(railPaste, "L61 a long press on a rail item showed the edit menu")
        app.terminate()
    }

    private func launch() throws {
        app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.terminate()
        app.launchEnvironment["YEBISU_TERMINAL_QA"] = "1"
        app.launchEnvironment["YEBISU_BOARD_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment["YEBISU_QA_AX"] = "1"
        app.launchEnvironment["YEBISU_QA_AX_FRAMES"] = "1"
        XCUIDevice.shared.orientation = .portrait
        app.launch()
        qa = QAAx(app, useWindowFrames: true)
        try qa.waitState("the qa-alpha terminal is connected", timeout: 60) {
            $0.string("screen") == "terminal" && $0.bool("connected") == true && $0.isNull("sheet")
                && $0.string("host") == "qa-alpha" && $0.active
                && $0.windowFrame(QAIds.sidebarHostAlpha) != nil && $0.windowFrame(QAIds.terminalInput) != nil
                && $0.windowFrame(QAIds.tabsMore) != nil && $0.windowFrame(QAIds.sidebarList) != nil
        }
        // The pointer clicks below need the terminal's input ready, as the owner's app had it.
        try qa.waitState("terminal input ready", timeout: 30) { $0.string("input_pane") != nil }
        save("l61-launched")
    }

    private func frame(_ state: QAState, _ id: String) throws -> CGRect {
        try XCTUnwrap(state.windowFrame(id), "L61 no GPUI frame for \(id): \(state.text)")
    }

    /// A point in GPUI window coordinates, as QAAx turns painted bounds into a touch.
    private func coordinate(_ point: CGPoint) -> XCUICoordinate {
        app.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: point.x, dy: point.y))
    }

    private func secondaryClick(_ target: Target) throws {
        coordinate(target.point).rightClick()
        let shown = app.menuItems["Paste"].waitForExistence(timeout: target.paste ? 5 : 2)
        let items = app.menuItems.allElementsBoundByIndex.map(\.label)
        report(["name": target.name, "x": target.point.x, "y": target.point.y, "paste": shown, "expected": target.paste,
                "items": items])
        save("l61-\(target.name)")
        XCTAssertEqual(shown, target.paste, "L61 \(target.name): Paste shown \(shown), expected \(target.paste), items \(items)")
        if shown {
            // Close the menu without touching the terminal or a row.
            app.typeKey(XCUIKeyboardKey.escape, modifierFlags: [])
            if !app.menuItems["Paste"].waitForNonExistence(timeout: 3) {
                coordinate(CGPoint(x: target.point.x, y: 4)).tap()
                _ = app.menuItems["Paste"].waitForNonExistence(timeout: 3)
            }
        }
    }

    private func report(_ fields: [String: Any]) {
        if let data = try? JSONSerialization.data(withJSONObject: fields, options: [.sortedKeys]),
           let line = String(data: data, encoding: .utf8) {
            print("L61 TARGET \(line)")
        }
    }

    private func save(_ name: String) {
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
