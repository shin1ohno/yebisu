import XCTest
import Vision

// P2 keys and restart.
extension SmokeQA {
    func testP2KeysAndRestart() throws {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        XCUIDevice.shared.orientation = .portrait
        app.activate()
        sleep(3)
        save("p2-initial")
        try tapText("QA injected key provider", in: app, tap: false)
        try tapText("Read or initialize device key", in: app)
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
        app.launchEnvironment["YEBISU_P11_KEYS_QA"] = "1"
        app.launch()
        sleep(3)
        try tapText("QA injected key provider", in: app, tap: false)
        try tapText("Read or initialize device key", in: app)
        sleep(4)
        save("p2-restarted")
        XCTAssertEqual(app.state, .runningForeground)
    }
}
