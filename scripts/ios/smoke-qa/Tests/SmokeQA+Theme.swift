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
        let qa = QAAx(app)
        app.terminate()
        app.launchEnvironment["YEBISU_TERMINAL_QA"] = "1"
        app.launchEnvironment["YEBISU_BOARD_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment["YEBISU_P15_QA"] = "1"
        app.launchEnvironment["YEBISU_QA_AX"] = "1"
        XCUIDevice.shared.orientation = .portrait
        app.launch()
        // Before S6: sleep(4).
        try waitTerminal(qa, "the qa-alpha terminal is connected") { $0.string("host") == "qa-alpha" }
        save("p15-default")
        // Real UIKit typing brackets the modal operations in the peer's FIFO. Before S6 an OCR tap
        // on the terminal's last row focused it (TODO.md: in one run that tap landed outside the
        // terminal and P15-start never arrived); now the terminal's input area by identifier.
        try typeP15(qa, app, "P15-start", ui: &ui)
        try openSettingsSheet(qa)
        try tapAppearance(qa, ui: &ui)
        try p15Host(qa, "qa-alpha", stale: "qa-beta", stage: "alpha", ui: &ui)
        for (id, label, key, stage) in [(QAIds.themeHighContrast, "High contrast", "high-contrast", "high-first"),
                                        (QAIds.themeOwnerDefault, "Owner default", "owner-default", "owner-reset"),
                                        (QAIds.themeHighContrast, "High contrast", "high-contrast", "selected")] {
            // Before S6: OCR found the label, then sleep(2).
            try qa.press(id, what: "the local theme is \(key)") { $0.string("theme") == key }
            ui.append(["kind": "theme-click", "label": label, "time": Date().timeIntervalSince1970])
            save("p15-" + stage)
        }
        try qa.press(QAIds.settingsClose, what: "the settings sheet is closed") { $0.isNull("sheet") }
        save("p15-closed")
        try typeP15(qa, app, "P15-end", ui: &ui)
        // Use the existing production host rows, not direct actions.
        try pressHostRow(qa, QAIds.sidebarHostBeta, host: "qa-beta")
        try openSettingsSheet(qa)
        try tapAppearance(qa, ui: &ui)
        try p15Host(qa, "qa-beta", stale: "qa-alpha", stage: "beta", ui: &ui)
        try qa.press(QAIds.settingsClose, what: "the settings sheet is closed") { $0.isNull("sheet") }
        XCUIDevice.shared.press(.home)
        try requireState(app, [.runningBackground, .runningBackgroundSuspended], "P15 app left the foreground after press(.home)")
        app.activate()
        try requireState(app, [.runningForeground], "P15 activate() after press(.home)")
        try waitTerminal(qa, "the terminal after returning") { $0.string("theme") == "high-contrast" }
        save("p15-returned")
        app.terminate()
        app.launch()
        try waitTerminal(qa, "the relaunched qa-alpha terminal with the saved theme") {
            $0.string("host") == "qa-alpha" && $0.string("theme") == "high-contrast"
        }
        save("p15-relaunch")
        try openSettingsSheet(qa)
        try tapAppearance(qa, ui: &ui)
        // Before S6: OCR found "High contrast" without tapping it.
        try qa.find(QAIds.themeHighContrast)
        try p15Host(qa, "qa-alpha", stale: "qa-beta", stage: "relaunch-host", ui: &ui)
        try qa.press(QAIds.settingsClose, what: "the settings sheet is closed") { $0.isNull("sheet") }
        XCTAssertEqual(app.state, .runningForeground)
    }

    /// Focuses the terminal by its input area and types `text`, then waits up to 10 s until the
    /// private peer received one more input message (qa.state `peer.inputs`; before S6: sleep(1)).
    /// Whether it arrived is recorded (`typed`), not asserted: p15-check.py's FIFO gate judges the
    /// peer's transcript, as before S6, so the rest of the test still runs when it did not (TODO.md:
    /// P15-start has not reached the peer on the Mac mini; the app queues it with input not ready).
    private func typeP15(_ qa: QAAx, _ app: XCUIApplication, _ text: String,
                                  ui: inout [[String: Any]]) throws {
        try qa.settleIfMoved("before typing \(text)")
        let before = try qa.press(QAIds.terminalInput, what: "the terminal took the tap")
        qa.keyboardMayMove("the terminal tap may raise the keyboard")
        app.typeText(text)
        let start = Date()
        var received = true
        do {
            try qa.waitState("the peer received '\(text)'", timeout: 10) { $0.peerInputs > before.peerInputs }
        } catch QAAxError.timeout(_, _, _, let last) {
            received = false
            print("P15 the private peer received no input within 10 s of typing '\(text)'; p15-check.py's FIFO gate decides. last qa.state=\(last)")
        }
        ui.append(["kind": "typed", "text": text, "peer_received": received,
                   "waited": Date().timeIntervalSince(start), "time": Date().timeIntervalSince1970])
    }

    /// Presses the sidebar's host row of `host` (shell/sidebar/view.rs, a list that scrolls under
    /// its HOSTS heading). A row below the list's visible part is first dragged into it, as the
    /// drag placed by the OCR'd "HOSTS" heading did before S6.
    private func pressHostRow(_ qa: QAAx, _ id: String, host: String) throws {
        try qa.settleIfMoved("before the host list")
        let list = try qa.find(QAIds.sidebarList)
        for drag in 0..<3 {
            let row = try qa.find(id).frame
            if list.frame.contains(CGPoint(x: row.midX, y: row.midY)) { break }
            print("P15 host row \(id) at \(row) is outside the list \(list.frame); drag \(drag)")
            list.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.78))
                .press(forDuration: 0.1, thenDragTo: list.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.11)))
            _ = try qa.waitState("qa.state after the list drag") { _ in true }
            Thread.sleep(forTimeInterval: 0.5)
        }
        // Before S6: OCR tapped "qa-beta", then sleep(2).
        try qa.press(id, what: "the selected host is \(host)") { $0.string("host") == host }
        qa.keyboardMayMove("the host row may dismiss the keyboard")
    }

    /// Presses the Theme page and records how many controls carry its identifier: QAAx.find fails
    /// on a duplicate, so the count is 1 (before S6 the number of OCR lines carrying "Theme",
    /// where more than one meant the tap could land on another text).
    private func tapAppearance(_ qa: QAAx, ui: inout [[String: Any]]) throws {
        try qa.find(QAIds.settingsPageTheme)
        let state = try qa.press(QAIds.settingsPageTheme, what: "the Theme page is open") { $0.string("settings_page") == "appearance" }
        ui.append(["kind": "qa-match", "id": QAIds.settingsPageTheme, "count": state.duplicates.contains(QAIds.settingsPageTheme) ? 2 : 1,
                   "time": Date().timeIntervalSince1970])
    }

    /// The one screenshot text check the UI tests keep (owner decision: screenshot checks stay
    /// where the text is the subject, ui-test-redesign-decision.json): the host settings block
    /// must show the current host, never the old one, and the fail-closed texts. qa.state first
    /// confirms the app has that host selected (before S6: sleep(2)).
    private func p15Host(_ qa: QAAx, _ host: String, stale: String, stage: String,
                         ui: inout [[String: Any]]) throws {
        // This proves rendered current-host/Unavailable projection only. There is
        // no settings read request whose stale response could be injected yet.
        try qa.waitState("the Theme page shows host \(host)") {
            $0.string("host") == host && $0.string("settings_page") == "appearance"
        }
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
