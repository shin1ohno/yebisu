// Source pattern: SH1-153 visual probe (cbdc150400a04524, attempt 3).
// Only native OS menu buttons execute commands. OCR is for fixture controls only.
import XCTest
import Vision

final class P12QA: XCTestCase {
    private var events: [[String: Any]] = []
    private let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
    private let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")

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
        for (label, name) in [("Remote", "Split Right"), ("Sidebar", "Toggle Sidebar"), ("Palette", "Command Palette")] {
            try fixture("P12 " + label)
            try choose(name, caseName: label.lowercased(), negative: false)
        }
        try fixture("P12 Release")
        try fixture("P12 Idle", tap: false)
        for label in ["Host", "Pane", "Revision", "Focus", "Capability"] {
            try fixture("P12 " + label)
            try choose("Split Right", caseName: label.lowercased(), negative: true)
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

    private func choose(_ title: String, caseName: String, negative: Bool) throws {
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.001))
            .press(forDuration: 0.05, thenDragTo: app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.09)))
        sleep(1)
        hierarchy("p12-revealed-" + caseName)
        screenshot("p12-revealed-" + caseName)
        // Pinned herdr-uikit b2fa781 menu::placement maps GPUI position 0
        // to UIMenuApplication, regardless of the consumer title "Terminal".
        // The installed Springboard menu-bar control is therefore "yebisu".
        let owner = springboard
        let applicationMenu = owner.buttons.matching(NSPredicate(format: "label == %@", "yebisu")).firstMatch
        XCTAssertTrue(applicationMenu.waitForExistence(timeout: 3), "Installed OS application menu must exist")
        XCTAssertTrue(applicationMenu.isHittable, "Installed OS application menu must be reachable")
        XCTAssertLessThanOrEqual(applicationMenu.frame.maxY, 64, "Application menu must be in the OS menu bar")
        applicationMenu.tap()
        sleep(1)
        screenshot("p12-menu-" + caseName)
        hierarchy("p12-menu-" + caseName)
        // Do not fall back to GPUI toolbar text or an app helper action.
        let item = owner.buttons.matching(NSPredicate(format: "label == %@ OR label BEGINSWITH %@", title, title + ",")).firstMatch
        XCTAssertTrue(item.waitForExistence(timeout: 3), "Native command missing: \(title)")
        XCTAssertTrue(item.isEnabled, "Captured native command must start enabled")
        if negative { sleep(7) } // App injects the current-context fault after 6 seconds.
        XCTAssertTrue(item.exists, "Native menu disappeared; this is not refusal evidence")
        let enabled = item.isEnabled
        let tapped = enabled && item.isHittable
        if !negative { XCTAssertTrue(tapped, "Positive native command must be actionable") }
        if enabled { XCTAssertTrue(item.isHittable, "Enabled item must remain hittable") }
        screenshot("p12-before-" + caseName)
        let pickStart = Date().timeIntervalSince1970
        let buttonLabel = item.label
        if tapped { item.tap() }
        else { app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.85)).tap() }
        sleep(1)
        events.append(["kind": "os-menu-pick", "case": caseName, "command": title,
                       "owner": owner === springboard ? "com.apple.springboard" : "com.shin1ohno.yebisu", "buttonLabel": buttonLabel,
                       "started": pickStart, "ended": Date().timeIntervalSince1970,
                       "enabled": enabled, "tapped": tapped, "negative": negative])
        screenshot("p12-after-" + caseName)
    }

    private func fixture(_ label: String, tap: Bool = true) throws {
        for _ in 0..<8 {
            let image = try XCTUnwrap(XCUIScreen.main.screenshot().image.cgImage)
            let request = VNRecognizeTextRequest()
            request.recognitionLevel = .accurate
            request.customWords = [label]
            try VNImageRequestHandler(cgImage: image).perform([request])
            for result in (request.results ?? []).sorted(by: { $0.boundingBox.maxY > $1.boundingBox.maxY }) {
                for text in result.topCandidates(5) {
                    guard let range = text.string.range(of: label),
                          let bounds = try text.boundingBox(for: range)?.boundingBox else { continue }
                    if tap {
                        let setupStart = Date().timeIntervalSince1970
                        app.coordinate(withNormalizedOffset: CGVector(dx: bounds.midX, dy: 1 - bounds.midY)).tap()
                        sleep(1)
                        events.append(["kind": "fixture-control", "step": String(label.dropFirst(4)).lowercased(),
                                       "started": setupStart, "ended": Date().timeIntervalSince1970])
                    }
                    return
                }
            }
            sleep(1)
        }
        screenshot("p12-missing-fixture")
        XCTFail("Missing fixture control/state: \(label)")
        throw NSError(domain: "P12.MissingFixture", code: 1)
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
