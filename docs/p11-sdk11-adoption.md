# Provisional SDK11 consumer adoption — source only

This is a frozen source candidate, not SDK/native acceptance. The controller
admitted plan 0461ca658933341948bc47aa09fe5b1015fe6e1bb3d1be2f85a6f8623c6bf4d9,
not the implementation. Dependency: herdr-uikit PR11 commit
944ff7e21a43fbdacb0e78a5be4056c32029db73, based on SDK8
3e1d03c43965145f13860e8e344c27bf5b09521d. Independent SDK code review and all
actual target/native gates are UNRUN. Do not merge SDK or treat this pin as
proven. Revisions require a new exact-head review, not silent candidate drift.

Consumer history merges only PR25 de65d2adcb7f77b9373c71f105c4621e1924954b
(parent dc2511fe695935cdce60ba6bb3cfb4288a425778), retaining SH1-153 explicit
edit availability, worker-slot notification, and regressions. Modal suppresses
the production availability predicate. No P13/main sweep. Cargo.lock's SDK
source locator and existing yebisu dependency list are edited to the exact
candidate; dependency resolution and target compilation are UNRUN. No other
SDK dependencies were moved.

## Compile and runtime boundaries

`yebisu/test-support` is default-off and forwards `herdr-uikit/test-support`.
`dispatch2` and `libc` are optional iOS Simulator dependencies. Every consumer
observer import/module/field/call is under ios+sim+test-support. A device build
must contain no observer code even if this feature is accidentally requested;
actual device/off compilation is a controller gate, not established by grep.
No new production provider, signing, entitlement, or permission is enabled.

Real mock-provider startup/read/confirm/copy preflight from f57557e remains
independent of observer arming. The harness does not read/copy/replace keys.
Each native launch capture must contain the actual provider preflight. A flag
selecting a test mode is never proof of the runtime-selected provider.

## Actual API and serialization

`p11_observer.rs` uses the actual SDK APIs begin_run, arm, invoke_native_selector,
drain, run_status, disable, invalidate_run, current_native_observation_key.
On-click captures immutable values and enqueues `DispatchQueue::main().exec_async`;
no GPUI app/window/entity borrow crosses actual synchronous native dispatch.
The live window arm and phase manifest are recorded BEFORE invocation. Policy
installation and restoration must both return true. The original production
policy is restored immediately after invocation, including a returned error.
An unwind cannot be claimed as successful restoration or successful phase.

SDK records are serialized as fixed fields in `p11_observer/schema.rs`:
32-hex-digit process run, u64 arm epoch/view generation/invocation, numeric
phase, fixed action/origin/decision/outcome, bool handled, fixed fallback.
There are no raw pointers or clipboard/text/host/key payload fields. Actual
sheet and terminal KeyDown/Up callbacks acquire the SDK's scoped key directly;
no delayed task invents a key from its arrival phase. All queue admissions are
checked. A failed observation/completion channel invalidates the run. Drain
queued is the PRE-consumption length; the subsequent status must be empty.
Counters/drop/sticky invalid/outstanding/reserved must all be zero.

No single SDK record proves transport delivery. `workers==0 && !preparing`
is required before sending the end marker. A later click advances only after
the private peer actually receives that exact phase end marker and writes and
flushes its transcript. One fresh process/private peer per test is mandatory:
the receiver ack is process-local, never a cross-peer/reconnect capability.
The runner reads the entire transcript after process termination, including
a tail after the last marker. No elapsed timer or scheduled bool satisfies
any completion check. Swift delays only pace UI retries; timeout fails.

## Frozen supported phase matrix

| Suite | Cases | Policy / expected route |
|---|---:|---|
| Text positive | 24 | Four Command/Priority settings x Paste/Copy/Cut/SelectAll/physical-v/physical-c adapters; Text origin on four selectors |
| Drawing positive | 24 | Same independent 24 exact receiving segments; Drawing origin |
| Modal None | 8 | Four selectors x both origins; settled modal requires RefusedNoTextMode; no consumer callback |
| Modal Some deny | 8 | Four selectors x both origins; RefusedPolicyDisallowed; no consumer callback |
| Modal Some allow | 8 | Four selectors x both origins; AllowedExplicitPolicy + Dispatched, actual sheet-modal Down/Up, handled=true, NotEligible |
| Runtime disabled | 24 modal | Same real stimuli; no observer pair/key, intended missing-native-pair negative after positive controls |

SH1-153 SelectAll is false, so positive SelectAll expects SDK policy refusal,
not a manufactured consumer callback. Copy/Cut/Paste require the actual
production policy. Physical controls retain original adapter semantics and
must have no native observation key; they are NOT hardware-key proof. The
positive clipboard oracle records only comparison with fixed fake expected
content, never arbitrary clipboard text.

None reason is phase-specific: SDK lifecycle loses text focus synchronously
but deactivates Text mode on a deferred responder update. Settled modal means
KeyOnly -> RefusedNoTextMode. During transition Text+false focus would be
RefusedNoTextFocus. That is a wrong reason for this frozen settled case, not
an interchangeable pass. Public SDK11 has no completion/state probe to force
that transition deterministically. The first actual run must preserve a wrong
reason and fail; no arrival-time relabeling.

## Unsupported gates are explicit

Dedicated inactive/no-responder and controlled transient-no-focus phases
cannot be driven/acknowledged with this candidate's public API. They remain
unsupported and prevent full matrix admission. No private selector, fabricated
policy/state record or blanket success fills the gap.

The pinned transport has disconnect(), which sets stop, not an acknowledged
final writer drain. Full terminal transport drain/termination is therefore
NOT proven by process termination or the consumer's end marker. The collector
preserves the complete on-disk tail after termination, but cancellation of
queued unsent data cannot prove the final-publication property. F19's SDK
final writer fence remains open independently of SDK11 observation. Controller
must separately review an actual transport drain/fence contract before closing
this gate. Existing final-send/Unknown/one-worker/reconcile checks are retained.

Old `scripts/ios/p11-check.py` remains the historical 0/4 failure oracle.
New supported-control results cannot delete that failure or waive P1-P5/F01-F26.
`docs/p11-security-fixtures.md` remains the full26 source/runnable-case map;
all target Rust/native cases remain UNRUN.

## Isolated collector handoff (UNRUN)

Controller must first review the exact combined SDK/consumer heads and issue
an exclusive lease JSON with consumer_head, sdk_head, device, exclusive:true.
No SSH/build is authorized by this document. Under that separate lease:

```
python3 scripts/ios/p11-observer-check.py --lease /absolute/lease.json \
  --head EXACT_CONSUMER_HEAD --device LEASED_SIMULATOR \
  --target-dir /absolute/fresh/head-specific-target
```

The runner rejects dirty/wrong-head source and an existing target directory.
It uses that fresh CARGO_TARGET_DIR, rejects a bundle escaping it, records the
actual executable SHA256, and keeps XCTest derived data there. This prevents
reuse of the unrelated Runner/shared-target artifact. Compile cfg-off/device,
SDK off/on tests and independent native mutants remain separate required
leased commands; the collector is not a substitute for those gates.

Capture redirects actual process fd1/fd2 at app startup, before diagnostics or
provider selection, into a create-new per-PID file. Every XCTest relaunch has
its own file. All native captures, app observations, peer messages, rotated
diagnostics, and command output are copied before validation. The manifest
hashes exact source/head/run and every sink; absent/empty/extra/uncollected
logs, bad digest, missing per-launch preflight, or fake-key sentinels fail.
Old container logs are archived before cleaning this isolated test's logs.
No real Keychain operation is permitted by the collector.

After supported controls, the runner deliberately exits nonzero with
acceptance:false while unsupported/full26/independent gates remain open.
Synthetic oracle regressions exercise malformed pairs, correlation, status,
disabled intended failure and receiving tails; they are source checks only,
not native or production serializer execution. Do not use fabricated fixtures
as actual runtime provider/callback proof.
