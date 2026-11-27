import Darwin
import XCTest

/// Class T-reap (issue #247): what EditMenuQA's terminate wait decides, run against a stub process
/// and a fake clock (TerminateWait.wait), so the 180 s bound costs no time. Pure like the rest of
/// QAAxLogicTests: no app is launched.
extension QAAxLogicTests {
    /// A fake clock that `sleep` advances, and a count of the times the stub was asked.
    private final class Clock {
        var now: TimeInterval = 1_000
        var asked = 0
    }

    /// Negative control: a stub process that never ends fails the wait by the class's name once the
    /// bound has passed, after polling it once a second, not before.
    func testTerminateWaitGivesUpOnAStubThatNeverEnds() {
        let clock = Clock()
        let outcome = TerminateWait.wait(timeout: 180, poll: 1, now: { clock.now }, sleep: { clock.now += $0 }) {
            clock.asked += 1
            return true
        }
        XCTAssertEqual(outcome, .neverEnded(seconds: 180))
        XCTAssertEqual(clock.asked, 181, "asked at 0, 1, ..., 180 s")
        let message = TerminateWait.neverEndedMessage("E1-key", seconds: 240.4,
                                                      issues: ["Failed to terminate com.shin1ohno.yebisu:16999"])
        XCTAssertTrue(message.hasPrefix("SH1-152 TERMINATE NEVER ENDED (class T-reap) E1-key: the app was still running 240.4 s"),
                      message)
        XCTAssertTrue(message.hasSuffix("Failed to terminate com.shin1ohno.yebisu:16999"), message)
    }

    /// The killed app that the OS reports gone later (51.5 s in run 45890478629) ends the wait at
    /// the first poll that finds it gone, and one already gone ends it at once.
    func testTerminateWaitEndsWhenTheStubEnds() {
        let clock = Clock()
        let outcome = TerminateWait.wait(timeout: 180, poll: 1, now: { clock.now }, sleep: { clock.now += $0 }) {
            clock.asked += 1
            return clock.asked <= 52
        }
        XCTAssertEqual(outcome, .ended(seconds: 52))
        let gone = TerminateWait.wait(timeout: 180, poll: 1, now: { clock.now }, sleep: { _ in XCTFail("no pause") }) { false }
        XCTAssertEqual(gone, .ended(seconds: 0))
    }

    /// The issue text of run 45890478629 (EditMenuQA.swift:63) names the held pid, then pid 0.
    func testTerminateIssueNamesThePidXCTestHeld() {
        let text = "Failed to terminate com.shin1ohno.yebisu:16999: Failed to terminate com.shin1ohno.yebisu:0"
        XCTAssertTrue(TerminateWait.isFailedTerminate(text))
        XCTAssertEqual(TerminateWait.pids(in: text, bundle: "com.shin1ohno.yebisu"), [16999])
        XCTAssertEqual(TerminateWait.pids(in: "Failed to terminate com.shin1ohno.yebisu:0", bundle: "com.shin1ohno.yebisu"), [])
        XCTAssertFalse(TerminateWait.isFailedTerminate("Failed to launch com.shin1ohno.yebisu: Timed out attempting to launch app"))
        XCTAssertFalse(TerminateWait.isFailedTerminate("Application 'com.shin1ohno.yebisu' does not have a process ID"))
    }

    /// kill(pid, 0) tells a live process from a missing one: this runner is alive, pid_t.max is not.
    func testProcessExistsTellsALiveProcessFromAMissingOne() {
        XCTAssertTrue(TerminateWait.processExists(getpid()))
        XCTAssertFalse(TerminateWait.processExists(pid_t.max))
    }
}
