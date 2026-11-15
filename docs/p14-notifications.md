# P14 connected notifications — frozen P11 contract integration

The SDK side (herdr-uikit PR #10, P14 response lifetimes) is merged. This
consumer pins herdr-uikit `1e9f6094c394ec79fb88b966e7a02a9aa5bb753b` (main's pin,
which contains PR #10 and e571546) in Cargo.toml and Cargo.lock; earlier
heads of this PR pinned e571546 and then 7f70d2d. No SDK8/9 integration. The
consumer is merged only after independent review and owner approval.

The source integrated the controller-frozen P11 contract at
f54b1ecc02abd458b428bb2366fe004b5917d2a1 into this owned branch; main's final P11
(PR #49) has since been merged over it and this document's P14 parts sit on top. Controls mount in the production Notifications page and use
the sheet's existing button style/focus/save owner. The existing 33ms timer
polls notifications. Other writers' repositories and branches are untouched.
Combined native verification is pending; P11's prior gates do not prove this head.

## Routes and authorization

- Input is connected SemanticNotification, captured by the single reader only
  with coherent metadata/surface, bounded IDs and current pane membership.
- Active: foreground toast or OS local route, never both; independent built-in
  sound worker. Remote title/body is discarded; presentation uses kind text.
- Inactive: connection policy is unchanged; foreground effects are cancelled.
  New connected events may use the explicitly selected OS route, without sound.
- Background/Disconnected: existing policy closes connections. Clear pending
  effects/registrations and cancel audio. No background keepalive or APNs.
- Suspended/terminated: no receiving loop or delivery claim. Refresh status on
  return; old tags do not reconnect or replay events.
- OS permission is queried from UserNotifications. Only an explicit user click
  requests it. Denied, NotDetermined, Provisional, Ephemeral and Unknown are
  distinct typed states. Native acceptance is not banner display proof.

Defaults derive from reference 398c132a config/notifications.rs: Off delivery,
1s delay, BottomRight toast, quiet hours unset, volume 50%; shared sound defaults
enabled (herdr_settings.rs:230-235). Mute starts false. The 22:00–08:00 control
is an opt-in preset, not a new default. No runtime feel fork is introduced.

## Bounds and failure semantics

32 reader effects, 32 delayed effects, 5s maximum age, 2s deduplication window,
64 single-use response records with 5-minute expiry from the original event, one in-flight OS request,
one waiting sound job, 15s playback deadline and cancellation every 25ms.
The source has no remote sound path, URL, shell execution or payload logging.
Quiet hours, mute, permission and overload failures are explicit and not retried.
Response handling locks the current reader, checks host/boot/generation/coherent
projection/pane, then enqueues the existing focus command under the same lock.
It never starts a connection. Received invalid/busy/stale/action responses show
status. The SDK routes only its recent issued tags; unissued/evicted/out-of-window
tags fail closed with a typed SDK rejection. Its separate tag-only receipt cache
(64 entries, 600 seconds, lazy expiry) allows late/duplicate refusal without
retaining display predicates or extending the 300-second action window. The
consumer rechecks current quiet/mute policy at response time. A valid six-second
tap reaches this guard even though the five-second display fence has closed.

Settings use the existing background persistence worker and pre-load edit
tracking. Real consumers exist for each exposed control. Notification taps while
the sheet is open are consumed with Busy status in its header and no focus
command. The overlay remains below the modal. Combined native/modal/narrow-layout
behavior still requires receiving-system verification.

## Verification status

Native results exist from the Mac mini under the exclusive lease (task 8910aef8de114bc9):

- PR #45 head 03391ad / SDK 7f70d2d (attempt 4): `check`, `lint-ios` (device and Simulator
  targets) and `cargo test --workspace` (284 tests) passed; the SDK Simulator notification tests
  passed 13 with 1 ignored; the D4 display-fence and D5 six-second-response mutants failed at the
  intended assertions and were restored byte for byte; the six consumer response tests passed.
- Head 9f97578 (attempt 5): the same gates passed again (291 Rust tests); Python `scripts/ios`
  tests that import `tomllib` could not run on the mini (Python 3.9) and run in hosted CI.
- Head 3651478 / SDK 1e9f6094 (attempt 8, after merging main with the final P11 and the herdr-gpui pins): `check`, SDK Simulator tests (13 pass/1 ignored, D4 and D5 mutants killed and restored, 6 consumer tests), the real-app `p14-check.py` on a disposable Simulator with the keyboard up (toast above it, status expiry), `lint-ios`, `cargo test --workspace` (319) and `scripts/tests` passed; only the `tomllib` TestFlight Python tests failed on the mini's Python 3.9.
- The real-app gate `scripts/ios/p14-check.py` (XCTest `testP14ConnectedNotifications`) passed earlier at
  6d924fc (software keyboard hidden by the harness, no toast-position measurement) and at f164c79
  (keyboard up; toast above it). The runs between them failed on the oracle for harness timing; the
  full history is in docs/p14-native-qa.md. Hosted CI is green on c10e1df and f164c79.

None of this is OS banner, sound, tap, suspension or physical proof (still UNRUN), and heads after
a layout or status change must be re-measured.

The consumer response tests in `notification_view/response_tests.rs` are compiled
only for `target_os = "ios"` (they call the SDK's iOS response dispatch), so the
hosted macOS host `check` job does not run them; the Simulator run above is their
evidence.

Under explicit controller lease: run exact-head guarded check/lint-ios/test,
then run private-peer SemanticNotification
through real native authorization/submission/response APIs. Record actual native
receiving callbacks and negative mutations for stale generation, old boot,
disconnected, incoherent projection, denied permission, duplicate, flood, mute,
quiet hours and cancellation. Model tests alone cannot prove OS delivery.

Physical grant/denial, real volume/silent mode, background/suspension and signing
remain owner gates. Never answer dialogs or alter permission databases.

The reader and Link invalidate a shared notification generation immediately
on disconnect, mismatch, reset or drop. Native submission additionally rechecks
this proof, event age and the coherent inbox after authorization and after OS
completion (`post_notification_when`). Sound checks that same generation in its
worker, independently of the UI poll cadence.

Lock resolution (measured against main 68fd709, Cargo.lock): 40 packages are added
(UserNotifications 0.3.2, Rodio 0.22.2, CPAL 0.17.3, Symphonia 0.5.5 and their
closure); no existing package changes version and none is removed. Existing packages
whose dependency edges changed: herdr-uikit (new source ID and the
UserNotifications edge), yebisu (chrono, rodio), objc2-core-foundation (block2,
libc), objc2-foundation (block2, libc, objc2-core-foundation), and two Windows-only
edges that the resolver rebinds when the new dependencies are added: iana-time-zone
(windows-core 0.57.0 to 0.62.2) and winapi-util (windows-sys 0.48.0 to 0.61.2).
Neither Windows edge is built for the iOS or macOS targets.

## Review repair D1-D4 (source stage only)

P11 f54b1ecc02abd458b428bb2366fe004b5917d2a1 is merged, including explicit
Regular intent and observation-first tests; see p11-observer-dependency.md.
P10 and SH1-151 exact-file oracles include notification defaults, backed by a
fixture whose full object is compared with the actual Rust writer in a new test.
The SDK retains per-tag context through willPresent and consumes responses once.
The consumer removes retained tags on drop and reuses the current host/boot/pane,
connection activity, event-age and quiet-hours predicate in the audio worker.
Policy/lifecycle changes cancel queued and playing audio; native audio remains
unverified. Permission results carry actual queried OS state, not a delivery proof.

Native obligations: exact-head three gates on both targets, writer-fixture Rust
regression, Regular production callback tests, SDK callback-order tests, native
center/delegate entry plus show/no-show receipts, predicate-removal mutant failure
and restored pass, foreground/Inactive/Background/suspension, quiet/mute/sound,
permission denial and stale tap. Background OS delivery may bypass willPresent;
no claim of cancellation/display correctness is made without native evidence.
P11's modal-refusal observer is a separate missing SDK dependency. All original
N1-N5 and physical owner gates remain unverified until their evidence is recorded.

## Review repair B1, S1-S6 (attempt 5)

Independent review of 03391ad found one blocking and six should-fix items.

- B1 (reader lock contention cancelled valid notifications): fixed. A failed
  `try_lock` of the reader inbox, and a snapshot that is waiting for its surface, are
  `Verdict::Undecided`, never `Invalid`. The 33 ms scan only cancels on a decided
  mismatch; a waiting event stays queued until it is decided or ages out (5 s,
  then `Stale`). The sound worker and the SDK display predicate share one
  `Validity` latch: it starts valid, contention keeps the last decided answer, and
  only a decided mismatch revokes it (sticky). Tests hold the inbox lock from
  another thread (`notifications/tests.rs`, `notification_view/tests.rs`).
- S1 (terminal closed to the Hosts list): fixed. The current host's reader is drained
  like a hidden host while its terminal is closed (clipboard effects are dropped, as
  for hidden hosts), so notifications are collected and delivered.
- S2 (refused tap): fixed. A refused tap never closes the open terminal. A transient
  `Busy` (reader busy or navigation pending) returns the single-use tag; every other
  refusal still consumes it, so a stale or duplicate tag cannot be replayed.
- S3 (snapshot/surface mismatch at capture): fixed. Capture needs connectedness, the
  snapshot boot and pane membership only, as the reference implementation checks
  connection only (herdr-gpui 398c132a state.rs:755-777). The coherence check
  remains at delivery time and is retried, not dropped.
- S4: this document and the PR body now state the measured lock facts and the
  current verification status.
- S5: the bundle ships the license texts (`Licenses/` in the app: repository Apache-2.0
  LICENSE, Rodio MIT, Symphonia MPL-2.0 and a notice with source locations);
  `scripts/ios/test_bundle.py` asserts them.
- S6: five negative cases were added to `test_p14_check.py` (backlog replay, focus boot,
  per-reason rejections, disconnect, late tap); `notification_lifecycle` is tested
  directly. The real-app gate `p14-check.py` stays unrun until the controller grants
  the Mac mini.

Nits: n4 is corrected (40 packages). n1-n3 and n5 are not changed: they are
diagnostics and documented limits, not behaviour defects.

## Re-review repair R1-R3 and toast placement (attempt 6)

- R1: a mismatch already decided by the newest snapshot is `Invalid`, even while that
  snapshot waits for its surface (a closed pane, a new boot). `Context::validate`
  checks host, generation, boot and pane against `pending_snapshot` (or the snapshot)
  first and reports `Incoherent` only for a target the newest snapshot still holds, so
  the cancel bound no longer depends on the server sending the surface.
- R2: a waiting event that cannot be decided yet no longer holds back the queue: each
  event is decided on its own, an undecided one stays queued (FIFO order kept) and ages
  out as `Stale`, while events for other hosts are delivered in the same tick.
- R3: the predicate shared by the sound worker and the SDK display decision is one
  function, `display_predicate`, exercised on the host under a held reader lock (1000
  calls stay true), revoked by a decided mismatch and sticky afterwards.
- Toast placement (owner decision): the toast and its status text sit above the software
  keyboard. The overlay's bottom offset is the window's keyboard inset
  (`window.insets().ime.bottom`, set by herdr-uikit from the keyboard frame) and is zero
  when no software keyboard is shown, so layout without a keyboard is unchanged. The
  real-app XCTest raises the keyboard and fails if the recognized toast text lies below
  the keyboard's top edge (`p14-toast` screenshot, taken with the keyboard raised by a terminal tap).
- r2 (nit): the closed-terminal test also pins that clipboard effects are dropped.
  r1 (nit, a Busy tap returns the tag but the toast is gone) is a limitation: the tag is
  only kept for the 33 ms scan and the OS notification, not for a new toast.

## Status line lifetime (attempt 7)

- F1 (owner decision): the on-screen status line expires after five seconds, like the toast
  (`STATUS_TTL`). The text stays in the notification state, so the notification settings page
  still shows the last result, but the bottom-right overlay draws nothing once the toast, the
  status and the QA controls are gone, so no filled box sits over the terminal's bottom row.
  Tests: `status_line_expires_like_the_toast_but_the_settings_text_stays`,
  `nothing_is_drawn_over_the_terminal_without_a_toast_status_or_control`,
  `expired_status_is_redrawn_away_by_the_notification_tick`.
- F2: the verification paragraphs above and in docs/p14-native-qa.md now list what ran at which
  head and what stays UNRUN. The overlay can still cover the right end of the bottom row while a
  toast or a fresh status is shown (five seconds at most).
