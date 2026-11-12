# Deferred P15 integration gates

- Integrate the model with P11 SettingsHost and app module registration, including
  explicit crate Error mapping and theme rendering. Reason: shared writer is owned
  by another task. First step: rebase on accepted P11 and apply README entry points.
  Close: exact-head native check/lint-ios/test and reviewer approval; delete on closure.
- Implement remote read only after separate security-plan review and controller network
  scope. Reason: this task has no SSH/credential authorization. First step: re-review security-plan-2.md, including retained ClientKey injection,
  coherent credential/trust revision snapshots and the unresolved teardown receipt.
  Without these, keep CredentialUnavailable/UnsupportedIdentityFence/UnsupportedTeardownFence;
  do not call the creating convenience runner or add a network fallback. Close: two controlled hosts'
  receiving-side operation records pass all negatives, including a write mutation;
  delete on closure.
- Observe real theme/background/relaunch and host-switch UI on the owner device.
  Reason: parser/reducer/JSON tests cannot prove these product criteria. First step:
  controller obtains isolated Mac lease and owner supplies authorized disposable peers.
  Close: native and physical evidence attached separately; delete on closure.
# Integration and receiving gates added by task9c49b897b8024dfe

- Reconcile the mounted candidate with P11 final API and verify native behavior.
  Reason: candidate source integration is not an exact-final-head test result.
  First step: diff the controller-confirmed P11 final merge against the integrated
  candidate and run exact-head gates plus actual Appearance button/lifecycle tests.
  Close: mounted route, immediate theme, early-edit/background/relaunch persistence
  and unchanged owner defaults/other fields pass native tests and Simulator.
- Supply retained authority and observable teardown through separately owned APIs.
  Reason: the current public-line/convenience API cannot satisfy the reviewed contract.
  First step: review the task-local API proposal artifact against the owner and SDK.
  Close: actual Rust adapter/SSH receiver R01-R16 and write-mutant receipts satisfy
  security-plan-2.md; retain all unmet P1/P2/P3 criteria until then.
- Reconcile the P14 notifications writer without losing theme or unknown JSON.
  Reason: P14 candidate c639876 serializes a new object, while this branch preserves
  existing fields; taking either side wholesale loses the other's typed state.
  First step: compare settings.rs, settings_view.rs and store.rs at both exact heads,
  integrate both field-intent maps and extend the full-file native oracles.
  Close: both notification edits and theme edits survive background/relaunch with
  unrelated fields intact on the integrated exact head; delete in resolving commit.
- Validate the attempt-2 native source and separate SDK observer dependency.
  Reason: no Mac lease exists, and pinned SDK refuses modal selectors before the
  consumer without an observation hook. First step: obtain the controller's specific
  lease and observer contract, then run the exact-head gates and p11/p15 oracles.
  Close: outside positive and inside refusal/consumer evidence plus actual theme
  screenshots/painter/disk/relaunch pass independent review; delete on resolution.
