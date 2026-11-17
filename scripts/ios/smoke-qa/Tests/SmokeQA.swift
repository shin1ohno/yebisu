import XCTest
import Vision

final class SmokeQA: XCTestCase {
    // P9 normally takes about 60 s. A hang inside it (the app never reports idle, so
    // every XCUITest query waits its 60 s animation timeout) must fail the test here
    // instead of running into the 40-minute job timeout. p9-check.py passes
    // -test-timeouts-enabled YES; XCTest rounds the allowance up to whole minutes.
    override func setUpWithError() throws {
        if name.contains("testP9Core") {
            executionTimeAllowance = 150
        }
    }

    private func stateName(_ state: XCUIApplication.State) -> String {
        switch state {
        case .notRunning: return "notRunning"
        case .runningBackgroundSuspended: return "runningBackgroundSuspended"
        case .runningBackground: return "runningBackground"
        case .runningForeground: return "runningForeground"
        default: return "unknown"
        }
    }

    /// Waits for the app to reach one of `states`; fails at once, naming `what`, if it does not.
    /// `app.state` alone is not refreshed while it is polled (run 43770292492: Home pressed at
    /// t=61 s, state still runningForeground at t=92 s), so each pass asks XCTest to wait for a state.
    func requireState(_ app: XCUIApplication, _ states: [XCUIApplication.State], _ what: String,
                      timeout: TimeInterval = 30) throws {
        let start = Date()
        while true {
            for state in states where app.wait(for: state, timeout: 0.5) {
                print(String(format: "WAIT ok %@: app.state=%@ after %.2f s", what, stateName(state), Date().timeIntervalSince(start)))
                return
            }
            if Date().timeIntervalSince(start) >= timeout {
                // Read the state once: the verdict and the message must come from the same
                // observation, and `save()` takes seconds, long enough for the state to change.
                let observed = app.state
                if states.contains(observed) {
                    print(String(format: "WAIT ok %@: app.state=%@ after %.2f s (late)", what, stateName(observed), Date().timeIntervalSince(start)))
                    return
                }
                save("state-wait-failed")
                XCTFail("\(what): app.state is \(stateName(observed)) after \(Int(timeout)) s, wanted \(states.map(stateName))")
                throw NSError(domain: "SmokeQA.StateWait", code: 1)
            }
        }
    }

    /// Waits until a Vision pass over the screen reads `word` (case-insensitive). P0 paints only
    /// fixed source strings, so on a miss the recognized strings are printed for diagnosis.
    func waitForRenderedText(_ word: String, what: String, timeout: TimeInterval = 20) throws {
        let start = Date()
        while true {
            let image = try XCTUnwrap(XCUIScreen.main.screenshot().image.cgImage)
            let request = VNRecognizeTextRequest()
            request.recognitionLevel = .accurate
            request.customWords = [word]
            try VNImageRequestHandler(cgImage: image).perform([request])
            let strings = (request.results ?? []).flatMap { $0.topCandidates(5).map { $0.string } }
            if strings.contains(where: { $0.localizedCaseInsensitiveContains(word) }) {
                print(String(format: "WAIT ok %@: read '%@' on screen after %.2f s", what, word, Date().timeIntervalSince(start)))
                return
            }
            if Date().timeIntervalSince(start) >= timeout {
                save("text-wait-failed")
                print("WAIT failed \(what): recognized \(strings)")
                XCTFail("\(what): '\(word)' not on screen after \(Int(timeout)) s")
                throw NSError(domain: "SmokeQA.TextWait", code: 1)
            }
            Thread.sleep(forTimeInterval: 0.5)
        }
    }
    // The settings sheet shows PageId::id() as is ("appearance"), and tapText matches
    // case-sensitively. test_p15_check.py ties this value to settings_pages/mod.rs.
    let appearancePage = "appearance"
    // Last body row of the private wire peer surface (mock_peer.rs).
    let terminalInputRow = "Type here"

    /// Number of recognized text lines where any candidate contains `label`.
    /// Pure so the 0 and 2 cases do not need a Simulator.
    static func ocrLineCount(_ lines: [[String]], label: String) -> Int {
        lines.filter { candidates in candidates.contains { $0.contains(label) } }.count
    }

    /// First occurrence of `label` in `text` that is a whole word: the characters on both
    /// sides are not letters or digits, and the next word is not `excludingNextWord`
    /// (compared case-insensitively). "Paste" is therefore not found in "Paste image" or
    /// "Pasted", but is found in "Paste Paste image" (the first word) and in "Copy Paste".
    /// Pure so the cases do not need a Simulator; mirrored in test_smoke_qa_probe.py.
    static func wholeWordRange(of label: String, in text: String, excludingNextWord next: String? = nil) -> Range<String.Index>? {
        var from = text.startIndex
        while from < text.endIndex, let range = text.range(of: label, range: from..<text.endIndex) {
            from = text.index(after: range.lowerBound)
            if range.lowerBound > text.startIndex {
                let before = text[text.index(before: range.lowerBound)]
                if before.isLetter || before.isNumber { continue }
            }
            if range.upperBound < text.endIndex {
                let after = text[range.upperBound]
                if after.isLetter || after.isNumber { continue }
            }
            if let next = next {
                let following = text[range.upperBound...].drop { $0.isWhitespace }.prefix { $0.isLetter || $0.isNumber }
                if following.lowercased() == next.lowercased() { continue }
            }
            return range
        }
        return nil
    }

    /// Non-asserting OCR lookup. `matching` counts the recognized lines of the
    /// whole-screenshot pass that carry `label`, so a caller can reject an ambiguous tap.
    /// With `bands`, a miss on the whole screenshot is followed by overlapping horizontal
    /// bands: a busy frame can make Vision drop a whole row of small controls that the
    /// screenshot shows (P11 adds a Settings button above the SH1-154 strip).
    /// `attachAs` keeps the very screenshot the match came from, for a measurement made on it.
    private func locateText(_ label: String, excludingNextWord: String? = nil, bands: Bool = false, attachAs: String? = nil) throws -> (box: CGRect, matching: Int)? {
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
                    guard let range = SmokeQA.wholeWordRange(of: label, in: candidate.string, excludingNextWord: excludingNextWord),
                          let observation = try candidate.boundingBox(for: range) else { continue }
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
    func textVisible(_ label: String) -> Bool {
        (try? locateText(label)) != nil
    }

    func scrollBoard(in app: XCUIApplication) throws {
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
    func tapText(_ label: String, in app: XCUIApplication, tap: Bool = true, attempts: Int = 3, excludingNextWord: String? = nil, dx: CGFloat? = nil) throws -> (box: CGRect, matching: Int) {
        // GPUI controls have no UIKit accessibility nodes. Match source labels,
        // allowing Vision alternatives and a bounded wait for the next paint.
        // The whole screenshot is tried on every attempt, as before; the horizontal
        // bands are only the last resort, so a control the whole-screenshot pass finds
        // is never replaced by a band match.
        for attempt in 0..<attempts {
            if let found = try locateText(label, excludingNextWord: excludingNextWord, bands: attempt == attempts - 1) {
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

    /// Taps a box recorded earlier by `tapText(tap: false)`. No OCR runs here.
    func tapRecorded(_ box: CGRect, _ label: String, in app: XCUIApplication) {
        print("recorded tap \(label) box=\(box)")
        app.coordinate(withNormalizedOffset: CGVector(dx: box.midX, dy: 1 - box.midY)).tap()
    }

    /// P14 toast: one pass that screenshots (kept as `attachAs`), recognizes and taps, and
    /// returns the matched box in normalized screenshot coordinates for the keyboard-clearance check.
    func tapTextBox(_ label: String, in app: XCUIApplication, attachAs: String) throws -> CGRect {
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

    func save(_ name: String) {
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
