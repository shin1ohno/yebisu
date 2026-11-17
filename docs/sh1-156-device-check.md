# SH1-156 device checks

Hardware keyboard and IME steps after a discarded composition. Moved from
[device-check.md](device-check.md) so that file stays within 1,000 lines; the
P8 steps there still hold (P8 second review, step 4).

## SH1-156: tap and Select after composition discard

Run these first; they repeat the P13 owner check steps 7-13 and 21-25.

1. In a disposable terminal, leave a composition discarded (status "Text input
   paused …"). With Start: Drag, tap the terminal once with a finger. Type `a`
   on the hardware keyboard. `a` must arrive; Select must stay off and no cell
   may stay highlighted.
2. Repeat step 1 with a trackpad click.
3. Press Select (it reads "Select: On"), type `b`: `b` arrives and Select turns
   off. Press Select twice (on, then off), type `c`: `c` arrives.
4. Without touching the screen, discard a composition, release every key, then
   type `d`. Record whether `d` arrives and the status line text.

## SH1-156: hardware input after composition discard (partial repair)

These are acceptance steps, not recorded passes. The partial selection/key repair
is not a complete SH1-156 fix. Signing requires the owner's locally unlocked login keychain;
never put the password in a command or send it to a worker.

1. In an authorized disposable Herdr 1 terminal, enter `cat` and Return. Start
   Japanese composition, select terminal text, then type `select-156` on the
   hardware keyboard without tapping the terminal. Record whether the first key
   arrives, whether selection remains, and the exact status message. The required
   result is one copy of the new input and none of the discarded composition.
2. Start another composition, reconnect with the existing reconnect control, wait
   for a coherent terminal frame, then type `reconnect-156` without tapping.
   Also type while reconnect is pending: pending keys must never be replayed.
3. On Herdr 2, enter `cat`, start composition, Detach, reattach, wait for the frame,
   then type `detach-156` without tapping. The first new key must arrive once.
4. Repeat all three using a new software-keyboard Japanese composition; commit
   it without first tapping the terminal. Only the new composition must arrive.
5. Repeat a tab/pane command with its reply deliberately delayed. Type during
   the barrier, then after it resolves. Only post-barrier input may arrive, in
   order. Record device/iPadOS/layout, build SHA, and receiving terminal output.
6. To settle the callback-origin boundary, capture whether UIKit can deliver an
   old `insertText:` after a new KeyDown or `setMarkedText:`. The consumer currently
   receives no composition identity. Host adapter tests cannot prove the ordering
   of physical UIKit callbacks.

7. Repeat Select with no range, Select with a range, and ordinary drag selection.
   Test **Typing: Clear highlight** and **Typing: Keep highlight** from the handoff
   strip. After a fresh Ctrl-A/Left press, Select must be off; only Keep retains
   a copyable highlight. Held repeats and Command-C must not exit selection.
   Change pane/host, reconnect and Detach: neither variant may retain an old
   selection gate. Verify report-all press/release once, then report-all off.
8. Distinguish semantic keys (Ctrl-A, Left) from printable hardware text in English
   and Japanese layouts. Record GPUI focus, native first responder, select mode,
   cancellation state, connection generation and peer receipt when tracing is
   available. A passing Ctrl-A check alone does not pass steps 1-4.
9. While selecting, the status keeps its previous text (Select: On shows the
   selection). During a command, expect “Navigating…”. After Detach, expect “Detached · tap Reconnect to reattach”.
   After a semantic key resumes, “Text input paused” may remain because the IME
   fence is still active; report this as unresolved full recovery. A healthy new
   session with no refusal must not retain a “tap terminal” warning.
10. Start a text paste, then press a new semantic key while preparation is pending.
    Verify paste then key at the same target exactly once. Switching target before
    completion must deliver neither old paste nor held input to the new target.

### SH1-156 D1: clean connection must accept ordinary text

These are required results, not executed device checks.

1. Start a fresh app session with an English hardware layout and no composition.
   Connect to a disposable Herdr 1 terminal. Without tapping the terminal, type
   `fresh-156`, then Ctrl-A and Left. Verify the letters and semantic keys arrive
   once, and the status does not claim text input is paused.
2. Repeat on a new Herdr 2 connection, then Detach/reattach without any marked
   text. New `clean-reattach-156` must arrive without a terminal tap.
3. With no composition, initiate a pane command, type while the command is
   pending, then type `after-156` after the coherent destination appears. Verify
   pending text is absent and new text arrives once at the new destination.
4. Repeat the barrier with marked text before it and with a mark arriving while
   input is refused. Also switch hosts and return from Hosts with a marked setup
   field. Discarded marked text must never appear. Until the SDK retirement
   integration is verified, new composition recovery in these cases remains U1;
   do not mark the original SH1-156 checklist passed from steps 1-3 alone.
5. Record initial versus previously composing session, layout, exact app/SDK SHA,
   command status, target and receiving output separately for each sequence.


### SH1-156 SDK retirement: B1/B2 remain acceptance blockers

These checks are required and unrun on the integration head. Record both app and
SDK SHAs. The authorized SDK prototype is 28a30b72bb3c8de7937344d41571846775f65bfd;
its standalone probe does not certify yebisu's physical keyboard behavior.

1. Run the consumer Simulator probe described in input-retirement-integration.md.
   Save actual native client IDs, responder state, the receiving peer sequence,
   complete stderr and screenshot. Its expected B1 refusal is a product failure,
   even if its defensive no-old-text assertions pass.
2. On the physical keyboard, mark Japanese text, then confirm it through native
   unmark. It must commit once to the current peer, and a second unmark must not
   duplicate it. This is not a cancellation case. Start a new composition without
   a tap. Separately discard a mark using Select, reconnect or Detach, and exercise
   native Paste with a mark active. Record the first missing key/mark, visible
   reason, native identity and exact peer sequence; on the pinned prototype the
   paste may arrive once while fresh IME remains fenced. Do not treat a setup
   failure or wrapper selector injection as physical B1/B2 evidence.
3. Repeat reconnect and Herdr 2 Detach/reattach with new English printable text,
   Japanese composition and identical old/new text, without a terminal tap.
   Keep both highlight variants and report-all modes. Record no replay/reorder.
4. At the native responder transfer, exercise queued old commit/new mark and
   changed/blurred focus. A selector sent to a retained old object cannot prove
   UIKit's destination choice for already queued OS events. Mark B2 unverified
   unless the real routing and receiving evidence settles both retarget and loss.


## SH1-156 — SDK 1ec8df9 physical receiving matrix

These checks remain unverified. Unlock the signing keychain locally, install the
exact PR head with `just ipad`, and record head, device, iPadOS, keyboard layout,
IME and Herdr protocol. Use an already authorized disposable terminal running
`cat`; save the terminal transcript as well as a screenshot for each sequence.

1. With hardware keyboard attached, type `fresh-156` once. Start Japanese marked
   text `旧変換😀`, select terminal text, then type `select-156` without tapping.
   Confirm the first key and entire new string arrive once and `旧変換😀` never
   arrives. Repeat with the runtime Typing Clear/Keep highlight alternatives.
2. While Japanese text is marked, press Reconnect, wait for the displayed current
   terminal, and type `reconnect-156` without tapping. Repeat with identical old
   and new composition text. Confirm only the new composition reaches the peer.
3. In Herdr 2, mark text, Detach, then Reconnect. Confirm Detach does not reopen
   the software keyboard. Type `reattach-156` without tapping after reattachment;
   record old-peer closure and delivery only to the reattached destination.
4. Repeat 1–3 with the software Japanese keyboard, partial emoji edits, IME-first
   hardware printable input and Ctrl-A separately. Native unmark commits once;
   do not treat unmark as a discard operation. Repeat report-all on/off.
5. With no hardware input since launch, mark text and invoke native Edit > Paste.
   Check explicit paste once, no old marked commit, and a fresh composition once.
   Repeat after a hardware press. SDK 1ec8df9 refuses retirement after hardware
   history; record the visible refusal and missing recovery as a failure, not a
   pass. Neither retrying a timer nor tapping qualifies as no-tap recovery.
6. During P7 focus-command and H2 split/orientation barriers, type a distinct
   marker. Confirm it is not replayed after acknowledgement; only fresh input
   reaches the current pane. Repeat command failure and reversed reply/projection
   order with a controlled peer. Save the complete receiver sequence to sentinel.
7. During replacement, change host/focus, background the app, and return. Confirm
   late old callbacks cannot open a newer discard gate or send into another host.
   Exercise delayed OS IME commits across replacement. Retained-old-object
   injection alone cannot establish the OS recipient or no-loss guarantee.
8. Record each failed/unavailable row explicitly. Simulator native subset evidence
   does not replace these physical checks, and unsupported SDK contexts cannot
   be marked successful merely because their rejection preserved the fence.

### SH1-156 evidence record for each physical sequence

Before each sequence, clear the disposable receiver transcript and record the
exact commit, keyboard/IME, protocol, Typing Clear/Keep highlight and report-all
settings. Keep the terminal visible after the discard/reconnect/reattach; do not
tap it, toggle input mode, or restart the app before entering the new marker.

Record the first physical key separately from the full marker, the count of new
commits at the receiver (expected one), and the count of discarded commits
(expected zero). For identical old/new composition text, record the discard and
new composition boundaries on video so one received copy is not misattributed.
Save the receiver transcript and screenshot with the same sequence identifier.
If the first key is lost, a tap is required, or retirement is refused, record
FAIL and the visible status before performing any recovery. An unavailable
physical sequence remains UNVERIFIED; a Simulator probe cannot fill its result.

