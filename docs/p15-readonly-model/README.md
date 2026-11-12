# P15 read-only model preparation

This is an isolated source deliverable, not a network/UI feature or native acceptance.
The app does not register these modules yet. No Linux Rust builds were performed.

## Provenance and authority

Baseline yebisu dc2511fe695935cdce60ba6bb3cfb4288a425778:
- `crates/yebisu/src/theme.rs:18-46`: the only implemented palette is the owner default.
- `crates/yebisu/src/settings.rs:44-109`: SettingsHost draft/revision and serial worker.
- `crates/yebisu/src/settings/store.rs:9-58`: existing JSON persistence owner.
- `docs/p10-settings.md:18-25`: per-host settings and theme assignment.

Read-only reference `shin1ohno/herdr-gpui` at398c132a06723a0cc1494842a34666d173ec0200,
`crates/sheprd-gpui/src/herdr_settings/remote.rs:14-27,53-69`: bounds, path provenance,
and pane_history field. Its save/CAS code is deliberately not imported. This is
original implementation, not a copy of its lenient parser or racy shell reader.
Owner D5: task71c4ec6ae3c444a6/evidence/A.md:10,14 and D.md:21 override old P15
write acceptance. Never add save, CAS, reload or remote editing.

## Entry points for the P11 integrator (not applied)

1. Register `mod host_settings; mod theme_picker;` in the consumer root, add the pinned
   `toml` dependency consistent with the harness, map model errors into crate Error.
2. Keep one Model for the view lifetime. On host switch/disconnect cancel the worker
   and invalidate state BEFORE checking/resolving the next path. Missing path uses
   `Error::UnknownPath`; do not leave an old host visible if `ConfigPath::new` fails.
   On every new transport use a new connection generation, including reconnects.
   `Model::begin` assigns a monotonic request ID; the adapter captures the entire
   Request identity and echoes it. Never reuse Model while old channels can deliver.
3. A future reviewed worker uses queue capacity1 and latest-state completion mailbox.
   `Model::complete(snapshot, Instant::now())` rejects stale/colliding/duplicate replies.
   Schedule `expire` independently of network traffic. Model bounds total20s; worker
   must also bound idle20s and accumulated bytes65536 BEFORE parser allocation.
4. Render `State`/`FileState` as read-only values and fixed diagnostic strings, including
   disconnect, missing and parse/error. Show exact path and host, label file settings
   versus unverified daemon-effective state. `pane_history: None` means unspecified.
5. P11 adds a typed Palette to its own draft/Edited tracking, preserving pre-load edits.
   `read_local` parses existing JSON; `patch_local` changes only the theme key in a
   serializer-owned JSON object. Preserve all unrelated keys with the same writer.
   These helpers must never be attached as an independent persistence worker.
6. In the renderer map `Palette::OwnerDefault => crate::theme::Theme::default()`.
   Existing colors/keys/defaults are untouched. The catalog has exactly one implemented
   palette; this does not claim alternate palettes or visible theme switching.
   A later implemented palette needs explicit rendering values and tests before listing.
7. Keep Follow host absent: there is no host-to-local-theme conversion or precedence.
   Use SettingsHost edit/poll/flush/retry/background acknowledgement lifecycle;
   restart/background persistence must be observed after integration, not inferred
   from the JSON round-trip test.

## Validation and remaining gates

The exact modules have separate Rust tests in `host_settings/tests.rs` and
`theme_picker/tests.rs`. Their isolated manifest is
`crates/yebisu/src/host_settings/harness/Cargo.toml`; it has no GPUI dependency.
Run only with controller-approved Mac scope through the existing guarded recipe.
No standalone harness pass establishes app registration, UI, transport or device QA.
The source-only Python check verifies fixture TOML, baseline preservation and a
negative write-vocabulary mutation; it does not execute Rust or prove outbound IO.

Current proposal is `security-plan-2.md`, awaiting independent re-review. The original
`security-plan.md` is preserved after REQUEST_CHANGES. A plan PASS and actual T2
receiving IO proof are separate; neither is claimed here. No network implementation.
Remaining full P15 gates: controlled two-host actual outbound no-write evidence,
symlink/TOCTOU handling, credential/trust isolation, exact-head guarded native
check/lint-ios/test, reviewer approval, actual local theme persistence and owner QA.
