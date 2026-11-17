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
        // Locate the four strip controls once, in the first frame, and tap the recorded
        // points from here on. Re-reading the strip after a "QA image ready" repaint lets
        // Vision drop the "Paste" label or return a truncated "Paste image", and the tap
        // then starts the wrong paste (L15 no control found, L16 text=0; main 2ed2beb,
        // job 132955457568). The handoff row never moves between taps (handoff_view.rs,
        // handoff_qa.rs), and p9-check validate_images proves what each tap did.
        let qaImageOnly = try tapText("QA Image only", in: app, tap: false, attempts: 15).box
        let qaImage = try tapText("QA Image", in: app, tap: false, excludingNextWord: "only").box
        let paste = try tapText("Paste", in: app, tap: false, excludingNextWord: "image").box
        let pasteImage = try tapText("Paste image", in: app, tap: false).box
        // A misread box would otherwise surface only as put/path/text counts at the end:
        // each pair is laid out left to right on one row, so reject a box that is not.
        for (left, right, what) in [(qaImage, qaImageOnly, "QA Image / QA Image only"),
                                    (paste, pasteImage, "Paste / Paste image")] {
            XCTAssertTrue(left.minX < right.minX && abs(left.midY - right.midY) < max(left.height, right.height),
                          "\(what) not located left to right on one row: \(left) \(right)")
        }
        tapRecorded(qaImageOnly, "QA Image only", in: app)
        try tapText("QA image ready", in: app, tap: false, attempts: 30)
        tapRecorded(pasteImage, "Paste image", in: app)
        try tapText("paste-0.png", in: app, tap: false, attempts: 30)
        tapRecorded(paste, "Paste", in: app)
        try tapText("paste-1.png", in: app, tap: false, attempts: 30)
        tapRecorded(qaImage, "QA Image", in: app)
        try tapText("QA image ready", in: app, tap: false, attempts: 30)
        tapRecorded(paste, "Paste", in: app)
        sleep(1)
        tapRecorded(pasteImage, "Paste image", in: app)
        try tapText("paste-2.png", in: app, tap: false, attempts: 30)
        save("sh154-core-image")
        XCTAssertEqual(app.state, .runningForeground)
    }
}
