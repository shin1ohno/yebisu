import XCTest

// L57: Herdr prefix style with a Herdr 1 host whose [keys].prefix is ctrl+a. prefix-check.py
// saves Shortcut style = Herdr prefix in the settings file before this test launches the app,
// and reads the wire peer's p4-peer.jsonl after it stops the app. Each step ends with a marker
// letter, so the peer log splits into the steps: z, y after the software keyboard steps, 0, 1,
// 2, 3 after the hardware ones. The key band has no accessibility element, so its ctrl key is
// pressed at its layout position (shell/key_band/mod.rs: 8 pt row gap, 4 pt group padding,
// esc and tab 52 pt wide with 2 pt gaps; the 76 pt band sits on the keyboard).
extension SmokeQA {
    private static let prefixEnvironment = [
        "YEBISU_TERMINAL_QA": "1",
        "YEBISU_SMOKE_OBSERVE": "1",
        "YEBISU_QA_AX": "1",
        "YEBISU_QA_KEY_BAND_SOFTWARE_ONLY": "1",
        "YEBISU_HERDR_KEYS_QA": "[keys]\nprefix = 'ctrl+a'\n",
    ]

    private func softwareKey(_ app: XCUIApplication, _ letter: String) {
        let keys = app.keyboards.keys
        let key = keys[letter].exists ? keys[letter] : keys[letter.uppercased()]
        print("PREFIX software key \(letter): exists=\(key.exists)")
        key.tap()
        usleep(600_000)
    }

    /// `keyboard` is the inset the app reports (qa.state); XCUITest's keyboard frame leaves out
    /// the shortcut bar above the keys.
    private func bandControl(_ app: XCUIApplication, keyboard: Double) {
        let y = app.frame.height - keyboard - 38
        let origin = app.coordinate(withNormalizedOffset: CGVector(dx: 0, dy: 0))
        print(String(format: "PREFIX band ctrl at (146, %.0f), keyboard inset %.0f", y, keyboard))
        origin.withOffset(CGVector(dx: 146, dy: y)).tap()
        usleep(600_000)
    }

    private func hardwareKey(_ app: XCUIApplication, _ key: String, _ flags: XCUIElement.KeyModifierFlags = []) {
        app.typeKey(key, modifierFlags: flags)
        usleep(600_000)
    }

    func testHerdrPrefixInputPaths() throws {
        continueAfterFailure = true
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        let qa = QAAx(app)
        app.terminate()
        app.launchEnvironment = Self.prefixEnvironment
        XCUIDevice.shared.orientation = .portrait
        app.launch()
        try waitTerminal(qa, "the wire-peer terminal is connected")
        try qa.press(QAIds.terminalInput, effect: 10, what: "the software keyboard is up") { $0.keyboardUp }
        let keyboard = try qa.settledKeyboard("prefix keyboard up").keyboard
        print("PREFIX qa.state \(try qa.waitState("qa.state for the record", timeout: 5) { _ in true }.raw)")
        save("prefix-keyboard")

        // S1: the band's ctrl latch, then the software keyboard's a and c.
        bandControl(app, keyboard: keyboard)
        save("prefix-s1-latched")
        softwareKey(app, "a")
        softwareKey(app, "c")
        sleep(1)
        save("prefix-s1-after-c")
        softwareKey(app, "z")
        // S2: the latch with a twice sends one Control-A.
        bandControl(app, keyboard: keyboard)
        softwareKey(app, "a")
        bandControl(app, keyboard: keyboard)
        softwareKey(app, "a")
        softwareKey(app, "y")
        sleep(1)
        save("prefix-s2")

        // H0: one plain hardware key first. In the Simulator the first hardware Control-A
        // after software-keyboard typing arrived as the text "a" (L57 report); H0 keeps that
        // switch out of the steps below.
        hardwareKey(app, "0")
        // H1: a hardware Control-A, then c. H2: Control-A twice. H3: Control-A, then c again.
        hardwareKey(app, "a", .control)
        hardwareKey(app, "c")
        sleep(1)
        save("prefix-h1-after-c")
        hardwareKey(app, "1")
        hardwareKey(app, "a", .control)
        hardwareKey(app, "a", .control)
        hardwareKey(app, "2")
        sleep(1)
        save("prefix-h2")
        hardwareKey(app, "a", .control)
        hardwareKey(app, "c")
        hardwareKey(app, "3")
        sleep(2)
        save("prefix-h3")
        XCTAssertEqual(app.state, .runningForeground)
    }
}
