import Darwin
import XCTest

// Issue 140: a port forward from the app to a private sshd on this Mac (scripts/ios/forward-check.py
// runs both tests). testForwardKeyLine prints the app's public key line for the harness to
// authorize; testForwardPort opens Forward port… from the host row's long press, forwards the
// port the harness serves a page on, opens the row in Safari and reads the page, then goes home
// and finds the port closed after the grace, and back finds it listening on the same port. Every
// press is by QA identifier and every result read from qa.state (`forwards`, `forward_sheet`).
// The Simulator proves neither real suspension nor IME or hardware-keyboard behaviour.
extension SmokeQA {
    private static let safariID = "com.apple.mobilesafari"

    private func forwardEnvironment(_ name: String) throws -> String {
        let value = ProcessInfo.processInfo.environment[name] ?? ""
        try XCTSkipIf(value.isEmpty, "\(name) is set by scripts/ios/forward-check.py")
        return value
    }

    private func launchForwardApp(_ app: XCUIApplication) {
        app.terminate()
        // The terminal is the wire peer (qa@private-peer), so the main frame and its sidebar show;
        // the forward has its own SSH connection to the harness's host.
        // YEBISU_FORWARD_QA: qa.state carries the public line of the key a forward signs with.
        app.launchEnvironment = ["YEBISU_QA_AX": "1", "YEBISU_TERMINAL_QA": "1", "YEBISU_FORWARD_QA": "1"]
        XCUIDevice.shared.orientation = .landscapeLeft
        app.launch()
    }

    private func forwards(_ state: QAState) -> [[String: Any]] {
        state.raw["forwards"] as? [[String: Any]] ?? []
    }

    private func forward(_ state: QAState, remote: Int) -> [String: Any]? {
        forwards(state).first { ($0["remote"] as? Int) == remote }
    }

    private func sheet(_ state: QAState) -> [String: Any]? {
        state.raw["forward_sheet"] as? [String: Any]
    }

    /// Whether a TCP connection to 127.0.0.1:`port` is refused (nothing listens).
    private func refused(_ port: Int) -> Bool {
        let fd = socket(AF_INET, SOCK_STREAM, 0)
        guard fd >= 0 else { return false }
        defer { close(fd) }
        var address = sockaddr_in()
        address.sin_family = sa_family_t(AF_INET)
        address.sin_port = in_port_t(UInt16(port).bigEndian)
        address.sin_addr.s_addr = inet_addr("127.0.0.1")
        let result = withUnsafePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                connect(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size))
            }
        }
        return result != 0 && errno == ECONNREFUSED
    }

    func testForwardKeyLine() throws {
        continueAfterFailure = false
        _ = try forwardEnvironment("FORWARD_QA_TARGET")
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        let qa = QAAx(app)
        launchForwardApp(app)
        let state = try qa.waitState("the device key line", timeout: 60) {
            ($0.string("device_key_line") ?? "").contains("ecdsa-sha2-nistp256 ")
        }
        let line = try XCTUnwrap(state.string("device_key_line"))
        print("FORWARD-KEY-LINE \(line)")
    }

    func testForwardPort() throws {
        continueAfterFailure = false
        let target = try forwardEnvironment("FORWARD_QA_TARGET")
        let marker = try forwardEnvironment("FORWARD_QA_MARKER")
        let port = try XCTUnwrap(Int(try forwardEnvironment("FORWARD_QA_PORT")))
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        let qa = QAAx(app)
        launchForwardApp(app)
        _ = try qa.waitState("the wire peer is connected", timeout: 60) { $0.bool("connected") == true }
        // Long press of the saved host's row. It is not the selected host (the wire peer is), so
        // this also shows Forward port… working on another host's row (owner decision 7).
        // A long press ends in the row's menu, not in the press counter's mouse-up, so it is judged
        // by qa.state's row_menu.
        let hostRow = try qa.find(QAIds.sidebarHostForwardQA, timeout: 20)
        try qa.coordinate(hostRow, id: QAIds.sidebarHostForwardQA, at: CGVector(dx: 0.5, dy: 0.5))
            .press(forDuration: 1.2)
        _ = try qa.waitState("the host row's menu", timeout: 10) { $0.string("row_menu") == "host" }
        save("forward-host-menu")
        try qa.press(QAIds.hostMenuForward, timeout: 10, what: "the forward sheet is open") {
            (self.sheet($0)?["host"] as? String) == target
        }
        // The chips: the host's listening ports, numbers only.
        let scanned = try qa.waitState("the chips list port \(port)", timeout: 40) {
            ((self.sheet($0)?["chips"] as? [Int]) ?? []).contains(port)
        }
        print("FORWARD chips \(sheet(scanned)?["chips"] ?? "nil")")
        save("forward-sheet")
        // A tap on the field gives it the text input and the keyboard, as a finger would.
        try qa.press(QAIds.forwardField, timeout: 10)
        // One digit at a time, each confirmed in qa.state, as a finger types:
        // typeText("8000") in one burst once left only "8" in the field.
        var typed = ""
        for digit in "\(port)" {
            typed.append(digit)
            app.typeText(String(digit))
            let want = typed
            _ = try qa.waitState("the field holds \(want)", timeout: 10) {
                (self.sheet($0)?["field"] as? String) == want
            }
        }
        try qa.press(QAIds.forwardSubmit, timeout: 10, what: "the forward starts and the sheet closes") {
            self.sheet($0) == nil && self.forward($0, remote: port) != nil
        }
        let listening = try qa.waitState("port \(port) listens", timeout: 60) {
            (self.forward($0, remote: port)?["state"] as? String) == "listening"
        }
        let local = try XCTUnwrap(forward(listening, remote: port)?["local"] as? Int)
        print("FORWARD \(port) listens on 127.0.0.1:\(local)")
        XCTAssertNotEqual(local, port, "the harness holds 127.0.0.1:\(port), so the forward moves")
        save("forward-listening")
        // Open in Safari and read the page through the forward.
        let opens = listening.count("forward_open")
        let row0 = try qa.find(QAIds.forwardRow, timeout: 10)
        try qa.coordinate(row0, id: QAIds.forwardRow, at: CGVector(dx: 0.5, dy: 0.5)).tap()
        let safari = XCUIApplication(bundleIdentifier: Self.safariID)
        XCTAssertTrue(safari.wait(for: .runningForeground, timeout: 30), "Safari did not come to the front")
        let page = safari.staticTexts.containing(NSPredicate(format: "label CONTAINS %@", marker)).firstMatch
        XCTAssertTrue(page.waitForExistence(timeout: 60), "Safari did not show the forwarded page")
        save("forward-safari")
        app.activate()
        try requireState(app, [.runningForeground], "back to yebisu after Safari")
        _ = try qa.waitState("one Open", timeout: 20) { $0.count("forward_open") == opens + 1 }
        // Off screen: after the 5 s grace the listener is closed.
        XCUIDevice.shared.press(.home)
        sleep(9)
        XCTAssertTrue(refused(local), "127.0.0.1:\(local) still accepted after the grace")
        app.activate()
        try requireState(app, [.runningForeground], "back to yebisu after home")
        // Until the app's next poll, qa.state is the snapshot from before going
        // home and still says listening, so the wait also asks the port itself.
        let resumed = try qa.waitState("port \(port) listens again", timeout: 60) {
            guard (self.forward($0, remote: port)?["state"] as? String) == "listening" else { return false }
            return (self.forward($0, remote: port)?["local"] as? Int) != local || !self.refused(local)
        }
        XCTAssertEqual(forward(resumed, remote: port)?["local"] as? Int, local, "resumed on the same port")
        XCTAssertFalse(refused(local), "127.0.0.1:\(local) refused after the resume")
        save("forward-resumed")
    }
}
