# P14 connected notifications — frozen P11 contract integration

Base: dc2511fe695935cdce60ba6bb3cfb4288a425778. SDK draft PR:
https://github.com/shin1ohno/herdr-uikit/pull/10 at
the exact revision in Cargo.toml/Cargo.lock. No SDK8/9 integration.
Do not merge this consumer before separate SDK review and approval.

The source integrates the controller-frozen P11 contract at
f54b1ecc02abd458b428bb2366fe004b5917d2a1 into this owned branch, preserving both
parents' history. Controls mount in the production Notifications page and use
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

## Verification remaining

Source static batch passed formatting, provenance (105 rows), sorted regions,
file limits and diff checks before final integration edits. Native build/test
results are NOT available. Both lockfiles are resolved and the exact SDK pin is
verified by locked offline metadata; no Linux Rust product build was run.

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

Lock resolution completed without compilation: SDK adds UserNotifications 0.3.2;
consumer adds its exact SDK pin and Rodio 0.22.2 / CPAL 0.17.3 / Symphonia 0.5.5
closure. Existing package versions are retained. Unrelated resolver rebinding of
iana-time-zone and winapi-util Windows edges was restored to the prior lock.
Both owned manifests pass `cargo metadata --locked --offline --no-deps` resolution.


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
