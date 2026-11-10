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

    func testP8Handoff() throws {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.terminate()
        app.launchEnvironment["YEBISU_TERMINAL_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment["YEBISU_HANDOFF_QA"] = "1"
        XCUIDevice.shared.orientation = .portrait
        app.launch()
        sleep(3)
        try tapText("QA Text", in: app)
        try tapText("Paste", in: app)
        sleep(2)
        try tapText("QA Image", in: app)
        sleep(8)
        try tapText("QA image ready", in: app, tap: false)
        try tapText("Paste", in: app)
        sleep(1)
        try tapText("Paste image", in: app)
        app.typeText("P8-after")
        sleep(10)
        try tapText("QA Selection", in: app)
        sleep(2)
        save("p8-handoff")
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

    func testP9Core() throws {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.terminate()
        app.launchEnvironment["YEBISU_CORE_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment["YEBISU_INPUT_QA"] = "1"
        app.launch()
        sleep(3)
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

    func testP4Input() throws {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.terminate()
        app.launchEnvironment["YEBISU_TERMINAL_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment["YEBISU_INPUT_QA"] = "1"
        XCUIDevice.shared.orientation = .portrait
        app.launch()
        sleep(3)
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.35)).tap()
        app.typeText("P4-once")
        sleep(2)
        try tapText("Input options", in: app)
        sleep(1)
        try tapText("Option: Native", in: app)
        try tapText("Command: Local", in: app)
        try tapText("Candidate: Clause", in: app)
        sleep(2)
        save("p4-variants")
        try tapText("QA mark", in: app)
        sleep(2)
        save("p4-composition")
        try tapText("QA delete", in: app)
        sleep(2)
        save("p4-partial-delete")
        try tapText("QA cancel", in: app)
        sleep(2)
        save("p4-cancelled")
        try tapText("QA mark", in: app)
        sleep(2)
        try tapText("QA commit", in: app)
        sleep(2)
        save("p4-committed")
        XCTAssertEqual(app.state, .runningForeground)
    }

    func testP5Reconnect() throws {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.terminate()
        app.launchEnvironment["YEBISU_TERMINAL_QA"] = "1"
        app.launchEnvironment["YEBISU_P5_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launch()
        sleep(3)
        save("p5-before")
        XCUIDevice.shared.press(.home)
        sleep(2)
        app.activate()
        sleep(4)
        save("p5-returned")
        try tapText("Return: Automatic", in: app)
        XCUIDevice.shared.press(.home)
        sleep(2)
        app.activate()
        sleep(3)
        save("p5-manual")
        try tapText("Reconnect", in: app)
        sleep(3)
        save("p5-tapped")
        try tapText("Hosts", in: app)
        sleep(2)
        save("p5-hosts")
        try tapText("Developer: simulate save failure", in: app)
        sleep(2)
        save("p5-save-failure")
        XCTAssertEqual(app.state, .runningForeground)
    }

    func testR3Mismatch() throws {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.terminate()
        app.launchEnvironment["YEBISU_TERMINAL_QA"] = "1"
        app.launchEnvironment["YEBISU_R3_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        XCUIDevice.shared.orientation = .portrait
        app.launch()
        sleep(3)
        try tapText("Version mismatch", in: app)
        XCUIDevice.shared.press(.home)
        sleep(2)
        app.activate()
        sleep(3)
        try tapText("Version mismatch", in: app)
        try tapText("Reconnect", in: app)
        sleep(2)
        try tapText("Version mismatch", in: app)
        save("r3-mismatch-returned")
        XCTAssertEqual(app.state, .runningForeground)
    }

    func testR5LateMismatch() throws {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.terminate()
        app.launchEnvironment["YEBISU_TERMINAL_QA"] = "1"
        app.launchEnvironment["YEBISU_R5_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        XCUIDevice.shared.orientation = .portrait
        app.launch()
        sleep(3)
        try tapText("Return: Automatic", in: app, tap: false)
        XCUIDevice.shared.press(.home)
        sleep(2)
        app.activate()
        sleep(3)
        try tapText("R5-late-reader", in: app, tap: false)
        try tapText("Reconnect", in: app)
        sleep(2)
        try tapText("R5-late-reader", in: app, tap: false)
        save("r5-late-mismatch-returned")
        XCTAssertEqual(app.state, .runningForeground)
    }

    func testP6Board() throws {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.terminate()
        app.launchEnvironment["YEBISU_TERMINAL_QA"] = "1"
        app.launchEnvironment["YEBISU_BOARD_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launch()
        sleep(4)
        save("p6-dense")
        try tapText("Board: Dense", in: app)
        sleep(2)
        save("p6-roomy")
        try tapText("Board: Roomy", in: app)
        sleep(1)
        try tapText("Space 2", in: app)
        sleep(2)
        save("p6-space")
        try tapText("reviewer", in: app)
        sleep(2)
        save("p6-agent")
        try scrollBoard(in: app)
        try tapText("qa-beta", in: app)
        sleep(3)
        save("p6-beta")
        XCTAssertEqual(app.state, .runningForeground)
    }

    func testP7Tabs() throws {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.terminate()
        app.launchEnvironment["YEBISU_TERMINAL_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launch()
        sleep(3)
        try tapText("Toggle Sidebar", in: app)
        sleep(1)
        save("p7-initial")
        try tapText("New Tab", in: app)
        sleep(2)
        save("p7-tab")
        try tapText("Split Right", in: app)
        sleep(2)
        save("p7-split")
        try tapText("main", in: app)
        sleep(2)
        save("p7-return")
        try tapText("Command Palette", in: app)
        sleep(1)
        save("p7-palette")
        XCTAssertEqual(app.state, .runningForeground)
    }

    func testR6Shortcuts() throws {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.terminate()
        app.launchEnvironment["YEBISU_TERMINAL_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment["YEBISU_R6_QA"] = "1"
        app.launch()
        sleep(3)
        try tapText("Toggle Sidebar", in: app)
        sleep(1)
        try tapText("New Tab", in: app)
        sleep(2)
        save("r6-created")
        try tapText("QA Next", in: app)
        sleep(2)
        save("r6-next")
        try tapText("QA Previous", in: app)
        sleep(2)
        save("r6-previous")
        try tapText("QA Escape", in: app)
        sleep(2)
        save("r6-escape")
        XCTAssertEqual(app.state, .runningForeground)
    }

    private func scrollBoard(in app: XCUIApplication) throws {
        let image = try XCTUnwrap(XCUIScreen.main.screenshot().image.cgImage)
        let request = VNRecognizeTextRequest()
        request.recognitionLevel = .accurate
        try VNImageRequestHandler(cgImage: image).perform([request])
        let header = try XCTUnwrap(request.results?.first {
            $0.topCandidates(1).first?.string.contains("Board: Dense") == true
        })
        // The source defines a 180 pt Dense board immediately below this header.
        let top = 1 - header.boundingBox.minY + 10 / app.frame.height
        let start = app.coordinate(withNormalizedOffset: CGVector(dx: 0.75, dy: top + 140 / app.frame.height))
        let end = app.coordinate(withNormalizedOffset: CGVector(dx: 0.75, dy: top + 20 / app.frame.height))
        start.press(forDuration: 0.1, thenDragTo: end)
        sleep(1)
    }

    private func tapText(_ label: String, in app: XCUIApplication, tap: Bool = true) throws {
        let screenshot = XCUIScreen.main.screenshot()
        let image = try XCTUnwrap(screenshot.image.cgImage)
        let request = VNRecognizeTextRequest()
        request.recognitionLevel = .accurate
        try VNImageRequestHandler(cgImage: image).perform([request])
        let observation = try XCTUnwrap(request.results?.first {
            $0.topCandidates(1).first?.string.contains(label) == true
        }, "Missing source-defined control: \(label)")
        let candidate = try XCTUnwrap(observation.topCandidates(1).first)
        let range = try XCTUnwrap(candidate.string.range(of: label))
        let box = try XCTUnwrap(candidate.boundingBox(for: range)).boundingBox
        print("OCR tap \(label) frame=\(app.frame) box=\(box)")
        if tap {
            app.coordinate(withNormalizedOffset: CGVector(dx: box.midX, dy: 1-box.midY)).tap()
        }
    }

    private func save(_ name: String) {
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
