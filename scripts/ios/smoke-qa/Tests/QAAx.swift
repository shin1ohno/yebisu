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
    /// The docked keyboard's height in points (0 without one); `keyboardUp` is the software
    /// keyboard (above 120 pt, as the status bar tells it from a hardware keyboard's bar).
    var keyboard: Double { raw["keyboard"] as? Double ?? (raw["keyboard"] as? Int).map(Double.init) ?? -1 }
    var keyboardUp: Bool { keyboard > 120 }
    /// Herdr 2's pane layout (`core`: panes, active, vertical, terminals), nil before the
    /// core worker published one.
    var core: [String: Any]? { raw["core"] as? [String: Any] }
    var corePanes: Int? { core?["panes"] as? Int }
    var coreActive: Int? { core?["active"] as? Int }
    var coreVertical: Bool? { core?["vertical"] as? Bool }
    func coreTerminal(_ index: Int) -> String? {
        guard let terminals = core?["terminals"] as? [String], terminals.indices.contains(index) else { return nil }
        return terminals[index]
    }

    /// A count a QA mode reported (`sh152_start`, `lifecycle_active`, ...), 0 before the first.
    func count(_ key: String) -> Int { raw[key] as? Int ?? 0 }
    /// The window's scene is foreground-active (S4: false behind Control Center).
    var active: Bool { raw["active"] as? Bool ?? false }
    /// What the in-process mock peers received (S4, crates/yebisu/src/qa_ax_peer.rs): input
    /// messages, and the last pasted texts (markers and clipboard texts).
    var peerInputs: Int { (raw["peer"] as? [String: Any])?["inputs"] as? Int ?? 0 }
    var peerPastes: [String] { (raw["peer"] as? [String: Any])?["pastes"] as? [String] ?? [] }

    /// The root fields that say what is on screen and what covers it; a press that changes one
    /// of them was taken by the app, or by the system when `active` went false (S4: Control
    /// Center over the app, run 44226480108).
    static let screenFields = ["screen", "sheet", "menu", "active"]

    /// What in `after` shows that a press meant for `id` reached something else: a changed
    /// screen field, or another marked control's presses growing. A QA mode's own reported
    /// fields (`key_ready`, `copied`, ...) are not evidence, because they can change by
    /// themselves while a touch is lost (review F3); `seq` moves with them, so it is not either.
    func takenElsewhere(_ after: QAState, by id: String) -> [String] {
        var moved = QAState.screenFields.filter { key in
            !((raw[key] as AnyObject?)?.isEqual(after.raw[key]) ?? (after.raw[key] == nil))
        }
        let others = Set(((raw["presses"] as? [String: Any]) ?? [:]).keys)
            .union(((after.raw["presses"] as? [String: Any]) ?? [:]).keys)
            .subtracting([id])
        moved += others.sorted().filter { after.presses($0) > presses($0) }.map { "presses[\($0)]" }
        return moved
    }
}

/// What a press whose `presses[id]` did not grow ran into, from the state before it and the
/// state read after the wait (nil when it could not be read). Pure, so QAAxLogicTests checks
/// each branch without an app (review SF3).
enum PressVerdict: Equatable {
    /// `launch` changed: the app restarted and its counts began again.
    case relaunched
    /// A screen field changed or another control's presses grew: something else took it.
    case takenElsewhere([String])
    /// Nothing that says what is on screen moved: the touch was lost; press once more.
    case lost

    static func of(before: QAState, after: QAState?, id: String) -> PressVerdict {
        guard let after else { return .lost }
        if after.launch != before.launch { return .relaunched }
        let moved = before.takenElsewhere(after, by: id)
        return moved.isEmpty ? .lost : .takenElsewhere(moved)
    }
}

/// Why a QA layer step failed. XCTest prints the description when a test throws it, so each case
/// names the identifier or the wait and carries the last state read.
enum QAAxError: Error, CustomStringConvertible {
    case notExposed(id: String, seconds: Double, state: String)
    case duplicate(id: String, state: String)
    case timeout(what: String, seconds: Double, reads: Int, last: String)
    case pressNotDelivered(id: String, attempts: Int, before: String, after: String)
    case pressTakenElsewhere(id: String, moved: [String], before: String, after: String)
    case relaunched(id: String, before: String, after: String)
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
        case let .pressTakenElsewhere(id, moved, before, after):
            return "press on \(id) was taken by something else (\(moved.joined(separator: ", ")) moved, presses[\(id)] did not); before=\(before) after=\(after)"
        case let .relaunched(id, before, after):
            return "the app relaunched while \(id) was pressed (launch changed, so presses restarted from 0); before=\(before) after=\(after)"
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

    /// The element `id`, once it exists, and only if exactly one control carries `id`. One
    /// `exists` query comes first: `waitForExistence` took 1.08-1.17 s even for an element that
    /// was already there (S0 and the first S1 runs), a plain query about 30 ms.
    @discardableResult
    func find(_ id: String, timeout: TimeInterval = 10) throws -> XCUIElement {
        let start = Date()
        let element = element(id)
        guard element.exists || element.waitForExistence(timeout: timeout) else {
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
    /// If neither `presses[id]` nor what is on screen moved, the touch was lost, and it presses
    /// once more (a QA mode's own fields changing meanwhile do not count, review F3). If a screen
    /// field (`screen`, `sheet`, `menu`) changed or another control's presses grew, something else
    /// (a sheet, a menu backdrop) took the press, and it fails without pressing again, because a
    /// second press would land on a changed screen. A changed `launch` fails by name too (the app
    /// restarted and its counts began again).
    /// `at` is the point in the element's frame (normalized; the centre by default), for a long press
    /// on the terminal or a control part of which an edit menu covers (S4). `timeout` bounds each
    /// wait for the press to arrive; `effect` bounds the wait for `until` once it arrived (default
    /// `timeout`), so a slow effect such as an image put does not also delay noticing a lost press.
    @discardableResult
    func press(_ id: String, duration: TimeInterval? = nil, at offset: CGVector = CGVector(dx: 0.5, dy: 0.5),
               timeout: TimeInterval = 5, effect: TimeInterval? = nil, what: String = "the press's effect",
               until: ((QAState) -> Bool)? = nil) throws -> QAState {
        let before = try waitState("qa.state before pressing \(id)", timeout: timeout) { _ in true }
        let element = try find(id, timeout: timeout)
        let start = Date()
        for attempt in 1...2 {
            // The element's own coordinate: XCTest resolves the frame, and no hittability check runs.
            let centre = element.coordinate(withNormalizedOffset: offset)
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
                    let done = try waitState(what, timeout: effect ?? timeout, until)
                    print(String(format: "QAAX press %@ confirmed with '%@' after %.3f s (attempt %ld)", id, what, QAAx.since(start), attempt))
                    return done
                } catch QAAxError.timeout(_, let seconds, _, let last) {
                    throw QAAxError.pressWithoutEffect(id: id, what: what, seconds: seconds, last: last)
                }
            } catch QAAxError.timeout {
                let after = read()
                switch PressVerdict.of(before: before, after: after, id: id) {
                case .relaunched:
                    throw QAAxError.relaunched(id: id, before: before.text, after: after?.text ?? "nil")
                case let .takenElsewhere(moved):
                    throw QAAxError.pressTakenElsewhere(id: id, moved: moved, before: before.text, after: after?.text ?? "nil")
                case .lost:
                    break
                }
                if attempt == 2 {
                    throw QAAxError.pressNotDelivered(id: id, attempts: attempt, before: before.text, after: after?.text ?? "nil")
                }
                print("QAAX press \(id) lost (presses[\(id)] and the screen fields did not move); pressing once more")
            }
        }
        throw QAAxError.pressNotDelivered(id: id, attempts: 2, before: before.text, after: read()?.text ?? "nil")
    }

    /// Waits until the docked keyboard's height in `qa.state` reads the same twice 0.4 s apart,
    /// so a press does not land while a rising or falling keyboard still moves the controls
    /// above it (Mac mini 32f1e21, 5977881). Returns the settled state.
    @discardableResult
    func settledKeyboard(_ what: String, timeout: TimeInterval = 10) throws -> QAState {
        let start = Date()
        var last = try waitState("qa.state before the keyboard settles (\(what))", timeout: timeout) { _ in true }
        while true {
            usleep(400_000)
            let now = try waitState("qa.state while the keyboard settles (\(what))", timeout: timeout) { _ in true }
            if now.keyboard == last.keyboard {
                print(String(format: "QAAX keyboard settled at %.0f pt after %.3f s (%@)", now.keyboard, QAAx.since(start), what))
                return now
            }
            last = now
            if QAAx.since(start) >= timeout {
                throw QAAxError.timeout(what: "keyboard settles (\(what))", seconds: QAAx.since(start), reads: 0, last: now.text)
            }
        }
    }

    /// Hides the software keyboard if `qa.state` shows it up, with the keyboard's own hide key
    /// (a UIKit element), and waits until `qa.state` shows it down. A press the keyboard did not
    /// take is repeated once (Mac mini f95123d).
    func hideKeyboard(_ what: String) throws {
        let state = try settledKeyboard(what)
        guard state.keyboardUp else {
            print("QAAX \(what): no software keyboard (keyboard \(state.keyboard) pt)")
            return
        }
        let hide = app.keyboards.buttons.matching(NSPredicate(format: "label CONTAINS[c] 'Hide keyboard'")).firstMatch
        for press in 1...2 {
            guard hide.exists else {
                throw QAAxError.timeout(what: "\(what): the keyboard's hide key", seconds: 0, reads: 1, last: state.text)
            }
            hide.tap()
            do {
                try waitState("\(what): software keyboard down", timeout: 5) { !$0.keyboardUp }
                try settledKeyboard(what)
                return
            } catch QAAxError.timeout where press == 1 {
                print("QAAX \(what): software keyboard still up 5 s after hide press 1; pressing once more")
            }
        }
    }
}
