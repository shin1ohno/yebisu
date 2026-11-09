import XCTest

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
    private func save(_ name: String) {
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
