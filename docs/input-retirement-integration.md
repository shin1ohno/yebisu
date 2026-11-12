# SH1-156 native retirement integration requirements

This is the yebisu consumer contract, not an existing SDK API or a native proof.
The SDK retirement worker owns its implementation and branch. Integrate only a
concrete reviewed commit/API relayed by the controller; do not assign guessed
symbols or claim that a proposal is available in the pinned SDK.

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
and legacy tap/Paste/Select-off still lacks native composition retirement proof.

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

## SDK PR 9 candidate: adoption gate

The controller relayed experimental commit
`e02ca054eccd2c73b91d4a072ff91443f1db75bf` from SDK PR 9. It is a prototype,
not reviewed native acceptance. Its `src/text_input_retirement.rs` exports
`retire_text_input_session`, a receipt with window-local retired/replacement IDs,
and `text_input_session_state`. `src/window/retirement.rs` fences the old client,
then invokes the consumer callback after the GPUI update, before replacement
responder acquisition. `src/window/text_input/retirement.rs` refuses an active
native callback with `NativeCallbackInFlight`; `Pending` and failed acquisition
are explicitly not readiness.

Before adoption, test these consumer-specific consequences against the concrete
reviewed SDK head:

- Mouse/menu selection, reconnect completion and Detach can request retirement
  only when the SDK accepts the actual calling context. Key/text callbacks must
  preserve refusal; do not replace it with deferred key replay or a timer reset.
- Match the retired/replacement IDs and a consumer discard revision captured at
  request time. An old callback must not release a newer discard, focus change,
  host transition or closed view. Keep destination and paste FIFO checks intact.
- Clear old marked state inside the accepted callback using weak entity/window
  handles. Observe the receipt and current native responder separately before
  reporting recovery; the callback runs before native acquisition completes.
- Keep failed acquisition, window closure and duplicate/in-flight refusal as
  explicit outcomes. A nested native event loop can expose a retired client
  before replacement; test fresh-input loss and OS-retargeted old commits with
  receiving evidence, including identical strings and IME-first hardware input.

This head does not pin that SDK or release the existing U1 fence. SDK-native
verification and independent review are still required. A source-only protocol
model cannot certify OS delivery or turn the ignored new-composition test green.
