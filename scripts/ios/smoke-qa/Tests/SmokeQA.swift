import XCTest
import Vision

final class SmokeQA: XCTestCase {
    func testInputAndLifecycleReceipts() throws {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        XCUIDevice.shared.orientation = .portrait
        app.activate()
        sleep(2)
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.425)).tap()
        app.typeText("P0-input")
        sleep(2)
        save("initial")
        XCUIDevice.shared.press(.home)
        sleep(2)
        app.activate()
        sleep(2)
        app.typeText("-returned")
        sleep(2)
        save("returned")
        XCUIDevice.shared.orientation = .landscapeLeft
        sleep(2)
        app.typeText("-rotated")
        sleep(2)
        save("rotated")
        XCUIDevice.shared.orientation = .portrait
        sleep(2)
        save("idle-before")
        sleep(60)
        app.typeText("-idle")
        sleep(2)
        save("idle")
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.97, dy: 0.96)).tap()
        sleep(2)
        save("image-clear")
        XCTAssertEqual(app.state, .runningForeground)
        // This liveness assertion is auxiliary. smoke_oracle.py asserts rendered
        // regions and app-written receipt state for every saved stage.
    }
    func testP2KeysAndRestart() throws {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        XCUIDevice.shared.orientation = .portrait
        app.activate()
        sleep(3)
        save("p2-initial")
        try tapText("Create / show public key", in: app)
        sleep(4)
        save("p2-key")
        try tapText("Copy authorized_keys line", in: app)
        sleep(2)
        save("p2-copied")
        // Home and return are a lifecycle check; terminate+launch is a restart.
        XCUIDevice.shared.press(.home)
        sleep(1)
        app.activate()
        app.terminate()
        app.launchEnvironment["YEBISU_P2_OBSERVE"] = "1"
        app.launch()
        sleep(3)
        try tapText("Create / show public key", in: app)
        sleep(4)
        save("p2-restarted")
        XCTAssertEqual(app.state, .runningForeground)
    }

    func testSH1153EditMenu() throws {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.terminate()
        app.launchEnvironment["YEBISU_TERMINAL_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment["YEBISU_EDIT_MENU_QA"] = "1"
        XCUIDevice.shared.orientation = .portrait
        app.launch()
        sleep(3)
        for step in 0..<10 {
            try tapText("Menu prepare", in: app)
            sleep(1)
            if step % 5 == 1 || step % 5 == 4 {
                try tapText("Menu select", in: app)
                sleep(1)
            }
            try tapText("Menu execute", in: app)
            sleep(1)
            save("sh1-153-case-\(step)")
            if step == 1 || step == 6 {
                print("SH1-153 MENU HIERARCHY case=\(step)\n\(app.debugDescription)")
                let edit = app.descendants(matching: .any).matching(NSPredicate(format: "label == 'Edit'")).firstMatch
                if edit.exists && edit.isHittable {
                    edit.tap()
                    sleep(1)
                    save("sh1-153-system-edit-\(step)")
                    print("SH1-153 OPEN MENU case=\(step)\n\(app.debugDescription)")
                    // Dismiss without choosing an extra command outside the receipt cases.
                    app.coordinate(withNormalizedOffset: CGVector(dx: 0.9, dy: 0.5)).tap()
                } else {
                    print("SH1-153 system Edit menu is not exposed to XCTest in case \(step)")
                }
            }
        }
        try tapText("Menu prepare", in: app)
        sleep(1)
        XCTAssertEqual(app.state, .runningForeground)
    }

    func testP8NativeEditing() throws {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.terminate()
        app.launchEnvironment["YEBISU_TERMINAL_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment["YEBISU_NATIVE_QA"] = "1"
        XCUIDevice.shared.orientation = .portrait
        app.launch()
        sleep(3)
        for _ in 0..<25 {
            try tapText("Native next", in: app)
            sleep(1)
        }
        save("p8-native")
        XCTAssertEqual(app.state, .runningForeground)
    }

    func testP3Terminal() throws {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.terminate()
        app.launchEnvironment["YEBISU_TERMINAL_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        XCUIDevice.shared.orientation = .portrait
        app.launch()
        sleep(3)
        save("p3-standard")
        try tapText("Standard 14/20", in: app)
        sleep(2)
        save("p3-compact")
        try tapText("Compact 12/17", in: app)
        sleep(2)
        XCUIDevice.shared.orientation = .landscapeLeft
        sleep(3)
        save("p3-landscape")
        XCUIDevice.shared.orientation = .portrait
        sleep(2)
        save("p3-returned")
        XCTAssertEqual(app.state, .runningForeground)
    }

    func testSH1151SelectionSettings() throws {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.terminate()
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment["YEBISU_TERMINAL_QA"] = "1"
        app.launchEnvironment["YEBISU_SELECTION_SETTINGS_QA"] = "1"
        XCUIDevice.shared.orientation = .portrait
        app.launch()
        sleep(3)
        try tapText("Start: Drag", in: app, tap: false)
        try tapText("Copy: Manual", in: app, tap: false)
        save("sh1-151-fresh")
        try tapText("Start: Drag", in: app)
        try tapText("Copy: Manual", in: app)
        sleep(2)
        app.terminate()
        app.launch()
        sleep(3)
        try tapText("Start: Select / double tap", in: app, tap: false)
        try tapText("Copy: On selection", in: app, tap: false)
        save("sh1-151-restored")
        XCTAssertEqual(app.state, .runningForeground)
    }

    func testP10Settings() throws {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.terminate()
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment["YEBISU_SETTINGS_QA"] = "1"
        app.launch()
        sleep(3)
        save("p10-before")
        XCUIDevice.shared.press(.home)
        sleep(3)
        app.activate()
        sleep(2)
        app.terminate()
        app.launch()
        sleep(3)
        save("p10-restored")
        XCTAssertEqual(app.state, .runningForeground)
    }

    func testP9Core() throws {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.terminate()
        app.launchEnvironment["YEBISU_CORE_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment["YEBISU_INPUT_QA"] = "1"
        app.launch()
        sleep(3)
        // Drag selection consumes the tap that must open native input here.
        try tapText("Start: Drag", in: app)
        try tapText("Start: Select / double tap", in: app, tap: false)
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.4)).tap()
        app.typeText("P9-native\n")
        try tapText("QA mark", in: app)
        try tapText("QA delete", in: app)
        try tapText("QA commit", in: app)
        try tapText("Split (2 panes)", in: app)
        sleep(2)
        try tapText("Toggle split direction", in: app)
        sleep(2)
        try tapText("Pane 2", in: app)
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.4)).tap()
        app.typeText("P9-pane2\n")
        save("p9-split")
        try tapText("Detach", in: app)
        sleep(1)
        try tapText("Reconnect", in: app)
        sleep(3)
        try tapText("Pane 1", in: app)
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.4)).tap()
        app.typeText("P9-pane1\n")
        save("p9-reattached")
        XCTAssertEqual(app.state, .runningForeground)
    }

    func testSH154CoreImage() throws {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.terminate()
        app.launchEnvironment["YEBISU_CORE_QA"] = "1"
        app.launchEnvironment["YEBISU_HANDOFF_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launch()
        try tapText("QA Image only", in: app, attempts: 15)
        try tapText("QA image ready", in: app, tap: false, attempts: 30)
        try tapText("Paste image", in: app)
        try tapText("paste-0.png", in: app, tap: false, attempts: 30)
        try tapText("Paste", in: app, excludingSuffix: " image")
        try tapText("paste-1.png", in: app, tap: false, attempts: 30)
        try tapText("QA Image", in: app, excludingSuffix: " only")
        try tapText("QA image ready", in: app, tap: false, attempts: 30)
        try tapText("Paste", in: app, excludingSuffix: " image")
        sleep(1)
        try tapText("Paste image", in: app)
        try tapText("paste-2.png", in: app, tap: false, attempts: 30)
        save("sh154-core-image")
        XCTAssertEqual(app.state, .runningForeground)
    }

    private func tapText(_ label: String, in app: XCUIApplication, tap: Bool = true, attempts: Int = 3, excludingSuffix: String? = nil) throws {
        // GPUI controls have no UIKit accessibility nodes. Match source labels,
        // allowing Vision alternatives and a bounded wait for the next paint.
        for attempt in 0..<attempts {
            let image = try XCTUnwrap(XCUIScreen.main.screenshot().image.cgImage)
            let request = VNRecognizeTextRequest()
            request.recognitionLevel = .accurate
            request.customWords = [label]
            try VNImageRequestHandler(cgImage: image).perform([request])
            // Controls precede the terminal surface. Prefer the uppermost
            // match so shell output containing e.g. "Detach" cannot steal taps.
            let candidates = (request.results ?? [])
                .sorted { $0.boundingBox.midY > $1.boundingBox.midY }
                .flatMap { $0.topCandidates(5) }
            for candidate in candidates {
                guard let range = candidate.string.range(of: label),
                      let observation = try candidate.boundingBox(for: range) else { continue }
                if let suffix = excludingSuffix, candidate.string[range.upperBound...].hasPrefix(suffix) {
                    continue
                }
                let box = observation.boundingBox
                print("OCR tap \(label) frame=\(app.frame) box=\(box)")
                if tap {
                    app.coordinate(withNormalizedOffset: CGVector(dx: box.midX, dy: 1-box.midY)).tap()
                }
                return
            }
            print("OCR attempt \(attempt): \(candidates.map { $0.string })")
            sleep(1)
        }
        save("missing-control")
        XCTFail("Missing source-defined control: \(label)")
        throw NSError(domain: "SmokeQA.MissingControl", code: 1)
    }

    private func save(_ name: String) {
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
