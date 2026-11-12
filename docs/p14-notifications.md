# P14 connected notifications — nonfinal integration

Base: dc2511fe695935cdce60ba6bb3cfb4288a425778. SDK draft PR:
https://github.com/shin1ohno/herdr-uikit/pull/10 at
`a157c4b87cde721ac6aba6948c09711de60c9fdb`. No SDK8/9 integration.
Do not merge this consumer before separate SDK review and approval.

The current source adapter mounts controls on Hosts and polls from the existing
33ms app timer. It is an integration patch against the verified main reference,
not the frozen P11 settings sheet. The controller will provide the stable P11
API/head. Other writers' repositories and branches are untouched.

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
64 single-use response records with 5-minute expiry, one in-flight OS request,
one waiting sound job, 15s playback deadline and cancellation every 25ms.
The source has no remote sound path, URL, shell execution or payload logging.
Quiet hours, mute, permission and overload failures are explicit and not retried.
Response handling locks the current reader, checks host/boot/generation/coherent
projection/pane, then enqueues the existing focus command under the same lock.
It never starts a connection. Invalid/busy/unknown/action responses show status.

Settings use the existing background persistence worker and pre-load edit
tracking. Real consumers exist for each exposed control. Production sheet/modal
integration and narrow-layout behavior remain pending P11's stable contract.

## Verification remaining

Source static batch passed formatting, provenance (105 rows), sorted regions,
file limits and diff checks before final integration edits. Native build/test
results are NOT available. Lockfile regeneration needs an authorized guarded
native dependency resolution; no Linux Rust product build was run.

Under explicit controller lease: regenerate only changed dependency closure,
run exact-head guarded check/lint-ios/test, then run private-peer SemanticNotification
through real native authorization/submission/response APIs. Record actual native
receiving callbacks and negative mutations for stale generation, old boot,
disconnected, incoherent projection, denied permission, duplicate, flood, mute,
quiet hours and cancellation. Model tests alone cannot prove OS delivery.

Physical grant/denial, real volume/silent mode, background/suspension and signing
remain owner gates. Never answer dialogs or alter permission databases.
