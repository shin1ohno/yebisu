# SH1-156 native retirement integration requirements

This documents the yebisu consumer contract and experimental SDK adoption.
The SDK worker owns SDK source; the app integrates only the explicitly authorized
head. Source and controlled tests are not native or OS delivery proof.

## D1 versus U1

D1 is ordinary direct-text loss on a clean initial connection or after a clean
barrier. `TerminalInput::detach` must drop destination/queues/held keys without
inventing a discarded composition. It preserves an actual discard across repeated
detaches and writer failures. A rejected nonempty mark also requires a fence.
The consumer carries that fence across host switches and from a marked setup
field into the terminal. No key clears it automatically. These are app-level
state rules, not proof of a native cancellation boundary.

U1 is safe resumption after an actual composition discard, including hardware
printable keys that UIKit routes before GPUI KeyDown. D1's clean-state distinction
does not solve U1. The ignored new-composition test remains a failure requirement,
and the implemented retirement call sites still lack exact-head consumer native
execution evidence.

## Required SDK semantics

1. Retirement identifies the native client/session that owned the old mark before
   any asynchronous transition. A late callback must not acquire the current
   epoch by reading a mutable global or the current input handler at arrival.
2. An acknowledgement must identify which request/client was retired. The consumer
   can clear only that pending native fence. A later discard, changed focused
   entity, replaced window or newer request must not be released by an old ack.
3. Replacement/native activation must finish before delivering fresh direct text
   and fresh composition, without requiring a tap. No dropping, buffering,
   replaying or reordering of new keystrokes is acceptable as recovery.
4. Old insert, replace, marked update and unmark cannot commit into the new
   session. A new session may intentionally type the same exact text as the old
   one. Content comparisons and timers are not freshness evidence.
5. Keep UIKit-first IME routing, dead keys, native clipboard action origin and
   ordinary consumer defaults. Report text versus key-only responder behavior,
   focus/blur and keyboard dismissal independently; neither Cmd/Ctrl injection
   nor snapshot-empty state proves native session retirement.
6. If new-client replacement needs asynchronous native work, document failure and
   refusal behavior, ownership and acknowledgement ordering. Do not report
   successful recovery from consumer code that only submits a request.

## Consumer integration sites to recheck

- `terminal_input.rs`: `sync`, `discard`, `detach`, `reject_mark`, `retain_discard`,
  `mark`, `commit`, and the legacy `fresh_gesture` callers.
- `input.rs`: all EntityInputHandler callbacks, including rejected marks when
  selection/focus/barrier prevents normal insertion and unmark without a commit.
- `board.rs`: shared native-client fence versus host-local queues, first connect,
  host switch and navigation. Setup-to-terminal ownership changes need the same
  client boundary; a destination epoch alone is insufficient.
- `terminal_view.rs`, `core_view.rs`, `reconnect.rs`: reconnect, native focus
  restoration, Herdr 2 Detach/reattach, background/foreground and error paths.
- `handoff_view.rs`, `actions.rs`, `terminal_controls.rs`: selection, legacy
  tap/Paste/Select-off, P7 barrier and writer failure. Paste FIFO and destination
  revalidation remain independent of native retirement.

## Receiving evidence required before claiming original acceptance

Use the actual SDK native/editing adapters and a private receiving peer. Keep a
trace of client/request/epoch, focused entity, native responder, destination and
barrier state without logging arbitrary user text. Assert the complete received
sequence through a FIFO sentinel or acknowledged drain, not a quiet sleep.

- Clean English printable key plus its direct insertion callback on initial
  Herdr 1/2, after a clean P7 barrier and after clean Detach/reattach.
- Real marked text before and during barrier, rejected selection/blur mark,
  repeated detach, host/setup transition, writer failure: discarded text absent.
- Old native client callbacks before/after retirement, new client with identical
  text, new software mark/partial emoji edit/commit/unmark exactly once.
- Japanese hardware printable through UIKit-first routing as well as Ctrl/arrow;
  report-all on/off, repeat/release, Copy and native edit remain independent.
- P7 success/failure/no-op and response/projection order; P9 old/new destination
  and focus, host switch, setup isolation, pending paste cancellation and FIFO.
- A native Simulator run and screenshot tied to the exact consumer and SDK heads.
  Synthetic callbacks establish fence behavior only. Any physical UIKit delivery
  uncertainty remains unverified, not a relaxed Z1-Z7 criterion.

`direct_text_recovery_tests.rs` adds D1 fixtures without calling fresh_gesture.
`discard_recovery_tests.rs`, `discard_connection_tests.rs` and the preserved
`handoff_resume_tests.rs` cover the existing app-side portion. Their presence is
not execution evidence. All must run with exact-head Mac gates after an exclusive
build lease. `check-discard-mutations.py` requires each expected assertion reason;
a fixture timeout or compiler error cannot certify a mutation negative.

## Authorized SDK adoption and implemented consumer boundary

The controller authorized exact SDK PR 9 head
`28a30b72bb3c8de7937344d41571846775f65bfd` for implementation/controlled tests.
Cargo.toml and Cargo.lock pin that head. SDK-native tests exist at that exact
head, but independent review and queued OS retarget/loss are separate gates.
This is experimental adoption, not approval to merge or a complete U1 repair.

`input_retirement.rs` captures request serial, host, connection generation,
board selection, discard revision, destination, lifecycle and backend owner.
`native_retirement.rs` binds a weak entity to its window and calls the public
retirement API after a complete consumer transition. The callback verifies
retired/replacement IDs, current owner and focus before acknowledging one discard
revision. Registration precedes the API call because callback delivery can precede
the receipt return. The receipt's Pending, WindowClosed and failed acquisition
statuses are distinct from acknowledgement. Nested host/connect operations submit
one request for their final owner; no timer retries or input replay were added.

Production tap/Paste/Select-off no longer call fresh_gesture. That method now
exists only in host test builds for old fixture setup. The Simulator QA mark
button also cannot clear a fence. Clean fresh connections keep D1 behavior.
The legacy D4 test now requires an explicit controlled retirement boundary and
rejects its formerly admitted old text. The original ignored new-composition
regression remains unchanged and must still be explicitly executed.

`input_retirement_tests.rs` exercises ownership changes, stale/native-ID mismatch,
callback-before-receipt registration, duplicate completion, refusal and serial
exhaustion. Its receiving-peer case intentionally uses identical old/new emoji
text, with one new commit and no discarded commit through a FIFO sentinel.
`terminal_input_tests.rs` checks revision preservation and saturation. These are
implemented controlled tests; they have not been run on the new app head yet.
They never stand in for native execution.

## Actual consumer native probe, prepared but unrun

The opt-in `retirement-probe` feature and `YEBISU_RETIREMENT_QA=1` run
`retirement_qa.rs` inside the normal UIApplicationMain app. It installs a private
recording peer and waits for the actual native responder. Its legal Select
transition enters the real platform callback slot through a reserved probe key;
that injection is explicitly not physical hardware input proof. It retains old
native objects, invokes their mark/insert/unmark/delete/edit selectors after
retirement, and verifies a new client's identical composition/commit. The probe
then separates positive native `unmarkText` commit from native Paste B1 refusal.
With a focused, ready, unfenced peer, unmark commits `b1-unmark` once; a second
unmark has no marked range and adds nothing. The exact peer sequence through the
FIFO sentinel is `TextCommit("same😀")`, `TextCommit("b1-unmark")`,
`Paste("native-paste")`. The original discarded `same😀`, old-object selectors,
`b1-paste`, and the refused post-Paste text must never add events. The Paste setup
requires ready/current destination, focus, no handoff gate, no worker and no
unfinished cancellation; failure of these preconditions is not B1 evidence. It reports `CONSUMER_NATIVE_PARTIAL_PASS`
with `B1_BLOCKED B2_UNVERIFIED`, never complete SH1-156 acceptance.

After an explicit MINI lease, run the normal exact-head gates, all 16 mutations,
and the ignored original regression. Also clippy the Simulator with
`--features retirement-probe`. Build the probe with the existing guarded
`bash scripts/ios/bundle.sh --package yebisu --bin yebisu --features retirement-probe`.
Launch it using the existing sim.sh runner with
`SIMCTL_CHILD_YEBISU_RETIREMENT_QA=1` and `SIMCTL_CHILD_YEBISU_SMOKE_OBSERVE=1`.
Do not combine other QA modes. Save full launch stderr, receiving observation,
screenshot, exact app/SDK heads and guard output locally before lease release.
Probe frame callbacks schedule test work and observation only; they do not retry
failed production retirement, resend a key or clear cancellation state.

## Remaining API gap: precise B1 case and minimal SDK request

Positive trace at pinned SDK28a: `NativeClient.unmark` (`probe.rs:37`) ->
ObjC `unmarkText` (`view_class.rs:316`) -> SDK editing helper
(`editing.rs:214-226`) -> handler `replace_text_in_range` -> consumer
`input.rs:145-165` -> `TerminalInput::replace/commit` (`terminal_input.rs:153-178`).
It restores selection and refreshes the snapshot after replacement. It does NOT
call `Smoke::unmark_text` (`input.rs:134`), a separate consumer discard entry.
Do not change SDK unmark defaults to make a cancellation oracle pass.

B1 trace: native Paste (`SDK text_input/actions.rs:82-105,127-142`) -> guarded
KeyDown dispatch -> `terminal_key_down` (`terminal_controls.rs:91-130`) ->
`start_paste` (`handoff_view.rs:52-78`) -> discard -> outer `input_transition`
(`input_retirement.rs:53-60`) -> `retire_text_input_session`. SDK
`text_input/retirement.rs:26-33` refuses the busy client before replacement.
The consumer keeps the fence and the explicit native-callback-active reason.
The test helper invokes Copy/Cut/Paste/SelectAll; with no selection Copy/Cut
cannot produce terminal text and SelectAll is consumed. This remains selector
injection, not a physical menu or keyboard run. Both paths are UNRUN here.

The minimal requested SDK capability is an explicit retirement request usable
from its real native text/key callback entry, with a receipt distinguishing
accepted-pending from refused. The SDK must own the exact boundary after the old
callback/continuation finishes and before replacement accepts input, preserve
callback slots/focus, and permanently fence the captured old client. It must not
ask the consumer to guess a delay, replay the triggering key, or clear state
without acknowledgement. Its native receiving test must establish old callbacks
cannot retarget and new input is not lost. If this contract is impossible, report
that limitation; an application timer cannot turn it into original Z1 acceptance.
B2 queued OS retarget/loss remains unverified regardless of selector probe results.

## Pending whole-operation adoption contract (requirements, not an API)

No new API is admitted or implemented by this document. SDK28a stays pinned.
SDK789's reviewed T1 responder acquisition does not solve consumer B1 or B2.
Implementation must wait for the controller's exact reviewed SDK commit and
public types, entry-classification table, cancellation/legacy transition table,
and receiving evidence. The following names describe states, not callable APIs.

| Consumer site | Existing behavior to preserve | Required handoff before new implementation |
| --- | --- | --- |
| `input_retirement.rs:36-60` | Final owner captured after nested app transitions | App depth zero is not the whole native-operation boundary; enqueue only at a classified SDK entry. |
| `native_retirement.rs:20-43` | Register serial/ticket before submission; window binding | One pending request per window; identify immutable old client at submission. Queued is distinct from legacy Pending and refusal. |
| `native_retirement.rs:43-74` | Weak entity/window/focus checks | A prepare callback runs only after SDK handler/slot/RefCell/AppCell borrows and the native tail end. Recheck full owner before allowing cutover. |
| `input_retirement.rs:95-126` | Match pending ticket, retired/replacement IDs, owner/revision | Revalidate at terminal completion too; release only the matching fence once, after replacement readiness, never on queue acceptance. |
| `terminal_input.rs:90-138` | Carry discard revision across detach/host; saturation stays closed | Do not refresh a stale epoch or infer freshness from text. Failed/declined/cancelled requests retain the fence. |
| `handoff_view.rs:52-78`, `terminal_controls.rs:91-130` | Real native Paste discards mark and preserves paste FIFO | Native KeyDown/KeyUp/modifier restoration/fallback/snapshot are one accounted operation; no early drain after one dispatch. |
| `board.rs:143,217`, `terminal_view.rs:66,91`, `core_view.rs:84,131`, `reconnect.rs:35`, `actions.rs:126` | Host, connect, Detach, navigation, lifecycle and P7 change owners | Invalidate a queued ticket on owner change; prepare declines it and SDK releases its pending slot. No implicit retry or migration to the new owner. |
| `input.rs:134,145,181` | Distinct consumer discard, commit, rejected mark callbacks | Default native unmark remains commit. Accepted-new text must not erase an unrelated discard; no arrival-time current-client relabel. |

Owner correlation includes window binding, weak entity, focus, request serial,
old native client ID, host, connection generation, board selection, discard
revision, destination boot/epoch/target, open/suspended and backend. Future SDK
IDs need the reviewed identity contract, not an invented successor arithmetic.
A consumed completion/end cannot acknowledge again, reopen a newer revision or
be reassigned to another input owner. The existing controlled tests are only
app-level ticket checks, not proof of the proposed SDK completion ordering.

A queued request must document when the old token is fenced. For a proposed
commit-time fence, only the already admitted whole-operation tail may finish;
a later independent/reentrant native entry is classified separately. If instead
submission fences immediately, no old continuation may be re-enabled. The SDK
must account for KeyUp/modifier restoration/native Paste fallback, unmark's
selection/snapshot tail, and hardware held-route/repeat/forward work. Future
Ended/Cancelled/repeat must be consumed exactly once by the old origin or refused
before side effects under the reviewed entry table; it cannot fall through to a
fresh target. Per-dispatch busy count or GPUI DEPTH=0 alone is insufficient.

Prepare must decline a changed owner, closed/moved window, lost weak entity,
wrong focus, stale serial/revision or old-client mismatch. Decline/cancel/close
and acquisition failure need distinct terminal outcomes and an observable reason.
The consumer's logical invalidation is separate from dropping/cloning a receipt;
neither receipt operation cancels or duplicates SDK work. Define opt-in-to-legacy,
legacy-to-opt-in and completion-to-other-window legacy behavior before adoption;
one pending/window and bounded draining must hold without silently changing the
legacy default. No new implementation of these transitions exists in this patch.

Required exact-head native negatives: early completion after KeyDown; incomplete
KeyUp/modifier/fallback/snapshot; borrowed handler or callback slot; reused old
origin; owner change between queue/prepare/complete; duplicate or consumed end;
close/repeated request; nested independent callback; receipt drop/clone; legacy
cross-window request; failed focus acquisition. Each needs baseline, one exact
mutation, its specific assertion, exact restore and restored pass. A setup or
compiler failure, an earlier unmark assertion, a timeout, or PARTIAL_PASS in a
mutant log cannot certify its intended kill. The Rust classifier additionally
requires named tests to pass in baseline/restore (zero matched tests fails) and
the reason inside the single named panic. Its Python fixtures prove parsing only.
There is no executed consumer-native mutant suite in this change.

B2 needs real OS queued-retarget and no-loss evidence plus fresh software input,
hardware/IME-first delivery and NoOldMarkedSending on the current peer. Retained
selector injection and any new SDK operation boundary do not settle these gates.
Original Z1-Z7, the ignored U1 regression and exact-head Mac gates remain required.
