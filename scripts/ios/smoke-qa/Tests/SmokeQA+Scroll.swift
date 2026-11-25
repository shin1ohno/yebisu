import XCTest

// Issue #137 PR 1: real touches over the terminal reach the wire peer as Herdr wheel input.
// scroll-check.py starts the app with YEBISU_SCROLL_QA (the private peer's frames list one
// pane, as a Herdr 1 host's do), runs this test, stops the app and only then reads
// Documents/p4-peer.jsonl. Each step ends with a marker key, so the peer log splits into the
// steps: s after a slow drag down, t after a fast swipe up that coasts after the lift, u after
// a sideways drag.
extension SmokeQA {
    private static let scrollEnvironment = [
        "YEBISU_TERMINAL_QA": "1",
        "YEBISU_SMOKE_OBSERVE": "1",
        "YEBISU_QA_AX": "1",
        "YEBISU_SCROLL_QA": "1",
    ]

    /// Waits for any momentum to end, then types `key` on the hardware keyboard.
    private func scrollMarker(_ app: XCUIApplication, _ key: String) {
        sleep(3)
        app.typeKey(key, modifierFlags: [])
        usleep(600_000)
    }

    func testTerminalScroll() throws {
        continueAfterFailure = true
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        let qa = QAAx(app)
        app.terminate()
        app.launchEnvironment = Self.scrollEnvironment
        XCUIDevice.shared.orientation = .portrait
        app.launch()
        try waitTerminal(qa, "the wire-peer terminal is connected")
        try qa.press(QAIds.terminalInput, effect: 10, what: "the software keyboard is up") { $0.keyboardUp }
        _ = try qa.settledKeyboard("scroll keyboard up")
        let terminal = qa.element(QAIds.terminalInput)
        print("SCROLL terminal frame \(terminal.frame)")
        save("scroll-start")

        // s: a slow drag down by 300 pt, held still before the lift so nothing coasts.
        let top = terminal.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.15))
        top.press(forDuration: 0.05, thenDragTo: top.withOffset(CGVector(dx: 0, dy: 300)),
                  withVelocity: XCUIGestureVelocity(200), thenHoldForDuration: 0.5)
        scrollMarker(app, "s")
        save("scroll-s")

        // t: a fast fling up by 300 pt, lifted while still moving, scrolls back down and keeps
        // going after the lift. XCUITest's swipeUp() lifts without a fling GPUI recognizes (its
        // events stopped within 133 ms, scroll-check run 1).
        let low = terminal.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.6))
        low.press(forDuration: 0.05, thenDragTo: low.withOffset(CGVector(dx: 0, dy: -300)),
                  withVelocity: .fast, thenHoldForDuration: 0)
        scrollMarker(app, "t")
        save("scroll-t")

        // u: a sideways drag reaches no pane that does not read the mouse.
        let left = terminal.coordinate(withNormalizedOffset: CGVector(dx: 0.2, dy: 0.2))
        left.press(forDuration: 0.05, thenDragTo: left.withOffset(CGVector(dx: 400, dy: 0)),
                   withVelocity: XCUIGestureVelocity(200), thenHoldForDuration: 0.5)
        scrollMarker(app, "u")
        save("scroll-u")
        XCTAssertEqual(app.state, .runningForeground)
    }
}
