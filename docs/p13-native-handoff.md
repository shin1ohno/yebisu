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
keeps input paused. An owner that is unavailable at request time, or that no
longer matches when the acknowledgement lands (a reader publication holding the
inbox, a destination not yet re-synced), re-arms the request: the next render
requests a fresh retirement for the owner it then finds, so input returns once
the contention clears. A focus change or native identity mismatch is not
re-armed. Neither terminal taps nor GPUI key events can release this
P13 fence. No input is buffered or replayed for recovery.

The SDK was pinned to the herdr-uikit PR 14 merge commit
99d792815864a6dde4cca23b6fad5166327cf319 (on herdr-uikit main); L61 moved it
to the PR 18 merge commit af7c496099dd6d308055be1818ddf971a321e4f8, which adds
PR 17 (test-support held-press) and PR 18 (empty edit menu for unpresented
configurations) and keeps the same helper. The Mac control-letter fix then
moved it to the herdr-uikit PR 24 merge commit
bd0d13047de730475166ec95cc5416ef54372c14, which also brings PRs 19, 20 and 22.
The helper is unchanged; PR 22 only adds a delegate-notification counter
beside it (`retirement_probe::delegate_notifications`). 99d7928 contains
the earlier fe3a0da query helper, main's 3ec67ca, and a test-support-only
Cut/first-query helper whose production-path hooks are all
`cfg(feature = "test-support")`. Only the opt-in `p13-native-probe` feature
calls that helper. This pin move merges no SDK change itself and does not
establish its original S/Z acceptance or OS delivery guarantees.
The SDK permanently fences the old captured object before the callback, then
installs its distinct replacement. Its receipt is not an OS cancellation ack.

The supported path is an ordinary Close button outside a native text/key
callback. Native Escape close can receive `NativeCallbackInFlight` from this
SDK. The consumer retains its fence and displays the refusal; it does not retry
from a timer, clear cancellation, replay input or claim keyboard-close recovery.
Whole-operation-tail support remains an SDK-owned dependency. Queued OS input
retarget/loss and physical IME/hardware behavior remain separate obligations.

The Escape press that closes search stays with the UI until it ends
(`find/closing_press.rs`). The replacement native view carries no press or
repeat state and a `KeyDownEvent` has no press identity, so a KeyDown of the
still-held press can arrive with `is_held == false`. On device (step 18 of
the 2026-11-17 check) one did, and reached zsh. The latch ends on a KeyUp of
the key (the SDK sends one for both an ended and a cancelled press) or when
the window loses activation; a press ledger that reads empty never ends it.
Repeats stay with the UI. A non-held KeyDown stays with the UI only while the
window's press ledger holds a press that began before the close; otherwise
it is a new press, which ends the latch and reaches the terminal once. Host
tests stand in for the ledger (`TEST_OLDEST_HOLD`); the iOS read of
`native_hardware_hold` is one line, checked only by `lint-ios`. Left open: a
release lost by both GPUI and the ledger, or another key held since before
the close, makes the next press look like the closing one until its own
release or a deactivation. The keyboard-focus status samples the live
responder, since the receipt records it only once, when the view is replaced.

Under an exclusive Mac lease, first run exact-head `just check`, `just lint-ios`
and `just test`, preserving the prior macro-import repair. Then run
`just guarded python3 scripts/ios/p13-mutate.py <new-evidence-dir> --expected-head <head>`.
This requires each named D1–D6 host assertion, and the four device-D1
closing-press assertions, to fail for its own reason and pass
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

Close can resize the terminal, and production holds generated input until the
surface matches the requested size (`terminal_input.rs` `sync`). A barrier
request goes straight to the ordered writer, so a barrier sent during that hold
overtakes the fresh input. Both probes therefore send each after-close barrier
only once `TerminalInput::flushed` reads true (nothing held, last sync at the
requested size), waiting at most 5 s, and record `p13-barrier` with `flushed`.
Both oracles reject a missing or `flushed: false` record by name
(`p13-barrier-before-flush`, `p13-query-barrier-before-flush`) and still check
the receiving-wire order on their own, so an early barrier fails either way
(`scripts/ios/test_p13_barrier.py`). The hold itself is unchanged.

The admitted helper adds nonmutating selection/document/position observations
on the same retained native object. Setup setters are used only before Q01/Q02.
The separate query mode invokes actual native insert/mark and Cut. In the same
native turn as the Q02 setup, `NativeClient::cut_and_observe_selection` sends the
real `cut:` once on the captured object and stores that object's FIRST selection
query before any timer, frame, flush or next turn; Q03's remaining queries and
consumer state follow. The next phase only injects native-origin Escape into
the actual consumer field listener. Escape's expected caret is 4..4: Input::replace removes
the marked 4..6 range and positions the caret at replacement start. Native unmark
is not used as an Escape substitute. Query helpers never repair selection.

After a fresh exclusive lease, collect the independent query sequence with
`just guarded python3 scripts/ios/p13-query-check.py <new-dir> --run --expected-head <head>`.
It checks abcdef/caret3, UTF-16 emoji/mark/suffix before and after actual Cut,
selection before/after every query, error/nil/zero distinctions, captured IDs,
real Close and a current length6 positive versus retained-old length0. Separate
FIFO barriers require zero old/search wire and one fresh native commit.

Run the three actual consumer negatives with
`HERDR_RETIREMENT_LEASE=exclusive just guarded python3 scripts/ios/p13-query-mutate.py <new-dir> --expected-head <head>`
only after that lease is granted. It removes the exact production document-length
override, separately makes Cut leave a wrong caret, and separately defers the
same first query to the next native turn (`p13-cut-first-query-order`: the
passive trace fails even if the later caret looks right). The collector permits
only each declared exact patch, records hashes, requires its named actual native
receipt assertion and exact-byte restore/repass; compile/setup failure cannot
certify a negative. This does not replace the original D1-D6 host mutants.
Every collector run writes `provenance.json` from its own outputs: head, tree,
changed-file hashes, lock/toolchain hashes, `rustc -vV`/`cargo -V`/Xcode, the
yebisu compiler artifact (own manifest, features, fresh flag), and the built,
bundled and installed executables. bundle.sh ad-hoc signs the bundled copy, so
their Mach-O UUIDs must match and the installed bundle's executable and
Info.plist must be byte-identical to the built bundle. A mutant run
must be a rebuilt (`fresh=false`) binary differing from the baseline, and each
restored run must have the baseline tree with no changed file. Keep evidence
directories outside the checkout. Python here is 3.9-compatible (no tomllib).

SDK helper Q05 recapture/nonmutation/nil/numeric negatives remain SDK-owner work:
the reviewed fe3 script has four distinct definitions, including actual current
recapture. Its source-only PASS closes the former R1 definition gap, not runtime
proof. Execute those in a separately owned exact SDK checkout under a fresh
lease, never by mutating this consumer's dependency cache or archived snapshot.
All native baseline/negative/restored runs are UNRUN at source adoption.
Host/pane/boot/generation/mailbox/reconnect native matrices, B1 and B2, P11/P12
integration and physical owner checks must not be inferred from host models.

## Model sources outside PROVENANCE.tsv

PROVENANCE.tsv registers external copies and models from merged upstream
refs only. These P13 files were modeled on this repository's own files or on
unmerged SDK branches, so their sources are recorded here instead:

- `crates/yebisu/src/find/qa.rs` ← shin1ohno/yebisu `crates/yebisu/src/native_edit_qa.rs` at `dc2511fe6959` (main)
- `scripts/ios/p13-check.py` ← shin1ohno/yebisu `scripts/ios/smoke-qa/Tests/SmokeQA.swift` at `dc2511fe6959` (main)
- `crates/yebisu/src/find/retirement.rs` ← shin1ohno/yebisu `crates/yebisu/src/input_retirement.rs` at `0f1f34212784` (feat/composition-retirement)
- `crates/yebisu/src/find/retirement.rs` ← shin1ohno/yebisu `crates/yebisu/src/native_retirement.rs` at `0f1f34212784` (feat/composition-retirement)
- `crates/yebisu/src/find/native_probe.rs` ← shin1ohno/herdr-uikit `src/window/retirement_probe.rs` at `789fb43b947b` (feat/composition-retirement)
- `crates/yebisu/src/find/native_queries.rs` ← shin1ohno/herdr-uikit `src/window/text_input/probe/query.rs` at `fe3a0da1d1a9` (fix/yebisu-composition-retirement)
- `crates/yebisu/src/find/native_queries.rs` ← shin1ohno/herdr-uikit `src/window/text_input/probe/cut_query.rs` at `33e288d762e1` (feat/p13-cut-first-query-probe)
- `scripts/ios/p13-query-check.py` ← shin1ohno/yebisu `scripts/ios/p13-check.py` at `9df0d73ba377` (feat/p13-terminal-search)
- `scripts/ios/p13-query-mutate.py` ← shin1ohno/yebisu `scripts/ios/p13-mutate.py` at `9df0d73ba377` (feat/p13-terminal-search)
