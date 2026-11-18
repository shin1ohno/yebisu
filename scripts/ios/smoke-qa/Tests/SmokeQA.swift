import XCTest
import Vision

final class SmokeQA: XCTestCase {
    // P9 takes about 72 s on the Mac mini. On a hosted runner it took 96-121 s with the
    // Herdr 2 band, and 209.8 s in run 44276079121 once L8 moved the band into the ...
    // menu: about 30 s more for five menu trips (cut by tapMenuItem) and one 60.09 s wait
    // for an animation (CASDFElementLayer) that did not end, the C-anim signature below.
    // The allowance keeps room for the body (about 130 s hosted) plus one such 60 s wait,
    // as the 3 minutes did before L8 (96 + 60 s), while a hang (every query waiting 60 s)
    // still fails the test here instead of running into the 40-minute job timeout.
    // p9-check.py passes -test-timeouts-enabled YES; XCTest rounds the allowance up to
    // whole minutes (210 s is 4 minutes).
    override func setUpWithError() throws {
        if name.contains("testP9Core") {
            executionTimeAllowance = 210
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
    // SH1-157 L1: the sheet lists sections; the appearance page is the Theme
    // section (settings_pages/sections.rs). "Terminal theme" does not match
    // case-sensitively.
    let appearancePage = "Theme"
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
    /// `minX` skips matches left of that normalized x: a control in a column (the ... menu,
    /// anchored at the right edge) below shell output carrying the same word.
    private func locateText(_ label: String, excludingNextWord: String? = nil, bands: Bool = false, attachAs: String? = nil, minX: CGFloat? = nil) throws -> (box: CGRect, matching: Int)? {
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
                    if let minX = minX, box.minX < minX { continue }
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
            $0.topCandidates(1).first?.string.contains("HOSTS") == true
        })
        // board.rs (BOARD_HEIGHT) defines a 180 pt board immediately below the HOSTS header.
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
    func tapText(_ label: String, in app: XCUIApplication, tap: Bool = true, attempts: Int = 3, excludingNextWord: String? = nil, dx: CGFloat? = nil, minX: CGFloat? = nil) throws -> (box: CGRect, matching: Int) {
        // GPUI controls have no UIKit accessibility nodes. Match source labels,
        // allowing Vision alternatives and a bounded wait for the next paint.
        // The whole screenshot is tried on every attempt, as before; the horizontal
        // bands are only the last resort, so a control the whole-screenshot pass finds
        // is never replaced by a band match.
        for attempt in 0..<attempts {
            if let found = try locateText(label, excludingNextWord: excludingNextWord, bands: attempt == attempts - 1, minX: minX) {
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

    /// L2b retired the toolbar and the handoff band (shell/entry_move.rs). Settings is
    /// the sidebar footer's "Settings" (shell/footer.rs); the caller then taps the page
    /// title in the sheet's left column (settings_pages/sections.rs) and its control.
    /// The sheet is open when its "Close" (settings_sheet/view.rs) is on screen. A press the
    /// footer did not take is repeated once, with the reason printed (Mac mini 76c3332: the
    /// Settings press right after the keyboard was hidden opened nothing, then "Input" was
    /// missing).
    func openSettings(in app: XCUIApplication) throws {
        for press in 0..<2 {
            try tapText("Settings", in: app)
            for _ in 0..<4 {
                sleep(1)
                if textVisible("Close") { return }
            }
            print("Settings sheet not open 4 s after press \(press): the press was not taken")
        }
        save("settings-not-open")
        XCTFail("The Settings sheet did not open after two presses")
        throw NSError(domain: "SmokeQA.MissingControl", code: 4)
    }

    /// Opens the tab row's ... menu (shell/tab_row.rs `shell-more`) and returns the point
    /// that opened it. The button is one "…" glyph that Vision does not read reliably, so
    /// the test taps its place: the last 44 pt pinned button (PINNED_ACTION_WIDTH) at the
    /// right end of the 36 pt tab row (TAB_ROW_HEIGHT), under the safe-area top that the
    /// frame keeps (24 pt on the iPad Simulator; the other tops are fallbacks). The menu is
    /// open when its "Select text" item (shell/overflow_menu.rs) is on screen. Tapping the
    /// returned point again closes it: the open menu's backdrop takes that press.
    @discardableResult
    func openOverflowMenu(in app: XCUIApplication, at known: CGVector? = nil) throws -> CGVector {
        let width = app.frame.width
        let points = known.map { [$0] } ?? [24, 32, 20, 0].map { CGVector(dx: width - 22, dy: $0 + 18) }
        for point in points {
            print("overflow menu tap \(point)")
            app.coordinate(withNormalizedOffset: .zero).withOffset(point).tap()
            for _ in 0..<3 {
                sleep(1)
                if textVisible("Select text") { return point }
            }
        }
        save("missing-overflow-menu")
        XCTFail("The ... menu did not open (no \"Select text\" on screen)")
        throw NSError(domain: "SmokeQA.MissingControl", code: 2)
    }

    /// Closes the open ... menu by pressing its backdrop at `point` (the place that opened it)
    /// and fails unless the menu is gone ("Select text" off the screen). A press that the
    /// menu did not take is repeated once, with the reason printed: in run 44297932478 (job
    /// 134779801456) the close press left the menu open (screen recording at 47.8 s and
    /// 48.4 s), and the next tap, meant for QA Image only, only closed it.
    func closeOverflowMenu(in app: XCUIApplication, at point: CGVector) {
        for press in 0..<2 {
            print("overflow menu close \(point) press \(press)")
            app.coordinate(withNormalizedOffset: .zero).withOffset(point).tap()
            for _ in 0..<3 {
                sleep(1)
                if !textVisible("Select text") { return }
            }
            print("overflow menu still open 3 s after close press \(press): the press was not taken")
        }
        save("overflow-menu-still-open")
        XCTFail("The ... menu stayed open after two close presses")
    }

    /// Opens the ... menu at `point` (from `openOverflowMenu`) and taps `label` in the menu's
    /// column (`minX`, see `locateText`). One OCR pass per try both shows the menu is open (its
    /// item is in the column) and finds the item, where `openOverflowMenu` + `tapText` take two:
    /// on a hosted runner each pass costs about 5 s (run 44276079121, testP9Core).
    @discardableResult
    func tapMenuItem(_ label: String, in app: XCUIApplication, at point: CGVector, minX: CGFloat) throws -> CGRect {
        print("overflow menu tap \(point)")
        app.coordinate(withNormalizedOffset: .zero).withOffset(point).tap()
        for attempt in 0..<3 {
            sleep(1)
            if let found = try locateText(label, bands: attempt == 2, minX: minX) {
                print("OCR tap \(label) frame=\(app.frame) box=\(found.box)")
                app.coordinate(withNormalizedOffset: CGVector(dx: found.box.midX, dy: 1 - found.box.midY)).tap()
                return found.box
            }
            print("OCR attempt \(attempt) for \(label)")
        }
        save("missing-control")
        XCTFail("Missing source-defined control in the ... menu: \(label)")
        throw NSError(domain: "SmokeQA.MissingControl", code: 3)
    }

    /// Hides the software keyboard if it comes up within `wait` seconds, with the keyboard's own
    /// hide key, and fails unless it is gone. The terminal takes focus when it connects, and on
    /// a Simulator whose software keyboard is on the keyboard then raises the QA strip and the
    /// status bar, so a test that reads their places must not inherit it (testSH154CoreImage
    /// after testP9Core in one xcodebuild: QA Image and QA Image only read on two rows while
    /// the keyboard rose, Mac mini 32f1e21).
    func hideSoftwareKeyboard(in app: XCUIApplication, _ what: String, wait: TimeInterval = 3) {
        // The keyboard element exists off screen too (frame below the window, height 0), so
        // a hittable "Hide keyboard" key (the iPad keyboard's last key) is what shows it is up.
        let hide = app.keyboards.buttons.matching(NSPredicate(format: "label CONTAINS[c] 'Hide keyboard'")).firstMatch
        let up = { hide.exists && hide.isHittable }
        let deadline = Date().addingTimeInterval(wait)
        while !up() && Date() < deadline {
            usleep(250_000)
        }
        guard up() else {
            print("\(what): no software keyboard")
            return
        }
        // A press the keyboard did not take is repeated once, with the reason printed (Mac
        // mini f95123d: the keyboard stayed up 5 s after the first press).
        for press in 0..<2 {
            hide.tap()
            let gone = Date().addingTimeInterval(5)
            while up() && Date() < gone {
                usleep(250_000)
            }
            if !up() { break }
            print("\(what): software keyboard still up 5 s after hide press \(press): the press was not taken")
        }
        guard !up() else {
            XCTFail("\(what): software keyboard still up after two hide presses")
            return
        }
        sleep(1)
        print("\(what): software keyboard hidden")
    }

    func save(_ name: String) {
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
