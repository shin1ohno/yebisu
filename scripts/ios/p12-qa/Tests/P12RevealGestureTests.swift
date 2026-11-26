import XCTest

/// Which gesture reveals the OS menu bar (`P12QA.RevealGesture`): iPadOS 26 and earlier take the
/// top-edge swipe, iPadOS 27 a tap on the app's name in the status bar.
final class P12RevealGestureTests: XCTestCase {
    private func version(_ major: Int, _ minor: Int = 0) -> OperatingSystemVersion {
        OperatingSystemVersion(majorVersion: major, minorVersion: minor, patchVersion: 0)
    }

    func testIPadOS26AndEarlierSwipeFromTheTopEdge() {
        for system in [version(17), version(26), version(26, 5), version(26, 9)] {
            XCTAssertEqual(P12QA.revealGesture(for: system), .topEdgeSwipe, "\(system)")
        }
    }

    func testIPadOS27AndLaterTapTheAppName() {
        for system in [version(27), version(27, 2), version(28)] {
            XCTAssertEqual(P12QA.revealGesture(for: system), .appNameTap, "\(system)")
        }
    }

    /// The Simulator this test runs on: the gesture P12QA sends there.
    func testThisSystemUsesItsGesture() {
        let system = ProcessInfo.processInfo.operatingSystemVersion
        let expected: P12QA.RevealGesture = system.majorVersion >= 27 ? .appNameTap : .topEdgeSwipe
        XCTAssertEqual(P12QA.revealGesture(for: system), expected)
    }
}
