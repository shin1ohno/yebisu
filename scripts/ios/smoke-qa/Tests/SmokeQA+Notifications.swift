import XCTest
import Vision

// P14 connected notifications.
extension SmokeQA {
    func testP14ConnectedNotifications() throws {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.terminate()
        app.launchEnvironment["YEBISU_TERMINAL_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment["YEBISU_P14_QA"] = "1"
        app.launch()
        sleep(4)
        // Owner decision: with the software keyboard up, the toast sits just above it.
        // A tap on the terminal focuses its input, which raises the keyboard.
        try tapText("Type here", in: app)
        sleep(2)
        let keyboard = app.keyboards.element(boundBy: 0)
        let keyboardTop = keyboard.exists ? keyboard.frame.minY : app.frame.height
        XCTAssertLessThan(keyboardTop, app.frame.height - 100, "the software keyboard must be up for this measurement")
        try tapText("P14 next", in: app)
        sleep(1)
        // The toast lives five seconds: a single pass screenshots, recognizes and taps it.
        let toast = try tapTextBox("Agent needs attention", in: app, attachAs: "p14-toast")
        let toastBottom = app.frame.minY + (1 - toast.minY) * app.frame.height
        print("P14 keyboard top=\(keyboardTop) toast bottom=\(toastBottom)")
        XCTAssertLessThanOrEqual(toastBottom, keyboardTop + 2, "toast is covered by the software keyboard")
        sleep(2)
        save("p14-focus")
        for name in ["p14-quiet", "p14-invalid", "p14-disconnect", "p14-stale-tap"] {
            try tapText("P14 next", in: app)
            sleep(3)
            save(name)
            if name == "p14-quiet" {
                // The on-screen status line lives five seconds like the toast.
                XCTAssertTrue(textVisible("QuietHours"), "the status line must be visible right after the refusal")
                sleep(4)
                save("p14-status-expired")
                XCTAssertFalse(textVisible("QuietHours"), "the status line must disappear after its lifetime")
            }
        }
        XCTAssertEqual(app.state, .runningForeground)
        // Never visit notification settings or answer an OS permission dialog.
    }
}
