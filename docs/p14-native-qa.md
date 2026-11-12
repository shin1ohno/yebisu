# P14 native harness readiness

Status: AUTHORED, native UNRUN. D5 separates SDK display eligibility from issued-tag responses.
The exact SDK revision is in Cargo.toml and Cargo.lock. P11's SDK entry/refusal observer is
still missing and its modal gate stays closed. No permission request, OS database
change, APNs, live host, signing, Keychain or after-suspension delivery is added.

## Entry points (run only after an exclusive controller lease)

From the leased exact consumer checkout, using the lease's simulator selection:

```sh
just guarded python3 scripts/ios/p14-check.py --lease-id CONTROLLER_LEASE_ID
just guarded python3 scripts/ios/p14-sdk-check.py --sdk-root /absolute/leased/herdr-uikit --lease-id CONTROLLER_LEASE_ID
```

Both commands compile native code and require a Mac, Xcode, xcodegen, the
installed simulator and the existing Rust iOS target. They were NOT run during
source staging. Keep the 6 GiB guard and the controller's single-build rule.
The SDK checkout must be clean and match the manifest's exact revision; the
mutant is restored even when the test returns an error or SIGTERM interrupts it.
A forced SIGKILL cannot run cleanup: use the saved original.rs / response-original.rs and compare the
recorded SHA256 before reusing the checkout. Never push a mutant.

App route: `SmokeOracle/SmokeQA/testP14ConnectedNotifications` launches the real
com.shin1ohno.yebisu Simulator app with YEBISU_TERMINAL_QA,
YEBISU_SMOKE_OBSERVE and YEBISU_P14_QA. Only a private Stream::pair peer is used.
The five-click fixture is bounded; commands stop after step 5. QA controls and
wire emissions compile only for iOS Simulator. QA settings use p14-qa, not the
ordinary settings file. Existing peer receipts and isolated settings are copied
before reset; each run uses a new timestamped output directory.

1. The real UI sends p14-live to the private peer. That peer sends a wire
   SemanticNotification for w1:p3 through the ordinary reader/Live/consumer.
   The production toast renderer paints its redacted title; Swift OCR verifies
   the text, saves p14-toast, then taps that actual toast. The oracle requires
   exactly one pane.focus(w1:p3) peer receipt and the new focused-pane paint.
2. A source-controlled quiet interval covering the current hour suppresses
   p14-quiet with QuietHours. No sound is requested in this fixture.
3. p14-invalid names a missing pane; the actual capture path rejects it with
   InvalidTarget. This is an invalid-target test, not a simulated boot change.
4. p14-disconnect is written to the wire immediately before the peer closes.
   The consumer must observe disconnected/inactive, and no toast may appear.
5. A late duplicate of the already-consumed positive toast tag calls the actual
   response adapter. It must report Stale without a generation change or new
   focus request. This controlled late tap is not an OS notification response.

The app already queries actual OS authorization without prompting. Its result
is recorded with paint receipts; Unknown is not query evidence. NotDetermined
and Denied remain UNVERIFIED for permission grant/display. Even Authorized is
only a query result, not proof of a displayed OS notification.

## Evidence and gates

`target/ios-p14/<run>/` stores command stdout/stderr/exit, head/lease, raw
p0-smoke.jsonl, p4-peer.jsonl, p14-sent.jsonl, xcresult and six screenshots before
validation. Required states have strict positive/negative controls; the oracle
fails on missing peer/paint/focus/screenshot/query records. XCTest failure still
preserves available raw files. The Python fixture suite validates the oracle,
not the app. Check runner diagnostics for dropped observation records; any drop
invalidates the native run even when sampled assertions pass.

SDK route: `scripts/sim-runner.sh` invokes the test binary with simctl spawn,
outside UIApplicationMain. The existing six callback-order/registry regressions
use the production final-decision helper. They can establish native callback
options, NOT actual UNUserNotificationCenter delivery. The wrapper runs the
specific race baseline, replaces only the final current predicate with
`ticket.is_some()`, requires failure at `final native display decision`, restores
exact bytes, then separately restores D5 by applying the display predicate to
response dispatch. The six-second response baseline must pass; the mutant must
fail at `D5 six-second SDK response must reach consumer`. Restoring both sources
must run all SDK notification tests (13 pass, 1 ignored expected). The six
consumer `notification_view::response_tests` then invoke the same SDK boundary,
production Smoke guards, and real private-peer wire/FIFO barrier. They verify
six-second focus once, duplicate, 301-second expiry, disconnected/removed tags,
generation/boot/pane mismatch, quiet/mute/action and denied/unissued behavior.
No aging sleeps are used. The SDK fixture is not the actual ObjC delegate or OS
center; dispatch wiring and OS callback ordering still require separate proof.
Compilation or harness failure is never a successful mutant kill. Raw outputs
and original/restored hashes are saved under target/ios-p14-sdk/<run>/.

The ignored actual_center_authorization_callback test needs an app-bundled
harness and is NOT run through simctl spawn. Use the real app query above for
read-only authorization evidence. Actual OS center entry/completion, visible
banner/no-banner, OS response, sound and physical volume/mute/suspension still
need separate primary receipts under an approved native/owner session. Never
reinterpret direct delegate/model fixtures as those receipts. No test here
requests permission or clicks a system dialog. Original N1-N5 remain unverified.


## Exact-head owned-output handoff

The runner requires clean exact SDK pin and consumer head, retains originals and
command stdout/stderr, and uses new per-run owned-sdk-target and
owned-consumer-target directories. These outputs must not be shared with other
leases or used as proof of an app bundle. Before running, the controller records
`rustc -Vv`, `cargo -V`, Xcode/SDK/target versions, selected simulator, lease ID,
manifest/lock/source SHA256 and test inventory. Native execution is UNRUN here.
The app GUI runner needs a separately owned target/profile/output tree; record
its cargo JSON compiler-artifact executable path, bundle binary UUID/SHA256,
bundle identifier and installed/launching app path before accepting screenshots.
A binary found in a shared target directory or an old installed app is not
exact-head evidence. Retain raw receipts on failures. No cleanup of shared
artifacts, permission dialogs, signing or Keychain work is authorized.
