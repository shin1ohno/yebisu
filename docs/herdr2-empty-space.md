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
| A stop fails | The status line shows `Could not remove Herdr 2 space <name>: <reason>`; the space stays listed. The connected space is not retried until it changes; a polled space is not retried until its server changes |
| A successful poll lists no server for a space that has only a saved layout (`Session unconfirmed`) | Removes the row and deletes the layout file. A space the poll lists but whose server does not answer must stay so in two successful polls in a row |
| A poll fails (transport error, timeout) | Nothing is deleted. After two failures in a row every row is `Session unconfirmed`, as before, with its file |

`default` is never stopped, and its layout is kept. A space with a live
terminal that is in no tab (another client's `hc run`) is never stopped.

## How a server is stopped

herdr-core 0.1.13 (`crates/herdr-core/src/api.rs` `"server.stop"` →
`server_stop` → `core.shutdown()`; the CLI is `hc server stop`,
`cli.rs` `stop`). The client library (`sheprd-core-client` `Method`) has no
`server.stop`, so yebisu runs `core/vacate.rs` `stop_command` over SSH, the way
discovery runs: it finds `hc` where the connect command does, checks that
`hc server status --json` still names the core id that was seen, and only then
runs `hc server stop` with `HC_SESSION=<space>`. Another server behind the same
name is left running (exit status 3).

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
`core/vacate_tests.rs` (always run) covers the rules, the 5 s grace, the
successor and the stop command's text.
