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
    func windowFrame(_ id: String) -> CGRect? {
        guard let frames = raw["frames"] as? [String: [Double]],
              let values = frames[id], values.count == 4, values.allSatisfy(\.isFinite),
              values[2] > 0, values[3] > 0 else { return nil }
        return CGRect(x: values[0], y: values[1], width: values[2], height: values[3])
    }

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
    /// herdr-uikit's input receipts as the app counted them (issue #142 stage 2,
    /// crates/yebisu/src/qa_ax_input.rs): nil when this launch publishes none.
    var input: [String: Any]? { raw["input"] as? [String: Any] }
    /// One `input` counter (`touch_began`, `menu_perform`, ...), 0 when absent.
    func inputCount(_ key: String) -> Int { input?[key] as? Int ?? 0 }
    /// The last touch that reached the app's drawing view, in window points.
    var lastTouch: (stage: String, kind: String, x: Double, y: Double)? {
        guard let touch = input?["last_touch"] as? [String: Any],
              let stage = touch["stage"] as? String, let kind = touch["kind"] as? String,
              let x = (touch["x"] as? NSNumber)?.doubleValue, let y = (touch["y"] as? NSNumber)?.doubleValue
        else { return nil }
        return (stage, kind, x, y)
    }
    /// `lastTouch` for a failure message: "began direct (516.0, 559.5)", or "none".
    var lastTouchText: String {
        lastTouch.map { String(format: "%@ %@ (%.1f, %.1f)", $0.stage, $0.kind, $0.x, $0.y) } ?? "none"
    }

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
/// settled state read after the wait (nil when it could not be read). Pure, so QAAxLogicTests
/// checks each branch without an app (review SF3). Issue #142 stage 3: a timeout only says the
/// press has not arrived yet, so it counts as lost only when the app's input receipts (`input`,
/// crates/yebisu/src/qa_ax_input.rs) show no touch reached the drawing view either.
enum PressVerdict: Equatable {
    /// `launch` changed: the app restarted and its counts began again.
    case relaunched
    /// A screen field changed or another control's presses grew: something else took it.
    case takenElsewhere([String])
    /// No touch reached the app and nothing on screen moved: proven lost; press once more.
    case lostProven
    /// A touch reached the app's drawing view but `presses[id]` did not grow: it is still being
    /// handled, or it landed elsewhere. Never pressed again (class F2: a late press that was
    /// resent ran its setup twice).
    case arrivedNotCounted(lastTouch: String)
    /// The state cannot prove the press was lost (unreadable, or no receipts): fail, no resend.
    case unproven(String)

    static func of(before: QAState, after: QAState?, id: String) -> PressVerdict {
        guard let after else { return .unproven("qa.state unreadable") }
        if after.launch != before.launch { return .relaunched }
        let moved = before.takenElsewhere(after, by: id)
        if !moved.isEmpty { return .takenElsewhere(moved) }
        guard after.input != nil else { return .unproven("qa.state has no input receipts") }
        if after.inputCount("touch_began") > before.inputCount("touch_began") {
            return .arrivedNotCounted(lastTouch: after.lastTouchText)
        }
        return .lostProven
    }
}

/// What P12 does when an OS menu input shows no result (issue #142 stage 3, its decision table).
/// `before` is qa.state read before the menu was revealed (no app query runs while the menu is
/// open unless an input is missing), `after` the settled state read once the input showed nothing.
/// Pure, and mirrored by p12-oracle.py's `pick_decision` and `open_decision`, so QAAxLogicTests and
/// test_p12_oracle.py reproduce each row. A pick whose menu closed is judged by the oracle from the
/// app's receipts (a callback, or the stage where it stopped, class A2); here only a menu that
/// stayed open is decided.
enum MenuInputDecision: Equatable {
    /// Proven lost before it reached the app (A1 for a pick, M for the open tap): send it once more.
    case resend
    /// Not proven lost, or the app or SDK took it and dropped it: fail by name, no resend.
    case fail(String)

    /// The receipts of `before` and `after` that say the input reached the app, or why they cannot
    /// prove anything. nil when the input reached nothing and nothing on screen moved.
    private static func reached(before: QAState?, after: QAState?) -> String? {
        guard let before, let after else { return "qa.state unreadable" }
        guard before.input != nil, after.input != nil else { return "qa.state has no input receipts" }
        if after.launch != before.launch { return "the app relaunched" }
        let dropped = ["menu_dropped", "menu_global"].filter { after.inputCount($0) > before.inputCount($0) }
        if !dropped.isEmpty {
            return "the command reached the app and stopped there (\(dropped.joined(separator: ", ")) grew): an SDK or app defect"
        }
        let touches = after.inputCount("touch_began") - before.inputCount("touch_began")
        if touches > 0 { return "tap reached the app (touch_began +\(touches), last_touch \(after.lastTouchText))" }
        if after.inputCount("menu_perform") > before.inputCount("menu_perform") { return "the app received menu-perform" }
        let moved = before.takenElsewhere(after, by: "")
        if !moved.isEmpty { return "the screen changed (\(moved.joined(separator: ", ")))" }
        return nil
    }

    /// The menu is still open 5 s after the pick tap and the same item is `itemEnabled` (row 2):
    /// resend only when no touch, no menu-perform and no screen change reached the app (A1).
    static func pick(itemEnabled: Bool, before: QAState?, after: QAState?) -> MenuInputDecision {
        if let why = reached(before: before, after: after) { return .fail(why.hasPrefix("tap reached") ? "pick " + why : why) }
        return itemEnabled ? .resend : .fail("the item is no longer enabled")
    }

    /// The menu item never appeared after the open tap (row 5): resend the open tap only when no
    /// touch and no screen change reached the app (class M).
    static func open(before: QAState?, after: QAState?) -> MenuInputDecision {
        if let why = reached(before: before, after: after) { return .fail(why.hasPrefix("tap reached") ? "open " + why : why) }
        return .resend
    }
}

/// Why a QA layer step failed. XCTest prints the description when a test throws it, so each case
/// names the identifier or the wait and carries the last state read.
enum QAAxError: Error, CustomStringConvertible {
    case notExposed(id: String, seconds: Double, state: String)
    case duplicate(id: String, state: String)
    case timeout(what: String, seconds: Double, reads: Int, last: String)
    case pressNotDelivered(id: String, attempts: Int, reason: String, before: String, after: String)
    case pressArrivedElsewhere(id: String, lastTouch: String, before: String, after: String)
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
        case let .pressNotDelivered(id, attempts, reason, before, after):
            return "press on \(id) never reached it (\(attempts) attempts, \(reason)); before=\(before) after=\(after)"
        case let .pressArrivedElsewhere(id, lastTouch, before, after):
            return "press on \(id): a touch reached the app (last_touch \(lastTouch)) but presses[\(id)] did not grow, also after "
                + "waiting once more; it was not pressed again (a resend would repeat its effect, class F2); before=\(before) after=\(after)"
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

    /// What one QAAx (one launch) remembers between calls; a class, so the copies of the struct
    /// that tests pass around share it.
    final class Session {
        /// Something since the last settled read may have moved the software keyboard (UI test
        /// redesign S5, PR 81 review S2): `settleIfMoved` waits only then. True at launch.
        var keyboardMayMove = true
        /// Why each press that was lost was sent once more, as `id: reason` (P12 logs these as its
        /// `fixture-retry` records).
        var retries: [String] = []
        /// Negative controls: the first press of these identifiers is not sent, so the lost-press
        /// path (press once more) runs (P12's `drop-tap:<step>`). Each is spent once.
        var dropFirstPress: Set<String> = []
        /// Direct reads of `qa.state` since launch and the seconds they took (UI test redesign S6,
        /// S5 follow-up S6-1: P12 counts the reads it makes while a request is held).
        var reads = 0
        var readSeconds = 0.0
    }

    let app: XCUIApplication
    let session = Session()
    private let useWindowFrames: Bool

    init(_ app: XCUIApplication, useWindowFrames: Bool = false) {
        self.app = app
        self.useWindowFrames = useWindowFrames
    }

    private static func since(_ start: Date) -> Double { Date().timeIntervalSince(start) }

    func element(_ id: String) -> XCUIElement {
        app.descendants(matching: .any)[id]
    }

    /// One direct read of `qa.state`, or nil when the element is not there or its value is not JSON.
    func read() -> QAState? {
        let start = Date()
        defer {
            session.reads += 1
            session.readSeconds += QAAx.since(start)
        }
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

    /// Reads `qa.state` until `seq` has not moved for `quiet` seconds, so the app has drawn what the
    /// last input caused (issue #142 stage 3), and returns that state. Throws `.timeout` when it does
    /// not hold still, or cannot be read, within `timeout`. `interval` is the pause between reads
    /// (P12's presses while a request is held read every 250 ms).
    @discardableResult
    func settled(_ what: String, quiet: TimeInterval = 0.5, timeout: TimeInterval = 5,
                 interval: useconds_t = QAAx.readInterval) throws -> QAState {
        let start = Date()
        var reads = 0
        var last: QAState?
        var still = Date()
        while true {
            reads += 1
            if let state = read() {
                if state.seq != last?.seq { still = Date() }
                last = state
                if Date().timeIntervalSince(still) >= quiet {
                    print(String(format: "QAAX settled '%@' after %.3f s (%ld reads) seq=%ld", what, QAAx.since(start), reads, state.seq))
                    return state
                }
            }
            if QAAx.since(start) >= timeout {
                throw QAAxError.timeout(what: "qa.state settles (\(what))", seconds: QAAx.since(start), reads: reads, last: last?.text ?? "nil")
            }
            usleep(interval)
        }
    }

    /// `settled`, or nil with the reason when it could not be read or did not hold still.
    func settledOrReason(_ what: String, interval: useconds_t = QAAx.readInterval) -> (state: QAState?, reason: String) {
        do {
            return (try settled(what, interval: interval), "")
        } catch {
            return (nil, "\(error)")
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

    private func windowFrame(_ id: String) throws -> CGRect {
        let state = try waitState("active window frame for \(id)") { $0.active && $0.windowFrame(id) != nil }
        return try XCTUnwrap(state.windowFrame(id))
    }

    func screenFrame(_ element: XCUIElement, id: String) throws -> CGRect {
        guard useWindowFrames else { return element.frame }
        let origin = app.frame.origin
        return try windowFrame(id).offsetBy(dx: origin.x, dy: origin.y)
    }

    func coordinate(_ element: XCUIElement, id: String, at offset: CGVector) throws -> XCUICoordinate {
        guard useWindowFrames else { return element.coordinate(withNormalizedOffset: offset) }
        let frame = try windowFrame(id)
        let origin = app.frame.origin
        let expected = frame.offsetBy(dx: origin.x, dy: origin.y)
        let reported = element.frame
        let framesAgree = abs(reported.minX - expected.minX) < 1 && abs(reported.minY - expected.minY) < 1 &&
            abs(reported.width - expected.width) < 1 && abs(reported.height - expected.height) < 1
        let anchor = framesAgree ? "element" : "window"
        print("QAAX input frame \(id): XCTest=\(reported) GPUI in screen=\(expected) \(anchor)")
        if framesAgree {
            return element.coordinate(withNormalizedOffset: offset)
        }
        // In CI run 44787126992 the application frame was correct while the QA element's origin
        // and size were halved. Use the painted bounds for the touch if the AX frames disagree.
        return app.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(
            dx: frame.minX + frame.width * offset.dx,
            dy: frame.minY + frame.height * offset.dy
        ))
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
        /// The press arrived (`arrived`); waits for `until`, if any.
        func confirm(_ arrived: QAState, attempt: Int) throws -> QAState {
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
        }
        for attempt in 1...2 {
            let centre = try coordinate(element, id: id, at: offset)
            if attempt == 1 && session.dropFirstPress.remove(id) != nil {
                print("QAAX press \(id): injected fault, the first press is not sent")
            } else if let duration {
                centre.press(forDuration: duration)
            } else {
                centre.tap()
            }
            do {
                let arrived = try waitState("presses[\(id)] > \(before.presses(id))", timeout: timeout) {
                    $0.presses(id) > before.presses(id)
                }
                return try confirm(arrived, attempt: attempt)
            } catch QAAxError.timeout {
                // Issue #142 stage 3: decide on the state the app settled in, with its input receipts.
                let (after, unsettled) = settledOrReason("after the press on \(id)")
                switch PressVerdict.of(before: before, after: after, id: id) {
                case .relaunched:
                    throw QAAxError.relaunched(id: id, before: before.text, after: after?.text ?? "nil")
                case let .takenElsewhere(moved):
                    throw QAAxError.pressTakenElsewhere(id: id, moved: moved, before: before.text, after: after?.text ?? "nil")
                case let .unproven(reason):
                    let why = unsettled.isEmpty ? reason : reason + ": " + unsettled
                    throw QAAxError.pressNotDelivered(id: id, attempts: attempt, reason: "not proven lost, so not pressed again: " + why,
                                                      before: before.text, after: after?.text ?? "nil")
                case let .arrivedNotCounted(lastTouch):
                    print("QAAX press \(id): a touch reached the app (last_touch \(lastTouch)); waiting once more, not pressing again")
                    do {
                        let arrived = try waitState("presses[\(id)] > \(before.presses(id)) after a touch reached the app", timeout: timeout) {
                            $0.presses(id) > before.presses(id)
                        }
                        return try confirm(arrived, attempt: attempt)
                    } catch QAAxError.timeout(_, _, _, let last) {
                        throw QAAxError.pressArrivedElsewhere(id: id, lastTouch: lastTouch, before: before.text, after: last)
                    }
                case .lostProven:
                    break
                }
                if attempt == 2 {
                    throw QAAxError.pressNotDelivered(id: id, attempts: attempt, reason: "proven lost twice: no touch receipt, qa.state did not move",
                                                      before: before.text, after: after?.text ?? "nil")
                }
                let reason = String(format: "presses[%@], input.touch_began and the screen fields did not move within %.0f s of the press", id, timeout)
                session.retries.append("\(id): \(reason)")
                print("QAAX press \(id) lost (\(reason)); pressing once more")
            }
        }
        throw QAAxError.pressNotDelivered(id: id, attempts: 2, reason: "proven lost twice", before: before.text, after: read()?.text ?? "nil")
    }

    /// Presses `id`, a marked empty space with no handler, until GPUI counts a press on it (at most
    /// `presses`, 3 s each), and returns how many it took. The first touch after a launch can reach
    /// nothing although `qa.state` already shows the screen: PR 84 run 44618790826 (edit-menu,
    /// E3f-text) pressed Begin twice 5.8 s apart while the screen recording showed the relaunched
    /// window still black with only the keyboard drawn, and `qa.state` stayed at seq 7; S5's P12
    /// trial and S4's first SH1-153 press were the same class. A control with an effect would be
    /// pressed once more only, so this empty space is pressed first (as P12's `touchReady` does).
    /// Negative control: QA_TOUCH_READY_DROP=<n> (TEST_RUNNER_ from the driver) does not send the
    /// first n presses of the first call in the process.
    @discardableResult
    func touchReady(_ id: String, what: String, presses: Int = 8) throws -> Int {
        let start = Date()
        let empty = try find(id, timeout: 10)
        var drop = QAAx.touchReadyDrop
        QAAx.touchReadyDrop = 0
        for attempt in 1...presses {
            let before = try waitState("qa.state before touch-ready press \(attempt) (\(what))", timeout: 5) { _ in true }
            if drop > 0 {
                drop -= 1
                print("QAAX touch-ready \(what): injected fault, press \(attempt) is not sent")
            } else {
                try coordinate(empty, id: id, at: CGVector(dx: 0.5, dy: 0.5)).tap()
            }
            if (try? waitState("touch-ready press \(attempt) reached GPUI (\(what))", timeout: 3, { $0.presses(id) > before.presses(id) })) != nil {
                print(String(format: "QAAX touch-ready %@ after %ld press(es), %.2f s", what, attempt, QAAx.since(start)))
                return attempt
            }
            print("QAAX touch-ready \(what): press \(attempt) reached nothing (presses[\(id)] did not grow in 3 s)")
        }
        throw QAAxError.pressNotDelivered(id: id, attempts: presses, reason: "touch-ready: presses[\(id)] did not grow",
                                          before: "touch-ready \(what)", after: read()?.text ?? "nil")
    }

    /// QA_TOUCH_READY_DROP, spent by the first `touchReady` of the process.
    static var touchReadyDrop = Int(ProcessInfo.processInfo.environment["QA_TOUCH_READY_DROP"] ?? "") ?? 0

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
                session.keyboardMayMove = false
                return now
            }
            last = now
            if QAAx.since(start) >= timeout {
                throw QAAxError.timeout(what: "keyboard settles (\(what))", seconds: QAAx.since(start), reads: 0, last: now.text)
            }
        }
    }

    /// Notes that `why` (a launch, a focus change, a new scene, a gesture) may move the software
    /// keyboard, so the next `settleIfMoved` waits for it to stop.
    func keyboardMayMove(_ why: String) {
        session.keyboardMayMove = true
        print("QAAX keyboard may move: \(why)")
    }

    /// `settledKeyboard`, but only when something since the last settled read may have moved the
    /// keyboard (`keyboardMayMove`). PR 81 review S2: the edit-menu job settled before all 227 of
    /// its presses, about 115 s of its 768 s, and in the three Mac mini runs of 48c8b55 (681
    /// settles) no settle ever saw the keyboard move; between settles it moved only after a
    /// launch, Begin, Fresh, Mutate, Restore, the E1 raise and a long press. Returns nil when
    /// nothing could have moved it.
    @discardableResult
    func settleIfMoved(_ what: String, timeout: TimeInterval = 10) throws -> QAState? {
        guard session.keyboardMayMove else {
            print("QAAX keyboard settle not needed (\(what)): nothing since the last settle moves it")
            return nil
        }
        return try settledKeyboard(what, timeout: timeout)
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
