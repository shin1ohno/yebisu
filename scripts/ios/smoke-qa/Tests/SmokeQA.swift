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

    private func tapText(_ label: String, in app: XCUIApplication) throws {
        let screenshot = XCUIScreen.main.screenshot()
        let image = try XCTUnwrap(screenshot.image.cgImage)
        let request = VNRecognizeTextRequest()
        request.recognitionLevel = .accurate
        try VNImageRequestHandler(cgImage: image).perform([request])
        let observation = try XCTUnwrap(request.results?.first {
            $0.topCandidates(1).first?.string.contains(label) == true
        }, "Missing source-defined control: \(label)")
        let box = observation.boundingBox
        app.coordinate(withNormalizedOffset: CGVector(dx: box.midX, dy: 1-box.midY)).tap()
    }

    private func save(_ name: String) {
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
