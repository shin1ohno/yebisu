# Herdr 2: an empty space goes away

Each Herdr 2 space is its own herdr-core server: `hc connect --session
space-<ms>` over SSH starts one (`new_space.rs` names the space, `core::connect`
runs the connect command). The sidebar lists the union of the saved layouts in
the store (`core-<hex host>-<hex space>.json`, `core/store.rs`) and the servers
a discovery poll finds on the host (`core.rs` `DISCOVER`, `catalog_sessions`).
Before 2026-11-24 a space whose last tab closed stayed listed at 0 tabs and its
server kept running, and a saved layout whose server was gone stayed listed as
`Session unconfirmed` for good.

Owner's rule (2026-11-24): when the last tab of a space closes and no live
terminal remains there, yebisu stops that space's herdr-core server so the
space disappears from every client (the official app and `hc` too). The space
named `default` is never stopped. The history of the space's finished
terminals goes with the server.

## When a space is removed

| What yebisu sees | What it does |
|---|---|
| The connected space's last tab closes (the tab's x), or the last pane of its only tab ends (Control-D, `exit`) | Takes a fresh `terminal.list` of the space. If no terminal has a state other than `finished`, stops the server and removes the space. Otherwise the space stays with 0 tabs and is looked at again every 5 s |
| A discovery poll (every ~5 s while a Herdr 2 session is open) finds another space with 0 tabs and no terminal whose state is not finished | Notes it. Two snapshots at least 5 s apart must both find it so (another client's just-created space is left alone), then it stops that server |
| A stop fails | The status line shows `Could not remove Herdr 2 space <name>: <reason>` and keeps showing it over every screen refresh until the space is removed or the user acts (any command but a resize); the space stays listed. The connected space is not retried until it changes; a polled space is not retried until its server changes |
| A stopped space is started again by another client | It is hidden only while its server does not answer (or answers as the server that was stopped, pid). Once a different server answers with a terminal running, the row comes back without restarting the app |
| A successful poll lists no server for a space that has only a saved layout (`Session unconfirmed`) | Removes the row and deletes the layout file. A space the poll lists (its state directory remains after a stop) but whose socket is absent or refuses must be so in two successful polls in a row. A socket that connects but does not answer in 0.6 s (`["name", "unknown"]`: a busy server, a machine that does not reply) is not absent: its row and file stay |
| A poll fails (transport error, timeout) | Nothing is deleted. After two failures in a row every row is `Session unconfirmed`, as before, with its file |

`default` is never stopped, and its layout is kept. A space with a live
terminal that is in no tab (another client's `hc run`) is never stopped.

## How a server is stopped

herdr-core 0.1.13 (`crates/herdr-core/src/api.rs` `"server.stop"` →
`server_stop` → `core.shutdown()`; the CLI is `hc server stop`, `cli.rs`
`stop`). The client library (`sheprd-core-client` `Method`) has no
`server.stop`, so yebisu runs `shell/herdr2_frame/stop.py` on the host over SSH
(`core/vacate.rs` `stop_command`), in the way discovery runs `discover.py`. The
script talks to the session's socket itself, never through `hc`: `hc server
status` starts a core on Unix (herdr-core `cli.rs` `status_may_start_core`), so
stopping a space that was already stopped would have started it again. In the
one connection it stops through, it asks `hello` and `server.status` and stops
only if:

- the socket exists and answers (no socket, or nothing listening, means the
  server is already gone: it reports `absent` and starts nothing);
- the core id is the one seen **and** the pid is the one a poll saw, or (for
  the connected space, which has no pid) the server is at least as old as the
  connection that saw it: a core id is permanent (herdr-core `server.rs`
  `load_identity`: "kept until state is wiped"), so a restarted server has the
  same id, and only its pid and uptime tell it apart;
- `live` is 0 in that same `server.status` answer: a terminal started after
  the snapshot, or after the worker's list, keeps the space (exit status 4,
  `Stopped::Busy`).

Python 3 on the host is required (exit status 6 otherwise). `server.status`'s
`live` and the `server.stop` that follows are two requests in one connection,
not one atomic step: a terminal started in the milliseconds between them is
stopped with the space.

## Where the selection goes

When the selected space is removed, yebisu does what the official app does when
its selected space closes (`herdr-app/src/model.rs` `remove_space`:
`self.active = self.spaces.get(pi).or(self.spaces.last())`): the space that
takes the removed one's place in the sidebar's order, else the last. Only a
confirmed space can be connected to, so unconfirmed rows are passed over. A host
with no other space detaches (`Detached`), and the removed space is forgotten as
the host's selected one, so a reconnect goes to `default` and never starts the
removed space again (`shell/herdr2_frame/removed.rs`).

## Real-core tests

`core/empty_space_tests.rs` and `core/saved_space_tests.rs` run against real
herdr-core servers, one per space, in a private `HOME`, `XDG_*` and runtime
directory (`core/space_fleet.rs`). The stop runs the production `stop_command`
against those directories. Each server is ended only by the test
(`server stop` on its private session) and its exact child reaped.

```sh
YEBISU_HERDR_CORE_TEST_BINARY=/path/to/herdr-core TMPDIR=<short dir> \
  cargo test --locked -p yebisu -- empty_space saved_space --ignored --test-threads=1
```

Keep `TMPDIR` short: a server's holder socket path must stay within 107 bytes.
`core/stop_guard_tests.rs` covers the stop's guards (a terminal started after
the snapshot, an already stopped space, a restarted server with the same core
id, a per-socket timeout, a failed stop's notice over refreshes, a space started
again). `core/vacate_tests.rs` (always run) covers the rules, the 5 s grace, the
successor and the stop command's text.
