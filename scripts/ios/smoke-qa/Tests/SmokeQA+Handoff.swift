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
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.terminate()
        app.launchEnvironment["YEBISU_CORE_QA"] = "1"
        app.launchEnvironment["YEBISU_HANDOFF_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launch()
        // Locate the two QA strip controls and the ... menu's Paste and Paste image once,
        // and tap the recorded points from here on. Re-reading after a "QA image ready"
        // repaint lets Vision drop the "Paste" label or return a truncated "Paste image",
        // and the tap then starts the wrong paste (L15 no control found, L16 text=0; main
        // 2ed2beb, job 132955457568). L2b moved Paste and Paste image from the handoff
        // band to the tab row's ... menu (shell/overflow_menu.rs): the menu is anchored
        // under the tab row's right edge and lists its items in a fixed order, so neither
        // the QA strip nor the menu moves between taps, and p9-check validate_images
        // proves what each tap did.
        // Wait for the QA strip (the app is connected), then hide the software keyboard the
        // terminal's focus raises on some Simulators: it lifts the strip while it rises, and
        // two boxes read across that move land on two rows. Read both after it is gone.
        _ = try tapText("QA Image only", in: app, tap: false, attempts: 15)
        hideSoftwareKeyboard(in: app, "SH154 start")
        let qaImageOnly = try tapText("QA Image only", in: app, tap: false).box
        let qaImage = try tapText("QA Image", in: app, tap: false, excludingNextWord: "only").box
        // A misread box would otherwise surface only as put/path/text counts at the end:
        // the QA pair is laid out left to right on one row, the menu items one above the
        // other in one column, so reject a box that is not.
        for (left, right, what) in [(qaImage, qaImageOnly, "QA Image / QA Image only")] {
            XCTAssertTrue(left.minX < right.minX && abs(left.midY - right.midY) < max(left.height, right.height),
                          "\(what) not located left to right on one row: \(left) \(right)")
        }
        let more = try openOverflowMenu(in: app)
        let paste = try tapText("Paste", in: app, tap: false, excludingNextWord: "image").box
        let pasteImage = try tapText("Paste image", in: app, tap: false).box
        XCTAssertTrue(paste.midY > pasteImage.midY && abs(paste.minX - pasteImage.minX) < max(paste.height, pasteImage.height),
                      "Paste / Paste image not located one above the other in the ... menu: \(paste) \(pasteImage)")
        // Close the menu; its backdrop takes this press (overflow_menu.rs).
        app.coordinate(withNormalizedOffset: .zero).withOffset(more).tap()
        sleep(1)
        tapRecorded(qaImageOnly, "QA Image only", in: app)
        try tapText("QA image ready", in: app, tap: false, attempts: 30)
        try openOverflowMenu(in: app, at: more)
        tapRecorded(pasteImage, "Paste image", in: app)
        try tapText("paste-0.png", in: app, tap: false, attempts: 30)
        try openOverflowMenu(in: app, at: more)
        tapRecorded(paste, "Paste", in: app)
        try tapText("paste-1.png", in: app, tap: false, attempts: 30)
        tapRecorded(qaImage, "QA Image", in: app)
        try tapText("QA image ready", in: app, tap: false, attempts: 30)
        try openOverflowMenu(in: app, at: more)
        tapRecorded(paste, "Paste", in: app)
        sleep(1)
        try openOverflowMenu(in: app, at: more)
        tapRecorded(pasteImage, "Paste image", in: app)
        try tapText("paste-2.png", in: app, tap: false, attempts: 30)
        save("sh154-core-image")
        XCTAssertEqual(app.state, .runningForeground)
    }
}
