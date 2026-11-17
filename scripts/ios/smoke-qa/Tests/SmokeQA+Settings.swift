import XCTest
import Vision

// P10 settings, P11 settings sheet, SH1-151 selection settings.
extension SmokeQA {
    func testP11Settings() throws {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.terminate()
        app.launchEnvironment["YEBISU_TERMINAL_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment["YEBISU_P11_QA"] = "1"
        app.launchEnvironment["YEBISU_P11_KEYS_QA"] = "1"
        app.launch()
        sleep(4)
        try tapText("Settings", in: app)
        sleep(1)
        try tapText("Larger", in: app)
        try tapText("Probe modal callbacks", in: app)
        sleep(3)
        save("p11-modal")
        try tapText("Close", in: app)
        sleep(2)
        XCUIDevice.shared.press(.home)
        sleep(1)
        app.activate()
        save("p11-font")
        app.terminate()
        app.launch()
        sleep(4)
        save("p11-relaunch")
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
}
