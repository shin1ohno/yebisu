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

## Attempt 2 source correction contract

The P15 branch merges exact P11 candidate
`f54b1ecc02abd458b428bb2366fe004b5917d2a1` (PR29, observed 2026-11-13 JST).
This frozen integration input is not a final P11 acceptance or merge. Its delta
from `1721995` contains the device-only Unsupported lint scope (`f718acc`),
header shadow (`b55f48e`, already present), explicit Regular preload intent
(`e402f5b`) and observation-first native oracle (`f54b1ec`). Git reports no merge
conflicts. Manual comparison retains P15 theme intent alongside density intent.
The real Regular-button regression also preserves a stored HighContrast theme.
No SDK pin, modal guard, geometry proposal, gesture default or owner palette changed.

P10 and SH1-151 exact saved-file expectations now include owner-default, matching
SettingsHost's writer. Wrong/missing theme negatives retain full JSON comparison.
P14 candidate `c63987697f5bd61e510b5c00c40ce5623a8ff34a` defines notifications in
`notifications/preferences.rs`; the theme persistence regression uses that encode
shape and checks it survives. P14 is not integrated: its store writer must be
reconciled with this preserving writer when it is merged, never chosen wholesale.
Unknown valid JSON and both supported local themes remain covered.

Under a new controller lease, run `just guarded python3 scripts/ios/p15-check.py`
after the exact-head three gates. It authors real OCR taps of Appearance/High
contrast/Close, five screenshots, background/return and process relaunch. The
checker compares the complete receiving JSON (including P14-shaped notifications
and a future field), ordered click/load/lifecycle receipts and actual terminal
painter receipts while open, closed, returned and relaunched. It uses only p15-qa
settings, archives previous fixture bytes, and saves evidence before assertions.
Its Python fixtures are checker validation, not native or physical proof. The
Rust route, persistence, density and independent 12 model tests remain unrun here.

P11's separate outside-modal real-selector positive control and four distinct
inside-modal consumer receipts remain required. Pinned SDK pre-dispatch refusal
is still unobservable; no scheduling receipt or empty wire can pass that gate.
A separate reviewed observer dependency is required; P15 does not modify SDKs.

## Read-only closure re-estimate from source responsibilities

The old write/CAS/reload estimate is superseded. Remaining work is five concrete
packages, not a measured hours/cost forecast:
1. Owner/SDK authority contract: retained signer, serialized credential/trust/view
   fences and teardown receipt. Current provider returns only a public String;
   admission and implementation belong to the separate dependency owner.
2. Consumer adapter/service: mount the existing pure host_settings Model behind
   that contract, add total 20s/50ms collector, capacity-one scheduling and quarantine;
   availability.rs remains the honest pre-dispatch stop until all inputs exist.
3. Controlled receiver: exercise actual command.rs/reader.py and Rust adapter on
   two specifically leased peers; R01-R16 and actual write/rename/reload mutants.
4. Local-theme verification: exact-head Rust plus p15-check.py/native screenshots,
   including P11/P14 field integration and later final-head reconciliation.
5. Independent review plus owner physical/key/trust checks. No source fixture
   substitutes for these. Runtime latency, signer lifetime and cost are unmeasured.
Only package 4 source corrections are staged by this attempt; packages 1-3 and 5
are not silently removed from P1-P5 acceptance.
