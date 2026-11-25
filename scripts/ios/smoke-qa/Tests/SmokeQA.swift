import XCTest

/// The SmokeQA tests. Since the UI test redesign S6 none of them reads text off the screen to find
/// or confirm a control: controls are pressed by QA identifier and confirmed through `qa.state`
/// (QAAx.swift). The one screenshot text check left is P15's host block, where the text is the
/// subject (SmokeQA+Theme.swift, `p15Host`).
final class SmokeQA: XCTestCase {
    // P9 takes about 72 s on the Mac mini. On a hosted runner it took 96-121 s with the
    // Herdr 2 band, and 209.8 s in run 44276079121 once L8 moved the band into the ...
    // menu: about 30 s more for five menu trips (OCR, removed in S6) and one 60.09 s wait
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
        let (reached, observed) = reachState(app, states, what, timeout: timeout)
        if reached { return }
        save("state-wait-failed")
        XCTFail("\(what): app.state is \(stateName(observed)) after \(Int(timeout)) s, wanted \(states.map(stateName))")
        throw NSError(domain: "SmokeQA.StateWait", code: 1)
    }

    /// `requireState`'s wait without its failure: whether the app reached one of `states` within
    /// `timeout`, and the state read once at the end (issue #142 stage 4: QAAx.pressHome's wait).
    func reachState(_ app: XCUIApplication, _ states: [XCUIApplication.State], _ what: String,
                    timeout: TimeInterval = 30) -> (reached: Bool, observed: XCUIApplication.State) {
        let start = Date()
        while true {
            for state in states where app.wait(for: state, timeout: 0.5) {
                print(String(format: "WAIT ok %@: app.state=%@ after %.2f s", what, stateName(state), Date().timeIntervalSince(start)))
                return (true, state)
            }
            if Date().timeIntervalSince(start) >= timeout {
                // Read the state once: the verdict and the message must come from the same
                // observation, and `save()` takes seconds, long enough for the state to change.
                let observed = app.state
                if states.contains(observed) {
                    print(String(format: "WAIT ok %@: app.state=%@ after %.2f s (late)", what, stateName(observed), Date().timeIntervalSince(start)))
                    return (true, observed)
                }
                return (false, observed)
            }
        }
    }

    /// Home, then `requireState`'s background wait (30 s); a Home that never reached SpringBoard is
    /// pressed once more (QAAx.pressHome, class K), and a second miss fails as before, by `what`.
    func leaveForeground(_ app: XCUIApplication, _ qa: QAAx, _ what: String, timeout: TimeInterval = 30) throws {
        let background: [XCUIApplication.State] = [.runningBackground, .runningBackgroundSuspended]
        var observed = app.state
        let left = qa.pressHome(what) {
            let result = reachState(app, background, what, timeout: timeout)
            observed = result.observed
            return result.reached
        }
        if left { return }
        save("state-wait-failed")
        XCTFail("\(what): app.state is \(stateName(observed)) after \(Int(timeout)) s, wanted \(background.map(stateName))")
        throw NSError(domain: "SmokeQA.StateWait", code: 1)
    }

    func save(_ name: String) {
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
