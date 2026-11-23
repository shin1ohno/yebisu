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
        let longPressPaste = pasteMenu.waitForExistence(timeout: 1)
        report(["name": "host-row-long-press", "x": host.midX, "y": host.midY, "row_menu": opened, "paste": longPressPaste])
        save("l61-host-row-long-press")
        XCTAssertTrue(opened, "L61 the host row's long-press menu did not open")
        XCTAssertFalse(longPressPaste, "L61 a long press on the host row showed the edit menu")
        // A tap beside the menu closes it; the terminal takes no input from that tap.
        coordinate(CGPoint(x: terminal.midX, y: terminal.minY + terminal.height * 0.8)).tap()
        _ = try? qa.waitState("the host row's menu closed", timeout: 5) { $0.isNull("row_menu") }

        // The rail: Command-B collapses the sidebar to it.
        app.typeKey("b", modifierFlags: .command)
        guard (try? qa.waitState("the rail is shown", timeout: 10, { $0.windowFrame(QAIds.railSettings) != nil })) != nil else {
            save("l61-rail-missing")
            XCTFail("L61 the rail did not appear after Command-B")
            return
        }
        // Command-B can take the software keyboard down for a moment and bring it back, and a frame
        // read in between puts the point on the keyboard (PR 132 run 45148358822 hit its "~" key; a
        // mini run on 2026-11-24 read the rail's footer at 1208 pt while it was painted at 871 pt).
        // Wait for the keyboard height the app launched with, then read the rail and the terminal
        // from the same state.
        _ = try? qa.waitState("the keyboard is back at \(state.keyboard) pt", timeout: 10) { $0.keyboard == state.keyboard }
        _ = try? qa.settledKeyboard("after Command-B")
        guard let settled = qa.read(), let rail = settled.windowFrame(QAIds.railSettings),
              let content = settled.windowFrame(QAIds.terminalInput) else {
            save("l61-rail-missing")
            XCTFail("L61 no rail frame after the keyboard settled")
            return
        }
        print("L61 FRAMES rail-settings=\(rail) terminal=\(content) keyboard=\(settled.keyboard)")
        guard rail.minY - 60 > content.minY, rail.minY - 60 < content.maxY else {
            save("l61-rail-off")
            XCTFail("L61 the rail point \(rail.minY - 60) is outside the content rows \(content.minY)...\(content.maxY)")
            return
        }
        // The empty rail above the footer's Hosts and Settings buttons.
        try secondaryClick(Target(name: "rail", point: CGPoint(x: rail.midX, y: rail.minY - 60), paste: false))
        try secondaryClick(Target(name: "rail-item", point: CGPoint(x: rail.midX, y: host.midY), paste: false))
        coordinate(CGPoint(x: rail.midX, y: host.midY)).press(forDuration: 1.2)
        let railPaste = pasteMenu.waitForExistence(timeout: 1)
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
        // What the app saw: `menu_secondary` counts secondary (or Control-) clicks that reached the
        // terminal's listener, `menu_presented` the edit menus the app itself presented.
        // The first pointer event after launch can be lost before GPUI sees it (PR 132 run
        // 45142793709: secondary 0 on the first terminal click, 1 on the second). On the terminal
        // the app's receipt tells a lost click apart, so it is clicked once more; the attempts are
        // reported.
        let before = qa.read()
        var attempts = 0
        var shown = false
        var after: QAState?
        repeat {
            attempts += 1
            coordinate(target.point).rightClick()
            shown = pasteMenu.waitForExistence(timeout: target.paste ? 5 : 2)
            after = qa.read()
        } while target.paste && !shown && attempts < 2
            && (after?.count("menu_secondary") ?? 0) == (before?.count("menu_secondary") ?? 0)
        let items = app.menuItems.allElementsBoundByIndex.map(\.label)
        let cards = pasteQuery(.button).count
        let delta = { (key: String) -> Int in (after?.count(key) ?? 0) - (before?.count(key) ?? 0) }
        report(["name": target.name, "x": target.point.x, "y": target.point.y, "paste": shown, "expected": target.paste,
                "items": items, "cards": cards, "secondary": delta("menu_secondary"), "presented": delta("menu_presented"),
                "attempts": attempts])
        save("l61-\(target.name)")
        XCTAssertEqual(shown, target.paste, "L61 \(target.name): Paste shown \(shown), expected \(target.paste), items \(items)")
        if shown {
            // Close the menu without touching the terminal or a row.
            app.typeKey(XCUIKeyboardKey.escape, modifierFlags: [])
            if !pasteMenu.waitForNonExistence(timeout: 3) {
                coordinate(CGPoint(x: target.point.x, y: 4)).tap()
                _ = pasteMenu.waitForNonExistence(timeout: 3)
            }
        }
    }

    /// Paste in an edit menu, in either form: a MenuItem in the menu the app presents itself, or a
    /// Button in the card UIKit presents by itself on a secondary click (214 x 68 pt; the form the
    /// owner saw, and the one a mini run of this test on origin/main db1788f captured over every
    /// sidebar, tab row and status bar click while `menuItems` alone saw none). The software
    /// keyboard's shortcut bar keeps its own Paste button, which is not an edit menu.
    private var pasteMenu: XCUIElement {
        pasteQuery(.menuItem, .button).firstMatch
    }

    private func pasteQuery(_ types: XCUIElement.ElementType...) -> XCUIElementQuery {
        let type = NSCompoundPredicate(orPredicateWithSubpredicates: types.map {
            NSPredicate(format: "elementType == %d", Int($0.rawValue))
        })
        let paste = NSPredicate(format: "label == %@ AND identifier != %@", "Paste", "assistantPaste:forEvent:")
        return app.descendants(matching: .any).matching(NSCompoundPredicate(andPredicateWithSubpredicates: [type, paste]))
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
