// Source pattern: SH1-153 visual probe (cbdc150400a04524, attempt 3).
// Only native OS menu buttons execute commands. UI test redesign S5: the fixture controls are found
// by QA identifier (qa.p12.*, crates/yebisu/src/qa_ax_ids.rs) and pressed with a real touch, and
// each press is confirmed by the app's qa.state (`p12.setup` counts the p12-setup-end records), so
// nothing reads text off the screen or the app's files while it runs. QAAx.swift is the SmokeQA
// helper; p12-check.py copies it next to this file. The native menu is driven through Springboard.
import XCTest

final class P12QA: XCTestCase {
    private var events: [[String: Any]] = []
    private let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
    private let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
    private lazy var qa = QAAx(app)
    /// When the previous OS menu was last observed closed; the time between a close
    /// and the next swipe is one suspected reveal precondition.
    private var lastMenuClosed: Date?
    /// The reveal-precondition record of the swipe in progress, quoted by its failures.
    private var preReveal: [String: Any] = [:]
    /// Negative controls (TEST_RUNNER_P12_QA_FAULT, comma separated, empty in CI):
    /// `drop-tap:<step>` skips the first tap of that fixture step, `skip-swipe:<case>`
    /// skips the first reveal swipe of that case. Each fault is spent once, and the
    /// retry it forces must be logged and accepted by the oracle.
    private var faults = Set((ProcessInfo.processInfo.environment["P12_QA_FAULT"] ?? "").split(separator: ",").map(String.init))
    /// How long a fixture press gets to reach its control (`presses`) before it is pressed once
    /// more, and then to show its setup step in `p12.setup`.
    private static let receiptTimeout: TimeInterval = 5
    /// How long a swipe gets to reveal the OS menu bar before it is repeated once.
    private static let revealTimeout: TimeInterval = 10
    private static let menuItemTimeout: TimeInterval = 3
    private static let menuPollInterval: useconds_t = 50_000
    private typealias MenuBarPressPosition = (point: CGPoint, reported: CGRect, useButton: Bool)
    /// What the reveal of the case in progress needed, quoted by its failure.
    private var revealNote = ""
    /// The fixture controls in panel order: the setup step each prepares and its QA identifier.
    private static let fixtureControls: [(step: String, id: String)] = [
        ("remote", "qa.p12.remote"), ("rearm", "qa.p12.rearm"), ("sidebar", "qa.p12.sidebar"),
        ("palette", "qa.p12.palette"), ("release", "qa.p12.release"), ("host", "qa.p12.host"),
        ("pane", "qa.p12.pane"), ("boot", "qa.p12.boot"), ("focus", "qa.p12.focus"),
        ("current", "qa.p12.current"), ("capability", "qa.p12.capability"), ("reset", "qa.p12.reset"),
        ("finish", "qa.p12.finish"),
    ]
    /// The empty strip at the foot of the fixture panel: GPUI space with no handler, where a tap
    /// closes an open OS menu and changes nothing in the app.
    private static let emptySpace = "qa.p12.empty"
    /// Presses on the empty strip `touchReady` may make, 3 s apart, before it fails.
    private static let touchReadyPresses = 8
    /// The pause between two reads of qa.state while a request is held (S6-1; elsewhere QAAx
    /// reads every 50 ms).
    private static let heldReadInterval: useconds_t = 250_000
    /// The frames of the fixture controls pressed while a request is held, found just before the
    /// hold starts (S6-1), and the app window's origin they are measured from.
    private var heldFrames: [String: CGRect] = [:]
    private var appOrigin = CGPoint.zero

    /// A failed assertion ends the test method without running its `defer`, so the
    /// events are attached here: a failed run keeps them as evidence too.
    override func tearDown() {
        if let data = try? JSONSerialization.data(withJSONObject: events, options: [.sortedKeys]) {
            let attachment = XCTAttachment(data: data, uniformTypeIdentifier: "public.json")
            attachment.name = "p12-ui-events.json"
            attachment.lifetime = .keepAlways
            add(attachment)
        }
        super.tearDown()
    }

    func testInstalledTerminalMenu() throws {
        continueAfterFailure = false
        XCTAssertTrue(XCUIDevice.shared.supportsPointerInteraction, "P12 requires an iPad with pointer support")
        app.terminate()
        app.launchEnvironment["YEBISU_TERMINAL_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment["YEBISU_P12_QA"] = "1"
        app.launchEnvironment["YEBISU_QA_AX"] = "1"
        XCUIDevice.shared.orientation = .portrait
        app.launch()
        // Before S5: sleep(3), then a check that the app's p0-smoke.jsonl was readable.
        do {
            try qa.waitState("the P12 fixture is up: the terminal is connected and the panel idle", timeout: 60) {
                $0.string("screen") == "terminal" && $0.bool("connected") == true && self.pending($0) == false
            }
        } catch {
            XCTFail("P12 qa.state never showed the connected terminal and the idle fixture panel "
                    + "(YEBISU_QA_AX, YEBISU_P12_QA): \(error)")
            throw error
        }
        // A remote pick holds pane.split at the peer until "P12 Release", and the
        // pinned sheprd-client fails the link once a request is 60 s old
        // (limits.rs COMMAND_TIMEOUT). One native menu session took up to 44 s on
        // hosted runners, so each hold covers exactly one local case, and every
        // control is found before the first hold.
        try locateFixtures()
        try touchReady()
        for (remote, local, name) in [("remote", "sidebar", "Toggle Sidebar"), ("rearm", "palette", "Command Palette")] {
            try fixture(remote)
            try choose("Split Right", caseName: remote, negative: false)
            // The request is held from that pick (pane.split waits at the peer) until Release.
            let hold = (reads: qa.session.reads, seconds: qa.session.readSeconds, started: Date().timeIntervalSince1970)
            try fixture(local, held: true)
            try choose(name, caseName: local, negative: false)
            try fixture("release", held: true)
            // S5 follow-up S6-1: how many qa.state reads the hold carried and how long they took, so
            // the oracle can put them beside the hold (a read waits for the app to idle).
            events.append(["kind": "hold-queries", "case": remote, "reads": qa.session.reads - hold.reads,
                           "readSeconds": qa.session.readSeconds - hold.seconds,
                           "started": hold.started, "ended": Date().timeIntervalSince1970])
            try idle()
        }
        for step in ["host", "pane", "boot", "focus", "capability"] {
            try fixture(step)
            // The fixture freezes the original installed menu. Wait for the stale
            // scope before presenting it so a scope change cannot interrupt the pick.
            try qa.waitState("P12 \(step) changed with the old menu scope installed", timeout: 7) { state in
                self.p12(state)?["case"] as? String == step && self.p12(state)?["mutated"] as? Bool == true
            }
            try choose("Split Right", caseName: step, negative: true)
            if step == "focus" {
                try fixture("current")
                try choose("Split Right", caseName: "focus-current", negative: false, observeOnly: true)
            }
            try fixture("reset")
        }
        try fixture("finish")
        sleep(2)
        screenshot("p12-finished")
        // Real hardware Command-hold is a separate owner gate. This test proves
        // only the installed OS menu facility exercised below, never key hold.
        events.append(["kind": "scope", "osMenu": true, "commandHoldExercised": false,
                       "os": ProcessInfo.processInfo.operatingSystemVersionString])
    }

    /// A point that dismisses the open OS menu: the centre of the fixture panel's empty strip
    /// (qa.p12.empty), beside the menu, never on it. A fixed window point landed inside the menu once
    /// Integration 2 (L2b) added New Space and the menu reached y 0.86 with its right edge at x 0.52
    /// (run 44220412031, class A twice); 2958a1a then computed a point from the menu's right edge,
    /// and S5 takes the app's own empty space by identifier. The strip must lie clear of the menu's
    /// column (its frame plus the 24 pt of the menu's bubble); a strip under the menu fails by name.
    private func dismissPoint(clearOf item: XCUIElement, caseName: String) throws -> XCUICoordinate {
        let empty = try qa.find(Self.emptySpace, timeout: 5)
        let strip = empty.frame
        let menu = item.exists ? item.frame : CGRect.null
        let clear = menu.isNull || strip.minX > menu.maxX + 24 || strip.maxX < menu.minX - 24
        events.append(["kind": "dismiss-point", "case": caseName, "id": Self.emptySpace, "clear": clear,
                       "strip": [strip.minX, strip.minY, strip.width, strip.height],
                       "menuItem": menu.isNull ? [] : [menu.minX, menu.minY, menu.width, menu.height]])
        XCTAssertTrue(clear, "P12's empty strip \(strip) lies in the open OS menu's column \(menu) in case \(caseName)")
        return empty.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
    }

    private func choose(_ title: String, caseName: String, negative: Bool, observeOnly: Bool = false) throws {
        // Recorded before the swipe so a reveal that fails can say which
        // precondition was missing (L17 class B), not only that the bar is absent.
        recordRevealPrecondition(caseName)
        // The pinned herdr-uikit (see Cargo.toml) menu::placement maps GPUI position 0
        // to UIMenuApplication, regardless of the consumer title "Terminal".
        // The installed Springboard menu-bar control is therefore "yebisu".
        let owner = springboard
        let menuBar = owner.statusBars.containing(.button, identifier: "yebisu").firstMatch
        let applicationMenu = menuBar.buttons.matching(NSPredicate(format: "label == %@", "yebisu")).firstMatch
        guard let pressPosition = revealMenuBar(applicationMenu, in: menuBar, caseName: caseName) else {
            springboardHierarchy("p12-revealed-" + caseName)
            screenshotOnFailure("p12-revealed-" + caseName, false)
            XCTFail("OS menu bar not revealed (class B) in case \(caseName): " + revealNote + preRevealSummary())
            return
        }
        let target: XCUICoordinate
        if pressPosition.useButton {
            target = applicationMenu.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
        } else {
            let offset = CGVector(dx: pressPosition.point.x - appOrigin.x, dy: pressPosition.point.y - appOrigin.y)
            target = app.coordinate(withNormalizedOffset: .zero).withOffset(offset)
        }
        target.tap()
        // The menu belongs to Springboard. Let its row become interactive before
        // the pick, and leave yebisu's AX snapshots out of the open-menu window.
        let item = owner.buttons.matching(NSPredicate(format: "label == %@ OR label BEGINSWITH %@", title, title + "　")).firstMatch
        let available = waitForEnabledMenuItem(item)
        let hittable = available && !item.frame.isEmpty && item.isHittable
        // The current-focus menu stays a screenshot: it is the only visual proof of
        // the rebuilt menu after the stale rejection.
        if observeOnly { screenshot("p12-menu-" + caseName) }
        springboardHierarchy("p12-menu-" + caseName)
        if !observeOnly { screenshotOnFailure("p12-menu-" + caseName, hittable) }
        if !available {
            let exists = item.exists
            let enabled = exists && item.isEnabled
            XCTFail("Native command availability wait expired: '\(title)' in case \(caseName), exists=\(exists), enabled=\(enabled)")
            return
        }
        XCTAssertTrue(hittable, "Native command isHittable=false after becoming available: '\(title)' in case \(caseName)")
        // Recap can send a touch before BackBoard registers its virtual
        // digitizer. The trackpad waits for pointer readiness and delivers the
        // selection or dismissal through the real system menu.
        if observeOnly {
            events.append(["kind": "current-native-menu", "case": "focus", "enabled": item.isEnabled,
                           "owner": "com.apple.springboard", "buttonLabel": item.label,
                           "time": Date().timeIntervalSince1970])
            let point = try dismissPoint(clearOf: item, caseName: "focus-current")
            let dismissTime = Date().timeIntervalSince1970
            point.click()
            assertMenuClosed(item, title: title, caseName: "focus-current", step: "dismiss", tapTime: dismissTime)
            return
        }
        // Position the pointer before the click and check that the same menu
        // still has an actionable row after XCTest completes the move.
        let pointerStart = Date().timeIntervalSince1970
        if item.exists && item.isEnabled { item.hover() }
        let (present, enabled, tapped) = waitForMenuItemAfterHover(item)
        events.append(["kind": "pointer-ready", "case": caseName, "started": pointerStart,
                       "ended": Date().timeIntervalSince1970, "actionable": tapped])
        let mustTap = !negative || caseName == "focus"
        screenshotOnFailure("p12-before-" + caseName, present && (!mustTap || tapped) && (!enabled || tapped))
        XCTAssertTrue(present, "Native menu disappeared; this is not refusal evidence")
        if mustTap { XCTAssertTrue(tapped, "Positive native command must be actionable") }
        if enabled { XCTAssertTrue(tapped, "Enabled item must remain hittable") }
        let pickStart = Date().timeIntervalSince1970
        let buttonLabel = item.label
        if tapped { item.click() }
        else { try dismissPoint(clearOf: item, caseName: caseName).click() }
        // A pick or dismiss that was lost leaves the menu open, and the next
        // fixture tap would then only close it (L17 class A). Fail here, by name.
        let menuClosed = assertMenuClosed(item, title: title, caseName: caseName,
                                          step: tapped ? "pick" : "dismiss", tapTime: pickStart)
        sleep(1)
        events.append(["kind": "os-menu-pick", "case": caseName, "command": title,
                       "owner": owner === springboard ? "com.apple.springboard" : "com.shin1ohno.yebisu", "buttonLabel": buttonLabel,
                       "started": pickStart, "ended": Date().timeIntervalSince1970,
                       "enabled": enabled, "tapped": tapped, "negative": negative, "menuClosed": menuClosed])
        screenshot("p12-after-" + caseName)
    }

    private func waitForEnabledMenuItem(_ item: XCUIElement) -> Bool {
        // The first SpringBoard query can take seconds. Sample it before starting the
        // wait and keep hittability checks out of the availability predicate.
        if item.exists && item.isEnabled { return true }
        let available = XCTNSPredicateExpectation(
            predicate: NSPredicate(format: "exists == true AND enabled == true"), object: item)
        return XCTWaiter().wait(for: [available], timeout: Self.menuItemTimeout) == .completed
    }

    private func waitForMenuItemAfterHover(_ item: XCUIElement) -> (present: Bool, enabled: Bool, hittable: Bool) {
        let deadline = ProcessInfo.processInfo.systemUptime + Self.menuItemTimeout
        while item.exists {
            guard item.isEnabled else { return (true, false, false) }
            if !item.frame.isEmpty && item.isHittable { return (true, true, true) }
            if ProcessInfo.processInfo.systemUptime >= deadline { return (true, true, false) }
            usleep(Self.menuPollInterval)
        }
        return (false, false, false)
    }

    private static func screenScale(_ reported: CGFloat, expected: CGFloat) -> CGFloat? {
        [CGFloat(1), 2].first { abs($0 * reported - expected) < 1 }
    }

    static func menuBarButtonFrame(_ button: CGRect, in bar: CGRect, on screen: CGRect) -> CGRect? {
        // QAAx can compare an AX frame with GPUI's painted bounds. SpringBoard owns
        // the menu, so use its full-width status bar and screen width as references.
        guard !button.isEmpty, !bar.isEmpty, !screen.isEmpty,
              let barScale = screenScale(bar.width, expected: screen.width) else { return nil }
        let barFrame = bar.applying(CGAffineTransform(scaleX: barScale, y: barScale))
        guard abs(barFrame.minX - screen.minX) < 1, abs(barFrame.minY - screen.minY) < 1,
              barFrame.maxY <= 64,
              let buttonScale = screenScale(button.height, expected: barFrame.height) else { return nil }
        let buttonFrame = button.applying(CGAffineTransform(scaleX: buttonScale, y: buttonScale))
        return barFrame.contains(buttonFrame) ? buttonFrame : nil
    }

    private func waitForMenuBarPressPosition(_ button: XCUIElement, in bar: XCUIElement, on screen: CGRect)
        -> MenuBarPressPosition? {
        // A nested AX frame request can be interrupted by a predicate waiter.
        // Poll the geometry directly and use its centre for the touch.
        let deadline = ProcessInfo.processInfo.systemUptime + Self.revealTimeout
        repeat {
            if button.exists {
                let reported = button.frame
                if let frame = Self.menuBarButtonFrame(reported, in: bar.frame, on: screen) {
                    return (CGPoint(x: frame.midX, y: frame.midY), reported, frame == reported)
                }
            }
            if ProcessInfo.processInfo.systemUptime >= deadline { return nil }
            usleep(Self.menuPollInterval)
        } while true
    }

    /// Swipes down from the top edge and waits for a valid OS menu-bar button frame. A swipe
    /// that reveals nothing is repeated once, and the repeat is logged with its
    /// reason (`reveal-retry`); the second miss fails as class B. In the failed runs
    /// of 2026-11-17 the swipe call itself took 5.6 s and 5.7 s against 0.6-1.1 s
    /// for 14 of the 15 reveals that worked (the 15th took 4.2 s), so the gesture was
    /// spread over a stalled Simulator [unproven]. The assertion after this call is unchanged.
    private func revealMenuBar(_ applicationMenu: XCUIElement, in bar: XCUIElement, caseName: String)
        -> MenuBarPressPosition? {
        var attempt = 1
        revealNote = ""
        let screen = springboard.frame
        while true {
            let swipeStart = Date()
            let injected = attempt == 1 && faults.remove("skip-swipe:" + caseName) != nil
            if !injected {
                app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.001))
                    .press(forDuration: 0.05, thenDragTo: app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.09)))
            }
            let swipeSeconds = Date().timeIntervalSince(swipeStart)
            let revealStart = Date()
            let position = waitForMenuBarPressPosition(applicationMenu, in: bar, on: screen)
            let waited = Date().timeIntervalSince(revealStart)
            if position != nil || attempt == 2 {
                events.append(["kind": "reveal-result", "case": caseName, "revealed": position != nil,
                               "pressPosition": position.map { [$0.point.x, $0.point.y] } ?? [],
                               "axButtonFrame": position.map {
                                   [$0.reported.minX, $0.reported.minY, $0.reported.width, $0.reported.height]
                               } ?? [],
                               "pressAnchor": position.map { $0.useButton ? "button" : "window" } ?? "",
                               "waited": waited, "attempts": attempt, "swipeSeconds": swipeSeconds,
                               "time": Date().timeIntervalSince1970])
                return position
            }
            var reason = injected ? "injected fault: the first swipe was skipped; " : ""
            reason += "OS menu bar not revealed \(Int(waited)) s after the first swipe"
            reason += " (the swipe call took \(String(format: "%.1f", swipeSeconds)) s); swiping once more"
            revealNote = "after one retry (" + reason + "); "
            events.append(["kind": "reveal-retry", "case": caseName, "reason": reason, "injected": injected,
                           "swipeSeconds": swipeSeconds, "waited": waited, "time": Date().timeIntervalSince1970])
            attempt += 1
        }
    }

    /// Records what Springboard and the app window looked like just before a
    /// swipe reveal: whether the Window Controls button exists, the app's
    /// process state, whether its window has focus, and the time since the last
    /// menu closed. The whole hierarchy is saved as an attachment as well.
    private func recordRevealPrecondition(_ caseName: String) {
        let time = Date().timeIntervalSince1970
        let board = springboard.debugDescription
        let appTree = app.debugDescription
        let window = app.windows.firstMatch
        let sinceClose: Double = lastMenuClosed.map { Date().timeIntervalSince($0) } ?? -1
        let attachment = XCTAttachment(string: "APP\n\(appTree)\nSPRINGBOARD\n\(board)")
        attachment.name = "p12-pre-reveal-" + caseName + "-hierarchy"
        attachment.lifetime = .keepAlways
        add(attachment)
        preReveal = [
            "kind": "reveal-precondition", "case": caseName, "time": time,
            "windowControlsPresent": board.contains("Window Controls"),
            "appState": Self.stateName(app.state),
            "appWindowExists": window.exists, "appWindowFocused": window.hasFocus,
            "secondsSinceMenuClosed": sinceClose,
        ]
        events.append(preReveal)
    }

    private func preRevealSummary() -> String {
        func field(_ key: String) -> String { String(describing: preReveal[key] ?? "?") }
        let controls = "Springboard Window Controls present=" + field("windowControlsPresent")
        let appPart = "app state=" + field("appState") + ", app window focused=" + field("appWindowFocused")
        let closed = field("secondsSinceMenuClosed") + " s since the last menu closed"
        return "before the swipe " + controls + ", " + appPart + ", " + closed
            + " (saved as p12-pre-reveal-" + field("case") + "-hierarchy)"
    }

    private static func stateName(_ state: XCUIApplication.State) -> String {
        switch state {
        case .notRunning: return "notRunning"
        case .runningBackgroundSuspended: return "runningBackgroundSuspended"
        case .runningBackground: return "runningBackground"
        case .runningForeground: return "runningForeground"
        default: return "unknown"
        }
    }

    /// Waits on the observable condition (the picked or dismissed menu item is gone).
    /// `tapTime` is the start of the input, using the historical evidence key.
    @discardableResult
    private func assertMenuClosed(_ item: XCUIElement, title: String, caseName: String, step: String,
                                  tapTime: TimeInterval) -> Bool {
        let waitStart = Date()
        let closed = item.waitForNonExistence(timeout: 5)
        events.append(["kind": "menu-close", "case": caseName, "step": step, "closed": closed,
                       "tapTime": tapTime, "waited": Date().timeIntervalSince(waitStart),
                       "time": Date().timeIntervalSince1970])
        if closed {
            lastMenuClosed = Date()
        } else {
            screenshot("p12-menu-still-open-" + caseName)
            springboardHierarchy("p12-menu-still-open-" + caseName)
        }
        XCTAssertTrue(closed, "OS menu still open 5 s after the \(step) click of '\(title)' in case \(caseName): "
                      + "the click was lost; compare tapTime with the p12-menu-build records")
        return closed
    }

    /// The app's `p12` field in qa.state (native_menus_qa.rs): `pending`, `case`, `mutated`, and
    /// `setup` (step -> p12-setup-end records so far).
    private func p12(_ state: QAState) -> [String: Any]? { state.raw["p12"] as? [String: Any] }
    /// Whether a request is held ("P12 Pending" on the panel), nil before the field is published.
    private func pending(_ state: QAState) -> Bool? { p12(state)?["pending"] as? Bool }
    /// The p12-setup-end records the app counted for `step`.
    private func setupEnds(_ state: QAState, _ step: String) -> Int {
        (p12(state)?["setup"] as? [String: Any])?[step] as? Int ?? 0
    }

    private func fixtureId(_ step: String) throws -> String {
        try XCTUnwrap(Self.fixtureControls.first { $0.step == step }?.id, "no fixture control for step \(step)")
    }

    /// Finds every fixture control by identifier before the first held request, so a missing one
    /// fails by name before any hold (as the OCR search of all 13 did before S5). The frames are kept
    /// for the presses made while a request is held (`heldPress`): the panel does not move (it is
    /// placed at the window's top right, above any keyboard, and no sheet opens over it), and a press
    /// that misses its control is caught by qa.state as before. Reading them here, once, keeps app
    /// queries out of the time between the first OS menu and the holds (PR 84: reading them right
    /// before the Remote case went with 2 lost Springboard taps in 9 runs, main 0 in 3).
    private func locateFixtures() throws {
        let started = Date().timeIntervalSince1970
        var missing: [String] = []
        appOrigin = app.frame.origin
        for control in Self.fixtureControls {
            do { heldFrames[control.id] = try qa.find(control.id, timeout: 10).frame } catch { missing.append("\(control.id) (\(error))") }
        }
        if missing.isEmpty {
            events.append(["kind": "fixture-located", "count": Self.fixtureControls.count,
                           "started": started, "ended": Date().timeIntervalSince1970])
            return
        }
        screenshot("p12-missing-fixture")
        XCTFail("Missing fixture controls: \(missing.joined(separator: ", "))")
        throw NSError(domain: "P12.MissingFixture", code: 1)
    }

    /// Waits until a touch reaches GPUI at all before the first fixture press. On a fresh Simulator
    /// the first press after launch sometimes reaches nothing, and the press 7 s later does not
    /// either: S5 Mac mini trial run 2 (Remote, both presses lost, qa.state seq 8 unchanged, the
    /// screen recording shows the panel drawn and nothing over it), S4's first SH1-153 press (1 of 6
    /// fresh launches) and job 134059155142 (P12-fixture, the first tap was Remote). Why is not
    /// known (UIKit or herdr-uikit; neither is in this repository). So the empty strip, which has no
    /// handler, is pressed until GPUI counts a press on it (at most `touchReadyPresses`, 3 s each),
    /// and how many it took is logged; the fixture presses keep their own retry-once rule.
    private func touchReady() throws {
        let started = Date().timeIntervalSince1970
        let empty = try qa.find(Self.emptySpace, timeout: 10)
        for attempt in 1...Self.touchReadyPresses {
            let before = try qa.waitState("qa.state before touch-ready press \(attempt)", timeout: 5) { _ in true }
            empty.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
            let reached = try? qa.waitState("touch-ready press \(attempt) reached GPUI", timeout: 3) {
                $0.presses(Self.emptySpace) > before.presses(Self.emptySpace)
            }
            if reached != nil {
                print("P12 touch-ready after \(attempt) press(es)")
                events.append(["kind": "touch-ready", "ready": true, "presses": attempt,
                               "started": started, "ended": Date().timeIntervalSince1970])
                return
            }
            print("P12 touch-ready press \(attempt) reached nothing (presses[\(Self.emptySpace)] did not grow in 3 s)")
        }
        events.append(["kind": "touch-ready", "ready": false, "presses": Self.touchReadyPresses,
                       "started": started, "ended": Date().timeIntervalSince1970])
        screenshot("p12-touch-not-ready")
        XCTFail("No touch reached GPUI in \(Self.touchReadyPresses) presses on \(Self.emptySpace) after launch; qa.state=\(qa.read()?.text ?? "nil")")
        throw NSError(domain: "P12.TouchNotReady", code: 1)
    }

    /// Presses the fixture control of `step` and waits until the app counted one more
    /// p12-setup-end for it (qa.state `p12.setup`), not a fixed delay. In job 134059155142 the first
    /// tap (Remote) never reached the app and nothing in the test noticed until the oracle ran. A
    /// press that reaches nothing is pressed once more (QAAx.press), and the reason is logged as a
    /// `fixture-retry`; a press that something else took, or one that reached the control without
    /// its setup step, fails by name. Finish also waits until its FIFO fence reached the private
    /// peer (qa.state `peer`), which p12-check.py used to poll the peer's file for after XCTest.
    private func fixture(_ step: String, held: Bool = false) throws {
        let id = try fixtureId(step)
        let setupStart = Date().timeIntervalSince1970
        let before = try qa.waitState("qa.state before the \(step) fixture", timeout: Self.receiptTimeout) { _ in true }
        let ended = setupEnds(before, step)
        let injected = faults.remove("drop-tap:" + step) != nil
        if injected { qa.session.dropFirstPress.insert(id) }
        let retries = qa.session.retries.count
        var failure = ""
        do {
            if held {
                try heldPress(id, step: step, before: before, ended: ended)
            } else {
                try qa.press(id, timeout: Self.receiptTimeout, what: "p12-setup-end for \(step)") { self.setupEnds($0, step) > ended }
            }
            if step == "finish" {
                try qa.waitState("the FIFO fence reached the private peer", timeout: Self.receiptTimeout) {
                    $0.peerPastes.contains("p12-fence")
                }
            }
        } catch {
            failure = "\(error)"
        }
        for (index, reason) in qa.session.retries.dropFirst(retries).enumerated() {
            let prefix = injected && index == 0 ? "injected fault: the first tap was skipped; " : ""
            events.append(["kind": "fixture-retry", "step": step, "injected": injected && index == 0,
                           "reason": prefix + reason + "; pressing once more", "time": Date().timeIntervalSince1970])
        }
        let received = failure.isEmpty
        XCTAssertTrue(received, "Fixture tap on '\(id)' not received by the app, also after one retry (class P12-fixture): \(failure)")
        // Kept from before S5: the release's re-layout (geometry on the wire) and the finish fence
        // must land inside this step's window, which the oracle checks.
        sleep(1)
        events.append(["kind": "fixture-control", "step": step, "received": received,
                       "started": setupStart, "ended": Date().timeIntervalSince1970])
    }

    /// The fixture press while a request is held (S5 follow-up S6-1, review F2 of PR 83). QAAx.press
    /// queries the app about 15 times per press (an element query, a duplicate check, reads every
    /// 50 ms), and each query can wait for the app to idle (60-80 s on an old runner image), which
    /// would push the hold past the client's 60 s COMMAND_TIMEOUT. Here the control's frame was found
    /// by locateFixtures, before any hold, and qa.state is read every 250 ms until `presses[id]` and the step's
    /// p12-setup-end both grew. The receipt window starts after the synchronous tap returns.
    /// The decisions are QAAx.press's: a press that moved nothing is sent
    /// once more (logged as a retry), one that something else took or that never arrives fails.
    private func heldPress(_ id: String, step: String, before: QAState, ended: Int) throws {
        let frame = try XCTUnwrap(heldFrames[id], "no frame recorded before the hold for \(id)")
        let point = CGVector(dx: frame.midX - appOrigin.x, dy: frame.midY - appOrigin.y)
        let start = Date()
        for attempt in 1...2 {
            if attempt == 1 && qa.session.dropFirstPress.remove(id) != nil {
                print("P12 held press \(id): injected fault, the first press is not sent")
            } else {
                app.coordinate(withNormalizedOffset: .zero).withOffset(point).tap()
            }
            let receiptStarted = ProcessInfo.processInfo.systemUptime
            var last: QAState?
            while true {
                if let state = qa.read() {
                    last = state
                    if state.presses(id) > before.presses(id) && setupEnds(state, step) > ended {
                        print(String(format: "P12 held press %@ confirmed after %.3f s (attempt %ld)", id, Date().timeIntervalSince(start), attempt))
                        return
                    }
                }
                if ProcessInfo.processInfo.systemUptime - receiptStarted >= Self.receiptTimeout { break }
                usleep(Self.heldReadInterval)
            }
            if let last, last.presses(id) > before.presses(id) {
                throw QAAxError.pressWithoutEffect(id: id, what: "p12-setup-end for \(step)", seconds: Date().timeIntervalSince(start), last: last.text)
            }
            switch PressVerdict.of(before: before, after: last, id: id) {
            case .relaunched:
                throw QAAxError.relaunched(id: id, before: before.text, after: last?.text ?? "nil")
            case let .takenElsewhere(moved):
                throw QAAxError.pressTakenElsewhere(id: id, moved: moved, before: before.text, after: last?.text ?? "nil")
            case .lost:
                break
            }
            if attempt == 2 {
                throw QAAxError.pressNotDelivered(id: id, attempts: attempt, before: before.text, after: last?.text ?? "nil")
            }
            let reason = String(format: "presses[%@] and the screen fields did not move within %.0f s of the press", id, Self.receiptTimeout)
            qa.session.retries.append("\(id): \(reason)")
            print("P12 held press \(id) lost (\(reason)); pressing once more")
        }
    }

    /// "P12 Idle": the held request was answered. Before S5 OCR looked for the panel's text (up to 8
    /// reads, 1 s apart); the app draws that text from the same `pending` it reports.
    private func idle() throws {
        do {
            try qa.waitState("P12 Idle (p12.pending false)", timeout: 30) { self.pending($0) == false }
        } catch {
            screenshot("p12-missing-fixture")
            XCTFail("Missing fixture control/state: P12 Idle: \(error)")
            throw error
        }
    }

    /// Takes the screenshot only when the check that the assertions after it make
    /// failed, so the failure keeps its picture and a pass takes none.
    private func screenshotOnFailure(_ name: String, _ passed: Bool) {
        if !passed { screenshot(name) }
    }
    private func screenshot(_ name: String) {
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
    private func springboardHierarchy(_ name: String) {
        let attachment = XCTAttachment(string: "\nSPRINGBOARD\n\(springboard.debugDescription)")
        attachment.name = name + "-hierarchy"
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
