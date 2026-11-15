# P11 settings sheet — verification status and known gaps

This page is the single current statement. Older P11 pages (`p11-settings.md`,
`p11-sdk11-adoption.md`, `p11-security-fixtures.md`, `p11-publication-reproducers.md`,
`p11-final-correction.md`) were written before the Mac runs and may still say UNRUN;
where they disagree with this page, this page wins.

## What has run

- Mac mini, head `c8379979be4405f7f5f8544038cd908d64533fd7`: `just check`, `just lint-ios`
  and `just test` passed (Rust 289 passed, 2 ignored). The native collector
  `scripts/ios/p11-observer-check.py` ran its four XCTest cases (Text, Drawing, Modal,
  Disabled) with 0 failures and recorded observed controls for all four.
- The collector always exits non-zero and writes `acceptance.json` with `accepted:false`.
  That is by design: collected controls are evidence, not acceptance. Full native acceptance
  stays closed; it is not a pass, and nothing here claims a device or Keychain result.
- Hosted CI covers `check`, `lint-ios`, `test`, `simulator-smoke` and `edit-menu`. It does
  not build or lint the `test-support` feature (the P11 observer, `native_observer_qa.rs`,
  `settings_sheet/observer_qa.rs`, `mock_peer.rs` observer hooks). Only the Mac mini collector
  compiles that build, so a break there is visible only on a leased mini run.
- `scripts/ios/p11-check.py` is the legacy probe. It is kept as the historical 0/4 failure
  oracle and is not an acceptance route; the collector above supersedes it.

## Known race B2 (tracked in Linear SH1-309)

Input that the core worker already accepted before the sheet opened can still reach the
terminal after the sheet opens. `begin_settings_modal` advances `input_epoch`, but
`core/worker.rs` compares the epoch and then calls `CoreTerminal::input`, and the pinned SDK
transport queues and writes independently, so a frame already past the comparison (or already
dequeued) is not cancelled. The two reproducers (`f19_a_after_compare_reproducer`,
`f19_b0_after_dequeue_reproducer`) are `#[ignore]` and are not part of `just test`.

P3 is therefore stated as: input typed **while the sheet is open** never reaches the
terminal (every ingress is gated on `settings_modal`); input accepted **before** the sheet
opens may still be delivered after it opens. Closing the race needs an SDK writer fence and is
a separate ticket (SH1-309), not part of this PR.

## Device-key error classes (SH1-299 S4)

`key_ops/provider.rs` maps the SDK's device-key errors to `Failure::Locked` and
`Failure::Missing` by matching the typed variants on iOS
(`Error::Ssh(InProcess(DeviceKey(Credentials::DeviceKey(device_key::Error::Locked | Missing))))`),
not by comparing display text. A renamed or removed variant is a compile error in the iOS
build (`just lint-ios`, part of the hosted `check` job). The device-key module exists only for
iOS (and the SDK's own tests), so no host test can reach this mapping: it is verified by the
iOS build, not by a unit test. An earlier commit (`7c37369`) and an earlier version of this PR
claimed a host test pinned the SDK strings; that test compared yebisu's constants with identical
literals and could not fail on an SDK wording change. It was removed.

## Hidden control

The terminal family picker is hidden (`FAMILY_PICKER = false` in `settings_sheet/view.rs`):
the design shows a family list only once a supported installed-font picker is proven, and a
proportional face breaks the cell grid. Font size and density stay adjustable. A stored
family is still preserved and applied; the persistence path is tested by calling the
listener's entry point directly (`settings/family_tests.rs`).

## Owner gates that stay open

Physical device checks and real Keychain persistence (`docs/device-check.md`), and an
independent review of the final head. The Simulator evidence above is not a substitute.
