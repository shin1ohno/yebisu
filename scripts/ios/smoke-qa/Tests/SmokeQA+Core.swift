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
        // Drag selection consumes the tap that must open native input here. L2b moved
        // the start mode from the handoff band to Settings › Input › Start a selection.
        try openSettings(in: app)
        try tapText("Input", in: app)
        sleep(1)
        try tapText("Double tap", in: app)
        sleep(1)
        try tapText("Close", in: app)
        sleep(2)
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.4)).tap()
        app.typeText("P9-native\n")
        try tapText("QA mark", in: app)
        try tapText("QA delete", in: app)
        try tapText("QA commit", in: app)
        // L8: the Herdr 2 band moved into the tab row's ... menu (patch 07). The menu
        // sits below the mock peer's banner "Split · Detach / Reattach" (core_mock.rs),
        // so the uppermost-match rule would tap the banner: band items are matched only
        // in the menu's column, right of where its first item starts. After the first trip,
        // tapMenuItem opens the menu and finds the item in one OCR pass.
        let more = try openOverflowMenu(in: app)
        let column = try tapText("Split (2 panes)", in: app).box.minX - 0.02
        sleep(2)
        try tapMenuItem("Toggle split direction", in: app, at: more, minX: column)
        sleep(2)
        try tapMenuItem("Pane 2", in: app, at: more, minX: column)
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.4)).tap()
        app.typeText("P9-pane2\n")
        save("p9-split")
        try tapMenuItem("Detach", in: app, at: more, minX: column)
        sleep(1)
        // The status bar shows Reconnect while the core is detached (L2a).
        try tapText("Reconnect", in: app)
        sleep(3)
        try tapMenuItem("Pane 1", in: app, at: more, minX: column)
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.4)).tap()
        app.typeText("P9-pane1\n")
        save("p9-reattached")
        XCTAssertEqual(app.state, .runningForeground)
    }
}
