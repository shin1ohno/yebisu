import XCTest

// UI test redesign S4: every GPUI control is found by its QA identifier (crates/yebisu/src/qa_ax_ids.rs)
// and pressed with a real touch at its frame, and every wait reads `qa.state` (QAAx.swift). Nothing
// reads the screen with OCR, remembers where a label was seen, or reads the app's receipt files while
// the app runs: sh1-152-check.py reads sh1-152-*.jsonl, p4-peer.jsonl and p9-core-wire.jsonl after
// the app was terminated, and its oracles decide each case from them, as before. A wait whose
// condition is not reached is logged, not failed, where it was before (the oracle decides); a press
// that never reaches its control, or that something else takes, fails by name (QAAx.press).
final class EditMenuQA: XCTestCase {
    private enum SetupError: Error {
        case portraitScreenUnavailable
    }

    // The case the app was launched with; markers and clipboard texts carry it.
    private var caseName = ""

    func testSH1152Basic() throws {
        for name in ["E1-text", "E1-key", "E2-text", "E2-key", "E1-core", "E2-core"] {
            let (app, ax) = try launch(name)
            try begin(QAIds.sh152Begin, ax)
            if name.hasPrefix("E2") { try select(QAIds.sh152Select, ax) }
            try menu(app, ax, name, selected: name.hasPrefix("E2"))
            let action = name.hasPrefix("E2") ? "Copy" : "Paste"
            XCTAssertTrue(app.menuItems[action].isEnabled)
            choose(action, app, ax)
            try end(QAIds.sh152End, app, ax)
            save(name)
            app.terminate()
        }
    }

    func testSH1152Transitions() throws {
        for name in ["E3a", "E3b", "E3c", "E3d", "E3e", "E3f-text", "E3f-key", "E3g-text", "E3g-key", "E3h", "E3i", "E3a-core"] {
            let (app, ax) = try launch(name)
            try begin(QAIds.sh152Begin, ax)
            let selected = name == "E3c" || name.hasPrefix("E3f")
            if selected { try select(QAIds.sh152Select, ax) }
            try menu(app, ax, name, selected: selected)
            if name == "E3d" {
                leaveAndReturn(app, ax, "E3d")
            } else if name == "E3e" {
                try overlay(app, ax)
            } else {
                if name == "E3h" {
                    print("SH1-152 HIDE KEYBOARD QUERY\n\(app.debugDescription)")
                    let hide = app.buttons.matching(NSPredicate(format: "label CONTAINS[c] 'Hide keyboard'")).firstMatch
                    if hide.exists && hide.isHittable {
                        let rows = ax.read()?.raw["rows"] as? Int
                        hide.tap()
                        ax.keyboardMayMove("E3h hide key")
                        wait("E3h keyboard hidden", ax) { $0.raw["rows"] as? Int != rows }
                    } else {
                        print("SH1-152 E3h keyboard hide key unavailable; independent switch inputs still recorded")
                    }
                }
                try mutate(name, app, ax)
                // Restore changes the keyboard; the controls above it move until it settles
                // (PR run 43633426435).
                if name.hasPrefix("E3f") {
                    try press(QAIds.sh152Restore, app, ax)
                    try ax.settledKeyboard("\(name) after Restore")
                }
            }
            try replay(QAIds.sh152Replay, ordinary: false, app, ax)
            try end(QAIds.sh152End, app, ax)
            save("stale-\(name)")
            if name == "E3i" {
                try fresh(app, ax)
                // Ask the real delegate for an unrelated configuration, then replay its ordinary commands.
                try menu(app, ax, name, selected: false)
                dismissTap(app, ax)
                dismissed(app, name)
                try replay(QAIds.sh152Ordinary, ordinary: true, app, ax)
                try end(QAIds.sh152End, app, ax)
            }
            try fresh(app, ax)
            try menu(app, ax, name, selected: false)
            choose("Paste", app, ax)
            try end(QAIds.sh152End, app, ax)
            app.terminate()
        }
    }

    func testSH1152DismissAndSystemEdit() throws {
        for name in ["E4-text", "E4-key", "E7"] {
            let (app, ax) = try launch(name)
            try begin(QAIds.sh152Begin, ax)
            try menu(app, ax, name, selected: false)
            dismissTap(app, ax)
            dismissed(app, name)
            if name == "E7" {
                let edit = app.descendants(matching: .any).matching(NSPredicate(format: "label == 'Edit'")).firstMatch
                if edit.exists && edit.isHittable {
                    edit.tap()
                    XCTAssertTrue(app.menuItems["Paste"].waitForExistence(timeout: 4))
                    app.menuItems["Paste"].tap()
                    print("SH1-152 E7 system Edit Paste selected")
                    pasted(ax)
                } else { print("SH1-152 E7 unverified: system Edit not exposed; device item 9") }
            } else {
                let inputs = ax.read()?.peerInputs ?? 0
                app.typeText("x")
                wait("\(name) typed input at peer", ax) { $0.peerInputs > inputs }
            }
            try end(QAIds.sh152End, app, ax)
            app.terminate()
        }
    }

    // E5: two real UIKit scenes. Scene A shows "AAA-152" on w1:p1; scene B (opened by Second, its
    // controls are qa.sh1-152.two.*) shows "BBB-152" on w1:p3. The QA layer follows the newest
    // active scene (qa_ax_layer.rs, `Windows`), so once B is focused its controls and its root
    // fields are the ones exposed.
    func testSH1152TwoScenes() throws {
        for name in ["E5a", "E5b", "E5c", "E5d"] {
            let (app, ax) = try launch(name)
            let selectFirst = name == "E5b" || name == "E5c"
            try begin(QAIds.sh152Begin, ax)
            if selectFirst { try select(QAIds.sh152Select, ax) }
            try menu(app, ax, name, selected: selectFirst)
            if name == "E5a" { choose("Paste", app, ax) }
            else if name == "E5b" { choose("Copy", app, ax) }
            else {
                dismissTap(app, ax)
                dismissed(app, name)
            }
            try end(QAIds.sh152End, app, ax)
            save("\(name)-first")
            try second(app, ax)
            save("\(name)-second-open")
            if name == "E5d" { try closeOther(app, ax) }
            try begin(QAIds.sh152TwoBegin, ax)
            if name == "E5b" { try select(QAIds.sh152TwoSelect, ax) }
            try menu(app, ax, name, selected: name == "E5b")
            choose(name == "E5b" ? "Copy" : "Paste", app, ax)
            try end(QAIds.sh152TwoEnd, app, ax)
            save(name)
            // Leave one scene session, as every other case expects at launch.
            if name != "E5d" {
                try closeOther(app, ax)
                // UIKit destroys the closed scene's session outside the app, which records nothing
                // more after close-other; the next launch must not restore it.
                sleep(3)
            }
            app.terminate()
        }
    }

    private func launch(_ name: String) throws -> (XCUIApplication, QAAx) {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.terminate()
        caseName = name
        app.launchEnvironment["YEBISU_SH1_152_CASE"] = name
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment["YEBISU_QA_AX"] = "1"
        app.launchEnvironment[name.contains("core") ? "YEBISU_CORE_QA" : "YEBISU_TERMINAL_QA"] = "1"
        // Keep the E-launch measurement scoped to app.launch() (docs/ci-simulator.md).
        let launchStarted = Date()
        app.launch()
        print("SH1-152 LAUNCH \(name) \(String(format: "%.1f", Date().timeIntervalSince(launchStarted)))")
        let ax = QAAx(app)
        // A Begin marker needs an input destination, which the app has once the terminal names its
        // pane. qa.state that never appears is a broken QA layer: fail by name.
        do {
            try ax.waitState("launch \(name) qa.state", timeout: 30) { _ in true }
        } catch {
            XCTFail("SH1-152 qa.state not exposed after launch \(name) (YEBISU_QA_AX): \(error)")
            throw error
        }
        try requirePortraitScreen(name)
        wait("launch \(name) input destination", ax, seconds: 30) { $0.string("input_pane") != nil }
        frame("launch \(name)", app)
        // Every relaunch (each case launches its own app) first waits until a touch reaches GPUI,
        // on the strip's empty space, so the case's first real press is not lost (QAAx.touchReady).
        do {
            let presses = try ax.touchReady(QAIds.sh152Empty, what: "launch \(name)")
            print("SH1-152 TOUCH-READY \(name) \(presses)")
        } catch {
            save("touch-not-ready-\(name)")
            XCTFail("SH1-152 no touch reached GPUI after launch \(name): \(error)")
            throw error
        }
        return (app, ax)
    }

    private func requirePortraitScreen(_ name: String) throws {
        let initial = screenSize()
        if initial.width > 0 && initial.height > initial.width {
            print("SH1-152 PORTRAIT \(name) already portrait: \(initial)")
            return
        }
        // A fresh Simulator can already show a portrait interface while an extra synthetic
        // orientation request is still awaiting confirmation (run 44765771959).
        if initial.height > 0 && initial.width > initial.height {
            XCUIDevice.shared.orientation = .portrait
        }
        let portrait = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in
            let screen = self.screenSize()
            return screen.width > 0 && screen.height > screen.width
        }, object: nil)
        guard XCTWaiter().wait(for: [portrait], timeout: 10) == .completed else {
            XCTFail("SH1-152 portrait screen not observed for \(name): initial=\(initial) last=\(screenSize())")
            throw SetupError.portraitScreenUnavailable
        }
        print("SH1-152 PORTRAIT \(name) settled: \(screenSize())")
    }

    // Records whether the app still fills the screen, so a moved window shows up in the log at the
    // step that moved it.
    private func frame(_ step: String, _ app: XCUIApplication) {
        let screen = screenSize()
        let frame = app.frame
        let full = abs(frame.minX) < 1 && abs(frame.minY) < 1 && abs(frame.width - screen.width) < 1 && abs(frame.height - screen.height) < 1
        print("SH1-152 FRAME \(step) app=\(frame) screen=\(screen) fullscreen=\(full)")
    }

    // A drag in screen space (Control Center from the status corner): app.coordinate is relative
    // to the app's own frame, which main run 43668219161 saw smaller than the screen.
    private func screenPoint(_ app: XCUIApplication, _ x: CGFloat, _ y: CGFloat) -> XCUICoordinate {
        let screen = screenSize()
        let origin = app.frame.origin
        return app.coordinate(withNormalizedOffset: .zero)
            .withOffset(CGVector(dx: screen.width * x - origin.x, dy: screen.height * y - origin.y))
    }

    // Screen size in points: SpringBoard always spans the whole screen.
    private func screenSize() -> CGSize {
        XCUIApplication(bundleIdentifier: "com.apple.springboard").frame.size
    }

    // Reads qa.state until `condition` holds, for at most `seconds`. A condition that is not
    // reached is logged, not failed: the receiving-system oracle in sh1-152-check.py decides the
    // case from the receipts, as it did when these waits read the files.
    @discardableResult
    private func wait(_ step: String, _ ax: QAAx, seconds: TimeInterval = 10, _ condition: (QAState) -> Bool) -> QAState? {
        do {
            let state = try ax.waitState(step, timeout: seconds, condition)
            print("SH1-152 WAIT \(step): reached")
            return state
        } catch {
            print("SH1-152 WAIT \(step): not reached in \(Int(seconds))s: \(error)")
            return nil
        }
    }

    // Presses the control `id` (QAAx.press: the press must reach it, once more if it was lost).
    // The QA strip moves with the keyboard, so the press waits for the keyboard to settle when
    // something since the last settle may have moved it (`keyboardMayMove`: a launch, Begin, Fresh,
    // Mutate, Restore, the E1 raise, a long press, a dismiss tap, Home, Control Center, a scene
    // opened or closed; S5, PR 81 review S2). If an open edit menu lies over
    // the control's centre, the end of the control outside the menu is pressed instead (runs
    // 43564245188, 43613569523 saw the menu cover the start of "152 Mutate").
    @discardableResult
    private func press(_ id: String, _ app: XCUIApplication, _ ax: QAAx, what: String = "the press's effect",
                       until: ((QAState) -> Bool)? = nil) throws -> QAState {
        try ax.settleIfMoved("before \(id)")
        let element = try ax.find(id)
        let offset = visiblePart(element.frame, app, id)
        return try ax.press(id, at: offset, what: what, until: until)
    }

    private func visiblePart(_ frame: CGRect, _ app: XCUIApplication, _ id: String) -> CGVector {
        let items = app.menuItems.allElementsBoundByIndex.map(\.frame).filter { !$0.isEmpty }
        guard let first = items.first else { return CGVector(dx: 0.5, dy: 0.5) }
        // The menu's bubble extends past its items.
        let bubble = items.dropFirst().reduce(first) { $0.union($1) }.insetBy(dx: -24, dy: -24)
        guard bubble.contains(CGPoint(x: frame.midX, y: frame.midY)) else { return CGVector(dx: 0.5, dy: 0.5) }
        for x in [0.12, 0.88] where !bubble.contains(CGPoint(x: frame.minX + frame.width * x, y: frame.midY)) {
            print("SH1-152 PRESS \(id): the edit menu covers its centre; pressing at x \(x) of \(frame)")
            return CGVector(dx: x, dy: 0.5)
        }
        print("SH1-152 PRESS \(id): the edit menu covers all of \(frame)")
        return CGVector(dx: 0.5, dy: 0.5)
    }

    // Owner rule (iOS convention): only usable items are shown. Without a selection the menu is
    // Paste alone; with a selection it is Copy and Paste. The long press is on the terminal's own
    // frame (qa.terminal.input): where in the terminal does not matter, the selection does
    // (edit_menu_gestures.rs presents for any point of the terminal grid).
    private func menu(_ app: XCUIApplication, _ ax: QAAx, _ name: String, selected: Bool) throws {
        // Only the E1 cases, which check what the menu presents, raise the keyboard first: a tap
        // would clear an E2 selection, and it moves the responder the E3 to E5 cases test
        // (Mac mini 6f2d709: E3g-key "successful internal switch revoked or did not occur").
        if name.hasPrefix("E1") { try keyboardUpBeforePress(app, ax, name) }
        try ax.settleIfMoved("\(name) before the long press")
        let before = try ax.waitState("\(name) qa.state before the long press", timeout: 5) { _ in true }
        let terminal = try ax.find(QAIds.terminalInput)
        // Upper third: the menu opens above the press, away from the QA strip under the terminal.
        terminal.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.3)).press(forDuration: 0.8)
        // The long press focuses the terminal; the keyboard moved after it in 6 of 126 Mac mini
        // long presses (48c8b55).
        ax.keyboardMayMove("\(name) long press")
        XCTAssertTrue(app.menuItems["Paste"].waitForExistence(timeout: 4), "real menu missing: \(name)\n\(app.debugDescription)")
        let labels = app.menuItems.allElementsBoundByIndex.map { $0.label }
        print("SH1-152 VISIBLE \(name) selected=\(selected) labels=\(labels.joined(separator: ","))")
        XCTAssertEqual(labels.sorted(), selected ? ["Copy", "Paste"] : ["Paste"])
        if !selected { XCTAssertFalse(app.menuItems["Copy"].exists, "Copy shown without a selection: \(name)") }
        save("menu-\(name)")
        // The app records the presented commands 150 ms after UIKit builds the menu; a command
        // chosen before that would precede its capture. A second long press would be a second
        // presentation, so a press that did not reach the terminal is not repeated.
        let captured = wait("\(name) menu capture recorded", ax, seconds: 5) { $0.count("sh152_capture") > before.count("sh152_capture") }
        print("SH1-152 LONG PRESS \(name) presses[\(QAIds.terminalInput)] \(before.presses(QAIds.terminalInput)) -> \(captured?.presses(QAIds.terminalInput) ?? -1)")
    }

    // The long press focuses the terminal. If the software keyboard was down, it rises and UIKit
    // dismisses the menu that same press opened: run 44297932478 (edit-menu job 134779801414),
    // E1-core, and E1-text on a fresh Mac mini Simulator (f95123d). So the keyboard is raised first
    // with a tap on the terminal, and the long press waits until it settled; its frame is the one
    // the app publishes after that. A Simulator whose keyboard stays down (a hardware keyboard) is
    // left as it is.
    private func keyboardUpBeforePress(_ app: XCUIApplication, _ ax: QAAx, _ name: String) throws {
        if try ax.settledKeyboard("\(name) keyboard before the press").keyboardUp { return }
        print("SH1-152 \(name): software keyboard down before the press; tapping the terminal to raise it")
        try ax.press(QAIds.terminalInput)
        ax.keyboardMayMove("\(name) terminal tap to raise the keyboard")
        wait("\(name) software keyboard up before the press", ax, seconds: 5) { $0.keyboardUp }
    }

    // Selects a menu item (UIKit's own element) and waits for its execution; an executed Paste is
    // also waited for at the peer.
    private func choose(_ item: String, _ app: XCUIApplication, _ ax: QAAx) {
        let before = ax.read()
        app.menuItems[item].tap()
        let after = wait("\(caseName) \(item) execution recorded", ax) { $0.count("menu_exec") > (before?.count("menu_exec") ?? 0) }
        if let after, after.count("menu_paste_executed") > (before?.count("menu_paste_executed") ?? 0) { pasted(ax) }
    }

    // Closes the edit menu with a tap beside it, on the terminal's right edge; the tap can reach
    // the terminal and focus it.
    private func dismissTap(_ app: XCUIApplication, _ ax: QAAx) {
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.98, dy: 0.5)).tap()
        ax.keyboardMayMove("tap beside the edit menu")
    }

    private func dismissed(_ app: XCUIApplication, _ name: String) {
        if app.menuItems["Paste"].waitForNonExistence(timeout: 5) {
            print("SH1-152 WAIT \(name) menu dismissed: reached")
        } else {
            print("SH1-152 WAIT \(name) menu dismissed: not reached in 5s")
        }
    }

    // Begin starts a stage: the app sends the start marker (and relabels the control "Begun",
    // ignoring a repeat). A press that never reached Begin is pressed once more by QAAx.press (PR run
    // 43659150803 lost one); a press that reached it without a start marker fails by name.
    private func begin(_ id: String, _ ax: QAAx) throws {
        let before = try ax.waitState("qa.state before \(id)", timeout: 5) { _ in true }
        do {
            try press(id, ax.app, ax, what: "start marker recorded") { $0.count("sh152_start") > before.count("sh152_start") }
            ax.keyboardMayMove("\(id)")
        } catch {
            save("missing-\(id)")
            XCTFail("\(id) was not acknowledged: \(error)")
            throw error
        }
        marked("start", ax)
    }

    private func fresh(_ app: XCUIApplication, _ ax: QAAx) throws {
        let before = try ax.waitState("qa.state before Fresh", timeout: 5) { _ in true }
        try press(QAIds.sh152Fresh, app, ax)
        ax.keyboardMayMove("Fresh")
        if wait("Fresh start marker recorded", ax, { $0.count("sh152_start") > before.count("sh152_start") }) != nil {
            marked("start", ax)
        }
    }

    // End records the stage's result and sends its end marker. The press is confirmed by
    // `presses` (main 55899c8, E5d: a tap on "152 Two End" was synthesized and nothing in the app
    // ran; QAAx.press now presses once more when the press never reached the control, and names a
    // press that reached it without the marker).
    private func end(_ id: String, _ app: XCUIApplication, _ ax: QAAx) throws {
        let before = try ax.waitState("qa.state before \(id)", timeout: 5) { _ in true }
        try press(id, app, ax)
        if wait("\(id) end marker recorded", ax, { $0.count("sh152_end") > before.count("sh152_end") }) != nil {
            marked("end", ax)
        }
    }

    private func select(_ id: String, _ ax: QAAx) throws {
        let before = try ax.waitState("qa.state before \(id)", timeout: 5) { _ in true }
        try press(id, ax.app, ax)
        wait("\(id) selection recorded", ax) { $0.count("sh152_selection") > before.count("sh152_selection") }
    }

    private func replay(_ id: String, ordinary: Bool, _ app: XCUIApplication, _ ax: QAAx) throws {
        let before = try ax.waitState("qa.state before \(id)", timeout: 5) { _ in true }
        try press(id, app, ax)
        let after = wait("\(id) replay recorded", ax) { $0.count("sh152_replay") > before.count("sh152_replay") }
        // A replayed Paste that UIKit executes, and the ordinary suggestions E3i replays without a
        // menu origin, both paste the current clipboard.
        if ordinary || (after?.count("menu_paste_executed") ?? 0) > before.count("menu_paste_executed") { pasted(ax) }
    }

    // Each Mutate ends on the state it changes.
    private func mutate(_ name: String, _ app: XCUIApplication, _ ax: QAAx) throws {
        let before = try ax.waitState("qa.state before Mutate", timeout: 5) { _ in true }
        try press(QAIds.sh152Mutate, app, ax)
        ax.keyboardMayMove("\(name) Mutate")
        let field = { (state: QAState, key: String) in "\(state.raw[key] ?? "none")" }
        switch name {
        case "E3a", "E3i", "E3a-core":
            wait("\(name) Mutate destination moved", ax) {
                $0.count("sh152_destination_change") > before.count("sh152_destination_change")
                    && field($0, "input_pane") != field(before, "input_pane")
            }
        case "E3b":
            // A new link generation that drew a connected frame.
            wait("\(name) Mutate new link generation", ax) {
                field($0, "generation") != field(before, "generation") && $0.bool("connected") == true && !$0.isNull("surface")
            }
        case "E3c":
            wait("\(name) Mutate menu revoked", ax) { $0.count("menu_revoked") > before.count("menu_revoked") }
        case "E3f-text", "E3f-key":
            wait("\(name) Mutate external responder", ax) { $0.count("sh152_external") > before.count("sh152_external") }
        case "E3g-text", "E3g-key":
            wait("\(name) Mutate keyboard resized the terminal", ax) { field($0, "rows") != field(before, "rows") }
        case "E3h":
            wait("\(name) Mutate switch observed", ax) { $0.count("sh152_switch") > before.count("sh152_switch") }
        default:
            break
        }
    }

    // E3e: Control Center over the app. A drag from the top centre does not open a system overlay
    // on the hosted iPadOS 26 Simulator: it grabs the full-screen app as a window and moves it
    // off-screen, with no lifecycle change (main run 43668219161 screen recording). Pull Control
    // Center from the top-right status corner, which the system owns, in screen space. The overlay
    // makes the scene inactive (`active` false, `lifecycle_inactive` grows); closing it makes it
    // active again. If it stays open, leaving through Home adds a Background phase, so the oracle
    // reports E3e unverified (device item 10), never a false pass.
    private func overlay(_ app: XCUIApplication, _ ax: QAAx) throws {
        let before = try ax.waitState("E3e qa.state before the overlay", timeout: 5) { _ in true }
        frame("E3e before overlay", app)
        screenPoint(app, 0.97, 0.002).press(forDuration: 0.1, thenDragTo: screenPoint(app, 0.97, 0.5))
        ax.keyboardMayMove("E3e Control Center")
        let opened = wait("E3e overlay open (scene inactive)", ax, seconds: 8) {
            !$0.active || $0.count("lifecycle_inactive") > before.count("lifecycle_inactive")
        } != nil
        save("E3e-overlay")
        if opened {
            // Tap the empty left side; Control Center's modules sit on the right.
            screenPoint(app, 0.2, 0.5).tap()
            wait("E3e Active after overlay", ax, seconds: 8) {
                $0.active && $0.count("lifecycle_active") > before.count("lifecycle_active")
            }
        }
        // An app that is no longer full screen means a gesture moved its window; the receipt names
        // it instead of a missing label.
        frame("E3e after overlay", app)
        if !(ax.read()?.active ?? false) {
            print("SH1-152 E3e overlay stayed open; leaving through Home")
            leaveAndReturn(app, ax, "E3e")
        }
    }

    // Home, then back: the app goes to the background (XCTest's own app state), and once it is
    // active again its qa.state shows the Background and Active it counted and an input
    // destination (it reconnected and drew the terminal).
    private func leaveAndReturn(_ app: XCUIApplication, _ ax: QAAx, _ step: String) {
        let before = ax.read()
        XCUIDevice.shared.press(.home)
        ax.keyboardMayMove("\(step) Home and back")
        if app.wait(for: .runningBackground, timeout: 10) || app.state == .runningBackgroundSuspended {
            print("SH1-152 WAIT \(step) Background: reached (\(app.state.rawValue))")
        } else {
            print("SH1-152 WAIT \(step) Background: not reached in 10s (\(app.state.rawValue))")
        }
        app.activate()
        wait("\(step) Background and Active with an input destination", ax) {
            $0.count("lifecycle_background") > (before?.count("lifecycle_background") ?? 0)
                && $0.count("lifecycle_active") > (before?.count("lifecycle_active") ?? 0)
                && $0.active && $0.string("input_pane") != nil
        }
    }

    // Opens scene B; it is ready once the QA layer moved to it (`second`), it is active, and its
    // terminal sends input to w1:p3 (the pane sh1_152_scenes.py expects).
    private func second(_ app: XCUIApplication, _ ax: QAAx) throws {
        let before = try ax.waitState("qa.state before Second", timeout: 5) { _ in true }
        try press(QAIds.sh152Second, app, ax)
        ax.keyboardMayMove("second scene")
        wait("second scene on w1:p3 and Active", ax, seconds: 15) {
            $0.count("sh152_second_ready") > before.count("sh152_second_ready")
                && $0.bool("second") == true && $0.active && $0.string("input_pane") == "w1:p3"
        }
    }

    private func closeOther(_ app: XCUIApplication, _ ax: QAAx) throws {
        let before = try ax.waitState("qa.state before Two Close", timeout: 5) { _ in true }
        try press(QAIds.sh152TwoClose, app, ax)
        ax.keyboardMayMove("first scene closed")
        wait("first scene close recorded", ax) { $0.count("sh152_close_other") > before.count("sh152_close_other") }
    }

    // The app counts a marker when it sends it; the oracle reads it at the peer (or the core wire),
    // so wait for both: the text is sh1-152-<edge>-<case>-<stage> with the stage the app reported.
    private func marked(_ edge: String, _ ax: QAAx) {
        let stage = ax.read()?.int("sh152_stage") ?? 0
        let marker = "sh1-152-\(edge)-\(caseName)-\(stage)"
        wait("\(marker) at peer", ax) { $0.peerPastes.contains(marker) }
    }

    // Waits until the current stage's clipboard text reaches the peer.
    private func pasted(_ ax: QAAx) {
        let stage = ax.read()?.int("sh152_stage") ?? 0
        let payload = "sh1-152-clip-\(caseName)-\(stage)"
        wait("\(payload) at peer", ax) { $0.peerPastes.contains(payload) }
    }

    private func save(_ name: String) {
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = "sh1-152-\(name)"
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
