import XCTest
import Vision

// P9 core.
extension SmokeQA {
    func testP9Core() throws {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.terminate()
        app.launchEnvironment["YEBISU_CORE_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment["YEBISU_INPUT_QA"] = "1"
        app.launch()
        sleep(3)
        // Drag selection consumes the tap that must open native input here.
        try tapText("Start: Drag", in: app)
        try tapText("Start: Select / double tap", in: app, tap: false)
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.4)).tap()
        app.typeText("P9-native\n")
        try tapText("QA mark", in: app)
        try tapText("QA delete", in: app)
        try tapText("QA commit", in: app)
        try tapText("Split (2 panes)", in: app)
        sleep(2)
        try tapText("Toggle split direction", in: app)
        sleep(2)
        try tapText("Pane 2", in: app)
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.4)).tap()
        app.typeText("P9-pane2\n")
        save("p9-split")
        try tapText("Detach", in: app)
        sleep(1)
        try tapText("Reconnect", in: app)
        sleep(3)
        try tapText("Pane 1", in: app)
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.4)).tap()
        app.typeText("P9-pane1\n")
        save("p9-reattached")
        XCTAssertEqual(app.state, .runningForeground)
    }
}
