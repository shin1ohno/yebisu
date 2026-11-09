# P6 snapshot status board

Each saved host starts its own `Link` when a terminal opens. One worker continuously drains each ordered receiver into a bounded latest-state mailbox, including hosts off screen. The UI polls selected and hidden mailboxes every 33 ms without waiting on their locks. Switching selection transfers the host's connection rather than reconnecting it. Hidden clipboard effects are drained without replay on selection.

The board reads the latest daemon snapshot, including a metadata snapshot awaiting its matching terminal surface. Terminal painting still requires a coherent boot/revision pair. Agent status, space status and harness names come directly from snapshot fields; there is no terminal-output parser or seen ledger. Unknown values remain explicit. All rows are reachable in the scrolling board.

Host-scoped asynchronous connection installation and a separate selection epoch fence old callbacks. Switching discards composition and queued input. Navigation validates the target against the current inbox, enqueues `workspace.focus` / `pane.focus`, and blocks input until a newer daemon snapshot confirms the requested focus (or a correlated response acknowledges an already-focused target). It does not synthesize keys or optimistically mutate the daemon snapshot. A rejected request is visible. Backgrounding replaces every host inbox; automatic return reconnects the retained hosts independently.

Rendering performs only memory reads, element construction and bounded observation enqueueing. Connection, key/trust file access and process startup remain on workers. Existing endpoint handles enqueue resize/API messages for their worker.

Validation: host tests cover latest metadata, unknown status, hidden-host burst draining, old-host selection races, same-generation paint callbacks, stale targets, both API methods and off-screen periodic snapshots. `scripts/ios/p6-check.py` checks both hosts and endpoint receipts from private peers; its oracle has missing-host/stalled-hidden/wrong-state/missing-harness negative controls. Physical acceptance remains in `docs/device-check.md`.
