import CryptoKit
import Vision
import XCTest

/// S0 probe of the QA accessibility layer (UI test redesign plan, L-A / O-A).
/// It measures, without asserting more than the probe needs:
/// (a) XCTest finds `qa.shell.more` by identifier, `isHittable`, and `tap()` reaches
///     the GPUI ... button (the app's own `qa.state` press count and menu state move);
/// (b) how long waiting for a `qa.state` change takes (polling and
///     XCTNSPredicateExpectation);
/// (c) whether this runner can read `UIPasteboard` for the p2 SHA-256 path;
/// (d) one identifier query against one OCR pass.
/// Every measurement is one `S0PROBE` line. Runs only with QA_AX_PROBE set
/// (xcodebuild passes TEST_RUNNER_QA_AX_PROBE), so no existing job runs it.
final class QAAxProbe: XCTestCase {
    private func log(_ text: String) {
        print("S0PROBE \(text)")
    }

    private func ms(_ since: Date) -> String {
        String(format: "%.1f", Date().timeIntervalSince(since) * 1000)
    }

    private func state(_ element: XCUIElement) -> [String: Any]? {
        guard let value = element.value as? String, let data = value.data(using: .utf8) else { return nil }
        return (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
    }

    private func sha256(_ text: String) -> String {
        SHA256.hash(data: Data(text.utf8)).map { String(format: "%02x", $0) }.joined()
    }

    /// Polls `qa.state` until its `seq` passes `after`; returns (ms, queries, state).
    private func pollSeq(_ element: XCUIElement, after: Int, timeout: TimeInterval = 10)
        -> (String, Int, [String: Any]?) {
        let start = Date()
        var queries = 0
        while Date().timeIntervalSince(start) < timeout {
            queries += 1
            if let value = state(element), let seq = value["seq"] as? Int, seq > after {
                return (ms(start), queries, value)
            }
        }
        return (ms(start), queries, nil)
    }

    func testS0Probe() throws {
        try XCTSkipUnless(ProcessInfo.processInfo.environment["QA_AX_PROBE"] != nil, "S0 probe only")
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        // The ... button is in the tab row, which the shell draws once a terminal is
        // open; YEBISU_TERMINAL_QA opens one on the private Herdr 1 mock peer.
        app.terminate()
        app.launchEnvironment = ["YEBISU_QA_AX": "1", "YEBISU_SMOKE_OBSERVE": "1", "YEBISU_TERMINAL_QA": "1"]
        XCUIDevice.shared.orientation = .portrait
        let launched = Date()
        app.launch()
        log("launch ms=\(ms(launched))")
        let more = app.descendants(matching: .any)["qa.shell.more"]
        let stateElement = app.descendants(matching: .any)["qa.state"]

        // (a) find by identifier
        let findStart = Date()
        let found = more.waitForExistence(timeout: 30)
        log("a.exists found=\(found) ms=\(ms(findStart))")
        XCTAssertTrue(found, "qa.shell.more not exposed")
        guard found else { return }
        log("a.frame element=\(more.frame) app=\(app.frame) type=\(more.elementType.rawValue) label=\(more.label)")
        log("a.state initial=\(String(describing: stateElement.value))")

        // (a)+(b) four presses at the element: open, close, open, close. With the menu
        // closed the ... button takes the press (the app counts it in `presses`); with the
        // menu open its backdrop covers the button and takes the press, closing the menu
        // without counting it. Each press must move `seq` and set `overflow` as expected.
        var last = state(stateElement)
        for press in 1...4 {
            let before = (last?["seq"] as? Int) ?? 0
            let pressesBefore = (last?["presses"] as? Int) ?? 0
            let hittable = more.isHittable
            let tapStart = Date()
            if hittable {
                more.tap()
            } else {
                more.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
            }
            let tapMs = ms(tapStart)
            let (waitMs, queries, value) = pollSeq(stateElement, after: before)
            let overflow = value?["overflow"] as? Bool
            let presses = value?["presses"] as? Int
            let opening = press % 2 == 1
            log("a.press n=\(press) hittable=\(hittable) via=\(hittable ? "element" : "coordinate") tap_ms=\(tapMs) b.poll_ms=\(waitMs) queries=\(queries) seq=\(value?["seq"] ?? "nil") presses=\(String(describing: presses)) overflow=\(String(describing: overflow)) expected_overflow=\(opening) taken_by=\(opening ? "button" : "backdrop")")
            XCTAssertEqual(overflow, opening, "menu state after press \(press)")
            XCTAssertEqual(presses, opening ? pressesBefore + 1 : pressesBefore, "press \(press) count")
            last = value ?? last
        }

        // (b) the same wait through XCTNSPredicateExpectation (XCTest's own polling):
        // press 5 opens the menu again, press 6 closes it.
        for press in 5...6 {
            let before = (state(stateElement)?["seq"] as? Int) ?? 0
            let predicate = NSPredicate { object, _ in
                guard let element = object as? XCUIElement, let value = element.value as? String,
                      let data = value.data(using: .utf8),
                      let json = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any],
                      let seq = json["seq"] as? Int else { return false }
                return seq > before
            }
            let expectation = XCTNSPredicateExpectation(predicate: predicate, object: stateElement)
            let expectStart = Date()
            more.tap()
            let result = XCTWaiter.wait(for: [expectation], timeout: 10)
            log("b.expectation press=\(press) result=\(result.rawValue) completed=\(result == .completed) ms=\(ms(expectStart)) state=\(String(describing: stateElement.value))")
            XCTAssertEqual(result, .completed, "qa.state did not change after press \(press)")
        }

        // (c) pasteboard: the app put a nonce on the pasteboard at the last press.
        let expected = state(stateElement)?["pasteboard"] as? String
        let board = UIPasteboard.general
        let metaStart = Date()
        let hasStrings = board.hasStrings
        let changeCount = board.changeCount
        log("c.meta has_strings=\(hasStrings) change_count=\(changeCount) types=\(board.types) ms=\(ms(metaStart))")
        var read: String?
        var readDone = false
        let lock = NSLock()
        let readStart = Date()
        DispatchQueue.global().async {
            let text = UIPasteboard.general.string
            lock.lock()
            read = text
            readDone = true
            lock.unlock()
        }
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        var alertSeen = false
        while Date().timeIntervalSince(readStart) < 20 {
            lock.lock()
            let done = readDone
            lock.unlock()
            if done { break }
            let allow = springboard.buttons["Allow Paste"]
            if allow.exists {
                alertSeen = true
                allow.tap()
            }
            usleep(200_000)
        }
        lock.lock()
        let finished = readDone
        let text = read
        lock.unlock()
        let equal = text != nil && text == expected
        log("c.read finished=\(finished) ms=\(ms(readStart)) alert=\(alertSeen) read_nil=\(text == nil) equal=\(equal) sha256_read=\(text.map(sha256) ?? "nil") sha256_expected=\(expected.map(sha256) ?? "nil")")

        // (d) one identifier query against one OCR pass, five and three times.
        for index in 1...5 {
            let start = Date()
            let frame = more.frame
            log("d.query n=\(index) ms=\(ms(start)) frame=\(frame)")
        }
        for index in 1...5 {
            let start = Date()
            let value = stateElement.value
            log("d.state_query n=\(index) ms=\(ms(start)) bytes=\((value as? String)?.utf8.count ?? -1)")
        }
        for index in 1...3 {
            let start = Date()
            let image = try XCTUnwrap(XCUIScreen.main.screenshot().image.cgImage)
            let shot = ms(start)
            let request = VNRecognizeTextRequest()
            request.recognitionLevel = .accurate
            try VNImageRequestHandler(cgImage: image).perform([request])
            log("d.ocr n=\(index) ms=\(ms(start)) screenshot_ms=\(shot) lines=\(request.results?.count ?? 0)")
        }
        let shot = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        shot.name = "s0-probe-end"
        shot.lifetime = .keepAlways
        add(shot)
        log("done")
    }
}
