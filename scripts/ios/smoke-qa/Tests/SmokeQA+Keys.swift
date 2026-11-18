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
        try readDeviceKey(in: app, "P2 first launch")
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
        try readDeviceKey(in: app, "P2 restart")
        save("p2-restarted")
        XCTAssertEqual(app.state, .runningForeground)
    }

    /// Taps Read or initialize device key until the key line replaces "No public key loaded."
    /// (the key field, shell/hosts_sheet/view.rs). A press the sheet did not take is repeated
    /// once, with the reason printed: Mac mini 6f2d709, the first launch recorded no "read"
    /// preflight and key_ready never became true, so p2-check failed "injected key not ready in
    /// each independent launch" although XCTest had passed.
    private func readDeviceKey(in app: XCUIApplication, _ what: String) throws {
        for press in 0..<2 {
            try tapText("Read or initialize device key", in: app)
            for _ in 0..<8 {
                sleep(1)
                if !textVisible("No public key loaded.") { return }
            }
            print("\(what): key line still empty 8 s after press \(press): the press was not taken")
        }
        save("p2-no-key")
        XCTFail("\(what): no public key after two presses of Read or initialize device key")
    }
}
