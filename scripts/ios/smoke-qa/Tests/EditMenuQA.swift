import XCTest
import Vision

// Every wait below ends on the receipt the previous action produces (the
// app's sh1-152-*.jsonl records, the peer's p4-peer.jsonl or the core wire),
// not on a fixed sleep. Each OCR pass takes about 6.7 s on the hosted runner
// (job 133186893192), so a label seen by one pass is reused until the layout
// may have changed, instead of reading the screen again before every tap.
final class EditMenuQA: XCTestCase {
    // The case the app was launched with; its receipts are filtered by it.
    private var caseName = ""
    // Vision boxes (screen-normalized) of the labels the last OCR passes saw.
    // Cleared whenever an action can move them.
    private var located: [String: CGRect] = [:]
    // The software keyboard frame (points) the boxes in `located` were seen with.
    // Since L2b the 152 controls sit in the QA strip above the status bar, which the
    // frame keeps clear of the keyboard, so they move when it shows or hides
    // (Integration 2 run 44214913068: 152 Begin at y 0.09 and 0.37, a recorded
    // 152 End tap landed on the keyboard).
    private var locatedKeyboard = CGRect(x: -1, y: -1, width: 0, height: 0)
    // Set while an edit menu may be open, so a remembered point it may cover
    // is read from the screen again, as every tap was before.
    private var menuOpen = false
    // The app's Documents directory on the host; sh1-152-check.py passes it as
    // TEST_RUNNER_SH1_152_DOCUMENTS, and a Simulator test runner can read it.
    private let documents = ProcessInfo.processInfo.environment["SH1_152_DOCUMENTS"].map { URL(fileURLWithPath: $0, isDirectory: true) }

    private static let labels = [
        "152 Begin", "152 Begun", "152 Select", "152 Mutate", "152 Replay", "152 Fresh", "152 End", "152 Ordinary",
        "152 Restore", "152 Second", "152 Two Begin", "152 Two Begun", "152 Two Select", "152 Two End", "152 Two Close",
        "private wire peer", "P9 Herdr", "AAA-152", "BBB-152",
    ]

    func testSH1152Basic() throws {
        for name in ["E1-text", "E1-key", "E2-text", "E2-key", "E1-core", "E2-core"] {
            let app = try launch(name)
            try begin("152", app)
            if name.hasPrefix("E2") { try select("152 Select", app) }
            try menu(app, name, selected: name.hasPrefix("E2"))
            let action = name.hasPrefix("E2") ? "Copy" : "Paste"
            XCTAssertTrue(app.menuItems[action].isEnabled)
            choose(action, app)
            try end("152 End", app)
            save(name)
            app.terminate()
        }
    }

    func testSH1152Transitions() throws {
        for name in ["E3a", "E3b", "E3c", "E3d", "E3e", "E3f-text", "E3f-key", "E3g-text", "E3g-key", "E3h", "E3i", "E3a-core"] {
            let app = try launch(name)
            try begin("152", app)
            let selected = name == "E3c" || name.hasPrefix("E3f")
            if selected { try select("152 Select", app) }
            try menu(app, name, selected: selected)
            if name == "E3d" {
                leaveAndReturn(app, "E3d")
            } else if name == "E3e" {
                // A drag from the top centre does not open a system overlay on
                // the hosted iPadOS 26 Simulator: it grabs the full-screen app
                // as a window and moves it off-screen, with no lifecycle change
                // (main run 43668219161 screen recording). The window state then
                // outlives the app and shifts every later launch. Pull Control
                // Center from the top-right status corner, which the system
                // owns, in screen space.
                let inactive = lifecycle("Inactive"), actives = lifecycle("Active")
                frame("E3e before overlay", app)
                screenPoint(app, 0.97, 0.002).press(forDuration: 0.1, thenDragTo: screenPoint(app, 0.97, 0.5))
                // Control Center dims the app but leaves its labels readable,
                // so a visible "152 Replay" does not mean the overlay closed
                // (PR run 43675932357: the Replay tap only dismissed it). Its
                // header shows "Carrier", which the app's status bar never does.
                let opened = waitText("Carrier", present: true, seconds: 8, app, "E3e overlay open")
                save("E3e-overlay")
                if opened {
                    // Tap the empty left side; Control Center's modules sit on the right.
                    screenPoint(app, 0.2, 0.5).tap()
                    _ = waitText("Carrier", present: false, seconds: 8, app, "E3e overlay closed")
                }
                if lifecycle("Inactive") > inactive {
                    waitFor("E3e Active after overlay") { self.lifecycle("Active") > actives }
                }
                located.removeAll()
                // An app that is no longer full screen means a gesture moved
                // its window; the receipt names it instead of a missing label.
                frame("E3e after overlay", app)
                // If the overlay stays open, leaving through Home adds a
                // Background phase, so the oracle reports E3e unverified
                // (device item 10), never a false pass.
                if (try? coordinate("152 Replay", app, required: false)) == nil {
                    print("SH1-152 E3e overlay stayed open; leaving through Home")
                    leaveAndReturn(app, "E3e")
                }
            } else {
                if name == "E3h" {
                    print("SH1-152 HIDE KEYBOARD QUERY\n\(app.debugDescription)")
                    let hide = app.buttons.matching(NSPredicate(format: "label CONTAINS[c] 'Hide keyboard'")).firstMatch
                    if hide.exists && hide.isHittable {
                        let terminal = records("terminal").last
                        hide.tap()
                        located.removeAll()
                        waitFor("E3h keyboard hidden") { self.field(self.records("terminal").last, "rows") != self.field(terminal, "rows") }
                    } else {
                        print("SH1-152 E3h keyboard hide key unavailable; independent switch inputs still recorded")
                    }
                }
                try mutate(name, app)
                // The keyboard change Restore makes is read from the screen
                // again before Replay is tapped (PR run 43633426435).
                if name.hasPrefix("E3f") {
                    try tap("152 Restore", app)
                    located.removeAll()
                }
            }
            try replay("152 Replay", app)
            try end("152 End", app)
            save("stale-\(name)")
            if name == "E3i" {
                try fresh(app)
                // Ask the real delegate for an unrelated configuration, then replay its ordinary commands.
                try menu(app, name, selected: false)
                app.coordinate(withNormalizedOffset: CGVector(dx: 0.98, dy: 0.5)).tap()
                dismissed(app, name)
                try replay("152 Ordinary", app)
                try end("152 End", app)
            }
            try fresh(app)
            try menu(app, name, selected: false)
            choose("Paste", app)
            try end("152 End", app)
            app.terminate()
        }
    }

    func testSH1152DismissAndSystemEdit() throws {
        for name in ["E4-text", "E4-key", "E7"] {
            let app = try launch(name)
            try begin("152", app)
            try menu(app, name, selected: false)
            app.coordinate(withNormalizedOffset: CGVector(dx: 0.98, dy: 0.5)).tap()
            dismissed(app, name)
            if name == "E7" {
                let edit = app.descendants(matching: .any).matching(NSPredicate(format: "label == 'Edit'")).firstMatch
                if edit.exists && edit.isHittable {
                    edit.tap()
                    XCTAssertTrue(app.menuItems["Paste"].waitForExistence(timeout: 4))
                    app.menuItems["Paste"].tap()
                    print("SH1-152 E7 system Edit Paste selected")
                    pasted()
                } else { print("SH1-152 E7 unverified: system Edit not exposed; device item 9") }
            } else {
                let inputs = peerInputs()
                app.typeText("x")
                waitFor("\(name) typed input at peer") { self.peerInputs() > inputs }
            }
            try end("152 End", app)
            app.terminate()
        }
    }

    // E5: two real UIKit scenes. Scene A shows "AAA-152" on w1:p1; scene B
    // (opened by "152 Second", controls "152 Two ...") shows "BBB-152" on w1:p3.
    func testSH1152TwoScenes() throws {
        for name in ["E5a", "E5b", "E5c", "E5d"] {
            let app = try launch(name)
            let selectFirst = name == "E5b" || name == "E5c"
            try begin("152", app)
            if selectFirst { try select("152 Select", app) }
            try menu(app, name, selected: selectFirst, target: "AAA-152")
            if name == "E5a" { choose("Paste", app) }
            else if name == "E5b" { choose("Copy", app) }
            else {
                app.coordinate(withNormalizedOffset: CGVector(dx: 0.98, dy: 0.5)).tap()
                dismissed(app, name)
            }
            try end("152 End", app)
            save("\(name)-first")
            try second(app)
            save("\(name)-second-open")
            if name == "E5d" { try closeOther(app) }
            try begin("152 Two", app)
            if name == "E5b" { try select("152 Two Select", app) }
            try menu(app, name, selected: name == "E5b", target: "BBB-152")
            choose(name == "E5b" ? "Copy" : "Paste", app)
            try end("152 Two End", app)
            save(name)
            // Leave one scene session, as every other case expects at launch.
            if name != "E5d" {
                try closeOther(app)
                // UIKit destroys the closed scene's session outside the app,
                // which records nothing more after close-other; the next
                // launch must not restore it.
                sleep(3)
            }
            app.terminate()
        }
    }

    private func launch(_ name: String) throws -> XCUIApplication {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.terminate()
        caseName = name
        located.removeAll()
        menuOpen = false
        app.launchEnvironment["YEBISU_SH1_152_CASE"] = name
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment[name.contains("core") ? "YEBISU_CORE_QA" : "YEBISU_TERMINAL_QA"] = "1"
        XCUIDevice.shared.orientation = .portrait
        // Observation only (E-launch in docs/ci-simulator.md): no retry, no wait or assertion changes.
        let launchStarted = Date()
        app.launch()
        print("SH1-152 LAUNCH \(name) \(String(format: "%.1f", Date().timeIntervalSince(launchStarted)))")
        // A Begin marker needs an input destination, which the app records
        // with the first terminal frame that names its pane.
        let ready = waitFor("launch \(name) input destination", seconds: 30) {
            self.records("terminal").contains { $0["input_pane"] is String }
        }
        if !ready && records("launch").isEmpty {
            XCTFail("SH1-152 receipts unreadable: \(documents?.path ?? "SH1_152_DOCUMENTS is not set")")
            throw NSError(domain: "EditMenuQA.Receipts", code: 1)
        }
        frame("launch \(name)", app)
        return app
    }

    // OCR reads screen positions, but app.coordinate is relative to the app's
    // own frame. In main run 43668219161 the app launched as a smaller window,
    // so taps on "152 Begin" landed on "New Tab" (three tab.create receipts in
    // E5a). Convert a screen-normalized point into a tap at that screen point.
    private func screenPoint(_ app: XCUIApplication, _ x: CGFloat, _ y: CGFloat) -> XCUICoordinate {
        let screen = screenSize()
        let origin = app.frame.origin
        return app.coordinate(withNormalizedOffset: .zero)
            .withOffset(CGVector(dx: screen.width * x - origin.x, dy: screen.height * y - origin.y))
    }

    // Records whether the app still fills the screen, so a moved window shows
    // up in the log at the step that moved it.
    private func frame(_ step: String, _ app: XCUIApplication) {
        let screen = screenSize()
        let frame = app.frame
        let full = abs(frame.minX) < 1 && abs(frame.minY) < 1 && abs(frame.width - screen.width) < 1 && abs(frame.height - screen.height) < 1
        print("SH1-152 FRAME \(step) app=\(frame) screen=\(screen) fullscreen=\(full)")
    }

    // Reads the screen until `label` is (or is no longer) shown, for at most
    // `seconds`; each read is one OCR pass, so no fixed sleep is added.
    // Prints the outcome and elapsed time; the OCR lines record what was seen.
    private func waitText(_ label: String, present: Bool, seconds: TimeInterval, _ app: XCUIApplication, _ step: String) -> Bool {
        let start = Date()
        repeat {
            let shown = (try? coordinate(label, app, required: false, attempts: 1)) != nil
            if shown == present {
                print("SH1-152 WAIT \(step): '\(label)' present=\(present) after \(String(format: "%.1f", Date().timeIntervalSince(start)))s")
                return true
            }
        } while Date().timeIntervalSince(start) < seconds
        print("SH1-152 WAIT \(step): '\(label)' present=\(present) not reached in \(Int(seconds))s")
        return false
    }

    // Polls `condition` every 0.1 s for at most `seconds`. A condition that is
    // not reached is logged, not failed: the receiving-system oracle in
    // sh1-152-check.py decides the case from the same receipts.
    @discardableResult
    private func waitFor(_ step: String, seconds: TimeInterval = 10, _ condition: () -> Bool) -> Bool {
        let start = Date()
        repeat {
            if condition() {
                print("SH1-152 WAIT \(step): after \(String(format: "%.1f", Date().timeIntervalSince(start)))s")
                return true
            }
            Thread.sleep(forTimeInterval: 0.1)
        } while Date().timeIntervalSince(start) < seconds
        print("SH1-152 WAIT \(step): not reached in \(Int(seconds))s")
        return false
    }

    // Screen size in points: SpringBoard always spans the whole screen.
    private func screenSize() -> CGSize {
        XCUIApplication(bundleIdentifier: "com.apple.springboard").frame.size
    }

    // Owner rule (iOS convention): only usable items are shown. Without a
    // selection the menu is Paste alone; with a selection it is Copy and Paste.
    private func menu(_ app: XCUIApplication, _ name: String, selected: Bool, target: String? = nil) throws {
        let captures = records("sh1-152-capture").count
        // The Herdr 2 mock banner is "yebisu P9 Herdr 2" (core_mock.rs). After L8 retired the
        // band, Vision split it after "yebisu" in 2 of 3 Mac mini E1-core runs and read
        // "P9 Herdr 2" whole every time, so the spot is found by "P9 Herdr".
        let label = target ?? (name.contains("core") ? "P9 Herdr" : "private wire peer")
        // Only the E1 cases, which check what the menu presents, raise the keyboard first: a
        // tap would clear an E2 selection, and it moves the responder the E3 to E5 cases test
        // (Mac mini 6f2d709: E3g-key "successful internal switch revoked or did not occur").
        if name.hasPrefix("E1") { try keyboardUpBeforePress(app, name, label) }
        let spot = try point(label, app)
        spot.press(forDuration: 0.8)
        XCTAssertTrue(app.menuItems["Paste"].waitForExistence(timeout: 4), "real menu missing: \(name)\n\(app.debugDescription)")
        menuOpen = true
        let labels = app.menuItems.allElementsBoundByIndex.map { $0.label }
        print("SH1-152 VISIBLE \(name) selected=\(selected) labels=\(labels.joined(separator: ","))")
        XCTAssertEqual(labels.sorted(), selected ? ["Copy", "Paste"] : ["Paste"])
        if !selected { XCTAssertFalse(app.menuItems["Copy"].exists, "Copy shown without a selection: \(name)") }
        save("menu-\(name)")
        // The app records the presented commands 150 ms after UIKit builds
        // the menu; a command chosen before that would precede its capture.
        waitFor("\(name) menu capture recorded", seconds: 5) { self.records("sh1-152-capture").count > captures }
    }

    // The long press focuses the terminal. If the software keyboard was down, it rises and
    // UIKit dismisses the menu that same press opened: run 44297932478 (edit-menu job
    // 134779801414), E1-core, the screen recording shows Paste at 205 s and the keyboard up
    // with no menu at 206 s, while the app recorded one sh1-152-capture; E1-text failed the
    // same way on a fresh Mac mini Simulator (f95123d, no keyboard at the press). A second press
    // would be a second presentation, so the keyboard is raised first with a tap and the
    // spot is read again once it has settled. A Simulator whose keyboard stays down (a
    // hardware keyboard) is left as it is.
    private func keyboardUpBeforePress(_ app: XCUIApplication, _ name: String, _ label: String) throws {
        let hide = app.keyboards.buttons.matching(NSPredicate(format: "label CONTAINS[c] 'Hide keyboard'")).firstMatch
        if hide.exists && hide.isHittable { return }
        print("SH1-152 \(name): software keyboard down before the press; tapping the terminal to raise it")
        try point(label, app).tap()
        if waitFor("\(name) software keyboard up before the press", seconds: 5, { hide.exists && hide.isHittable }) {
            sleep(1)
            located.removeAll()
        }
    }

    // Selects a menu item and waits for its execution receipt; an executed
    // Paste is also waited for at the peer.
    private func choose(_ item: String, _ app: XCUIApplication) {
        let executions = records("edit-menu-exec").count, pastes = executedPastes()
        app.menuItems[item].tap()
        menuOpen = false
        waitFor("\(caseName) \(item) execution recorded") { self.records("edit-menu-exec").count > executions }
        if executedPastes() > pastes { pasted() }
    }

    private func dismissed(_ app: XCUIApplication, _ name: String) {
        waitFor("\(name) menu dismissed", seconds: 5) { !app.menuItems["Paste"].exists }
        menuOpen = false
    }

    // A Begin tap can be lost: in PR run 43659150803, E3d had no start
    // marker and kept the previous case's clipboard. The app relabels the
    // control "Begun" once Begin runs and ignores a repeat, so retry until
    // its start marker is recorded.
    private func begin(_ prefix: String, _ app: XCUIApplication) throws {
        let before = markers("start").count
        for attempt in 0..<3 {
            if attempt > 0 {
                print("SH1-152 \(prefix) Begin not acknowledged; retry \(attempt)")
                located.removeAll()
            }
            if let control = try? point("\(prefix) Begin", app, required: false) { control.tap() }
            if marked("start", after: before, "\(prefix) Begin") {
                // "Begun" is wider than "Begin" and Begin moves the keyboard,
                // so the next tap reads the screen again.
                located.removeAll()
                return
            }
        }
        save("missing-\(prefix) Begun")
        XCTFail("\(prefix) Begin was not acknowledged")
        throw NSError(domain: "EditMenuQA.MissingControl", code: 2)
    }

    private func fresh(_ app: XCUIApplication) throws {
        let before = markers("start").count
        try tap("152 Fresh", app)
        marked("start", after: before, "152 Fresh")
    }

    private func end(_ label: String, _ app: XCUIApplication) throws {
        let before = markers("end").count
        try tap(label, app)
        marked("end", after: before, label)
    }

    private func select(_ label: String, _ app: XCUIApplication) throws {
        let before = records("sh1-152-selection").count
        try tap(label, app)
        waitFor("\(label) selection recorded") { self.records("sh1-152-selection").count > before }
    }

    private func replay(_ label: String, _ app: XCUIApplication) throws {
        let replays = records("sh1-152-replay").count, pastes = executedPastes()
        try tap(label, app)
        waitFor("\(label) replay recorded") { self.records("sh1-152-replay").count > replays }
        // A replayed Paste that UIKit executes, and the ordinary suggestions
        // E3i replays without a menu origin, both paste the current clipboard.
        if label.hasSuffix("Ordinary") || executedPastes() > pastes { pasted() }
    }

    // Each Mutate ends on the receipt of the state it changes.
    private func mutate(_ name: String, _ app: XCUIApplication) throws {
        let terminal = records("terminal").last
        let revoked = records("edit-menu-revoked").count, external = records("sh1-152-external").count
        let switched = records("sh1-152-switch").count, moved = records("sh1-152-destination-change").count
        try tap("152 Mutate", app)
        located.removeAll()
        switch name {
        case "E3a", "E3i", "E3a-core":
            waitFor("\(name) Mutate destination moved") {
                self.records("sh1-152-destination-change").count > moved
                    && self.field(self.records("terminal").last, "input_pane") != self.field(terminal, "input_pane")
            }
        case "E3b":
            waitFor("\(name) Mutate new link generation") {
                self.field(self.records("terminal").last, "generation") != self.field(terminal, "generation")
            }
        case "E3c":
            waitFor("\(name) Mutate menu revoked") { self.records("edit-menu-revoked").count > revoked }
        case "E3f-text", "E3f-key":
            waitFor("\(name) Mutate external responder") { self.records("sh1-152-external").count > external }
        case "E3g-text", "E3g-key":
            waitFor("\(name) Mutate keyboard resized the terminal") {
                self.field(self.records("terminal").last, "rows") != self.field(terminal, "rows")
            }
        case "E3h":
            waitFor("\(name) Mutate switch observed") { self.records("sh1-152-switch").count > switched }
        default:
            break
        }
    }

    // Home, then back: the app records Background, then Active, reconnects
    // and draws a terminal frame that names its input pane again.
    private func leaveAndReturn(_ app: XCUIApplication, _ step: String) {
        let backgrounds = lifecycle("Background"), actives = lifecycle("Active")
        XCUIDevice.shared.press(.home)
        waitFor("\(step) Background recorded") { self.lifecycle("Background") > backgrounds }
        app.activate()
        waitFor("\(step) Active with an input destination") {
            let active = self.records("lifecycle").filter { $0["phase"] as? String == "Active" }.dropFirst(actives).first
            guard let time = active?["time"] as? Double else { return false }
            return self.records("terminal").contains { ($0["time"] as? Double ?? 0) > time && $0["input_pane"] is String }
        }
        located.removeAll()
    }

    // Opens scene B; it is ready once it has moved to w1:p3 (the pane
    // sh1_152_scenes.py expects) and is Active.
    private func second(_ app: XCUIApplication) throws {
        let readies = records("sh1-152-second-ready").count
        try tap("152 Second", app)
        located.removeAll()
        waitFor("second scene on w1:p3 and Active", seconds: 15) {
            // Scene B's window records only its own frames and phases.
            guard let ready = self.records("sh1-152-second-ready").dropFirst(readies).first,
                  let window = ready["window"] as? Int else { return false }
            let moved = self.records("terminal").contains { $0["window"] as? Int == window && $0["input_pane"] as? String == "w1:p3" }
            let active = self.records("lifecycle").contains { $0["window"] as? Int == window && $0["phase"] as? String == "Active" }
            return moved && active
        }
    }

    private func closeOther(_ app: XCUIApplication) throws {
        let closes = records("sh1-152-close-other").count
        try tap("152 Two Close", app)
        waitFor("first scene close recorded") { self.records("sh1-152-close-other").count > closes }
        located.removeAll()
    }

    private func tap(_ label: String, _ app: XCUIApplication) throws {
        try point(label, app).tap()
    }

    // A label seen by an earlier OCR pass is tapped where it was seen unless
    // an open edit menu now lies over that point; then the screen is read
    // again, which also finds a control whose start the menu covers.
    private func point(_ label: String, _ app: XCUIApplication, required: Bool = true) throws -> XCUICoordinate {
        if settledKeyboard(app) != locatedKeyboard {
            located.removeAll()
        }
        if let box = located[label], !covered(box, app) {
            print("SH1-152 TAP \(label) seen at \(box)")
            return screenPoint(app, box.midX, 1-box.midY)
        }
        return try coordinate(label, app, required: required)
    }

    // One snapshot: `exists` then `frame` are two queries, and a keyboard that leaves between
    // them fails the test ("No matches found ... Keyboard", run 44241460741 job 134590165203).
    // A keyboard that is not there, or leaves while it is read, is .zero.
    private func keyboardFrame(_ app: XCUIApplication) -> CGRect {
        guard let snapshot = try? app.keyboards.firstMatch.snapshot() else { return .zero }
        return snapshot.frame.integral
    }

    // The keyboard frame once it stops moving (at most 3 s): the 152 controls move with it,
    // and a box read while it slides in or out is stale by the time it is tapped
    // (Integration 2, E3d after the return from the background).
    private func settledKeyboard(_ app: XCUIApplication) -> CGRect {
        var frame = keyboardFrame(app)
        for _ in 0..<10 {
            usleep(300_000)
            let next = keyboardFrame(app)
            if next == frame { return frame }
            frame = next
        }
        return frame
    }

    private func covered(_ box: CGRect, _ app: XCUIApplication) -> Bool {
        guard menuOpen else { return false }
        let items = app.menuItems.allElementsBoundByIndex
        if items.isEmpty {
            menuOpen = false
            return false
        }
        let screen = screenSize()
        let spot = CGPoint(x: box.midX * screen.width, y: (1 - box.midY) * screen.height)
        // The menu's bubble extends past its items.
        return items.contains { $0.frame.insetBy(dx: -24, dy: -24).contains(spot) }
    }

    private func coordinate(_ label: String, _ app: XCUIApplication, required: Bool = true, attempts: Int = 4) throws -> XCUICoordinate {
        for attempt in 0..<attempts {
            let keyboard = settledKeyboard(app)
            if keyboard != locatedKeyboard {
                located.removeAll()
                locatedKeyboard = keyboard
            }
            let image = try XCTUnwrap(XCUIScreen.main.screenshot().image.cgImage)
            let request = VNRecognizeTextRequest()
            request.recognitionLevel = .accurate
            request.customWords = [label]
            try VNImageRequestHandler(cgImage: image).perform([request])
            let candidates = (request.results ?? []).sorted { $0.boundingBox.midY > $1.boundingBox.midY }.flatMap { $0.topCandidates(5) }
            remember(candidates)
            for candidate in candidates {
                guard let range = candidate.string.range(of: label), let observed = try candidate.boundingBox(for: range) else { continue }
                let box = observed.boundingBox
                print("SH1-152 OCR \(label) \(box)")
                return screenPoint(app, box.midX, 1-box.midY)
            }
            // An open edit menu can cover the start of a control: "52 Mutate"
            // beside Paste, "utate" beside Copy and Paste (runs 43564245188,
            // 43613569523). Accept a whole candidate that is a suffix of the
            // label, at least five characters long, and tap its visible part.
            if label.hasPrefix("152 ") {
                for candidate in candidates {
                    let text = candidate.string
                    guard text.count >= 5, text.count < label.count, label.hasSuffix(text),
                          let observed = try candidate.boundingBox(for: text.startIndex..<text.endIndex) else { continue }
                    let box = observed.boundingBox
                    print("SH1-152 OCR \(label) partially covered as '\(text)' \(box)")
                    return screenPoint(app, box.midX, 1-box.midY)
                }
            }
            print("SH1-152 OCR attempt \(attempt): \(candidates.map { $0.string })")
            if attempt + 1 < attempts { sleep(1) }
        }
        if required {
            save("missing-\(label)")
            XCTFail("Missing source-defined label \(label)")
        }
        throw NSError(domain: "EditMenuQA.MissingControl", code: 1)
    }

    // Keeps every whole label one OCR pass found, topmost first as above.
    private func remember(_ candidates: [VNRecognizedText]) {
        for label in Self.labels where located[label] == nil {
            for candidate in candidates {
                guard let range = candidate.string.range(of: label),
                      let observed = try? candidate.boundingBox(for: range) else { continue }
                located[label] = observed.boundingBox
                break
            }
        }
    }

    // This case's app receipts of one kind, in time order (both scenes).
    private func records(_ kind: String) -> [[String: Any]] {
        guard let directory = documents,
              let files = try? FileManager.default.contentsOfDirectory(atPath: directory.path) else { return [] }
        var rows: [[String: Any]] = []
        for file in files where file.hasPrefix("sh1-152-\(caseName)-") && file.hasSuffix(".jsonl") {
            guard let text = try? String(contentsOf: directory.appendingPathComponent(file), encoding: .utf8) else { continue }
            for line in text.split(separator: "\n") {
                guard let row = (try? JSONSerialization.jsonObject(with: Data(line.utf8))) as? [String: Any],
                      row["case"] as? String == caseName, row["kind"] as? String == kind else { continue }
                rows.append(row)
            }
        }
        return rows.sorted { ($0["time"] as? Double ?? 0) < ($1["time"] as? Double ?? 0) }
    }

    private func field(_ row: [String: Any]?, _ key: String) -> String {
        guard let field = row?[key] else { return "none" }
        return "\(field)"
    }

    private func lifecycle(_ phase: String) -> Int {
        records("lifecycle").filter { $0["phase"] as? String == phase }.count
    }

    private func markers(_ edge: String) -> [[String: Any]] {
        records("sh1-152-marker").filter { $0["edge"] as? String == edge }
    }

    private func executedPastes() -> Int {
        records("edit-menu-exec").filter { $0["action"] as? String == "Paste" && $0["result"] as? String == "executed" }.count
    }

    // The app records a marker when it sends it; the oracle reads it at the
    // peer (or the core wire), so wait for both.
    @discardableResult
    private func marked(_ edge: String, after before: Int, _ step: String) -> Bool {
        guard waitFor("\(step) \(edge) marker recorded", { self.markers(edge).count > before }) else { return false }
        let stage = markers(edge).last?["stage"] as? Int ?? 0
        let marker = "sh1-152-\(edge)-\(caseName)-\(stage)"
        waitFor("\(marker) at peer") { self.peerReceived(marker) }
        return true
    }

    // Waits until the current stage's clipboard text reaches the peer.
    private func pasted() {
        let stage = markers("start").last?["stage"] as? Int ?? 0
        let payload = "sh1-152-clip-\(caseName)-\(stage)"
        waitFor("\(payload) at peer") { self.peerReceived(payload) }
    }

    // The Herdr 1 mock peer logs each Paste as a JSON string; the core wire
    // logs bracketed-paste bytes as a JSON array.
    private func peerReceived(_ text: String) -> Bool {
        guard let directory = documents else { return false }
        let peer = (try? String(contentsOf: directory.appendingPathComponent("p4-peer.jsonl"), encoding: .utf8)) ?? ""
        if peer.contains("\"\(text)\"") { return true }
        let wire = (try? String(contentsOf: directory.appendingPathComponent("p9-core-wire.jsonl"), encoding: .utf8)) ?? ""
        let bytes = text.utf8.map { String($0) }.joined(separator: ",")
        return wire.contains("27,91,50,48,48,126,\(bytes),27,91,50,48,49,126")
    }

    private func peerInputs() -> Int {
        guard let directory = documents,
              let peer = try? String(contentsOf: directory.appendingPathComponent("p4-peer.jsonl"), encoding: .utf8) else { return 0 }
        return peer.components(separatedBy: "ClientShellPaneInput").count - 1
    }

    private func save(_ name: String) {
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = "sh1-152-\(name)"
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
