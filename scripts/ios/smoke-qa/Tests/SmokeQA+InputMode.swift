import XCTest

// #139: the system input mode indicator at the terminal cursor (T-IM-S1..S8). input-mode-check.py
// writes the Simulator's input sources (en_US, ja_JP-Romaji) and Shortcut style = Herdr prefix
// before this runs, then judges each case from the INPUT_MODE_OBS lines printed here (one JSON
// object per step with qa.state.input_mode); this test records, it does not judge. Controls are
// pressed by QA identifier and confirmed through qa.state. Keys are XCUITest hardware keys: Shift
// alone puts the Simulator in the hardware keyboard state, Control-Space switches the input
// source. The key band has no accessibility element, so its ctrl and Hide keys are pressed at
// their layout positions (shell/key_band/mod.rs: the 76 pt band on the keyboard, 8 pt row
// padding, 4 pt group padding, Hide the last 52 pt key). Simulator results do not prove real
// IME or hardware keyboard behaviour (AGENTS.md).
extension SmokeQA {
    private static let inputModeEnvironment = [
        "YEBISU_TERMINAL_QA": "1",
        "YEBISU_SMOKE_OBSERVE": "1",
        "YEBISU_QA_AX": "1",
        "YEBISU_QA_KEY_BAND_SOFTWARE_ONLY": "1",
        "YEBISU_HERDR_KEYS_QA": "[keys]\nprefix = 'ctrl+a'\n",
    ]

    private static let coreEnvironment = [
        "YEBISU_CORE_QA": "1",
        "YEBISU_SMOKE_OBSERVE": "1",
        "YEBISU_INPUT_QA": "1",
        "YEBISU_QA_AX": "1",
        "YEBISU_QA_START_SELECTION": "double-tap",
    ]

    private func inputMode(_ state: QAState?) -> [String: Any]? {
        state?.raw["input_mode"] as? [String: Any]
    }

    private func flag(_ state: QAState?, _ key: String) -> Bool {
        inputMode(state)?[key] as? Bool == true
    }

    private func shown(_ state: QAState?) -> Bool {
        inputMode(state)?["accessory_frame"] is [Any]
    }

    /// Prints one observation for input-mode-check.py.
    @discardableResult
    private func observe(_ app: XCUIApplication, _ step: String, _ state: QAState?,
                         _ extra: [String: Any] = [:]) -> QAState? {
        var record: [String: Any] = ["step": step, "input_mode": inputMode(state) ?? NSNull(),
                                     "keyboard": state?.keyboard ?? -1, "window": [app.frame.width, app.frame.height],
                                     "sheet": state?.raw["sheet"] ?? NSNull()]
        for (key, value) in extra { record[key] = value }
        if let data = try? JSONSerialization.data(withJSONObject: record),
           let line = String(data: data, encoding: .utf8) {
            print("INPUT_MODE_OBS \(line)")
        }
        return state
    }

    /// Waits up to `timeout` for `until`, then records the last state either way.
    @discardableResult
    private func observeUntil(_ app: XCUIApplication, _ qa: QAAx, _ step: String, timeout: TimeInterval,
                              _ until: @escaping (QAState) -> Bool) -> QAState? {
        let state = (try? qa.waitState("input mode \(step)", timeout: timeout, until)) ?? qa.read()
        return observe(app, step, state)
    }

    /// One input source switch, once no earlier indicator is visible; records the indicator.
    private func switchSource(_ app: XCUIApplication, _ qa: QAAx, _ step: String) {
        _ = try? qa.waitState("no indicator before \(step)", timeout: 6) { !self.shown($0) }
        app.typeKey(" ", modifierFlags: .control)
        observeUntil(app, qa, step, timeout: 3) { self.shown($0) }
    }

    private func hardwareState(_ app: XCUIApplication) {
        app.typeKey(XCUIKeyboardKey.shift.rawValue, modifierFlags: [])
        sleep(2)
    }

    private func active(_ state: QAState) -> Bool {
        flag(state, "text_first_responder") && flag(state, "activated")
    }

    private func inactive(_ state: QAState) -> Bool {
        inputMode(state) != nil && !flag(state, "activated")
    }

    private func bandKey(_ app: XCUIApplication, x: Double, keyboard: Double, _ name: String) {
        let y = app.frame.height - keyboard - 38
        print(String(format: "INPUT_MODE band %@ at (%.0f, %.0f), keyboard inset %.0f", name, x, y, keyboard))
        app.coordinate(withNormalizedOffset: CGVector(dx: 0, dy: 0)).withOffset(CGVector(dx: x, dy: y)).tap()
        usleep(800_000)
    }

    /// Runs one case; an error ends that case only (its missing steps fail in the judge).
    private func inputModeCase(_ name: String, _ body: () throws -> Void) {
        do {
            try body()
        } catch {
            print("INPUT_MODE_CASE_ERROR \(name) \(error)")
            save("input-mode-error-\(name)")
        }
    }

    func testInputModeIndicator() throws {
        continueAfterFailure = true
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        let qa = QAAx(app)
        app.terminate()
        app.launchEnvironment = Self.inputModeEnvironment
        XCUIDevice.shared.orientation = .portrait
        app.launch()
        try waitTerminal(qa, "the wire-peer terminal is connected")

        // T-IM-S1: the terminal takes text, the Simulator goes to the hardware keyboard state, and
        // each switch shows the indicator under the cursor.
        inputModeCase("s1") {
            try qa.press(QAIds.terminalInput, effect: 10, what: "text view first responder, indicator active") {
                self.active($0)
            }
            observe(app, "s1-focused", qa.read())
            hardwareState(app)
            observeUntil(app, qa, "s1-hardware-keyboard", timeout: 5) { !$0.keyboardUp }
            for n in 1...3 { switchSource(app, qa, "s1-switch-\(n)") }
        }

        // T-IM-S8: no system caret over the cursor cell; four screenshots, as a caret blinks.
        inputModeCase("s8") {
            _ = try? qa.waitState("no indicator before s8", timeout: 6) { !self.shown($0) }
            observe(app, "s8-cursor", qa.read())
            for n in 1...4 {
                save("input-mode-s8-\(n)")
                usleep(300_000)
            }
        }

        // T-IM-S7: the Herdr prefix armed (hardware Control-A), then a switch; the indicator
        // follows the input source and the interaction stays on.
        inputModeCase("s7") {
            app.typeKey("a", modifierFlags: .control)
            usleep(600_000)
            observe(app, "s7-prefix-armed", qa.read())
            switchSource(app, qa, "s7-switch")
            app.typeKey(XCUIKeyboardKey.escape.rawValue, modifierFlags: [])
            usleep(600_000)
        }

        // T-IM-S2 (Herdr 1) and T-IM-S5 (Settings): a larger font moves the cursor cell;
        // caret_rect follows it once Settings closes. The wire peer's frame is fixed, so no
        // output moves this cursor.
        inputModeCase("s2-h1") {
            let before = observe(app, "s2-h1-before", qa.read())
            let beforeCaret = inputMode(before)?["caret_rect"] as? [Double]
            try pressLarger(qa)
            observeUntil(app, qa, "s5-settings-open", timeout: 5) { self.inactive($0) }
            try qa.press(QAIds.settingsClose, what: "Settings closed") { $0.isNull("sheet") }
            observeUntil(app, qa, "s2-h1-after", timeout: 10) {
                self.active($0) && (self.inputMode($0)?["caret_rect"] as? [Double]) != beforeCaret
            }
            observeUntil(app, qa, "s5-settings-closed", timeout: 10) { self.active($0) }
        }

        // T-IM-S5: Settings › Shortcuts and › Hosts & keys, the command palette, then Find.
        for (page, step) in [(QIds.shortcuts, "s5-shortcuts"), (QIds.hosts, "s5-hosts")] {
            inputModeCase(step) {
                try openSettingsSheet(qa)
                try qa.press(page, what: "\(step) page")
                observeUntil(app, qa, "\(step)-open", timeout: 5) { self.inactive($0) }
                try qa.press(QAIds.settingsClose, what: "Settings closed") { $0.isNull("sheet") }
                observeUntil(app, qa, "\(step)-closed", timeout: 10) { self.active($0) }
            }
        }
        inputModeCase("s5-palette") {
            try menuItem(qa, QIds.palette, what: "the command palette is open") { _ in true }
            observeUntil(app, qa, "s5-palette-open", timeout: 5) { self.inactive($0) }
            app.typeKey(XCUIKeyboardKey.escape.rawValue, modifierFlags: [])
            observeUntil(app, qa, "s5-palette-closed", timeout: 10) { self.active($0) }
        }
        inputModeCase("s5-find") {
            try menuItem(qa, QIds.find, what: "Find is open") { _ in true }
            observeUntil(app, qa, "s5-find-open", timeout: 5) { _ in true }
            app.typeKey(XCUIKeyboardKey.escape.rawValue, modifierFlags: [])
            observeUntil(app, qa, "s5-find-closed", timeout: 10) { self.active($0) }
        }

        // T-IM-S6: the band's ctrl latch and a letter change nothing; the band's Hide switches the
        // interaction off.
        inputModeCase("s6") {
            try qa.press(QAIds.terminalInput, effect: 10, what: "the software keyboard is up") { $0.keyboardUp }
            let keyboard = try qa.settledKeyboard("s6 keyboard up").keyboard
            observe(app, "s6-keyboard", qa.read())
            bandKey(app, x: 146, keyboard: keyboard, "ctrl")
            observe(app, "s6-latched", qa.read())
            app.keyboards.keys["b"].tap()
            usleep(800_000)
            observe(app, "s6-after-letter", qa.read())
            bandKey(app, x: app.frame.width - 38, keyboard: keyboard, "hide")
            observeUntil(app, qa, "s6-hidden", timeout: 5) { self.inactive($0) }
        }

        // T-IM-S3: background, then foreground; the text view is first responder again.
        inputModeCase("s3") {
            try qa.press(QAIds.terminalInput, effect: 10, what: "text view first responder again") { self.active($0) }
            XCUIDevice.shared.press(.home)
            try requireState(app, [.runningBackground, .runningBackgroundSuspended], "s3 background")
            app.activate()
            try requireState(app, [.runningForeground], "s3 foreground")
            observeUntil(app, qa, "s3-foreground", timeout: 15) { self.active($0) }
        }

        // T-IM-S2 (Herdr 2) and T-IM-S4: the core's shell echoes typed text, which moves the
        // cursor; a composition survives output that a background job prints while composing.
        inputModeCase("s2-h2") {
            app.terminate()
            app.launchEnvironment = Self.coreEnvironment
            app.launch()
            try qa.waitState("the Herdr 2 terminal is connected and drawn", timeout: 60) {
                $0.string("screen") == "terminal" && $0.bool("connected") == true && !$0.isNull("surface")
            }
            try qa.press(QAIds.terminalInput, effect: 10, what: "Herdr 2 text view active") { self.active($0) }
            hardwareState(app)
            let h2 = observe(app, "s2-h2-before", qa.read())
            let h2Caret = inputMode(h2)?["caret_rect"] as? [Double]
            app.typeText("echo yebisu-139")
            observeUntil(app, qa, "s2-h2-after", timeout: 10) {
                (self.inputMode($0)?["caret_rect"] as? [Double]) != h2Caret
            }
        }
        inputModeCase("s4") {
            app.typeText("\n(sleep 4; echo tick-139) &\n")
            sleep(1)
            // To Japanese (en_US -> ja_JP: up to three switches), then compose without committing.
            for n in 1...3 { switchSource(app, qa, "s4-switch-\(n)") }
            app.typeText("nihon")
            observe(app, "s4-composing", qa.read())
            sleep(6)
            observe(app, "s4-after-output", qa.read())
            app.typeKey(XCUIKeyboardKey.escape.rawValue, modifierFlags: [])
        }
        XCTAssertEqual(app.state, .runningForeground)
    }
}

/// The QA identifiers this test presses besides QAIds; each is a constant in
/// crates/yebisu/src/qa_ax_ids.rs.
private enum QIds {
    static let shortcuts = "qa.settings.page.shortcuts"
    static let hosts = "qa.settings.page.hosts-and-keys"
    static let palette = "qa.menu.command-palette"
    static let find = "qa.menu.find"
}
