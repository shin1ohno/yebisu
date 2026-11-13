# Provisional SDK11 consumer adoption — source only

This is a frozen source candidate, not SDK/native acceptance. The controller
admitted plan 0461ca658933341948bc47aa09fe5b1015fe6e1bb3d1be2f85a6f8623c6bf4d9,
not the implementation. Dependency: herdr-uikit PR11 commit
2dcd11a9f20762bb7070c3fdefb72d93ed952781, based on SDK8
3e1d03c43965145f13860e8e344c27bf5b09521d. The old 944ff7e candidate received REQUEST_CHANGES B1/P2. Controller now
authorizes this corrected provisional source pin. Independent source review
61126e0212de42f9 found B1/P2 resolved in this correction; that review does not
approve native behavior, consumer integration, or SDK merge. All actual
target/native gates remain UNRUN. Do not merge SDK or treat this pin as
proven. Revisions require a new exact-head review, not silent candidate drift.

Consumer history includes PR25 de65d2adcb7f77b9373c71f105c4621e1924954b
(parent dc2511fe695935cdce60ba6bb3cfb4288a425778), retaining SH1-153 explicit
edit availability, worker-slot notification, and regressions. Modal suppresses
the production availability predicate. It also merges exact main
aa457e9f0c06dbc706c64447cc3c4f839c3979f3 for PR34's raw-environment runner
helper: only sim-runner-env.py, sim-runner.sh and test_sim_runner.py differ
from the preceding consumer head. No P13 integration. Cargo.lock's SDK
source locator and existing yebisu dependency list are edited to the exact
candidate. One `cargo metadata --locked --format-version 1 --all-features`
resolved the exact SDK pin without changing the lock or dropping iOS/Simulator
dependency edges. Target compilation remains UNRUN. No other SDK dependencies
were moved. The four runner regression tests passed on Linux; this is not
Mac or actual Simulator proof.

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
an exclusive lease JSON with consumer_head, sdk_head, device, exclusive:true,
checkout (canonical absolute checkout), and run_root (the exact fresh path below).
No SSH/build is authorized by this document. Under that separate lease:

```
python3 scripts/ios/p11-observer-check.py --lease /absolute/lease.json \
  --head EXACT_CONSUMER_HEAD --device LEASED_SIMULATOR \
  --target-dir /absolute/owned-checkout/target/p11-native/EXACT_CONSUMER_HEAD/RUN_UUID
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


## Corrected candidate and output isolation

Controller source-only authorization is `sdk11-corrected-provisional-adoption-2.md`.
SDK hand-off-2 fixes generic Window ingress to suppress the new key, while
canonical native dispatch supplies its own key only for Down/Up. Public
observer mod/types and SDK Cargo files are blob-identical to 944ff7e. SDK author
reports off34/on40 and three mutants; these are source fixtures with platform
shells, not live views/ABI/native proof. Consumer does not independently patch
SDK or claim B1 admission. Old e2f2/SDK944 evidence remains archived.

Helper audit found `bundle.sh` hardcoded `/tmp/herdr-ios-bundle.*`; it now honors
TMPDIR. Before deleting/creating the sibling .app, its optional
HERDR_IOS_ARTIFACT_ROOT check rejects an executable/app escaping the owned Cargo
root. PROVENANCE.tsv already marks this copied helper modified=yes. Generic
helper defaults are preserved; the collector always sets the restrictive root.

Collector output is restricted to the canonical checkout's
`target/p11-native/<consumer-head>/<run-UUID>/`. A symlink to another checkout or
an existing root is rejected. Cargo target/build, Xcode derived/products/obj/
precompiled/modules/packages, generated project, temporary files and evidence
all reside below it. Inherited Cargo/Rust wrapper/flags and Xcode output
overrides are rejected before build. Do not point it at Runner/shared target.
`xcodegen` receives a copied project.yml/Tests tree; the source .xcodeproj is not
created or shared. Simulator OS-managed installation/data paths remain external
receiving locations, not build output directories; no simulator cache is erased.

The source manifest hashes every tracked file, including Cargo manifest/lock,
.cargo/config.toml, bundle/lib/Info.plist and XCTest source. HEAD/clean/hash state
is checked before/after build and after each collection. Actual dwarfdump UUIDs
for all architectures and codesign-final executable SHA256 are compared against
the installed app. This collector requires the single arm64 Simulator slice.
Installed bytes are copied to evidence before a later install can replace them.
Each case privacy manifest contains the same source/artifact binding digest plus
run UUID/case, and validates the source manifest and captured artifact bytes.

The copied XCTest source injects only that binding digest into app launch
environment. It is a correlation label, not proof of a loaded image. Each QA
process separately reads its current executable's bounded Mach-O load commands,
emits its actual LC_UUID plus binding/PID, and the collector compares that UUID
against the installed image. Missing/invalid UUID, wrong binding, absent launch
capture or missing real injected-provider preflight fails. Runtime UUID reading,
source-to-app binding, every actual launch and all target builds remain UNRUN.
No signing, trust, real Keychain, SDK API or production input guard is broadened.

Effective Cargo configuration is bounded as well: the collector uses a fresh
run-owned CARGO_HOME (dependency caches are not shared), rejects ancestor Cargo
config files and accepts only the tracked deployment-target/net configuration.
CARGO_BUILD_* / CARGO_TARGET_* aliases, RUSTC/RUSTDOC/toolchain overrides and
Rust flags/wrappers are rejected rather than silently inherited. Unexpected
configuration stops before build; it is not a reason to bypass the check.
The copied XCTest inputs AND generated project are hashed after xcodegen and
rechecked before/after every case. Their separate manifest digest is included
in each privacy manifest with the common artifact binding; this avoids a
circular binary/Swift-source hash and detects edits outside the source checkout.

The existing 6 GiB build guard is copied byte-for-byte beneath this run's guard/
scripts directory and hashes back to the source manifest. Bundle and xcodebuild
commands run through that copy, so its hardcoded relative target/disk.log and
fired sentinel remain within this run. Each case collects the guard log too.
The guard policy and original helper are unchanged. sim.sh's default
target/ios-sim output is not used by this collector; sim-runner.sh is not used
by its XCTest suite. Any separate cargo-test runner lease must audit its own
helper chain and keep its outputs distinct; these commands do not claim that
separate full test/native gate has been run or isolated already.
