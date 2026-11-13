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
fe3a0da1d1a9c2d52a94510c4d4f7302339cd5f7 (isolated query helper over SDK789).
This consumer adoption neither merges
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

The admitted helper adds nonmutating selection/document/position observations
on the same retained native object. Setup setters are used only before Q01/Q02.
The separate query mode invokes actual native insert/mark and Cut, reads the
post-Cut selection FIRST, and then injects native-origin Escape into the actual
consumer field listener. Escape's expected caret is 4..4: Input::replace removes
the marked 4..6 range and positions the caret at replacement start. Native unmark
is not used as an Escape substitute. Query helpers never repair selection.

After a fresh exclusive lease, collect the independent query sequence with
`just guarded python3 scripts/ios/p13-query-check.py <new-dir> --run --expected-head <head>`.
It checks abcdef/caret3, UTF-16 emoji/mark/suffix before and after actual Cut,
selection before/after every query, error/nil/zero distinctions, captured IDs,
real Close and a current length6 positive versus retained-old length0. Separate
FIFO barriers require zero old/search wire and one fresh native commit.

Run the two actual consumer negatives with
`HERDR_RETIREMENT_LEASE=exclusive just guarded python3 scripts/ios/p13-query-mutate.py <new-dir> --expected-head <head>`
only after that lease is granted. It removes the exact production document-length
override and separately makes Cut leave a wrong caret. The collector permits
only each declared exact patch, records hashes, requires its named actual native
receipt assertion and exact-byte restore/repass; compile/setup failure cannot
certify a negative. This does not replace the original D1-D6 host mutants.

SDK helper Q05 recapture/nonmutation/nil/numeric negatives remain SDK-owner work:
the reviewed fe3 script has four distinct definitions, including actual current
recapture. Its source-only PASS closes the former R1 definition gap, not runtime
proof. Execute those in a separately owned exact SDK checkout under a fresh
lease, never by mutating this consumer's dependency cache or archived snapshot.
All native baseline/negative/restored runs are UNRUN at source adoption.
Host/pane/boot/generation/mailbox/reconnect native matrices, B1 and B2, P11/P12
integration and physical owner checks must not be inferred from host models.
