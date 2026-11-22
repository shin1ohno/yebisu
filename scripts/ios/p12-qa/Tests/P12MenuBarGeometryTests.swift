import CoreGraphics
import XCTest

final class P12MenuBarGeometryTests: XCTestCase {
    private let screen = CGRect(x: 0, y: 0, width: 1032, height: 1376)
    private let bar = CGRect(x: 0, y: 0, width: 1032, height: 32)
    private let button = CGRect(x: 270.5, y: 0, width: 70.5, height: 32)
    private let half = CGAffineTransform(scaleX: 0.5, y: 0.5)

    func testBarAndButtonCanReportTheirFramesAtDifferentScales() throws {
        let bars = [bar, bar.applying(half)]
        let buttons = [button, button.applying(half)]
        for reportedBar in bars {
            for reportedButton in buttons {
                let corrected = try XCTUnwrap(
                    P12QA.menuBarButtonFrame(reportedButton, in: reportedBar, on: screen),
                    "button \(reportedButton), bar \(reportedBar)"
                )
                XCTAssertEqual(corrected, button)
                XCTAssertEqual(CGPoint(x: corrected.midX, y: corrected.midY), CGPoint(x: 305.75, y: 16))
            }
        }
    }

    func testInconsistentGeometryCannotSupplyAPressTarget() {
        let wrongBarHeight = CGRect(x: 0, y: 0, width: screen.width, height: 65)
        XCTAssertNil(P12QA.menuBarButtonFrame(.zero, in: bar, on: screen))
        XCTAssertNil(P12QA.menuBarButtonFrame(button, in: .zero, on: screen))
        XCTAssertNil(P12QA.menuBarButtonFrame(button, in: bar, on: .zero))
        XCTAssertNil(P12QA.menuBarButtonFrame(button, in: bar.offsetBy(dx: 1, dy: 0), on: screen))
        XCTAssertNil(P12QA.menuBarButtonFrame(button, in: wrongBarHeight, on: screen))
        XCTAssertNil(P12QA.menuBarButtonFrame(button.offsetBy(dx: 900, dy: 0), in: bar, on: screen))
        XCTAssertNil(P12QA.menuBarButtonFrame(button.insetBy(dx: 0, dy: 6), in: bar, on: screen))
        XCTAssertNil(P12QA.menuBarButtonFrame(button, in: bar, on: screen.applying(half)))
    }
}
