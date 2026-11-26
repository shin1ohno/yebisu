import XCTest

/// The decisions QAAx.press makes when a press did not show up in `presses[id]` (review SF3 of
/// PR 76): which fields count as "something else took it" (`QAState.takenElsewhere`), and when
/// the press is reported as a relaunch, as taken elsewhere, as proven lost, as arrived but not
/// counted, or as unproven (`PressVerdict.of`, issue #142 stage 3: by the app's input receipts).
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
        let before = try state(receipts(base, began: 3))
        let menu = try state(#"{"seq":5,"launch":10,"screen":"terminal","sheet":null,"menu":"overflow","presses":{"qa.tabs.more":1}}"#)
        XCTAssertEqual(PressVerdict.of(before: before, after: menu, id: "qa.tabs.more"), .takenElsewhere(["menu"]))
        let quiet = try state(receipts(#"{"seq":8,"launch":10,"screen":"terminal","sheet":null,"menu":null,"presses":{"qa.tabs.more":1},"copied":2}"#, began: 3))
        XCTAssertEqual(PressVerdict.of(before: before, after: quiet, id: "qa.tabs.more"), .lostProven,
                       "no touch receipt and nothing on screen moved: proven lost, press once more")
    }

    /// `base` with qa.state's `input` field (crates/yebisu/src/qa_ax_input.rs) counting `began` touches.
    private func receipts(_ json: String, began: Int, last: String = "null") -> String {
        let input = #","input":{"touch_began":\#(began),"touch_ended":\#(began),"touch_cancelled":0,"menu_perform":0,"# +
            #""menu_performed":0,"menu_delivered":0,"menu_dropped":0,"menu_global":0,"dropped":0,"unknown":0,"last_touch":\#(last)}}"#
        return String(json.dropLast()) + input
    }

    func testVerdictWithoutAReadableStateIsUnprovenNotLost() throws {
        // Before issue #142 an unreadable state counted as lost and the press was sent again.
        let before = try state(receipts(base, began: 3))
        XCTAssertEqual(PressVerdict.of(before: before, after: nil, id: "qa.tabs.more"), .unproven("qa.state unreadable"))
    }

    func testVerdictWithoutInputReceiptsIsUnproven() throws {
        let before = try state(base)
        let quiet = try state(#"{"seq":8,"launch":10,"screen":"terminal","sheet":null,"menu":null,"presses":{"qa.tabs.more":1}}"#)
        XCTAssertEqual(PressVerdict.of(before: before, after: quiet, id: "qa.tabs.more"), .unproven("qa.state has no input receipts"),
                       "no receipts: a lost press cannot be proven, so it fails closed")
    }

    func testVerdictTouchThatArrivedIsNeverPressedAgain() throws {
        // Class F2: the press reached the app 5-9 s late; a resend ran its setup twice. touch_began 3 -> 4
        // with presses[id] unchanged is a touch that arrived and is not lost.
        let before = try state(receipts(base, began: 3))
        let after = try state(receipts(base, began: 4, last: #"{"stage":"began","kind":"direct","x":516.0,"y":559.5}"#))
        XCTAssertEqual(after.presses("qa.tabs.more"), before.presses("qa.tabs.more"))
        XCTAssertEqual(PressVerdict.of(before: before, after: after, id: "qa.tabs.more"),
                       .arrivedNotCounted(lastTouch: "began direct (516.0, 559.5)"))
    }

    /// CI run 45388075978 (sh1-152 E3f-key, E3g-text): UIKit cancelled the touch 0.2 s after it began, so
    /// GPUI got no tap; such a press is proven lost and sent once more. A touch that ended is not.
    func testVerdictTouchThatUIKitCancelledIsProvenLost() throws {
        let before = try counted(["touch_began": 5, "touch_ended": 5])
        let cancelled = try counted(["touch_began": 6, "touch_ended": 5, "touch_cancelled": 1])
        XCTAssertEqual(PressVerdict.of(before: before, after: cancelled, id: "qa.sh1-152.end"),
                       .cancelled(lastTouch: "ended direct (934.0, 117.5)"))
        let ended = try counted(["touch_began": 6, "touch_ended": 6])
        XCTAssertEqual(PressVerdict.of(before: before, after: ended, id: "qa.sh1-152.end"),
                       .arrivedNotCounted(lastTouch: "ended direct (934.0, 117.5)"))
        let both = try counted(["touch_began": 7, "touch_ended": 6, "touch_cancelled": 1])
        XCTAssertEqual(PressVerdict.of(before: before, after: both, id: "qa.sh1-152.end"),
                       .arrivedNotCounted(lastTouch: "ended direct (934.0, 117.5)"), "one touch completed: not lost")
    }

    /// CI run 45574153676 (main d0aa07c; 45585296675 and 45578440109 the same): the first P0 touch began and
    /// UIKit cancelled it 0.15 s later, the press was resent once (press-retry), and the second touch began
    /// and ended. One completed touch plus one cancelled touch answered by one resend agree; the check
    /// before, `touch_began` +1 exactly, failed it (SmokeQA+Terminal.swift:48, "2" is not equal to "1").
    func testPressTouchesAgreeWithTheResendThatAnsweredACancelledTouch() throws {
        let before = try counted([:], screen: "p0")
        let after = try counted(["touch_began": 2, "touch_ended": 1, "touch_cancelled": 1], screen: "p0")
        XCTAssertNil(PressTouches.mismatch(before: before, after: after, resends: 1))
        XCTAssertNotNil(PressTouches.mismatch(before: before, after: after, resends: 0), "a cancelled touch needs its resend")
    }

    func testPressTouchesThatDisagreeAreNamed() {
        XCTAssertNil(PressTouches.mismatch(began: 1, ended: 1, cancelled: 0, resends: 0), "one tap")
        XCTAssertNil(PressTouches.mismatch(began: 1, ended: 1, cancelled: 0, resends: 1), "lost before the app, then resent")
        XCTAssertEqual(PressTouches.mismatch(began: 2, ended: 1, cancelled: 1, resends: 0),
                       "touch_began +2, touch_ended +1, touch_cancelled +1, 0 proven resend(s): a touch UIKit cancelled that no resend answered")
        XCTAssertEqual(PressTouches.mismatch(began: 2, ended: 2, cancelled: 0, resends: 1),
                       "touch_began +2, touch_ended +2, touch_cancelled +0, 1 proven resend(s): one press must end in exactly one touch")
        XCTAssertEqual(PressTouches.mismatch(began: 2, ended: 1, cancelled: 0, resends: 1),
                       "touch_began +2, touch_ended +1, touch_cancelled +0, 1 proven resend(s): a touch began that neither ended nor was cancelled")
        XCTAssertNotNil(PressTouches.mismatch(began: 1, ended: 0, cancelled: 1, resends: 1), "resent, and still no completed touch")
    }

    func testVerdictOrderRelaunchedThenElsewhereThenReceipts() throws {
        let before = try state(receipts(base, began: 3))
        // A changed screen field wins over a grown touch count: something else took it.
        let menu = try state(receipts(#"{"seq":5,"launch":10,"screen":"terminal","sheet":null,"menu":"overflow","presses":{"qa.tabs.more":1}}"#, began: 4))
        XCTAssertEqual(PressVerdict.of(before: before, after: menu, id: "qa.tabs.more"), .takenElsewhere(["menu"]))
        let relaunched = try state(#"{"seq":1,"launch":11,"screen":"p0","sheet":null,"menu":null,"presses":{}}"#)
        XCTAssertEqual(PressVerdict.of(before: before, after: relaunched, id: "qa.tabs.more"), .relaunched)
    }

    /// qa.state with `input` counters set; the rest of the counters are 0.
    private func counted(_ counters: [String: Int], screen: String = "terminal", launch: Int = 10, presses: String = "{}") throws -> QAState {
        let names = ["touch_began", "touch_ended", "touch_cancelled", "menu_perform", "menu_performed", "menu_delivered",
                     "menu_dropped", "menu_global", "dropped", "unknown"]
        let input = names.map { "\"\($0)\":\(counters[$0] ?? 0)" }.joined(separator: ",")
        let last = (counters["touch_began"] ?? 0) > 0 ? #"{"stage":"ended","kind":"direct","x":934.0,"y":117.5}"# : "null"
        return try state(#"{"seq":3,"launch":\#(launch),"screen":"\#(screen)","sheet":null,"menu":null,"active":true,"presses":\#(presses),"# +
                         #""input":{\#(input),"last_touch":\#(last)}}"#)
    }

    /// Issue #142 stage 3, decision table row 2 (class A1): the menu stayed open 5 s after the pick, no
    /// menu-perform, no touch reached the app, nothing on screen moved: the same item is tapped once more.
    func testMenuPickStillOpenWithNothingReceivedIsResentOnce() throws {
        let before = try counted(["touch_began": 2])
        XCTAssertEqual(MenuInputDecision.pick(itemEnabled: true, before: before, after: try counted(["touch_began": 2])), .resend)
        XCTAssertEqual(MenuInputDecision.pick(itemEnabled: false, before: before, after: try counted(["touch_began": 2])),
                       .fail("the item is no longer enabled"))
    }

    /// Row 4: menu-perform no_tag, a failed menu-deliver or menu-global is an SDK or app defect, never resent.
    func testMenuPickThatTheAppDroppedFailsWithoutResend() throws {
        let before = try counted(["touch_began": 2])
        for key in ["menu_dropped", "menu_global"] {
            let after = try counted(["touch_began": 2, "menu_perform": 1, key: 1])
            guard case let .fail(why) = MenuInputDecision.pick(itemEnabled: true, before: before, after: after) else {
                return XCTFail("\(key) grew: must fail without a resend")
            }
            XCTAssertTrue(why.contains("an SDK or app defect") && why.contains(key), why)
        }
        guard case let .fail(why) = MenuInputDecision.pick(itemEnabled: true, before: before, after: try counted(["touch_began": 2, "menu_perform": 1])) else {
            return XCTFail("a menu-perform reached the app: the pick was not lost")
        }
        XCTAssertEqual(why, "the app received menu-perform")
    }

    /// A pick tap that reached the app (the offset-pick control) is not lost: it fails by name.
    func testMenuPickTapThatReachedTheAppFailsByName() throws {
        let decision = MenuInputDecision.pick(itemEnabled: true, before: try counted(["touch_began": 2]), after: try counted(["touch_began": 3]))
        XCTAssertEqual(decision, .fail("pick tap reached the app (touch_began +1, last_touch ended direct (934.0, 117.5))"))
        let moved = MenuInputDecision.pick(itemEnabled: true, before: try counted([:]), after: try counted([:], screen: "p0"))
        XCTAssertEqual(moved, .fail("the screen changed (screen)"))
    }

    /// Without receipts or a readable state nothing proves the loss: fail closed.
    func testMenuDecisionsFailClosedWithoutReceipts() throws {
        let before = try counted([:])
        XCTAssertEqual(MenuInputDecision.pick(itemEnabled: true, before: before, after: nil), .fail("qa.state unreadable"))
        XCTAssertEqual(MenuInputDecision.pick(itemEnabled: true, before: nil, after: before), .fail("qa.state unreadable"))
        let bare = try state(base)
        XCTAssertEqual(MenuInputDecision.open(before: bare, after: bare), .fail("qa.state has no input receipts"))
        XCTAssertEqual(MenuInputDecision.open(before: before, after: try counted([:], launch: 11)), .fail("the app relaunched"))
    }

    /// Row 5 (class M): the menu item never appeared, no touch reached the app and nothing moved: the
    /// open tap is sent once more. A touch the app received means the open tap missed the menu bar.
    func testMenuOpenTapLostIsResentOnlyWhenNothingReachedTheApp() throws {
        let before = try counted(["touch_began": 2])
        XCTAssertEqual(MenuInputDecision.open(before: before, after: try counted(["touch_began": 2])), .resend)
        XCTAssertEqual(MenuInputDecision.open(before: before, after: try counted(["touch_began": 3])),
                       .fail("open tap reached the app (touch_began +1, last_touch ended direct (934.0, 117.5))"))
        XCTAssertEqual(MenuInputDecision.open(before: before, after: try counted(["touch_began": 2], presses: #"{"qa.p12.empty":1}"#)),
                       .fail("the screen changed (presses[qa.p12.empty])"))
    }

    /// Issue #142 stage 4 (class K): Home is pressed once more only when the app never went inactive
    /// and XCTest still sees it in the foreground.
    func testHomeIsResentOnlyWhenTheAppNeverWentInactive() throws {
        let before = try state(#"{"seq":4,"launch":10,"lifecycle_inactive":1,"presses":{}}"#)
        let unchanged = try state(#"{"seq":5,"launch":10,"lifecycle_inactive":1,"presses":{}}"#)
        XCTAssertEqual(HomeDecision.of(before: before, after: unchanged, appForeground: true), .resend)
        let inactive = try state(#"{"seq":6,"launch":10,"lifecycle_inactive":2,"presses":{}}"#)
        XCTAssertEqual(HomeDecision.of(before: before, after: inactive, appForeground: true),
                       .noResend("lifecycle_inactive grew: Home reached the app"))
        XCTAssertEqual(HomeDecision.of(before: before, after: unchanged, appForeground: false),
                       .noResend("the app is not in the foreground"))
        XCTAssertEqual(HomeDecision.of(before: before, after: nil, appForeground: true),
                       .noResend("qa.state unreadable (the app may have left)"))
        let relaunched = try state(#"{"seq":1,"launch":11,"lifecycle_inactive":0,"presses":{}}"#)
        XCTAssertEqual(HomeDecision.of(before: before, after: relaunched, appForeground: true), .noResend("the app relaunched"))
    }

    /// Class T: where the last touch landed against the pressed point.
    func testTouchClassNamesWhereTheTouchLanded() {
        let pressed = CGPoint(x: 100, y: 100)
        XCTAssertEqual(QAAx.touchClass(expected: pressed, last: nil), "no touch reached the app")
        XCTAssertEqual(QAAx.touchClass(expected: pressed, last: CGPoint(x: 50, y: 50)), "touch at about half the coordinates")
        XCTAssertEqual(QAAx.touchClass(expected: pressed, last: CGPoint(x: 200, y: 200)), "touch at about double the coordinates")
        XCTAssertEqual(QAAx.touchClass(expected: pressed, last: CGPoint(x: 300, y: 40)), "touch elsewhere")
        XCTAssertEqual(QAAx.touchClass(expected: pressed, last: CGPoint(x: 103, y: 98)), "touch at the pressed point")
    }

    func testInputReceiptFields() throws {
        let value = try state(receipts(base, began: 2, last: #"{"stage":"ended","kind":"pointer","x":10,"y":20.5}"#))
        XCTAssertEqual(value.inputCount("touch_began"), 2)
        XCTAssertEqual(value.inputCount("menu_perform"), 0)
        XCTAssertEqual(value.inputCount("no_such_counter"), 0)
        XCTAssertEqual(value.lastTouch?.stage, "ended")
        XCTAssertEqual(value.lastTouch?.kind, "pointer")
        XCTAssertEqual(value.lastTouch?.x, 10)
        XCTAssertEqual(value.lastTouch?.y, 20.5)
        XCTAssertEqual(value.lastTouchText, "ended pointer (10.0, 20.5)")
        let none = try state(base)
        XCTAssertNil(none.input)
        XCTAssertNil(none.lastTouch)
        XCTAssertEqual(none.lastTouchText, "none")
        XCTAssertEqual(try state(receipts(base, began: 0)).lastTouchText, "none")
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

    /// Issue #198: a wait reads once more only when its last read began before the bound and
    /// returned after it (run 45734485407: began at 4.6 s of a 7 s wait, returned at 17.57 s),
    /// and only once.
    func testWaitReadsOnceMoreOnlyAfterAReadThatStraddledTheBound() {
        XCTAssertTrue(QAAx.readAgain(began: 4.6, returned: 17.57, timeout: 7, readAgainBefore: false))
        XCTAssertFalse(QAAx.readAgain(began: 4.6, returned: 17.57, timeout: 7, readAgainBefore: true), "only once")
        XCTAssertFalse(QAAx.readAgain(began: 7.0, returned: 7.04, timeout: 7, readAgainBefore: false),
                       "a read that began at the bound is past it")
        XCTAssertFalse(QAAx.readAgain(began: 3.0, returned: 3.04, timeout: 7, readAgainBefore: false),
                       "before the bound the wait goes on anyway")
    }
}
