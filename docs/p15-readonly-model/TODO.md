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
