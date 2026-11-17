import XCTest
import Vision

// P0 input and lifecycle receipts, P3 terminal.
extension SmokeQA {
    func testInputAndLifecycleReceipts() throws {
        // A failed state wait below must stop the test at once under its own name,
        // not turn into three 30 s snapshot timeouts on whatever comes next.
        continueAfterFailure = false
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        XCUIDevice.shared.orientation = .portrait
        app.activate()
        try requireState(app, [.runningForeground], "initial activate()")
        try waitForRenderedText("Async counter", what: "first paint after launch")
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.425)).tap()
        app.typeText("P0-input")
        try waitForRenderedText("input", what: "typed 'P0-input' received")
        save("initial")
        XCUIDevice.shared.press(.home)
        // activate() issued while the Home press is still being handled leaves the app
        // suspended in the background (class C of the L17 diagnosis).
        try requireState(app, [.runningBackground, .runningBackgroundSuspended], "app left the foreground after press(.home)")
        app.activate()
        try requireState(app, [.runningForeground], "activate() after press(.home)")
        try waitForRenderedText("input", what: "repaint of 'P0-input' after returning")
        app.typeText("-returned")
        try waitForRenderedText("returned", what: "typed '-returned' received")
        save("returned")
        XCUIDevice.shared.orientation = .landscapeLeft
        sleep(2)
        // The native keyboard covers the input field in landscape, so no screen text
        // shows this value. smoke_oracle.py still requires its receipt in app.jsonl.
        app.typeText("-rotated")
        sleep(2)
        save("rotated")
        XCUIDevice.shared.orientation = .portrait
        sleep(2)
        save("idle-before")
        sleep(60)
        app.typeText("-idle")
        try waitForRenderedText("idle", what: "typed '-idle' received")
        save("idle")
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.97, dy: 0.96)).tap()
        sleep(2)
        save("image-clear")
        XCTAssertEqual(app.state, .runningForeground)
        // This liveness assertion is auxiliary. smoke_oracle.py asserts rendered
        // regions and app-written receipt state for every saved stage.
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
}
