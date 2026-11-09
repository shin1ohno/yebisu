import XCTest

final class SmokeQA: XCTestCase {
    func testGPUIInputAndLifecycle() throws {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.activate()
        sleep(2)
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.425)).tap()
        app.typeText("P0-input")
        save("initial", app)
        XCUIDevice.shared.press(.home)
        sleep(2)
        app.activate()
        sleep(2)
        app.typeText("-returned")
        save("returned", app)
        XCUIDevice.shared.orientation = .landscapeLeft
        sleep(2)
        app.typeText("-rotated")
        save("rotated", app)
        XCUIDevice.shared.orientation = .portrait
        sleep(60)
        app.typeText("-idle")
        save("idle", app)
        XCTAssertEqual(app.state, .runningForeground)
    }
    private func save(_ name: String, _ app: XCUIApplication) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
