import XCTest

/// The decisions QAAx.press makes when a press did not show up in `presses[id]` (review SF3 of
/// PR 76): which fields count as "something else took it" (`QAState.takenElsewhere`), and when
/// the press is reported as a relaunch, as taken elsewhere, or as lost (`PressVerdict.of`).
/// Pure: no app is launched, so p9-check.py runs these on every simulator-smoke job.
final class QAAxLogicTests: XCTestCase {
    private func state(_ json: String) throws -> QAState {
        try XCTUnwrap(QAState(json), "not qa.state JSON: \(json)")
    }

    private let base = #"{"seq":4,"launch":10,"screen":"terminal","sheet":null,"menu":null,"presses":{"qa.tabs.more":1},"copied":0}"#

    func testTakenElsewhereIgnoresAQAModesOwnFieldsAndSeq() throws {
        let before = try state(base)
        // A QA mode's field and seq moved, nothing on screen did: not evidence (review F3).
        let after = try state(#"{"seq":9,"launch":10,"screen":"terminal","sheet":null,"menu":null,"presses":{"qa.tabs.more":1},"copied":3}"#)
        XCTAssertEqual(before.takenElsewhere(after, by: "qa.tabs.more"), [])
    }

    func testTakenElsewhereNamesAChangedScreenField() throws {
        let before = try state(base)
        let menu = try state(#"{"seq":5,"launch":10,"screen":"terminal","sheet":null,"menu":"overflow","presses":{"qa.tabs.more":1}}"#)
        XCTAssertEqual(before.takenElsewhere(menu, by: "qa.tabs.more"), ["menu"])
        // From an open menu back to none (the backdrop took the press, F5) is a change too.
        XCTAssertEqual(menu.takenElsewhere(before, by: "qa.tabs.more"), ["menu"])
        let sheet = try state(#"{"seq":5,"launch":10,"screen":"terminal","sheet":"settings","menu":null,"presses":{"qa.tabs.more":1}}"#)
        XCTAssertEqual(before.takenElsewhere(sheet, by: "qa.tabs.more"), ["sheet"])
        // A screen field that disappears or appears counts as moved too (null is not missing).
        let missing = try state(#"{"seq":5,"launch":10,"screen":"terminal","presses":{}}"#)
        XCTAssertEqual(before.takenElsewhere(missing, by: "qa.tabs.more"), ["sheet", "menu"])
        XCTAssertEqual(missing.takenElsewhere(before, by: "qa.tabs.more"), ["sheet", "menu"])
        let screen = try state(#"{"seq":5,"launch":10,"screen":"p0","sheet":null,"menu":null,"presses":{}}"#)
        XCTAssertEqual(before.takenElsewhere(screen, by: "qa.tabs.more"), ["screen"])
    }

    func testTakenElsewhereNamesAnotherControlsPresses() throws {
        let before = try state(base)
        let after = try state(#"{"seq":5,"launch":10,"screen":"terminal","sheet":null,"menu":null,"presses":{"qa.tabs.more":1,"qa.menu.paste":1}}"#)
        XCTAssertEqual(before.takenElsewhere(after, by: "qa.tabs.more"), ["presses[qa.menu.paste]"])
        // The pressed control's own count is never "something else".
        let own = try state(#"{"seq":5,"launch":10,"screen":"terminal","sheet":null,"menu":null,"presses":{"qa.tabs.more":2}}"#)
        XCTAssertEqual(before.takenElsewhere(own, by: "qa.tabs.more"), [])
    }

    func testVerdictRelaunchedWinsOverEverythingElse() throws {
        let before = try state(base)
        let after = try state(#"{"seq":1,"launch":11,"screen":"p0","sheet":null,"menu":null,"presses":{}}"#)
        XCTAssertEqual(PressVerdict.of(before: before, after: after, id: "qa.tabs.more"), .relaunched)
    }

    func testVerdictTakenElsewhereAndLost() throws {
        let before = try state(base)
        let menu = try state(#"{"seq":5,"launch":10,"screen":"terminal","sheet":null,"menu":"overflow","presses":{"qa.tabs.more":1}}"#)
        XCTAssertEqual(PressVerdict.of(before: before, after: menu, id: "qa.tabs.more"), .takenElsewhere(["menu"]))
        let quiet = try state(#"{"seq":8,"launch":10,"screen":"terminal","sheet":null,"menu":null,"presses":{"qa.tabs.more":1},"copied":2}"#)
        XCTAssertEqual(PressVerdict.of(before: before, after: quiet, id: "qa.tabs.more"), .lost)
        XCTAssertEqual(PressVerdict.of(before: before, after: nil, id: "qa.tabs.more"), .lost, "no state read: press once more")
    }

    func testTakenElsewhereNamesTheSystemTakingTheWindow() throws {
        // S4 (F10, run 44226480108): Control Center over the app made the scene inactive and took
        // the press meant for 152 Begin; the verdict names it instead of pressing once more.
        let before = try state(#"{"seq":4,"launch":10,"screen":"terminal","sheet":null,"menu":null,"active":true,"presses":{}}"#)
        let after = try state(#"{"seq":5,"launch":10,"screen":"terminal","sheet":null,"menu":null,"active":false,"presses":{}}"#)
        XCTAssertEqual(before.takenElsewhere(after, by: "qa.sh1-152.begin"), ["active"])
        XCTAssertEqual(PressVerdict.of(before: before, after: after, id: "qa.sh1-152.begin"), .takenElsewhere(["active"]))
        XCTAssertTrue(before.active)
        XCTAssertFalse(after.active)
    }

    func testCountsAndPeerFields() throws {
        let value = try state(#"{"seq":2,"launch":3,"sh152_start":2,"peer":{"inputs":4,"pastes":["sh1-152-start-E1-text-0","sh1-152-clip-E1-text-0"]}}"#)
        XCTAssertEqual(value.count("sh152_start"), 2)
        XCTAssertEqual(value.count("sh152_end"), 0, "a counter never reported is 0")
        XCTAssertEqual(value.peerInputs, 4)
        XCTAssertEqual(value.peerPastes.last, "sh1-152-clip-E1-text-0")
        let empty = try state(#"{"seq":1,"launch":3}"#)
        XCTAssertEqual(empty.peerInputs, 0)
        XCTAssertEqual(empty.peerPastes, [])
        XCTAssertFalse(empty.active)
    }

    func testCoreAndKeyboardFields() throws {
        let value = try state(#"{"seq":1,"launch":3,"keyboard":398.0,"core":{"panes":2,"active":1,"vertical":true,"terminals":["t1","t2"]}}"#)
        XCTAssertEqual(value.keyboard, 398)
        XCTAssertTrue(value.keyboardUp)
        XCTAssertEqual(value.corePanes, 2)
        XCTAssertEqual(value.coreActive, 1)
        XCTAssertEqual(value.coreVertical, true)
        XCTAssertEqual(value.coreTerminal(1), "t2")
        XCTAssertNil(value.coreTerminal(2))
        let none = try state(#"{"seq":1,"launch":3,"keyboard":0,"core":null}"#)
        XCTAssertFalse(none.keyboardUp)
        XCTAssertNil(none.corePanes)
    }
}
