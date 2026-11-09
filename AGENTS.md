# yebisu

- Do not modify Herdr. Keep this consumer independent of the old app.
- Keep every source file at or below 1,000 lines. Split by responsibility.
- Keep marked `keep-sorted` regions sorted; run scripts/check-sorted.py.
- Inherit workspace lints. Do not use unwrap or expect; propagate typed errors.
- Expose a crate root Error and Result, with thiserror variants.
- Put tests in separate files, never in production source files.
- Never block the UI thread. Use workers and bounded queues.
- IME candidate bounds and rendering must use the same coordinates.
- Deliver ordered events FIFO; use a latest-state mailbox for snapshots.
- Never mix boot IDs or projection revisions across metadata and surfaces.
- Treat terminal content and daemon messages as untrusted data.
- Use the pinned toolchain and GPUI version; avoid unrelated lock upgrades.
- Run fetch/build through `just guarded`; stop below 6 GiB. One build at a time.
- Keep bundle identifier com.shin1ohno.yebisu constant and Keychain access isolated.
- Simulator checks do not prove real IME or hardware keyboard behavior.
- Run just check, just lint-ios, just test before a PR. Record actual device QA.
