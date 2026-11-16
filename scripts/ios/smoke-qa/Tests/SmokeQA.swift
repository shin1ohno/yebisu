import XCTest
import Vision

final class SmokeQA: XCTestCase {
    func testInputAndLifecycleReceipts() throws {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        XCUIDevice.shared.orientation = .portrait
        app.activate()
        sleep(2)
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.425)).tap()
        app.typeText("P0-input")
        sleep(2)
        save("initial")
        XCUIDevice.shared.press(.home)
        sleep(2)
        app.activate()
        sleep(2)
        app.typeText("-returned")
        sleep(2)
        save("returned")
        XCUIDevice.shared.orientation = .landscapeLeft
        sleep(2)
        app.typeText("-rotated")
        sleep(2)
        save("rotated")
        XCUIDevice.shared.orientation = .portrait
        sleep(2)
        save("idle-before")
        sleep(60)
        app.typeText("-idle")
        sleep(2)
        save("idle")
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.97, dy: 0.96)).tap()
        sleep(2)
        save("image-clear")
        XCTAssertEqual(app.state, .runningForeground)
        // This liveness assertion is auxiliary. smoke_oracle.py asserts rendered
        // regions and app-written receipt state for every saved stage.
    }
    // The settings sheet shows PageId::id() as is ("appearance"), and tapText matches
    // case-sensitively. test_p15_check.py ties this value to settings_pages/mod.rs.
    private let appearancePage = "appearance"
    // Last body row of the private wire peer surface (mock_peer.rs).
    private let terminalInputRow = "Type here"

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

    /// Number of recognized text lines where any candidate contains `label`.
    /// Pure so the 0 and 2 cases do not need a Simulator.
    static func ocrLineCount(_ lines: [[String]], label: String) -> Int {
        lines.filter { candidates in candidates.contains { $0.contains(label) } }.count
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

    func testSH1153EditMenu() throws {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.terminate()
        app.launchEnvironment["YEBISU_TERMINAL_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment["YEBISU_EDIT_MENU_QA"] = "1"
        XCUIDevice.shared.orientation = .portrait
        app.launch()
        sleep(3)
        for step in 0..<10 {
            try tapText("Menu prepare", in: app)
            sleep(1)
            if step % 5 == 1 || step % 5 == 4 {
                try tapText("Menu select", in: app)
                sleep(1)
            }
            try tapText("Menu execute", in: app)
            sleep(1)
            save("sh1-153-case-\(step)")
            if step == 1 || step == 6 {
                print("SH1-153 MENU HIERARCHY case=\(step)\n\(app.debugDescription)")
                let edit = app.descendants(matching: .any).matching(NSPredicate(format: "label == 'Edit'")).firstMatch
                if edit.exists && edit.isHittable {
                    edit.tap()
                    sleep(1)
                    save("sh1-153-system-edit-\(step)")
                    print("SH1-153 OPEN MENU case=\(step)\n\(app.debugDescription)")
                    // Dismiss without choosing an extra command outside the receipt cases.
                    app.coordinate(withNormalizedOffset: CGVector(dx: 0.9, dy: 0.5)).tap()
                } else {
                    print("SH1-153 system Edit menu is not exposed to XCTest in case \(step)")
                }
            }
        }
        try tapText("Menu prepare", in: app)
        sleep(1)
        XCTAssertEqual(app.state, .runningForeground)
    }

    func testP8NativeEditing() throws {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.terminate()
        app.launchEnvironment["YEBISU_TERMINAL_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment["YEBISU_NATIVE_QA"] = "1"
        XCUIDevice.shared.orientation = .portrait
        app.launch()
        sleep(3)
        for _ in 0..<25 {
            try tapText("Native next", in: app)
            sleep(1)
        }
        save("p8-native")
        XCTAssertEqual(app.state, .runningForeground)
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

    func testSH154CoreImage() throws {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.terminate()
        app.launchEnvironment["YEBISU_CORE_QA"] = "1"
        app.launchEnvironment["YEBISU_HANDOFF_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launch()
        try tapText("QA Image only", in: app, attempts: 15)
        try tapText("QA image ready", in: app, tap: false, attempts: 30)
        try tapText("Paste image", in: app)
        try tapText("paste-0.png", in: app, tap: false, attempts: 30)
        let paste = try tapText("Paste", in: app, excludingSuffix: " image").box
        try tapText("paste-1.png", in: app, tap: false, attempts: 30)
        try tapText("QA Image", in: app, excludingSuffix: " only")
        try tapText("QA image ready", in: app, tap: false, attempts: 30)
        // Tap the text-priority Paste where it was found above instead of reading the
        // label again. In this frame Vision can miss the "Paste" label, and then a
        // truncated reading of "Paste image" passes the suffix filter: the tap starts
        // an image paste and P8-shadow never reaches the peer (main 2ed2beb, job
        // 132955457568). Nothing in the handoff row changes between the two taps; the
        // receiving checker (p9-check validate_images) proves the text paste arrived.
        app.coordinate(withNormalizedOffset: CGVector(dx: paste.midX, dy: 1 - paste.midY)).tap()
        sleep(1)
        try tapText("Paste image", in: app)
        try tapText("paste-2.png", in: app, tap: false, attempts: 30)
        save("sh154-core-image")
        XCTAssertEqual(app.state, .runningForeground)
    }

    /// Non-asserting OCR lookup. `matching` counts the recognized lines of the
    /// whole-screenshot pass that carry `label`, so a caller can reject an ambiguous tap.
    /// With `bands`, a miss on the whole screenshot is followed by overlapping horizontal
    /// bands: a busy frame can make Vision drop a whole row of small controls that the
    /// screenshot shows (P11 adds a Settings button above the SH1-154 strip).
    /// `attachAs` keeps the very screenshot the match came from, for a measurement made on it.
    private func locateText(_ label: String, excludingSuffix: String? = nil, bands: Bool = false, attachAs: String? = nil) throws -> (box: CGRect, matching: Int)? {
        let screenshot = XCUIScreen.main.screenshot()
        let image = try XCTUnwrap(screenshot.image.cgImage)
        var passes: [(bottom: CGFloat, height: CGFloat)] = [(0, 1)]
        if bands {
            for index in 0..<5 {
                let top = CGFloat(index) * 0.18
                passes.append((bottom: max(0, 1 - top - 0.28), height: min(0.28, 1 - top)))
            }
        }
        var matching = 0
        var candidateCount = 0
        for (pass, band) in passes.enumerated() {
            // Controls precede the terminal surface. Prefer the uppermost
            // match so shell output containing e.g. "Detach" cannot steal taps.
            let lines = try recognize(image, label: label, bottom: band.bottom, height: band.height)
                .sorted { $0.midY > $1.midY }
            if pass == 0 {
                matching = SmokeQA.ocrLineCount(lines.map { $0.texts.map { $0.string } }, label: label)
                candidateCount = lines.reduce(0) { $0 + $1.texts.count }
            }
            for line in lines {
                for candidate in line.texts {
                    guard let range = candidate.string.range(of: label),
                          let observation = try candidate.boundingBox(for: range) else { continue }
                    if let suffix = excludingSuffix, candidate.string[range.upperBound...].hasPrefix(suffix) {
                        continue
                    }
                    let local = observation.boundingBox
                    let box = CGRect(x: local.minX, y: line.bottom + local.minY * line.height,
                                     width: local.width, height: local.height * line.height)
                    print("OCR match pass=\(pass) lines=\(matching)")
                    if let name = attachAs {
                        let attachment = XCTAttachment(screenshot: screenshot)
                        attachment.name = name
                        attachment.lifetime = .keepAlways
                        add(attachment)
                    }
                    return (box, matching)
                }
            }
        }
        print("OCR candidate count=\(candidateCount), text suppressed")
        return nil
    }

    // Non-asserting presence probe for polling loops. Use tapText(tap: false) only
    // where the control MUST already be on screen.
    private func textVisible(_ label: String) -> Bool {
        (try? locateText(label)) != nil
    }

    private func scrollBoard(in app: XCUIApplication) throws {
        let image = try XCTUnwrap(XCUIScreen.main.screenshot().image.cgImage)
        let request = VNRecognizeTextRequest()
        request.recognitionLevel = .accurate
        try VNImageRequestHandler(cgImage: image).perform([request])
        let header = try XCTUnwrap(request.results?.first {
            $0.topCandidates(1).first?.string.contains("Board: Dense") == true
        })
        // The source defines a 180 pt Dense board immediately below this header.
        let top = 1 - header.boundingBox.minY + 10 / app.frame.height
        let start = app.coordinate(withNormalizedOffset: CGVector(dx: 0.75, dy: top + 140 / app.frame.height))
        let end = app.coordinate(withNormalizedOffset: CGVector(dx: 0.75, dy: top + 20 / app.frame.height))
        start.press(forDuration: 0.1, thenDragTo: end)
        sleep(1)
    }

    /// One Vision pass over the whole screenshot (`height == 1`) or over a horizontal band of it.
    /// `bottom` and `height` are fractions of the screenshot measured from its bottom edge, and
    /// `midY` is mapped back to the full screenshot.
    private func recognize(_ image: CGImage, label: String, bottom: CGFloat = 0, height: CGFloat = 1) throws
        -> [(texts: [VNRecognizedText], midY: CGFloat, bottom: CGFloat, height: CGFloat)] {
        var target = image
        if height < 1 {
            let full = CGFloat(image.height)
            let area = CGRect(x: 0, y: full * (1 - bottom - height), width: CGFloat(image.width), height: full * height)
            guard let cropped = image.cropping(to: area) else { return [] }
            target = cropped
        }
        let request = VNRecognizeTextRequest()
        request.recognitionLevel = .accurate
        request.customWords = [label]
        try VNImageRequestHandler(cgImage: target).perform([request])
        return (request.results ?? []).map {
            (texts: $0.topCandidates(5), midY: bottom + height * $0.boundingBox.midY, bottom: bottom, height: height)
        }
    }

    @discardableResult
    private func tapText(_ label: String, in app: XCUIApplication, tap: Bool = true, attempts: Int = 3, excludingSuffix: String? = nil, dx: CGFloat? = nil) throws -> (box: CGRect, matching: Int) {
        // GPUI controls have no UIKit accessibility nodes. Match source labels,
        // allowing Vision alternatives and a bounded wait for the next paint.
        // The whole screenshot is tried on every attempt, as before; the horizontal
        // bands are only the last resort, so a control the whole-screenshot pass finds
        // is never replaced by a band match.
        for attempt in 0..<attempts {
            if let found = try locateText(label, excludingSuffix: excludingSuffix, bands: attempt == attempts - 1) {
                print("OCR tap \(label) frame=\(app.frame) box=\(found.box)")
                if tap {
                    app.coordinate(withNormalizedOffset: CGVector(dx: dx ?? found.box.midX, dy: 1-found.box.midY)).tap()
                }
                return found
            }
            print("OCR attempt \(attempt) for \(label)")
            sleep(1)
        }
        save("missing-control")
        XCTFail("Missing source-defined control: \(label)")
        throw NSError(domain: "SmokeQA.MissingControl", code: 1)
    }

    /// P14 toast: one pass that screenshots (kept as `attachAs`), recognizes and taps, and
    /// returns the matched box in normalized screenshot coordinates for the keyboard-clearance check.
    private func tapTextBox(_ label: String, in app: XCUIApplication, attachAs: String) throws -> CGRect {
        for attempt in 0..<3 {
            if let found = try locateText(label, bands: attempt == 2, attachAs: attachAs) {
                print("OCR tap \(label) frame=\(app.frame) box=\(found.box)")
                app.coordinate(withNormalizedOffset: CGVector(dx: found.box.midX, dy: 1-found.box.midY)).tap()
                return found.box
            }
            print("OCR attempt \(attempt) for \(label)")
            sleep(1)
        }
        save("missing-control")
        XCTFail("Missing source-defined control: \(label)")
        throw NSError(domain: "SmokeQA.MissingControl", code: 1)
    }

    private func save(_ name: String) {
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
