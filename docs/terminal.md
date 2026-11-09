# P3 terminal contract

The connection button preserves P2's scan/trust gate. It never trusts a host
implicitly. The in-process SSH implementation verifies the pinned host key on
the actual connection. P3 sends viewport updates but no terminal key input.

`Link` owns a connection generation, one reader, an inbox and a drained flag.
The reader continuously consumes the client's ordered receiver on its own
thread. The UI uses `try_lock` to take a latest coherent state; no UI poll or
paint callback waits for network I/O or joins a thread. Reset disconnects the
old handle and replaces both inbox and drained flag. Deferred startup and
paint callbacks check the generation. Old readers can only update old inboxes.

`Live` handles all nine ClientEvent variants explicitly. Connected retains
capabilities/version; Snapshot/Surface pass through the projection reducer;
SurfaceImages is explicitly discarded until P8; Response clears only the
matching pending correlation ID; CommandRejected is shown; VersionMismatch is
retained through Disconnected and does not reconnect; Disconnected clears the
surface and shows the reason; Message retains keyboard-report-all and bounded
OSC52 UTF-8 clipboard writes. All other messages are explicitly ignored.
Clipboard side effects have a separate eight-item queue, drained exactly once.

The displayed snapshot and surface must match both boot and projection
revision. A metadata-only revision waits while the old coherent pair remains
visible; a changed input target invalidates it. A future surface waits for its
snapshot. Old boot and regressing revisions are rejected. The dependency keeps
its own wire validation. The `HeldInput` preparation for P4 has boot and
Pane/Popup targets and a 64-event FIFO; it has no sending path in P3.

The painter closure includes images.rs with an empty image catalog, the pruned
terminal helpers, the bold-weight function, Theme and OSC52. Integration-test
performance branches are removed. PROVENANCE.tsv records copied tests and
fixtures as well as the state/connection models. Image delivery returns in P8.

Run `just ios-p3` from an interactive login shell. The QA peer is a private
UnixStream pair that speaks the real protocol through sheprd-client. Its launch
switch exists only in Simulator builds and selects no external endpoint. It is
inert without YEBISU_TERMINAL_QA; device builds contain no peer. The screenshot
oracle checks colours defined by theme.rs/mock_peer.rs and paint receipts from
both density settings; rotation is reported separately. It rejects a blank dark screen and missing
or incoherent receipts. Simulator evidence does not establish device IME or
hardware-keyboard behavior.

On the Mac mini's current headless Simulator, XCTest orientation requests did
not change the app viewport. The default `ios-p3` gate reports rotation as
UNVERIFIED; it verifies display, colours, density switching and wire resize.
Use `python3 scripts/ios/p3-check.py --require-rotation` under `just guarded`
when a Simulator with working rotation is available. This strict option fails
unless a landscape viewport is actually painted. The owner must still perform
the physical rotation steps in device-check.md. Both `open -a Simulator` and the current Xcode standard Simulator.app path
failed; this task does not install external software.
