import XCTest
import Vision

// SH1-153 edit menu, P8 native editing, SH1-154 core image.
extension SmokeQA {
    /// Presses `id` and waits for `until` (UI test redesign S4). The QA strip moves with the keyboard,
    /// so the press first waits for the keyboard to settle when something since the last settle may
    /// have moved it (`QAAx.settleIfMoved`). `movesKeyboard`: the press itself may move it (Menu
    /// prepare and Native next focus the terminal and ask for the keyboard), so the next press waits.
    @discardableResult
    func pressSettled(_ qa: QAAx, _ id: String, movesKeyboard: Bool, what: String,
                      until: @escaping (QAState) -> Bool) throws -> QAState {
        try qa.settleIfMoved("before \(id)")
        let state = try qa.press(id, what: what, until: until)
        if movesKeyboard { qa.keyboardMayMove("\(id) focuses the terminal") }
        return state
    }

    /// Logs whether `condition` held within `seconds`: the receiving-system oracle decides the case
    /// from the peer's receipts after the app was terminated, as it did after the old sleeps.
    @discardableResult
    func logWait(_ qa: QAAx, _ what: String, seconds: TimeInterval = 10, _ condition: (QAState) -> Bool) -> QAState? {
        do {
            let state = try qa.waitState(what, timeout: seconds, condition)
            print("EDIT WAIT \(what): reached")
            return state
        } catch {
            print("EDIT WAIT \(what): not reached in \(Int(seconds))s: \(error)")
            return nil
        }
    }

    /// The markers that bracket an SH1-153 or P8 case reached the private peer.
    func markersAtPeer(_ qa: QAAx, _ markers: [String]) {
        logWait(qa, "markers \(markers) at the peer") { state in markers.allSatisfy { state.peerPastes.contains($0) } }
    }

    /// Waits until a UIKit selector the press scheduled (performSelector afterDelay 0.1, `scheduled`
    /// grew) had its turn, and then until no paste is being prepared, so the next case's end marker
    /// follows its effect at the peer. Before S4 the tests slept 1 s here.
    ///
    /// Two waits, not one predicate (PR 81 review S1): S4 waited for both at once, and in the six
    /// cases whose selector never reaches the app's native edit action (`reachesApp` false: P8's
    /// `selectAll:`, and SH1-153's Copy after the selection was cleared, which UIKit refuses) that
    /// wait ran out at 5 s every time, so "no paste pending" was never checked there. Now those
    /// cases give the selector the old 1 s (it ends early if the selector did reach the app), and
    /// "no paste pending" is a wait of its own in every case.
    func scheduledActionsRan(_ qa: QAAx, _ what: String, before: QAState, after: QAState, scheduled: String,
                             reachesApp: Bool) {
        if after.count(scheduled) > before.count(scheduled) {
            let ran = "\(what): the scheduled selector reached the app's native edit action"
            logWait(qa, reachesApp ? ran : ran + " (not expected)", seconds: reachesApp ? 5 : 1) {
                $0.count("native_actions") > before.count("native_actions")
            }
        }
        logWait(qa, "\(what): no paste pending", seconds: 5) { $0.bool("pasting") == false }
    }

    /// Whether SH1-153 case `step`'s scheduled selector reaches the app's native edit action: in
    /// cases 4 and 9 (step % 5 == 4) Execute validated Copy with a selection and then cleared it,
    /// so UIKit refuses Copy (the oracle checks the clipboard did not change).
    static func editSelectorReachesApp(_ step: Int) -> Bool { step % 5 != 4 }

    /// Whether P8 case `step`'s scheduled selector reaches the app's native edit action: cases 3, 9,
    /// 15 and 21 (step % 6 == 3) schedule `selectAll:`, which does not (S4, 3 of 3 Mac mini runs).
    static func nativeSelectorReachesApp(_ step: Int) -> Bool { step % 6 != 3 }

    /// Launches the private-peer terminal with `qaMode` and waits until it can take input.
    func launchPeerTerminal(_ app: XCUIApplication, _ qa: QAAx, _ qaMode: String) throws {
        app.terminate()
        app.launchEnvironment["YEBISU_TERMINAL_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment[qaMode] = "1"
        app.launchEnvironment["YEBISU_QA_AX"] = "1"
        XCUIDevice.shared.orientation = .portrait
        app.launch()
        // Before S4: sleep(3). The QA markers go to the private peer, so the link must be up.
        try qa.waitState("the private peer is connected with an input destination", timeout: 60) {
            $0.string("screen") == "terminal" && $0.bool("connected") == true && $0.string("input_pane") != nil
        }
    }

    /// Menu prepare: ends case `step - 1` (its result and end marker) and starts case `step`.
    func editPrepare(_ qa: QAAx, _ step: Int) throws {
        let before = try qa.waitState("before Menu prepare \(step)", timeout: 5) { _ in true }
        try pressSettled(qa, QAIds.editPrepare, movesKeyboard: true, what: "Menu prepare ran") {
            $0.count("edit_prepare") > before.count("edit_prepare")
        }
        markersAtPeer(qa, (step > 0 ? ["edit-end-\(step - 1)"] : []) + (step < 10 ? ["edit-start-\(step)"] : []))
    }

    func testSH1153EditMenu() throws {
        continueAfterFailure = false
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        let qa = QAAx(app)
        try launchPeerTerminal(app, qa, "YEBISU_EDIT_MENU_QA")
        for step in 0..<10 {
            try editPrepare(qa, step)
            if step % 5 == 1 || step % 5 == 4 {
                let before = try qa.waitState("before Menu select \(step)", timeout: 5) { _ in true }
                try pressSettled(qa, QAIds.editSelect, movesKeyboard: false, what: "edit-selection recorded") {
                    $0.count("edit_selection") > before.count("edit_selection")
                }
            }
            let before = try qa.waitState("before Menu execute \(step)", timeout: 5) { _ in true }
            let after = try pressSettled(qa, QAIds.editExecute, movesKeyboard: false, what: "edit-validation recorded") {
                $0.count("edit_validation") > before.count("edit_validation")
            }
            scheduledActionsRan(qa, "SH1-153 case \(step)", before: before, after: after, scheduled: "edit_scheduled",
                                reachesApp: Self.editSelectorReachesApp(step))
            save("sh1-153-case-\(step)")
            if step == 1 || step == 6 {
                print("SH1-153 MENU HIERARCHY case=\(step)\n\(app.debugDescription)")
                let edit = app.descendants(matching: .any).matching(NSPredicate(format: "label == 'Edit'")).firstMatch
                if edit.exists && edit.isHittable {
                    edit.tap()
                    _ = app.menuItems.firstMatch.waitForExistence(timeout: 2)
                    save("sh1-153-system-edit-\(step)")
                    print("SH1-153 OPEN MENU case=\(step)\n\(app.debugDescription)")
                    // Dismiss without choosing an extra command outside the receipt cases.
                    app.coordinate(withNormalizedOffset: CGVector(dx: 0.9, dy: 0.5)).tap()
                    qa.keyboardMayMove("tap beside the system Edit menu")
                    _ = app.menuItems.firstMatch.waitForNonExistence(timeout: 2)
                } else {
                    print("SH1-153 system Edit menu is not exposed to XCTest in case \(step)")
                }
            }
        }
        try editPrepare(qa, 10)
        XCTAssertEqual(app.state, .runningForeground)
    }

    func testP8NativeEditing() throws {
        continueAfterFailure = false
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        let qa = QAAx(app)
        try launchPeerTerminal(app, qa, "YEBISU_NATIVE_QA")
        // Each Native next ends case step - 1 and starts case step (24 cases, then the last end).
        for step in 0..<25 {
            let before = try qa.waitState("before Native next \(step)", timeout: 5) { _ in true }
            let after = try pressSettled(qa, QAIds.nativeNext, movesKeyboard: true, what: "Native next ran") {
                $0.count("native_step") > before.count("native_step")
            }
            markersAtPeer(qa, (step > 0 ? ["native-end-\(step - 1)"] : []) + (step < 24 ? ["native-start-\(step)"] : []))
            if step < 24 {
                scheduledActionsRan(qa, "P8 case \(step)", before: before, after: after, scheduled: "native_scheduled",
                                    reachesApp: Self.nativeSelectorReachesApp(step))
            }
        }
        save("p8-native")
        XCTAssertEqual(app.state, .runningForeground)
    }

    func testSH154CoreImage() throws {
        continueAfterFailure = false
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        let qa = QAAx(app)
        app.terminate()
        app.launchEnvironment["YEBISU_CORE_QA"] = "1"
        app.launchEnvironment["YEBISU_HANDOFF_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment["YEBISU_QA_AX"] = "1"
        app.launch()
        // S3: the QA band's image controls and the ... menu's Paste and Paste image are pressed
        // by identifier, at the frames the app publishes each frame, so a band the keyboard
        // lifted or a menu that moved is pressed where it is (F4, Mac mini 32f1e21); each step
        // waits for the app's own count in qa.state instead of reading the result off the
        // screen. p9-check validate_images proves what each paste delivered.
        let start = try qa.waitState("the Herdr 2 terminal is connected and drawn", timeout: 60) {
            $0.string("screen") == "terminal" && $0.bool("connected") == true && !$0.isNull("surface")
        }
        try qa.find(QAIds.bandImageOnly, timeout: 30)
        // The terminal's focus raises the software keyboard on some Simulators; it lifts the band.
        try qa.hideKeyboard("SH154 start")
        // The QA pair is laid out left to right on one row (as the OCR boxes were checked before S3).
        let qaImage = try qa.find(QAIds.bandImage).frame
        let qaImageOnly = try qa.find(QAIds.bandImageOnly).frame
        XCTAssertTrue(qaImage.minX < qaImageOnly.minX && abs(qaImage.midY - qaImageOnly.midY) < max(qaImage.height, qaImageOnly.height),
                      "QA Image / QA Image only not left to right on one row: \(qaImage) \(qaImageOnly)")
        func count(_ state: QAState, _ key: String) -> Int { state.int(key) ?? 0 }
        var images = count(start, "qa_images")
        var pastedImages = count(start, "pasted_images")
        var pastedTexts = count(start, "pasted_texts")
        func noFailure(_ state: QAState) -> Bool { count(state, "paste_failures") == 0 }
        // An image put (a 2200x2200 PNG, about 950 KB, to the core mock and back) took 6.1-7.8 s on
        // hosted runners when S3 landed. In PR 81's run 44500184824 the puts that finished took
        // 7.7-9.1 s, and twice one was still running at the old 10 s deadline (attempt 1, job
        // 135478189932; attempt 2, job 135514978584). Whether those two finished is not known:
        // attempt 1's wire record shows only that the put's request reached the core after the test
        // had failed, and attempt 2 kept no artifact. So the measured fact is only that 10 s
        // left too little margin (the slowest put measured to its end took 9.14 s; run 44523467982
        // took 7.1-8.7 s). 30 s is a margin, not a measured bound; a press that never arrives is
        // still noticed within 10 s.
        let imagePut: TimeInterval = 30
        // 1. An image-only item; Paste image and Paste both put it (paste-0.png, paste-1.png).
        try qa.press(QAIds.bandImageOnly, timeout: 30, what: "QA image ready (image only)") {
            count($0, "qa_images") == images + 1
        }
        images += 1
        // Paste and Paste image sit one above the other in the menu's column.
        try openMenu(qa)
        let paste = try qa.find(QAIds.menuPaste).frame
        let pasteImage = try qa.find(QAIds.menuPasteImage).frame
        XCTAssertTrue(paste.midY < pasteImage.midY && abs(paste.minX - pasteImage.minX) < max(paste.height, pasteImage.height),
                      "Paste / Paste image not one above the other in the ... menu: \(paste) \(pasteImage)")
        try qa.press(QAIds.menuPasteImage, timeout: 10, effect: imagePut, what: "Paste image put paste-0.png") {
            $0.isNull("menu") && count($0, "pasted_images") == pastedImages + 1 && noFailure($0)
        }
        pastedImages += 1
        try menuItem(qa, QAIds.menuPaste, effect: imagePut, what: "Paste put paste-1.png (the item has no text)") {
            count($0, "pasted_images") == pastedImages + 1 && noFailure($0)
        }
        pastedImages += 1
        // 2. An item with text and an image: Paste sends the text, Paste image puts paste-2.png.
        try qa.press(QAIds.bandImage, timeout: 30, what: "QA image ready (text and image)") {
            count($0, "qa_images") == images + 1
        }
        try menuItem(qa, QAIds.menuPaste, what: "Paste sent the text") {
            count($0, "pasted_texts") == pastedTexts + 1 && count($0, "pasted_images") == pastedImages && noFailure($0)
        }
        pastedTexts += 1
        let last = try menuItem(qa, QAIds.menuPasteImage, effect: imagePut, what: "Paste image put paste-2.png") {
            count($0, "pasted_images") == pastedImages + 1 && noFailure($0)
        }
        XCTAssertEqual(count(last, "pasted_images"), 3, "\(last)")
        XCTAssertEqual(count(last, "pasted_texts"), 1, "\(last)")
        save("sh154-core-image")
        XCTAssertEqual(app.state, .runningForeground)
    }
}
