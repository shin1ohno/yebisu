import XCTest
import Vision

// P15 theme.
extension SmokeQA {
    func testP15Theme() throws {
        continueAfterFailure = false
        var ui: [[String: Any]] = []
        defer {
            if let data = try? JSONSerialization.data(withJSONObject: ui, options: [.sortedKeys]) {
                let attachment = XCTAttachment(data: data, uniformTypeIdentifier: "public.json")
                attachment.name = "p15-ui.json"
                attachment.lifetime = .keepAlways
                add(attachment)
            }
        }
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.terminate()
        app.launchEnvironment["YEBISU_TERMINAL_QA"] = "1"
        app.launchEnvironment["YEBISU_BOARD_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment["YEBISU_P15_QA"] = "1"
        XCUIDevice.shared.orientation = .portrait
        app.launch()
        sleep(4)
        save("p15-default")
        // Real UIKit typing brackets the modal operations in the peer's FIFO. Focus the
        // terminal on its last body row at mid-width, as the coordinate taps of P4/P9 do
        // (the row's text starts at the left edge, where the first run tapped).
        try tapText(terminalInputRow, in: app, dx: 0.5)
        app.typeText("P15-start")
        sleep(1)
        try tapText("Settings", in: app)
        try tapAppearance(in: app, ui: &ui)
        try p15Host("qa-alpha", stale: "qa-beta", stage: "alpha", ui: &ui)
        for (label, stage) in [("High contrast", "high-first"),
                               ("Owner default", "owner-reset"),
                               ("High contrast", "selected")] {
            try tapText(label, in: app)
            ui.append(["kind": "theme-click", "label": label, "time": Date().timeIntervalSince1970])
            sleep(2)
            save("p15-" + stage)
        }
        try tapText("Close", in: app)
        sleep(2)
        save("p15-closed")
        try tapText(terminalInputRow, in: app, dx: 0.5)
        app.typeText("P15-end")
        sleep(1)
        // Use the existing production board host buttons, not direct actions.
        try scrollBoard(in: app)
        try tapText("qa-beta", in: app)
        sleep(2)
        try tapText("Settings", in: app)
        try tapAppearance(in: app, ui: &ui)
        try p15Host("qa-beta", stale: "qa-alpha", stage: "beta", ui: &ui)
        try tapText("Close", in: app)
        XCUIDevice.shared.press(.home)
        sleep(2)
        app.activate()
        sleep(3)
        save("p15-returned")
        app.terminate()
        app.launch()
        sleep(4)
        save("p15-relaunch")
        try tapText("Settings", in: app)
        try tapAppearance(in: app, ui: &ui)
        try tapText("High contrast", in: app, tap: false)
        try p15Host("qa-alpha", stale: "qa-beta", stage: "relaunch-host", ui: &ui)
        try tapText("Close", in: app)
        XCTAssertEqual(app.state, .runningForeground)
    }

    /// Taps the Appearance page button and records how many OCR lines carried the label.
    /// More than one line means the tap could land on another text (e.g. terminal output).
    private func tapAppearance(in app: XCUIApplication, ui: inout [[String: Any]]) throws {
        let lines = try tapText(appearancePage, in: app).matching
        ui.append(["kind": "ocr-match", "label": appearancePage, "count": lines,
                   "time": Date().timeIntervalSince1970])
        XCTAssertEqual(lines, 1, "OCR lines containing \(appearancePage)")
    }

    private func p15Host(_ host: String, stale: String, stage: String,
                         ui: inout [[String: Any]]) throws {
        // This proves rendered current-host/Unavailable projection only. There is
        // no settings read request whose stale response could be injected yet.
        sleep(2)
        let shot = XCUIScreen.main.screenshot()
        let image = try XCTUnwrap(shot.image.cgImage)
        let request = VNRecognizeTextRequest()
        request.recognitionLevel = .accurate
        request.customWords = [host, stale, "Unavailable", "File path unavailable"]
        try VNImageRequestHandler(cgImage: image).perform([request])
        let lines = (request.results ?? []).compactMap { $0.topCandidates(1).first?.string }
        let text = lines.joined(separator: "\n")
        ui.append(["kind": "host-screen", "stage": stage, "host": host,
                   "stale": stale, "lines": lines, "time": Date().timeIntervalSince1970])
        save("p15-" + stage)
        XCTAssertTrue(text.contains("Host: " + host), "Current host must be visible")
        XCTAssertFalse(text.contains("Host: " + stale), "Old host must not occupy the settings view")
        XCTAssertTrue(text.contains("File path unavailable"), "No invented remote path")
        XCTAssertTrue(text.contains("Unavailable"), "Read authority is unavailable")
        XCTAssertTrue(text.contains("daemon-effective state unverified"))
    }
}
