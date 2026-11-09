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
        try tapText("qa-beta", in: app)
        sleep(3)
        save("p6-beta")
        try tapText("Space 1", in: app)
        sleep(2)
        save("p6-space")
        try tapText("reviewer", in: app)
        sleep(2)
        save("p6-agent")
        XCTAssertEqual(app.state, .runningForeground)
    }

    private func tapText(_ label: String, in app: XCUIApplication) throws {
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
        app.coordinate(withNormalizedOffset: CGVector(dx: box.midX, dy: 1-box.midY)).tap()
    }

    private func save(_ name: String) {
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
