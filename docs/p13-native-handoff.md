# P13 native input handoff and document queries

The search field reports the full UTF-16 document length, including a suffix
after a middle caret and after a selected cut inside an offset marked clause.
The ordinary IME `None` replacement still replaces the entire marked clause.
Callback tests are separate from real UIKit end/position query evidence.

Closing or invalidating search arms a consumer fence. The one native retirement
request captures the final terminal owner: host, board selection, connection
mailbox, generation, boot, target, input epoch, cancellation revision and window.
Only a matching native-client pair can acknowledge that revision. A changed
owner, newer close, focus change, unavailable destination or saturated identity
keeps input paused. Neither terminal taps nor GPUI key events can release this
P13 fence. No input is buffered or replayed for recovery.

The experimental SDK is pinned to the controller-authorized frozen PR9 head
789fb43b947b8c1b120c375cdf7fadeb1514e759. This consumer adoption neither merges
the SDK nor establishes its original S/Z acceptance or OS delivery guarantees.
The SDK permanently fences the old captured object before the callback, then
installs its distinct replacement. Its receipt is not an OS cancellation ack.

The supported path is an ordinary Close button outside a native text/key
callback. Native Escape close can receive `NativeCallbackInFlight` from this
SDK. The consumer retains its fence and displays the refusal; it does not retry
from a timer, clear cancellation, replay input or claim keyboard-close recovery.
Whole-operation-tail support remains an SDK-owned dependency. Queued OS input
retarget/loss and physical IME/hardware behavior remain separate obligations.

Under an exclusive Mac lease, first run exact-head `just check`, `just lint-ios`
and `just test`, preserving the prior macro-import repair. Then run
`just guarded python3 scripts/ios/p13-mutate.py <new-evidence-dir> --expected-head <head>`.
This requires each named D1–D6 host assertion to fail for its own reason and pass
after restoration; compilation failure cannot count. D6's host mutation does not
replace a real UIKit document-query mutation.

The Simulator command is
`just guarded python3 scripts/ios/p13-check.py <new-evidence-dir> --run --expected-head <head>`.
It builds `p13-native-probe` and uses captured actual SDK native clients. The
Close probe invokes old selectors before/after retirement and identical fresh
insert/mark/commit without preceding GPUI KeyDown or terminal tap. Separate
receiving FIFO barriers isolate old and fresh phases; exact target/event counts,
native IDs/readiness and renderer receipts are checked. Probe scheduling is
test-only and never retries production retirement or input. This probe is
prepared source, not a record that it has run.

The frozen SDK's public test-support lacks document-end/position/selection
queries. The controller has a concrete SDK-owned API proposal for real
`endOfDocument` / `positionFromPosition:offset:` regressions. Until that API is
reviewed, authorized and exercised, D6 native query proof stays unverified.
Host/pane/boot/generation/mailbox/reconnect native matrices, B1 and B2, P11/P12
integration and physical owner checks must not be inferred from host models.
