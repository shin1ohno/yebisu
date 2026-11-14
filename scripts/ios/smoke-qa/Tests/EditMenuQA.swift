import XCTest
import Vision

final class EditMenuQA: XCTestCase {
    func testSH1152Basic() throws {
        for name in ["E1-text", "E1-key", "E2-text", "E2-key", "E1-core", "E2-core"] {
            let app = launch(name)
            try tap("152 Begin", app)
            sleep(2)
            if name.hasPrefix("E2") { try tap("152 Select", app); sleep(1) }
            try menu(app, name, selected: name.hasPrefix("E2"))
            let action = name.hasPrefix("E2") ? "Copy" : "Paste"
            XCTAssertTrue(app.menuItems[action].isEnabled)
            app.menuItems[action].tap()
            sleep(2)
            try tap("152 End", app)
            sleep(1)
            save(name)
            app.terminate()
        }
    }

    func testSH1152Transitions() throws {
        for name in ["E3a", "E3b", "E3c", "E3d", "E3e", "E3f-text", "E3f-key", "E3g-text", "E3g-key", "E3h", "E3i", "E3a-core"] {
            let app = launch(name)
            try tap("152 Begin", app)
            sleep(2)
            let selected = name == "E3c" || name.hasPrefix("E3f")
            if selected { try tap("152 Select", app); sleep(1) }
            try menu(app, name, selected: selected)
            if name == "E3d" {
                XCUIDevice.shared.press(.home)
                sleep(2)
                app.activate()
                sleep(4)
            } else if name == "E3e" {
                let top = app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.001))
                let bottom = app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.8))
                top.press(forDuration: 0.1, thenDragTo: bottom)
                sleep(2)
                bottom.press(forDuration: 0.1, thenDragTo: top)
                sleep(2)
            } else {
                if name == "E3h" {
                    print("SH1-152 HIDE KEYBOARD QUERY\n\(app.debugDescription)")
                    let hide = app.buttons.matching(NSPredicate(format: "label CONTAINS[c] 'Hide keyboard'")).firstMatch
                    if hide.exists && hide.isHittable { hide.tap(); sleep(1) }
                    else { print("SH1-152 E3h keyboard hide key unavailable; independent switch inputs still recorded") }
                }
                try tap("152 Mutate", app)
                sleep(2)
                if name.hasPrefix("E3f") { try tap("152 Restore", app); sleep(2) }
            }
            try tap("152 Replay", app)
            sleep(2)
            try tap("152 End", app)
            sleep(1)
            save("stale-\(name)")
            if name == "E3i" {
                try tap("152 Fresh", app)
                sleep(2)
                // Ask the real delegate for an unrelated configuration, then replay its ordinary commands.
                try menu(app, name, selected: false)
                app.coordinate(withNormalizedOffset: CGVector(dx: 0.98, dy: 0.5)).tap()
                try tap("152 Ordinary", app)
                sleep(2)
                try tap("152 End", app)
            }
            try tap("152 Fresh", app)
            sleep(2)
            try menu(app, name, selected: false)
            app.menuItems["Paste"].tap()
            sleep(2)
            try tap("152 End", app)
            sleep(1)
            app.terminate()
        }
    }

    func testSH1152DismissAndSystemEdit() throws {
        for name in ["E4-text", "E4-key", "E7"] {
            let app = launch(name)
            try tap("152 Begin", app)
            sleep(2)
            try menu(app, name, selected: false)
            app.coordinate(withNormalizedOffset: CGVector(dx: 0.98, dy: 0.5)).tap()
            if name == "E7" {
                let edit = app.descendants(matching: .any).matching(NSPredicate(format: "label == 'Edit'")).firstMatch
                if edit.exists && edit.isHittable {
                    edit.tap()
                    sleep(1)
                    XCTAssertTrue(app.menuItems["Paste"].exists)
                    app.menuItems["Paste"].tap()
                    print("SH1-152 E7 system Edit Paste selected")
                } else { print("SH1-152 E7 unverified: system Edit not exposed; device item 9") }
            } else {
                app.typeText("x")
            }
            sleep(2)
            try tap("152 End", app)
            sleep(1)
            app.terminate()
        }
    }

    // E5: two real UIKit scenes. Scene A shows "AAA-152" on w1:p1; scene B
    // (opened by "152 Second", controls "152 Two ...") shows "BBB-152" on w1:p3.
    func testSH1152TwoScenes() throws {
        for name in ["E5a", "E5b", "E5c", "E5d"] {
            let app = launch(name)
            let selectFirst = name == "E5b" || name == "E5c"
            try tap("152 Begin", app)
            sleep(2)
            if selectFirst { try tap("152 Select", app); sleep(1) }
            try menu(app, name, selected: selectFirst, target: "AAA-152")
            if name == "E5a" { app.menuItems["Paste"].tap() }
            else if name == "E5b" { app.menuItems["Copy"].tap() }
            else { app.coordinate(withNormalizedOffset: CGVector(dx: 0.98, dy: 0.5)).tap() }
            sleep(2)
            try tap("152 End", app)
            sleep(1)
            save("\(name)-first")
            try tap("152 Second", app)
            sleep(6)
            save("\(name)-second-open")
            if name == "E5d" { try tap("152 Two Close", app); sleep(3) }
            try tap("152 Two Begin", app)
            sleep(2)
            if name == "E5b" { try tap("152 Two Select", app); sleep(1) }
            try menu(app, name, selected: name == "E5b", target: "BBB-152")
            app.menuItems[name == "E5b" ? "Copy" : "Paste"].tap()
            sleep(2)
            try tap("152 Two End", app)
            sleep(1)
            save(name)
            // Leave one scene session, as every other case expects at launch.
            if name != "E5d" { try tap("152 Two Close", app); sleep(3) }
            app.terminate()
        }
    }

    private func launch(_ name: String) -> XCUIApplication {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.terminate()
        app.launchEnvironment["YEBISU_SH1_152_CASE"] = name
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launchEnvironment[name.contains("core") ? "YEBISU_CORE_QA" : "YEBISU_TERMINAL_QA"] = "1"
        XCUIDevice.shared.orientation = .portrait
        app.launch()
        sleep(4)
        return app
    }

    // Owner rule (iOS convention): only usable items are shown. Without a
    // selection the menu is Paste alone; with a selection it is Copy and Paste.
    private func menu(_ app: XCUIApplication, _ name: String, selected: Bool, target: String? = nil) throws {
        let point = try coordinate(target ?? (name.contains("core") ? "yebisu P9" : "private wire peer"), app)
        point.press(forDuration: 0.8)
        XCTAssertTrue(app.menuItems["Paste"].waitForExistence(timeout: 4), "real menu missing: \(name)\n\(app.debugDescription)")
        let labels = app.menuItems.allElementsBoundByIndex.map { $0.label }
        print("SH1-152 VISIBLE \(name) selected=\(selected) labels=\(labels.joined(separator: ","))")
        XCTAssertEqual(labels.sorted(), selected ? ["Copy", "Paste"] : ["Paste"])
        if !selected { XCTAssertFalse(app.menuItems["Copy"].exists, "Copy shown without a selection: \(name)") }
        save("menu-\(name)")
        sleep(1)
    }

    private func tap(_ label: String, _ app: XCUIApplication) throws {
        try coordinate(label, app).tap()
    }

    private func coordinate(_ label: String, _ app: XCUIApplication) throws -> XCUICoordinate {
        for attempt in 0..<4 {
            let image = try XCTUnwrap(XCUIScreen.main.screenshot().image.cgImage)
            let request = VNRecognizeTextRequest()
            request.recognitionLevel = .accurate
            request.customWords = [label]
            try VNImageRequestHandler(cgImage: image).perform([request])
            let candidates = (request.results ?? []).sorted { $0.boundingBox.midY > $1.boundingBox.midY }.flatMap { $0.topCandidates(5) }
            for candidate in candidates {
                guard let range = candidate.string.range(of: label), let observed = try candidate.boundingBox(for: range) else { continue }
                let box = observed.boundingBox
                print("SH1-152 OCR \(label) \(box)")
                return app.coordinate(withNormalizedOffset: CGVector(dx: box.midX, dy: 1-box.midY))
            }
            // An open edit menu can cover the leading digits of a control
            // ("52 Mutate"). Accept only a whole candidate that is the same
            // label with leading digits missing, and tap its visible word.
            if label.hasPrefix("152 ") {
                let rest = String(label.dropFirst(4))
                for candidate in candidates {
                    let text = candidate.string
                    guard text.hasSuffix(" " + rest), "152".hasSuffix(String(text.dropLast(rest.count + 1))),
                          let range = text.range(of: rest, options: .backwards), let observed = try candidate.boundingBox(for: range) else { continue }
                    let box = observed.boundingBox
                    print("SH1-152 OCR \(label) partially covered as '\(text)' \(box)")
                    return app.coordinate(withNormalizedOffset: CGVector(dx: box.midX, dy: 1-box.midY))
                }
            }
            print("SH1-152 OCR attempt \(attempt): \(candidates.map { $0.string })")
            sleep(1)
        }
        save("missing-\(label)")
        XCTFail("Missing source-defined label \(label)")
        throw NSError(domain: "EditMenuQA.MissingControl", code: 1)
    }

    private func save(_ name: String) {
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = "sh1-152-\(name)"
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
