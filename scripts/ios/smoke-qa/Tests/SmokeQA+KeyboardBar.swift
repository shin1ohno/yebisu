import XCTest

// #229: the terminal keys on the software keyboard's own shortcuts bar. Docked, the keys are
// the bar's items (`key-bar-<name>`, UIKit's own buttons with herdr-uikit's identifiers) and
// the in-app band shows only the pane pager. Floating, UIKit draws no bar items, so the in-app
// band shows the same keys at the window's bottom (its ctrl key is marked qa.keyband.ctrl).
// Every step is confirmed by qa.state: `key_bar` (surface, placement, latch, item count) and
// `peer.keys` (the mock peer's last key presses, `Char('c')/2` for Control-C). A split keyboard
// cannot be made from XCUITest (herdr-uikit docs/keyboard-bar.md): an owner device check.
// keyboard-bar-check.py runs this test and keeps the screenshots. Simulator results do not
// prove real IME or hardware keyboard behaviour (AGENTS.md).
extension SmokeQA {
    private static let keyboardBarEnvironment = [
        "YEBISU_TERMINAL_QA": "1",
        "YEBISU_SMOKE_OBSERVE": "1",
        "YEBISU_QA_AX": "1",
        "YEBISU_QA_KEY_BAND_SOFTWARE_ONLY": "1",
    ]

    private func keyBar(_ state: QAState) -> [String: Any] {
        state.raw["key_bar"] as? [String: Any] ?? [:]
    }

    private func surface(_ state: QAState) -> String { keyBar(state)["surface"] as? String ?? "" }
    private func placement(_ state: QAState) -> String { keyBar(state)["placement"] as? String ?? "" }
    private func latch(_ state: QAState) -> [String] { keyBar(state)["latch"] as? [String] ?? [] }
    private func items(_ state: QAState) -> Int { keyBar(state)["items"] as? Int ?? -1 }

    private func peerKeys(_ state: QAState) -> [String] {
        (state.raw["peer"] as? [String: Any])?["keys"] as? [String] ?? []
    }

    private func report(_ qa: QAAx, _ step: String) {
        guard let state = qa.read() else {
            print("KBAR \(step) qa.state unreadable")
            return
        }
        let bar = (try? JSONSerialization.data(withJSONObject: keyBar(state), options: [.sortedKeys]))
            .flatMap { String(data: $0, encoding: .utf8) } ?? "{}"
        print("KBAR \(step) keyboard=\(state.keyboard) key_bar=\(bar) peer.keys=\(peerKeys(state).suffix(3))")
        save("kbar-\(step)")
    }

    /// UIKit's bar button (or menu entry) with herdr-uikit's identifier, or its label.
    private func barButton(_ app: XCUIApplication, _ id: String, label: String) -> XCUIElement {
        let byId = app.descendants(matching: .any).matching(identifier: id).firstMatch
        if byId.exists { return byId }
        return app.descendants(matching: .any).matching(NSPredicate(format: "label == %@", label)).firstMatch
    }

    private func tapBar(_ app: XCUIApplication, _ id: String, label: String, _ step: String) {
        let button = barButton(app, id, label: label)
        let found = button.waitForExistence(timeout: 5)
        print("KBAR \(step) \(id) exists=\(found) frame=\(found ? NSCoder.string(for: button.frame) : "-") hittable=\(found && button.isHittable)")
        XCTAssertTrue(found && button.isHittable, "\(step): \(id) is on the keyboard bar")
        if found { button.tap() }
    }

    private func softwareLetter(_ app: XCUIApplication, _ letter: String) {
        let keys = app.keyboards.keys
        let key = keys[letter].exists ? keys[letter] : keys[letter.uppercased()]
        print("KBAR software key \(letter): exists=\(key.exists)")
        key.tap()
    }

    /// The keyboard is docked again after a pinch-out (it stays floating across launches).
    private func dockKeyboard(_ app: XCUIApplication, _ qa: QAAx, _ step: String) {
        for attempt in 0..<4 {
            if let state = qa.read(), placement(state) == "docked" { return }
            print("KBAR \(step) dock attempt \(attempt)")
            app.keyboards.firstMatch.pinch(withScale: 4.0, velocity: 3.0)
            _ = try? qa.waitState("\(step): docked", timeout: 4) { self.placement($0) == "docked" }
        }
    }

    private func launchKeyboard(_ app: XCUIApplication, _ qa: QAAx, _ environment: [String: String],
                                _ step: String) throws {
        app.terminate()
        app.launchEnvironment = environment
        XCUIDevice.shared.orientation = .portrait
        app.launch()
        try waitTerminal(qa, "\(step): the wire-peer terminal is connected")
        try qa.press(QAIds.terminalInput, effect: 10, what: "\(step): the software keyboard is up") {
            $0.keyboardUp || self.placement($0) == "floating"
        }
    }

    func testKeyboardBarStates() throws {
        continueAfterFailure = true
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        let qa = QAAx(app)
        try launchKeyboard(app, qa, Self.keyboardBarEnvironment, "docked")
        dockKeyboard(app, qa, "docked")

        // Docked: the keys are on the keyboard's bar; the in-app band is gone.
        let docked = try qa.waitState("docked: keys on the keyboard bar", timeout: 10) {
            self.surface($0) == "bar" && self.placement($0) == "docked" && self.items($0) > 0
        }
        print("KBAR docked items=\(items(docked))")
        report(qa, "docked")
        tapBar(app, "key-bar-ctrl", label: "Control", "docked")
        try qa.waitState("docked: the bar's ctrl latched", timeout: 5) { self.latch($0) == ["ctrl"] }
        report(qa, "docked-latched")
        let before = peerKeys(try qa.waitState("docked: peer keys", timeout: 5) { _ in true }).count
        softwareLetter(app, "c")
        let sent = try qa.waitState("docked: ctrl then c sends Control-C", timeout: 10) {
            self.peerKeys($0).last == "Char('c')/2" && self.latch($0).isEmpty
        }
        print("KBAR docked sent \(peerKeys(sent).suffix(max(1, peerKeys(sent).count - before)))")
        report(qa, "docked-sent")

        // The … menu: Paste, the Herdr prefix, | ~ / - and Hide keyboard.
        let inputs = (try qa.waitState("menu: peer inputs", timeout: 5) { _ in true }).peerInputs
        tapBar(app, "key-bar-more", label: "More keys", "menu")
        let pipe = barButton(app, "key-bar-pipe", label: "Vertical bar")
        let opened = pipe.waitForExistence(timeout: 5)
        print("KBAR menu pipe exists=\(opened)")
        save("kbar-menu")
        XCTAssertTrue(opened, "menu: the … menu lists Vertical bar")
        if opened { pipe.tap() }
        try qa.waitState("menu: Vertical bar reaches the peer", timeout: 10) { $0.peerInputs > inputs }
        report(qa, "menu-sent")

        // Floating: UIKit draws no bar items; the in-app band has the same keys.
        app.keyboards.firstMatch.pinch(withScale: 0.4, velocity: -1.5)
        sleep(2)
        let pinched = app.keyboards.firstMatch
        print("KBAR pinched keyboards=\(app.keyboards.count) frame=\(pinched.exists ? NSCoder.string(for: pinched.frame) : "none")")
        report(qa, "pinched")
        try qa.waitState("floating: keys on the in-app band", timeout: 10) {
            self.surface($0) == "band" && self.placement($0) == "floating"
        }
        report(qa, "floating")
        let onBar = barButton(app, "key-bar-ctrl", label: "Control")
        print("KBAR floating key-bar-ctrl exists=\(onBar.exists) hittable=\(onBar.exists && onBar.isHittable)")
        try qa.press(QAIds.keyBandCtrl, effect: 5, what: "floating: the band's ctrl latched") {
            self.latch($0) == ["ctrl"]
        }
        report(qa, "floating-latched")
        softwareLetter(app, "c")
        try qa.waitState("floating: ctrl then c sends Control-C", timeout: 10) {
            self.peerKeys($0).last == "Char('c')/2" && self.latch($0).isEmpty
        }
        report(qa, "floating-sent")

        // Leave the Simulator's keyboard docked for the next test.
        try launchKeyboard(app, qa, Self.keyboardBarEnvironment, "redock")
        dockKeyboard(app, qa, "redock")
        report(qa, "redocked")
        XCTAssertEqual(app.state, .runningForeground)
    }

    /// Negative control: a hardware keyboard alone (CoreSimulator reports one unless the launch
    /// opts out with YEBISU_QA_KEY_BAND_SOFTWARE_ONLY) shows no keys on the bar or the band.
    func testKeyboardBarHardwareOnly() throws {
        continueAfterFailure = true
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        let qa = QAAx(app)
        var environment = Self.keyboardBarEnvironment
        environment.removeValue(forKey: "YEBISU_QA_KEY_BAND_SOFTWARE_ONLY")
        app.terminate()
        app.launchEnvironment = environment
        XCUIDevice.shared.orientation = .portrait
        app.launch()
        try waitTerminal(qa, "hardware: the wire-peer terminal is connected")
        _ = try? qa.press(QAIds.terminalInput, effect: 5, what: "hardware: the terminal takes input") { _ in true }
        let state = try qa.waitState("hardware: no keys anywhere", timeout: 10) {
            self.surface($0) == "none" && self.items($0) == 0
        }
        print("KBAR hardware items=\(items(state)) surface=\(surface(state))")
        report(qa, "hardware")
        let onBar = barButton(app, "key-bar-ctrl", label: "Control")
        print("KBAR hardware key-bar-ctrl exists=\(onBar.exists)")
        XCTAssertFalse(onBar.exists && onBar.isHittable, "hardware: no ctrl on the keyboard bar")
    }
}
