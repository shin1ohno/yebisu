import XCTest
import Vision

// P11 observer probes.
extension SmokeQA {
    // SDK11 candidate source harness. UI waits never certify native completion;
    // only the Rust state machine's actual callback/worker/peer receipts advance.
    private func observerProbe(modal: Bool, drawing: Bool = false, disabled: Bool = false) throws {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.terminate()
        app.launchEnvironment["YEBISU_TERMINAL_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment["YEBISU_P11_KEYS_QA"] = "1"
        app.launchEnvironment["YEBISU_P11_CAPTURE"] = "1"
        app.launchEnvironment["YEBISU_P11_BINDING"] = "P11_BINDING_UNSET"
        app.launchEnvironment[modal ? "YEBISU_P11_QA" : "YEBISU_NATIVE_QA"] = "1"
        if drawing { app.launchEnvironment["YEBISU_P11_DRAWING"] = "1" }
        if disabled { app.launchEnvironment["YEBISU_P11_OBSERVER_DISABLED"] = "1" }
        app.launch()
        sleep(3)
        if modal {
            try tapText("Settings", in: app)
            try tapText("Larger", in: app)
        }
        let done = modal ? "Observer matrix collected" : "Native matrix collected"
        var collected = false
        for _ in 0..<160 {
            if textVisible(done) { collected = true; break }
            try tapText(modal ? "Observer next" : "Native next", in: app)
            Thread.sleep(forTimeInterval: 0.2)
        }
        XCTAssertTrue(collected, "Missing actual callback/worker/receiving completion")
        save(modal ? "p11-observer-modal" : "p11-observer-positive")
        if modal {
            try tapText("Close", in: app)
            XCUIDevice.shared.press(.home)
            app.activate()
            app.terminate()
            app.launch()
            sleep(3)
            save("p11-observer-relaunch")
        }
        app.terminate()
        XCTAssertEqual(app.state, .notRunning)
    }
    func testP11ObserverText() throws { try observerProbe(modal: false) }
    func testP11ObserverDrawing() throws { try observerProbe(modal: false, drawing: true) }
    func testP11ObserverModal() throws { try observerProbe(modal: true) }
    func testP11ObserverDisabled() throws { try observerProbe(modal: true, disabled: true) }
}
