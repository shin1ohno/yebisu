import XCTest

/// Class L (issue #247): what EditMenuQA and P12QA decide about the issues XCTest recorded during one
/// app.launch() (LaunchIssue.decide), one test per row. Pure like the rest of QAAxLogicTests.
extension QAAxLogicTests {
    private static let noPid = "Application 'com.shin1ohno.yebisu' does not have a process ID"

    /// The issue text the CI runs recorded is the one the fault injects and the decision matches.
    func testNoPidIssueTextIsXCTestsOwn() {
        XCTAssertEqual(LaunchIssue.noProcessID(bundle: "com.shin1ohno.yebisu"), Self.noPid)
        XCTAssertTrue(LaunchIssue.isNoProcessID(Self.noPid))
        XCTAssertFalse(LaunchIssue.isNoProcessID("Failed to launch com.shin1ohno.yebisu: Timed out attempting to launch app."))
        XCTAssertFalse(LaunchIssue.isNoProcessID("Failed to terminate com.shin1ohno.yebisu:16999"))
    }

    /// A no-pid launch whose app shows qa.state is repeated once (run 46089691282's E3c).
    func testNoPidWithQAStateIsLaunchedOnceMore() {
        XCTAssertEqual(LaunchIssue.decide(issues: [Self.noPid], notRunning: false, stateReadable: true), .retryNoPidAppRunning)
        let reason = LaunchIssue.retryReason(.retryNoPidAppRunning, issues: [Self.noPid])
        XCTAssertTrue(reason.hasPrefix("no-pid-app-running: "), reason)
        XCTAssertTrue(reason.hasSuffix(Self.noPid), reason)
    }

    /// Negative control: without qa.state nothing proves the app runs, so the no-pid issue is
    /// recorded as XCTest recorded it and fails the test, as before issue #247.
    func testNoPidWithoutQAStateIsRecorded() {
        XCTAssertEqual(LaunchIssue.decide(issues: [Self.noPid], notRunning: false, stateReadable: false), .record)
    }

    /// A no-pid issue next to any other issue is recorded: only XCTest's missing pid is repeated.
    func testNoPidBesideAnotherIssueIsRecorded() {
        let other = "Failed to launch com.shin1ohno.yebisu: Timed out attempting to launch app."
        XCTAssertEqual(LaunchIssue.decide(issues: [Self.noPid, other], notRunning: false, stateReadable: true), .record)
        XCTAssertEqual(LaunchIssue.decide(issues: [other], notRunning: false, stateReadable: true), .record)
    }

    /// The rule before issue #247 is unchanged: a launch that left the app not running and without
    /// qa.state is repeated, whatever XCTest recorded; no issue means nothing to decide.
    func testLaunchThatStartedNothingIsLaunchedOnceMore() {
        let other = "Failed to launch com.shin1ohno.yebisu: Timed out attempting to launch app."
        XCTAssertEqual(LaunchIssue.decide(issues: [other], notRunning: true, stateReadable: false), .retryNotStarted)
        XCTAssertEqual(LaunchIssue.decide(issues: [Self.noPid], notRunning: true, stateReadable: false), .retryNotStarted)
        XCTAssertTrue(LaunchIssue.retryReason(.retryNotStarted, issues: [other])
            .hasPrefix("the launch failed and the app is not running, without qa.state: "))
        XCTAssertEqual(LaunchIssue.decide(issues: [], notRunning: true, stateReadable: false), .none)
    }

    /// Each fault is spent by its first launch; an unknown item does nothing.
    func testLaunchFaultsAreSpentOnce() {
        var faults = LaunchIssue.Faults("no-pid:E3c,no-pid-no-state:E1-text")
        XCTAssertTrue(faults.pending("no-pid-no-state", "E1-text"))
        XCTAssertFalse(faults.spend("no-pid", "E1-text"))
        XCTAssertTrue(faults.spend("no-pid", "E3c"))
        XCTAssertFalse(faults.spend("no-pid", "E3c"))
        XCTAssertTrue(faults.spend("no-pid-no-state", "E1-text"))
        XCTAssertFalse(faults.pending("no-pid-no-state", "E1-text"))
        var none = LaunchIssue.Faults("")
        XCTAssertFalse(none.spend("no-pid", "E3c"))
    }
}
