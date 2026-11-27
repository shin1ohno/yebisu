import Darwin
import Foundation

/// Class T-reap (issue #247): after XCUIApplication.terminate() the OS killed the app, but took
/// longer than XCTest's 60 s to report it gone. In runs 45846870924, 45864283800 and 45890478629
/// runningboardd's terminate_with_reason() and launchd's "termination reported" were 13.4, 46.4 and
/// 51.5 s apart (0.0-1.1 s for every other kill in those runs), and the app wrote nothing after the
/// kill. XCTest then records "Failed to terminate <bundle>:<pid>", and the next launch() can find
/// the old process ("Lost connection to the application (pid <pid>)"). EditMenuQA takes that issue,
/// waits here until the process is gone, and fails by this class's name only when it never goes.
enum TerminateWait {
    /// How long a killed app may take to be reported gone before the case fails.
    static let timeout: TimeInterval = 180
    /// How often the process is looked for while waiting.
    static let poll: TimeInterval = 1

    enum Outcome: Equatable {
        /// The process was gone `seconds` after the wait began.
        case ended(seconds: TimeInterval)
        /// The process was still there when the wait gave up after `seconds`.
        case neverEnded(seconds: TimeInterval)
    }

    /// Whether `description` is XCTest's terminate failure ("Failed to terminate <bundle>:<pid>").
    static func isFailedTerminate(_ description: String) -> Bool {
        description.contains("Failed to terminate ")
    }

    /// The non-zero pids an XCTest terminate failure names. Its text repeats the bundle with the pid
    /// XCTest held, then with 0 once it gave up: "Failed to terminate com.x:16999: Failed to
    /// terminate com.x:0" gives [16999].
    static func pids(in description: String, bundle: String) -> [pid_t] {
        let marker = "Failed to terminate \(bundle):"
        var found: [pid_t] = []
        var rest = Substring(description)
        while let range = rest.range(of: marker) {
            rest = rest[range.upperBound...]
            let digits = rest.prefix { $0.isASCII && $0.isNumber }
            if let pid = pid_t(digits), pid > 0, !found.contains(pid) { found.append(pid) }
        }
        return found
    }

    /// Whether a process with `pid` exists. kill(pid, 0) sends nothing: it succeeds, or fails with
    /// EPERM, while the process exists, and fails with ESRCH once it is gone.
    static func processExists(_ pid: pid_t) -> Bool {
        kill(pid, 0) == 0 || errno == EPERM
    }

    /// Asks `alive` every `poll` seconds until it says false or `timeout` seconds passed. `now` and
    /// `sleep` are the clock and the pause, so QAAxLogicTests can run it without waiting.
    static func wait(timeout: TimeInterval = timeout, poll: TimeInterval = poll,
                     now: () -> TimeInterval = { Date().timeIntervalSinceReferenceDate },
                     sleep: (TimeInterval) -> Void = { usleep(useconds_t($0 * 1_000_000)) },
                     alive: () -> Bool) -> Outcome {
        let start = now()
        while true {
            let elapsed = now() - start
            if !alive() { return .ended(seconds: elapsed) }
            if elapsed >= timeout { return .neverEnded(seconds: elapsed) }
            sleep(poll)
        }
    }

    /// The failure of a case whose app was killed but never reported gone.
    static func neverEndedMessage(_ name: String, seconds: TimeInterval, issues: [String]) -> String {
        String(format: "SH1-152 TERMINATE NEVER ENDED (class T-reap) %@: the app was still running %.1f s after terminate; ",
               name, seconds) + issues.joined(separator: "; ")
    }
}
