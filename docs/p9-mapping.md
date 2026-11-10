# P9 implementation contract

Fork source references below are at herdr-gpui commit
`398c132a06723a0cc1494842a34666d173ec0200`; the fork is read-only.
The original table was written before implementation; yebisu integration references describe the starting implementation.

| Concern | Source | yebisu replacement |
| --- | --- | --- |
| Browser tab types | `crates/sheprd-gpui/src/core_terminals.rs:38-42` (`Location`, `Scope`, `Store`, `TabId`); pane tree at `:84-99` | A dedicated core workspace attached to `TerminalView`, keyed by SSH target and session. Terminal IDs identify panes; no browser dependency. |
| Persistence | `crates/sheprd-gpui/src/core_terminals/remote.rs:9-33` identifies target **and** session; tab panes declare unsaved state in `core_terminals.rs:91-94` | Container-local versioned JSON records the host/session, terminal IDs, active pane and split orientation. Worker loads/saves, validates IDs and caps record sizes. Reattach uses saved IDs, never a different host's IDs. Detach keeps remote programs alive. |
| SSH ownership | `crates/sheprd-gpui/src/core_terminals/remote.rs:41-64,70-73` | `Core::over(SshStream)` owns the stream and an `Arc<herdr_ssh::Tunnel>`, including during hello. Existing host pin gate and installation key are reused. Only workers call SSH/core operations. |
| Completion isolation | `crates/yebisu/src/terminal_view.rs:122-141`, `crates/yebisu/src/board.rs:143-166` | Core completion writes only its immutable per-connection mailbox. Host selection replaces the entire view, and reset allocates a new mailbox; old workers cannot publish into it. The stop flag prevents obsolete successful hello results from spawning or attaching. Existing connection generations scope queued input. |
| Input and IME | `crates/yebisu/src/input.rs:100-150`, `crates/yebisu/src/terminal_controls.rs:7-59` | Keep the existing EntityInputHandler and key callbacks, modifiers and composition geometry; deliver semantic events through the core backend's terminal encoder. Queue identity includes connection and active terminal. |

Validation: run the old-tunnel completion test with its fence disabled, require
failure, restore and require pass. Exercise UIKit-shaped callbacks through the
real app adapter and framed core wire for all Input option combinations. Use
only an in-process mock core or a private HC_SOCKET. Never contact the owner's
running services. Physical input/IME, split, detach/reattach remain owner QA.


## Implemented locations

- `crates/yebisu/src/core.rs`: bounded command queue, cancellation and Arc-held SSH tunnel.
- `crates/yebisu/src/core/store.rs`: validated per-host/default-session persistence.
- `crates/yebisu/src/core/worker.rs`: serialized save/reattach, two-pane workspace,
  semantic input destination fencing and terminal emulation through CoreBackend.
- `crates/yebisu/src/core/screen.rs`: split compositor feeding the existing painter.
- `crates/yebisu/src/core/tests.rs`: all 32 input switch/reporting combinations,
  late tunnel completion, split, stale input and persisted reattachment.
- `scripts/ios/smoke-qa/Tests/SmokeQA.swift`: `testP9Core()` drives UIKit typing,
  composition callbacks, split direction and detach/reattach on Simulator.

Only `sheprd-core-client` and `sheprd-backend` are new direct dependencies;
`sheprd-terminal` is transitive. The bounded two-pane layout does not need
`sheprd-session` or the old browser's tab model. No existing package version is
upgraded. The existing UIKit pin prerequisite is a separate commit.


## Reproducible P9 check

Run `just ios-p9` in an interactive login shell. It builds the Simulator app,
uses only the private core peer, drives UIKit input and composition callbacks,
splits two distinct terminal IDs, detaches and reattaches, and validates wire
receipts plus paint generations. `testP9Core()` includes the parentheses required
by the installed XCTest enumerator; a missing screenshot fails the check even
if xcodebuild reports a successful run with no tests.

The mock host retains its terminal namespace across connections, while stream
channels belong to one connection. The saved-terminal split regression test
fails against the old allocator (duplicate t1) and passes with the new allocator.
Simulator QA persistence is isolated per mock-daemon process; device persistence
continues to use the stable host/default-session record.
