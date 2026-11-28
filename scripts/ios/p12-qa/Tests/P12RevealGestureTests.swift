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

    /// yebisu#242: only the app-name tap needs the status bar, and only a window that fills its screen
    /// with no app name shown is dragged out of full screen first.
    func testOnlyAFullScreenWindowWithNoAppNameIsTakenOutOfFullScreen() {
        XCTAssertTrue(P12QA.needsWindow(gesture: .appNameTap, fillsScreen: true, nameShown: false))
        XCTAssertFalse(P12QA.needsWindow(gesture: .appNameTap, fillsScreen: true, nameShown: true))
        XCTAssertFalse(P12QA.needsWindow(gesture: .appNameTap, fillsScreen: false, nameShown: false))
        XCTAssertFalse(P12QA.needsWindow(gesture: .appNameTap, fillsScreen: false, nameShown: true))
        for fills in [true, false] {
            for shown in [true, false] {
                XCTAssertFalse(P12QA.needsWindow(gesture: .topEdgeSwipe, fillsScreen: fills, nameShown: shown))
            }
        }
    }

    /// yebisu#242 main run 45907938365: one drag left the window filling the screen.
    func testADragThatLeftFullScreenUnchangedIsDoneAgain() {
        XCTAssertEqual(P12QA.windowDragAttempts, 3)
        XCTAssertTrue(P12QA.dragsAgain(attempt: 1, fillsScreen: true, nameShown: false))
        XCTAssertTrue(P12QA.dragsAgain(attempt: 2, fillsScreen: true, nameShown: false))
        XCTAssertFalse(P12QA.dragsAgain(attempt: 3, fillsScreen: true, nameShown: false), "at most three drags")
        XCTAssertFalse(P12QA.dragsAgain(attempt: 1, fillsScreen: true, nameShown: true))
        // A window that moved but shows no name is not dragged again.
        XCTAssertFalse(P12QA.dragsAgain(attempt: 1, fillsScreen: false, nameShown: false))
        XCTAssertFalse(P12QA.dragsAgain(attempt: 1, fillsScreen: false, nameShown: true))
    }

    /// The probe's frames (iPad Pro 13-inch (M5), 2026-11-27): full screen, and the window after the drag.
    func testFillsScreenComparesTheWindowWithTheScreen() {
        let screen = CGRect(x: 0, y: 0, width: 1032, height: 1376)
        XCTAssertTrue(P12QA.fillsScreen(screen, screen))
        XCTAssertTrue(P12QA.fillsScreen(CGRect(x: 0, y: 0, width: 1032.4, height: 1375.6), screen))
        XCTAssertFalse(P12QA.fillsScreen(CGRect(x: 67, y: 207, width: 899, height: 1209.5), screen))
        XCTAssertFalse(P12QA.fillsScreen(CGRect(x: 0, y: 0, width: 1032, height: 1300), screen))
        XCTAssertFalse(P12QA.fillsScreen(.zero, screen))
    }

    /// The Simulator this test runs on: the gesture P12QA sends there.
    func testThisSystemUsesItsGesture() {
        let system = ProcessInfo.processInfo.operatingSystemVersion
        let expected: P12QA.RevealGesture = system.majorVersion >= 27 ? .appNameTap : .topEdgeSwipe
        XCTAssertEqual(P12QA.revealGesture(for: system), expected)
    }
}
