import XCTest

// P14 connected notifications.
extension SmokeQA {
    /// P14's state in qa.state (notification_view/qa.rs): `step`, the shown toast's title (nil
    /// without one) and the status line the panel draws ("" once it expired).
    private func p14(_ state: QAState) -> (step: Int, toast: String?, status: String)? {
        guard let field = state.raw["p14"] as? [String: Any], let step = field["step"] as? Int else { return nil }
        return (step, field["toast"] as? String, field["status"] as? String ?? "")
    }

    /// Presses "P14 next" and waits until the app took step `step` (before S6: OCR found the
    /// control and a fixed sleep followed).
    private func p14Next(_ qa: QAAx, step: Int) throws {
        try qa.settleIfMoved("before P14 next (step \(step))")
        try qa.press(QAIds.p14Next, what: "P14 step \(step)") { self.p14($0)?.step == step }
    }

    func testP14ConnectedNotifications() throws {
        continueAfterFailure = false
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        let qa = QAAx(app)
        app.terminate()
        app.launchEnvironment["YEBISU_TERMINAL_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment["YEBISU_P14_QA"] = "1"
        app.launchEnvironment["YEBISU_QA_AX"] = "1"
        app.launch()
        // Before S6: sleep(4).
        try waitTerminal(qa, "the wire-peer terminal is connected") { self.p14($0)?.step == 0 }
        // Owner decision: with the software keyboard up, the toast sits just above it.
        // A tap on the terminal focuses its input, which raises the keyboard (before S6: OCR
        // tapped the terminal's last row, "Type here").
        try qa.settleIfMoved("before the terminal tap")
        try qa.press(QAIds.terminalInput, effect: 10, what: "the software keyboard is up") { $0.keyboardUp }
        try qa.settledKeyboard("P14 keyboard up")
        let keyboard = app.keyboards.element(boundBy: 0)
        let keyboardTop = keyboard.exists ? keyboard.frame.minY : app.frame.height
        XCTAssertLessThan(keyboardTop, app.frame.height - 100, "the software keyboard must be up for this measurement")
        try p14Next(qa, step: 1)
        // The toast lives five seconds (STATUS_TTL): its frame is read and it is pressed at once,
        // and the screenshot comes after the press (PR 84 review F3: a screenshot before the press
        // could use up the lifetime). The app's own p14-paint receipts prove the toast was drawn.
        try qa.waitState("the toast is up", timeout: 5) { self.p14($0)?.toast == "Agent needs attention" }
        let shown = Date()
        let frame = try qa.find(QAIds.toast, timeout: 2).frame
        try qa.press(QAIds.toast, what: "the toast's tap focused pane w1:p3") { $0.string("pane") == "w1:p3" }
        print(String(format: "P14 toast pressed and confirmed %.2f s after it was shown", Date().timeIntervalSince(shown)))
        save("p14-toast")
        // Before S6 the bottom of the OCR text box was measured; now the toast's own frame, which
        // holds that text, so the check is at least as strict.
        print("P14 keyboard top=\(keyboardTop) toast frame=\(frame)")
        XCTAssertLessThanOrEqual(frame.maxY, keyboardTop + 2, "toast is covered by the software keyboard")
        save("p14-focus")
        // Step 2: a quiet interval suppresses the notification; its status line lives five
        // seconds like the toast (STATUS_TTL).
        try p14Next(qa, step: 2)
        let shown = Date()
        try qa.waitState("the QuietHours status line is shown", timeout: 5) { self.p14($0)?.status.contains("QuietHours") == true }
        save("p14-quiet")
        try qa.waitState("the status line expired", timeout: 10) { self.p14($0)?.status.isEmpty == true }
        let lived = Date().timeIntervalSince(shown)
        save("p14-status-expired")
        print(String(format: "P14 status line gone %.2f s after step 2 was confirmed", lived))
        XCTAssertGreaterThanOrEqual(lived, 3.5, "the status line must stay for its lifetime (5 s), not vanish at once")
        // Steps 3-5: a missing pane, a disconnect and a stale tap after it; each waits for the
        // app's own outcome (before S6: sleep(3) each).
        try p14Next(qa, step: 3)
        try qa.waitState("InvalidTarget for the missing pane", timeout: 10) { self.p14($0)?.status.contains("InvalidTarget") == true }
        save("p14-invalid")
        try p14Next(qa, step: 4)
        try qa.waitState("the peer disconnected", timeout: 10) { $0.bool("connected") == false }
        save("p14-disconnect")
        try p14Next(qa, step: 5)
        try qa.waitState("the late tap is refused as Stale", timeout: 10) { self.p14($0)?.status.contains("Stale") == true }
        save("p14-stale-tap")
        XCTAssertEqual(app.state, .runningForeground)
        // Never visit notification settings or answer an OS permission dialog.
    }
}
