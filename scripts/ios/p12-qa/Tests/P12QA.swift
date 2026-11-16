// Source pattern: SH1-153 visual probe (cbdc150400a04524, attempt 3).
// Only native OS menu buttons execute commands. OCR is for fixture controls only.
import XCTest
import Vision

final class P12QA: XCTestCase {
    private var events: [[String: Any]] = []
    private let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
    private let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
    /// Tap points of the fixture controls, found by OCR before any RPC is held.
    private var fixturePoints: [String: CGVector] = [:]
    /// When the previous OS menu was last observed closed; the time between a close
    /// and the next swipe is one suspected reveal precondition.
    private var lastMenuClosed: Date?
    /// The reveal-precondition record of the swipe in progress, quoted by its failures.
    private var preReveal: [String: Any] = [:]
    private static let fixtureControls = ["Remote", "Rearm", "Sidebar", "Palette", "Release", "Host", "Pane",
                                          "Boot", "Focus", "Current", "Capability", "Reset", "Finish"]
                                          .map { "P12 " + $0 }

    func testInstalledTerminalMenu() throws {
        continueAfterFailure = false
        defer {
            if let data = try? JSONSerialization.data(withJSONObject: events, options: [.sortedKeys]) {
                let attachment = XCTAttachment(data: data, uniformTypeIdentifier: "public.json")
                attachment.name = "p12-ui-events.json"
                attachment.lifetime = .keepAlways
                add(attachment)
            }
        }
        app.terminate()
        app.launchEnvironment["YEBISU_TERMINAL_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment["YEBISU_P12_QA"] = "1"
        XCUIDevice.shared.orientation = .portrait
        app.launch()
        sleep(3)
        // A remote pick holds pane.split at the peer until "P12 Release", and the
        // pinned sheprd-client fails the link once a request is 60 s old
        // (limits.rs COMMAND_TIMEOUT). One native menu session took up to 44 s on
        // hosted runners, so each hold covers exactly one local case, and no OCR
        // search (3-31 s) runs inside a hold: every control is located first.
        try locateFixtures()
        for (remote, local, name) in [("Remote", "Sidebar", "Toggle Sidebar"), ("Rearm", "Palette", "Command Palette")] {
            try fixture("P12 " + remote)
            try choose("Split Right", caseName: remote.lowercased(), negative: false)
            try fixture("P12 " + local)
            try choose(name, caseName: local.lowercased(), negative: false)
            try fixture("P12 Release")
            try fixture("P12 Idle", tap: false)
        }
        for label in ["Host", "Pane", "Boot", "Focus", "Capability"] {
            try fixture("P12 " + label)
            try choose("Split Right", caseName: label.lowercased(), negative: true)
            if label == "Focus" {
                try fixture("P12 Current")
                try choose("Split Right", caseName: "focus-current", negative: false, observeOnly: true)
            }
            try fixture("P12 Reset")
        }
        try fixture("P12 Finish")
        sleep(2)
        screenshot("p12-finished")
        // Real hardware Command-hold is a separate owner gate. This test proves
        // only the installed OS menu facility exercised below, never key hold.
        events.append(["kind": "scope", "osMenu": true, "commandHoldExercised": false,
                       "os": ProcessInfo.processInfo.operatingSystemVersionString])
    }

    private func choose(_ title: String, caseName: String, negative: Bool, observeOnly: Bool = false) throws {
        // Recorded before the swipe so a reveal that fails can say which
        // precondition was missing (L17 class B), not only that the bar is absent.
        recordRevealPrecondition(caseName)
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.001))
            .press(forDuration: 0.05, thenDragTo: app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.09)))
        // The pinned herdr-uikit (see Cargo.toml) menu::placement maps GPUI position 0
        // to UIMenuApplication, regardless of the consumer title "Terminal".
        // The installed Springboard menu-bar control is therefore "yebisu".
        let owner = springboard
        let applicationMenu = owner.buttons.matching(NSPredicate(format: "label == %@", "yebisu")).firstMatch
        // Wait on the observable condition (the OS menu-bar control exists and is
        // hittable), not on a fixed sleep. Evidence is captured after the wait so a
        // reveal that never happens still leaves a hierarchy and screenshot.
        let revealed = XCTNSPredicateExpectation(
            predicate: NSPredicate(format: "exists == true AND hittable == true"), object: applicationMenu)
        let revealStart = Date()
        let reveal = XCTWaiter().wait(for: [revealed], timeout: 10)
        events.append(["kind": "reveal-result", "case": caseName, "revealed": reveal == .completed,
                       "waited": Date().timeIntervalSince(revealStart), "time": Date().timeIntervalSince1970])
        hierarchy("p12-revealed-" + caseName)
        screenshot("p12-revealed-" + caseName)
        XCTAssertEqual(reveal, .completed, "OS menu bar not revealed (class B) in case \(caseName): " + preRevealSummary())
        XCTAssertTrue(applicationMenu.exists, "Installed OS application menu must exist")
        XCTAssertTrue(applicationMenu.isHittable, "Installed OS application menu must be reachable")
        XCTAssertLessThanOrEqual(applicationMenu.frame.maxY, 64, "Application menu must be in the OS menu bar")
        applicationMenu.tap()
        sleep(1)
        screenshot("p12-menu-" + caseName)
        hierarchy("p12-menu-" + caseName)
        // Do not fall back to GPUI toolbar text or an app helper action.
        let item = owner.buttons.matching(NSPredicate(format: "label == %@ OR label BEGINSWITH %@", title, title + "　")).firstMatch
        XCTAssertTrue(item.waitForExistence(timeout: 3), "Native command missing: \(title)")
        XCTAssertTrue(item.isEnabled, "Captured native command must start enabled")
        if observeOnly {
            events.append(["kind": "current-native-menu", "case": "focus", "enabled": item.isEnabled,
                           "owner": "com.apple.springboard", "buttonLabel": item.label,
                           "time": Date().timeIntervalSince1970])
            let dismissTime = Date().timeIntervalSince1970
            app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.85)).tap()
            assertMenuClosed(item, title: title, caseName: "focus-current", step: "dismiss", tapTime: dismissTime)
            return
        }
        if negative { sleep(7) } // App injects the current-context fault after 6 seconds.
        XCTAssertTrue(item.exists, "Native menu disappeared; this is not refusal evidence")
        let enabled = item.isEnabled
        let tapped = enabled && item.isHittable
        if !negative || caseName == "focus" { XCTAssertTrue(tapped, "Positive native command must be actionable") }
        if enabled { XCTAssertTrue(item.isHittable, "Enabled item must remain hittable") }
        screenshot("p12-before-" + caseName)
        let pickStart = Date().timeIntervalSince1970
        let buttonLabel = item.label
        if tapped { item.tap() }
        else { app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.85)).tap() }
        // A pick or dismiss tap that was lost leaves the menu open, and the next
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
    /// `tapTime` is the instant of the tap, so the oracle can match it to a menu build.
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
            hierarchy("p12-menu-still-open-" + caseName)
        }
        XCTAssertTrue(closed, "OS menu still open 5 s after the \(step) tap of '\(title)' in case \(caseName): "
                      + "the tap was lost (class A); compare tapTime with the p12-menu-build records")
        return closed
    }

    /// Finds every fixture control on screen. The panel is absolutely positioned
    /// and its rows never move, so the points stay valid for the whole test.
    private func locateFixtures() throws {
        let started = Date().timeIntervalSince1970
        for _ in 0..<8 {
            let missing = Self.fixtureControls.filter { fixturePoints[$0] == nil }
            for (label, bounds) in try recognize(missing) {
                fixturePoints[label] = CGVector(dx: bounds.midX, dy: 1 - bounds.midY)
            }
            if fixturePoints.count == Self.fixtureControls.count {
                events.append(["kind": "fixture-located", "count": fixturePoints.count,
                               "started": started, "ended": Date().timeIntervalSince1970])
                return
            }
            sleep(1)
        }
        let missing = Self.fixtureControls.filter { fixturePoints[$0] == nil }
        screenshot("p12-missing-fixture")
        XCTFail("Missing fixture controls: \(missing.joined(separator: ", "))")
        throw NSError(domain: "P12.MissingFixture", code: 1)
    }

    private func fixture(_ label: String, tap: Bool = true) throws {
        if tap {
            // No OCR here: this may run while the Remote case holds pane.split.
            let point = try XCTUnwrap(fixturePoints[label], "Fixture control not located: \(label)")
            let setupStart = Date().timeIntervalSince1970
            app.coordinate(withNormalizedOffset: point).tap()
            sleep(1)
            events.append(["kind": "fixture-control", "step": String(label.dropFirst(4)).lowercased(),
                           "started": setupStart, "ended": Date().timeIntervalSince1970])
            return
        }
        for _ in 0..<8 {
            if try recognize([label])[label] != nil { return }
            sleep(1)
        }
        screenshot("p12-missing-fixture")
        XCTFail("Missing fixture control/state: \(label)")
        throw NSError(domain: "P12.MissingFixture", code: 1)
    }

    /// Normalized Vision bounds (origin bottom-left) of each label found in one screenshot.
    private func recognize(_ labels: [String]) throws -> [String: CGRect] {
        let image = try XCTUnwrap(XCUIScreen.main.screenshot().image.cgImage)
        let request = VNRecognizeTextRequest()
        request.recognitionLevel = .accurate
        request.customWords = labels
        try VNImageRequestHandler(cgImage: image).perform([request])
        var found: [String: CGRect] = [:]
        for result in (request.results ?? []).sorted(by: { $0.boundingBox.maxY > $1.boundingBox.maxY }) {
            for text in result.topCandidates(5) {
                for label in labels where found[label] == nil {
                    guard let range = text.string.range(of: label),
                          let bounds = try text.boundingBox(for: range)?.boundingBox else { continue }
                    found[label] = bounds
                }
            }
        }
        return found
    }
    private func screenshot(_ name: String) {
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
    private func hierarchy(_ name: String) {
        let attachment = XCTAttachment(string: "APP\n\(app.debugDescription)\nSPRINGBOARD\n\(springboard.debugDescription)")
        attachment.name = name + "-hierarchy"
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
