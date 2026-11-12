# P15 candidate integration, not native acceptance

P11 candidate e5d32a6b9066e3b702d42412a9458356993e9143 is incorporated in the
task-owned P15 branch. This is NOT a claim of frozen P11 API or P11 native success.
The subsequently verified PR29 head172199564e37bf7011240f8c2fcc07372abc2363 is
also merged, including its font host-switch fix and additional source tests.
The earlier STAGE.md describes the pre-integration f83d7be stage only.

## Mounted source

The production Settings sheet now routes Appearance to `p15_settings_view.rs`.
Its real buttons update the app-local palette and the existing SettingsHost.
The terminal painter consumes the selected palette on its next paint; default
Theme and its ANSI palette remain unchanged. There is no Follow mode.
`apply_settings`, initial load and lifecycle flush all carry the same theme field.
Explicit default selection before initial load is tracked independently; the store
preserves unrelated known and unknown JSON fields within its 4096-byte limit.

The host section projects current selected target and live connection state each
render. The file path is unknown; no discovery or read is dispatched. A connected
provider reports CredentialUnavailable. P11's new provider still returns
`Result<String, Failure>` from `read_or_initialize`, not retained signing authority
(`settings/key_ops/provider.rs:15-23,35-43`). Neither public-key initialization nor
replacement is called from P15. The fixed reader remains dormant until a separately
reviewed authority/teardown API exists. This is not full two-host read functionality.

The integration also replaces the P11 simulator header reassignment with a cfg-gated
shadow binding, since the original immutable header assignment cannot compile on
the simulator path. No P11 auth/key operation semantics are changed.

## Verification required after this exact source head

Run `just check`, `just lint-ios`, `just test` under the controller's physical-mini
lease and 6GiB guard, one build at a time. Hosted CI is separately authorized.
Tests under `settings/theme_tests.rs` exercise handler/load/background/reopen, JSON
preservation and unsupported persisted themes. `host_settings/view_tests.rs` clicks
the real sheet route and theme control, reads the receiving settings file and checks
selected-host/disconnect projection. These tests are SOURCE, not execution evidence.
The latter uses `P15_FIXTURE_ROOT` for task-local fixture placement when set.

The pure model/command tests remain in the independent harness; invoke that harness
explicitly on the leased Mac if it is not incorporated into the product test graph.
The earlier local Python reader receipt remains reader-only, not Rust/SSH/T2 proof.

Simulator observation can use existing P11/terminal fixture launch infrastructure.
`theme-selected`, `settings-loaded.theme` and `terminal.theme` are fixed enum-key
receipts. Verify actual theme click, next painted palette, background/relaunch load,
and preserved Drag/Manual defaults on the receiving file. Test the receipt checker
against a missing/wrong-theme negative before trusting it. Physical keyboard/IME,
owner trust/key gates and R01-R16 SSH receiving proofs stay separate and unverified.

After P11 freezes or merges, compare its exact final head against this candidate,
resolve any API/layout changes, and repeat exact-full-head native and independent
review gates. A prior patch application check or hosted green cannot replace that.
