import XCTest
import Vision

// SH1-153 edit menu, P8 native editing, SH1-154 core image.
extension SmokeQA {
    func testSH1153EditMenu() throws {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.terminate()
        app.launchEnvironment["YEBISU_TERMINAL_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment["YEBISU_EDIT_MENU_QA"] = "1"
        XCUIDevice.shared.orientation = .portrait
        app.launch()
        sleep(3)
        for step in 0..<10 {
            try tapText("Menu prepare", in: app)
            sleep(1)
            if step % 5 == 1 || step % 5 == 4 {
                try tapText("Menu select", in: app)
                sleep(1)
            }
            try tapText("Menu execute", in: app)
            sleep(1)
            save("sh1-153-case-\(step)")
            if step == 1 || step == 6 {
                print("SH1-153 MENU HIERARCHY case=\(step)\n\(app.debugDescription)")
                let edit = app.descendants(matching: .any).matching(NSPredicate(format: "label == 'Edit'")).firstMatch
                if edit.exists && edit.isHittable {
                    edit.tap()
                    sleep(1)
                    save("sh1-153-system-edit-\(step)")
                    print("SH1-153 OPEN MENU case=\(step)\n\(app.debugDescription)")
                    // Dismiss without choosing an extra command outside the receipt cases.
                    app.coordinate(withNormalizedOffset: CGVector(dx: 0.9, dy: 0.5)).tap()
                } else {
                    print("SH1-153 system Edit menu is not exposed to XCTest in case \(step)")
                }
            }
        }
        try tapText("Menu prepare", in: app)
        sleep(1)
        XCTAssertEqual(app.state, .runningForeground)
    }

    func testP8NativeEditing() throws {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.terminate()
        app.launchEnvironment["YEBISU_TERMINAL_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment["YEBISU_NATIVE_QA"] = "1"
        XCUIDevice.shared.orientation = .portrait
        app.launch()
        sleep(3)
        for _ in 0..<25 {
            try tapText("Native next", in: app)
            sleep(1)
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
        try qa.find(QAIds.bandImage)
        // The terminal's focus raises the software keyboard on some Simulators; it lifts the band.
        try qa.hideKeyboard("SH154 start")
        func count(_ state: QAState, _ key: String) -> Int { state.int(key) ?? 0 }
        var images = count(start, "qa_images")
        var pastedImages = count(start, "pasted_images")
        var pastedTexts = count(start, "pasted_texts")
        func noFailure(_ state: QAState) -> Bool { count(state, "paste_failures") == 0 }
        // 1. An image-only item; Paste image and Paste both put it (paste-0.png, paste-1.png).
        try qa.press(QAIds.bandImageOnly, timeout: 30, what: "QA image ready (image only)") {
            count($0, "qa_images") == images + 1
        }
        images += 1
        try menuItem(qa, QAIds.menuPasteImage, what: "Paste image put paste-0.png") {
            count($0, "pasted_images") == pastedImages + 1 && noFailure($0)
        }
        pastedImages += 1
        try menuItem(qa, QAIds.menuPaste, what: "Paste put paste-1.png (the item has no text)") {
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
        let last = try menuItem(qa, QAIds.menuPasteImage, what: "Paste image put paste-2.png") {
            count($0, "pasted_images") == pastedImages + 1 && noFailure($0)
        }
        XCTAssertEqual(count(last, "pasted_images"), 3, "\(last)")
        XCTAssertEqual(count(last, "pasted_texts"), 1, "\(last)")
        save("sh154-core-image")
        XCTAssertEqual(app.state, .runningForeground)
    }
}
