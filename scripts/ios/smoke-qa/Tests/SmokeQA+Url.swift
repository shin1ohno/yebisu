import XCTest

// L66: with a hardware keyboard, Command + tap on a web address in the terminal opens it in the
// default browser. YEBISU_URL_TAP_QA makes the Herdr 1 wire peer and the Herdr 2 core peer show
// the link fixture (crates/yebisu/src/terminal_links/qa.rs), and the app publishes each link's
// window point in qa.state `url_targets`. An open is judged by the app's own receipt of the request
// it handed to UIKit (`url_open`, `url_open_last`) and by Safari coming to the front, not by a
// fake browser; "no byte for the terminal" by the peer's input count (`peer.inputs`).
// The Simulator's hardware keyboard holds Command; a physical keyboard and the pointer are the
// owner's iPad check.
extension SmokeQA {
    private static let safari = "com.apple.mobilesafari"

    private func urlTarget(_ state: QAState, _ name: String) -> CGVector? {
        guard let targets = state.raw["url_targets"] as? [String: Any],
              let point = targets[name] as? [Double], point.count == 2 else { return nil }
        return CGVector(dx: point[0], dy: point[1])
    }

    private func openedURL(_ state: QAState) -> String? {
        (state.raw["url_open_last"] as? [String: Any])?["url"] as? String
    }

    private func point(_ app: XCUIApplication, _ at: CGVector) -> XCUICoordinate {
        app.coordinate(withNormalizedOffset: .zero).withOffset(at)
    }

    /// The fixture's state once link `name` has a window point.
    private func fixture(_ qa: QAAx, _ name: String) throws -> (QAState, CGVector) {
        let state = try qa.waitState("url_targets has \(name)", timeout: 30) { urlTarget($0, name) != nil }
        return (state, try XCTUnwrap(urlTarget(state, name)))
    }

    private func commandTap(_ app: XCUIApplication, _ at: CGVector) {
        XCUIElement.perform(withKeyModifiers: .command) {
            point(app, at).tap()
        }
    }

    /// Command + taps link `name`; the app must record one open request whose address passes
    /// `want`, send the terminal nothing, and Safari must come to the front. Then back to yebisu.
    private func opens(_ app: XCUIApplication, _ qa: QAAx, _ name: String, _ what: String,
                       want: (String) -> Bool) throws {
        let (before, at) = try fixture(qa, name)
        print("URL \(name): Command + tap at \(at), url_open=\(before.count("url_open")) inputs=\(before.peerInputs)")
        commandTap(app, at)
        // UIKit opened the browser: Safari comes to the front, and yebisu's qa.state is out of
        // reach until yebisu is back.
        let safari = XCUIApplication(bundleIdentifier: Self.safari)
        XCTAssertTrue(safari.wait(for: .runningForeground, timeout: 30), "\(what): Safari did not come to the front")
        save("url-\(name)-safari")
        app.activate()
        try requireState(app, [.runningForeground], "back to yebisu after \(name)")
        let after = try qa.waitState("\(what): the app's open request", timeout: 30) {
            $0.count("url_open") == before.count("url_open") + 1
        }
        let url = try XCTUnwrap(openedURL(after), "\(what): no url_open_last in \(after)")
        print("URL \(name): the app asked UIKit to open \(url)")
        XCTAssertTrue(want(url), "\(what): opened \(url)")
        XCTAssertEqual(after.peerInputs, before.peerInputs, "\(what): Command + tap sent the terminal input")
    }

    /// Command + taps link `name`, which must open nothing: one refusal receipt starting with
    /// `reason`, no open request, no input, and yebisu still in front.
    private func refuses(_ app: XCUIApplication, _ qa: QAAx, _ name: String, _ reason: String) throws {
        let (before, at) = try fixture(qa, name)
        commandTap(app, at)
        let after = try qa.waitState("\(name): refused", timeout: 15) {
            $0.count("url_refused") == before.count("url_refused") + 1
        }
        let said = after.string("url_refused_last") ?? ""
        print("URL \(name): refused with '\(said)'")
        XCTAssertTrue(said.hasPrefix(reason), "\(name): status '\(said)'")
        XCTAssertEqual(after.count("url_open"), before.count("url_open"), "\(name): opened")
        XCTAssertEqual(after.peerInputs, before.peerInputs, "\(name): sent the terminal input")
        XCTAssertEqual(app.state, .runningForeground)
        save("url-\(name)-refused")
    }

    private func launchFixture(_ app: XCUIApplication, core: Bool, mode: String = "1") {
        app.terminate()
        var environment = [
            "YEBISU_SMOKE_OBSERVE": "1",
            "YEBISU_QA_AX": "1",
            "YEBISU_URL_TAP_QA": mode,
        ]
        environment[core ? "YEBISU_CORE_QA" : "YEBISU_TERMINAL_QA"] = "1"
        app.launchEnvironment = environment
        XCUIDevice.shared.orientation = .landscapeLeft
        app.launch()
    }

    func testUrlCommandTapHerdr1() throws {
        continueAfterFailure = false
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        let qa = QAAx(app)
        launchFixture(app, core: false)
        let (start, plain) = try fixture(qa, "plain")
        save("url-herdr1-fixture")
        // Without Command a tap is today's: no open request, yebisu stays in front.
        point(app, plain).tap()
        sleep(2)
        let after = try XCTUnwrap(qa.read())
        XCTAssertEqual(after.count("url_open"), start.count("url_open"), "a tap without Command opened")
        XCTAssertEqual(after.count("url_refused"), start.count("url_refused"))
        XCTAssertEqual(app.state, .runningForeground)
        try refuses(app, qa, "mailto", "Not opened: mailto: link")
        try refuses(app, qa, "path", "Not opened: a file path")
        try opens(app, qa, "plain", "plain address after wide text") { $0 == "https://example.com/l66?q=plain" }
        try opens(app, qa, "wrapped", "address wrapped at the right edge") {
            $0 == "https://example.com/l66/wrapped/0123456789abcdef"
        }
        try opens(app, qa, "osc8", "OSC 8 hyperlink") { $0 == "https://example.com/l66/osc8" }
    }

    func testUrlCommandTapHerdr2() throws {
        continueAfterFailure = false
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        let qa = QAAx(app)
        launchFixture(app, core: true)
        _ = try qa.waitState("the Herdr 2 terminal is connected", timeout: 60) {
            $0.bool("connected") == true && $0.corePanes == 1
        }
        _ = try fixture(qa, "core_plain")
        save("url-herdr2-fixture")
        try opens(app, qa, "core_plain", "Herdr 2 plain address") { $0 == "https://example.com/l66/core-plain" }
        try opens(app, qa, "core_osc8", "Herdr 2 OSC 8 hyperlink") { $0 == "https://example.com/l66/core-osc8" }
        try opens(app, qa, "core_wrapped", "Herdr 2 address the terminal wrapped") {
            $0.hasPrefix("https://example.com/l66/core-wrapped/xxxxxxxxxx") && $0.hasSuffix("x")
        }
    }

    /// The Command underline as the app paints it: `YEBISU_URL_TAP_QA=hover` takes the pointer to
    /// rest on the wrapped address with Command held (XCTest cannot hover a pointer on iPad).
    func testUrlCommandUnderlineShot() throws {
        continueAfterFailure = false
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        let qa = QAAx(app)
        launchFixture(app, core: false, mode: "hover")
        _ = try fixture(qa, "wrapped")
        sleep(1)
        save("url-command-underline")
        XCTAssertEqual(app.state, .runningForeground)
    }
}
