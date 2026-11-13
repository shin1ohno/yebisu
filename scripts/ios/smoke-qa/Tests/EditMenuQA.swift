import XCTest
import Vision

final class EditMenuQA: XCTestCase {
    func testSH1152Basic() throws {
        for name in ["E1-text", "E1-key", "E2-text", "E2-key", "E1-core", "E2-core"] {
            let app = launch(name)
            try tap("152 Begin", app)
            sleep(2)
            if name.hasPrefix("E2") { try tap("152 Select", app); sleep(1) }
            try menu(app, name)
            let action = name.hasPrefix("E2") ? "Copy" : "Paste"
            XCTAssertTrue(app.menuItems[action].isEnabled)
            if action == "Paste" { XCTAssertFalse(app.menuItems["Copy"].isEnabled) }
            app.menuItems[action].tap()
            sleep(2)
            try tap("152 End", app)
            sleep(1)
            save(name)
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

    private func menu(_ app: XCUIApplication, _ name: String) throws {
        let point = try coordinate(name.contains("core") ? "yebisu P9" : "private wire peer", app)
        point.press(forDuration: 0.8)
        XCTAssertTrue(app.menuItems["Paste"].waitForExistence(timeout: 4), "real menu missing: \(name)\n\(app.debugDescription)")
        XCTAssertEqual(Set(app.menuItems.allElementsBoundByIndex.map { $0.label }), Set(["Copy", "Paste"]))
        XCTAssertEqual(app.menuItems.count, 2)
        print("SH1-152 VISIBLE \(name) \(app.menuItems.allElementsBoundByIndex.map { $0.label })")
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
