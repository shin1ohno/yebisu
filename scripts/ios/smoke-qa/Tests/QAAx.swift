import Foundation
import XCTest

/// One read of `qa.state`, the QA accessibility layer's state element (crates/yebisu/src/qa_ax.rs).
/// Its value is one line of JSON: `seq` (grows with every change), `launch` (the app's process id),
/// `presses` (identifier -> presses GPUI delivered to that marked control), `duplicates`, the root
/// fields `screen`, `sheet`, `menu`, `taps`, and whatever a QA mode reported.
struct QAState: CustomStringConvertible {
    let raw: [String: Any]
    let text: String

    init?(_ text: String) {
        guard let data = text.data(using: .utf8),
              let raw = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else { return nil }
        self.raw = raw
        self.text = text
    }

    var seq: Int { raw["seq"] as? Int ?? -1 }
    var launch: Int { raw["launch"] as? Int ?? -1 }
    var duplicates: [String] { raw["duplicates"] as? [String] ?? [] }
    var description: String { text }

    /// Presses GPUI delivered to the marked control `id` since launch.
    func presses(_ id: String) -> Int { (raw["presses"] as? [String: Any])?[id] as? Int ?? 0 }
    func string(_ key: String) -> String? { raw[key] as? String }
    func int(_ key: String) -> Int? { raw[key] as? Int }
    func bool(_ key: String) -> Bool? { raw[key] as? Bool }
    /// True when `key` is present and JSON null (for example `menu` with no menu open).
    func isNull(_ key: String) -> Bool { raw[key] is NSNull }
}

/// Why a QA layer step failed. XCTest prints the description when a test throws it, so each case
/// names the identifier or the wait and carries the last state read.
enum QAAxError: Error, CustomStringConvertible {
    case notExposed(id: String, seconds: Double, state: String)
    case duplicate(id: String, state: String)
    case timeout(what: String, seconds: Double, reads: Int, last: String)
    case pressNotDelivered(id: String, attempts: Int, before: String, after: String)
    case pressTakenElsewhere(id: String, before: String, after: String)
    case pressWithoutEffect(id: String, what: String, seconds: Double, last: String)

    var description: String {
        switch self {
        case let .notExposed(id, seconds, state):
            return String(format: "QA id %@ not exposed after %.1f s; qa.state=%@", id, seconds, state)
        case let .duplicate(id, state):
            return "QA id \(id) is marked more than once; qa.state=\(state)"
        case let .timeout(what, seconds, reads, last):
            return String(format: "qa.state wait '%@' timed out after %.2f s (%ld reads); last qa.state=%@", what, seconds, reads, last)
        case let .pressNotDelivered(id, attempts, before, after):
            return "press on \(id) never reached it (\(attempts) attempts, qa.state did not move); before=\(before) after=\(after)"
        case let .pressTakenElsewhere(id, before, after):
            return "press on \(id) was taken by something else (qa.state moved, presses[\(id)] did not); before=\(before) after=\(after)"
        case let .pressWithoutEffect(id, what, seconds, last):
            return String(format: "press on %@ reached it but '%@' did not hold within %.2f s; last qa.state=%@", id, what, seconds, last)
        }
    }
}

/// The QA accessibility layer seen from XCTest (UI test redesign plan 3.4, S1). It finds GPUI
/// controls by identifier instead of OCR, presses them with a real touch at the element's centre,
/// and confirms every press through `qa.state` (`presses[id]` must grow). It never decides by
/// `isHittable`: S0 showed it stays true while a GPUI menu backdrop covers the control. Waits read
/// the state element's value directly (one read is about 25 ms); XCTNSPredicateExpectation took
/// 1.4-1.5 s per wait in S0. The app must run with YEBISU_QA_AX set (a Simulator build only).
/// Identifiers are constants in crates/yebisu/src/qa_ax_ids.rs; qa-ax-ids-check.py ties the two.
struct QAAx {
    static let state = "qa.state"
    /// Pause between two direct reads of `qa.state`.
    static let readInterval: useconds_t = 50_000

    let app: XCUIApplication

    init(_ app: XCUIApplication) {
        self.app = app
    }

    private static func since(_ start: Date) -> Double { Date().timeIntervalSince(start) }

    func element(_ id: String) -> XCUIElement {
        app.descendants(matching: .any)[id]
    }

    /// One direct read of `qa.state`, or nil when the element is not there or its value is not JSON.
    func read() -> QAState? {
        let element = element(QAAx.state)
        guard element.exists, let value = element.value as? String else { return nil }
        return QAState(value)
    }

    /// Reads `qa.state` until `predicate` holds, at most `timeout` seconds. Throws `.timeout` naming
    /// `what` with the last state read.
    @discardableResult
    func waitState(_ what: String, timeout: TimeInterval = 10, _ predicate: (QAState) -> Bool) throws -> QAState {
        let start = Date()
        var reads = 0
        var last: QAState?
        while true {
            reads += 1
            if let state = read() {
                last = state
                if predicate(state) {
                    print(String(format: "QAAX wait ok '%@' after %.3f s (%ld reads) seq=%ld", what, QAAx.since(start), reads, state.seq))
                    return state
                }
            }
            if QAAx.since(start) >= timeout {
                throw QAAxError.timeout(what: what, seconds: QAAx.since(start), reads: reads, last: last?.text ?? "nil")
            }
            usleep(QAAx.readInterval)
        }
    }

    /// The element `id`, once it exists (XCTest's own existence wait), and only if exactly one
    /// control carries `id`.
    @discardableResult
    func find(_ id: String, timeout: TimeInterval = 10) throws -> XCUIElement {
        let start = Date()
        let element = element(id)
        guard element.waitForExistence(timeout: timeout) else {
            throw QAAxError.notExposed(id: id, seconds: QAAx.since(start), state: read()?.text ?? "nil")
        }
        if let state = read(), state.duplicates.contains(id) {
            throw QAAxError.duplicate(id: id, state: state.text)
        }
        print(String(format: "QAAX find %@ after %.3f s", id, QAAx.since(start)) + " frame=\(element.frame)")
        return element
    }

    /// Presses the control `id` (a tap, or a press held `duration` seconds) at its centre and
    /// waits until `qa.state` shows the press arrived (`presses[id]` grew) and `until` holds.
    /// If nothing at all moved in `qa.state` the touch was lost, and it presses once more; if the
    /// state moved but not `presses[id]`, something else (a sheet, a menu backdrop) took the press,
    /// and it fails without pressing again, because a second press would land on a changed screen.
    @discardableResult
    func press(_ id: String, duration: TimeInterval? = nil, timeout: TimeInterval = 5,
               what: String = "the press's effect", until: ((QAState) -> Bool)? = nil) throws -> QAState {
        let before = try waitState("qa.state before pressing \(id)", timeout: timeout) { _ in true }
        let element = try find(id, timeout: timeout)
        let start = Date()
        for attempt in 1...2 {
            // The element's own coordinate: XCTest resolves the frame, and no hittability check runs.
            let centre = element.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
            if let duration {
                centre.press(forDuration: duration)
            } else {
                centre.tap()
            }
            do {
                let arrived = try waitState("presses[\(id)] > \(before.presses(id))", timeout: timeout) {
                    $0.presses(id) > before.presses(id)
                }
                guard let until else {
                    print(String(format: "QAAX press %@ confirmed after %.3f s (attempt %ld)", id, QAAx.since(start), attempt))
                    return arrived
                }
                do {
                    let done = try waitState(what, timeout: timeout, until)
                    print(String(format: "QAAX press %@ confirmed with '%@' after %.3f s (attempt %ld)", id, what, QAAx.since(start), attempt))
                    return done
                } catch QAAxError.timeout(_, let seconds, _, let last) {
                    throw QAAxError.pressWithoutEffect(id: id, what: what, seconds: seconds, last: last)
                }
            } catch QAAxError.timeout {
                let after = read()
                if let after, after.seq != before.seq {
                    throw QAAxError.pressTakenElsewhere(id: id, before: before.text, after: after.text)
                }
                if attempt == 2 {
                    throw QAAxError.pressNotDelivered(id: id, attempts: attempt, before: before.text, after: after?.text ?? "nil")
                }
                print("QAAX press \(id) lost (qa.state did not move); pressing once more")
            }
        }
        throw QAAxError.pressNotDelivered(id: id, attempts: 2, before: before.text, after: read()?.text ?? "nil")
    }
}
