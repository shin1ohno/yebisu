# Herdr 2: when a pane's program ends

A Herdr 2 session is one `hc connect` link to a herdr-core. A program
ending in one of its panes (Control-D in a shell, `exit`, a command
finishing) ends that pane, never the session. The core worker
(`crates/yebisu/src/core/worker.rs`, rules in `core/ended.rs`) follows the
desktop client, the herdr-gpui fork:

| Event | What yebisu does | Reference |
|---|---|---|
| A pane in a split ends | The pane closes; its neighbour (left or above, else the first) takes the whole tab and the keyboard | fork `core_terminals/tabs.rs` `poll_core_panes` closes an ended pane in a split; herdr-app `src/window.rs` `terminal_ended` removes it |
| The last pane of a tab ends | The pane keeps its final screen and refuses input; the status shows `ended: 1`; the session stays connected, so other tabs, New Tab and Close Tab still work | fork `sheprd-session/src/panes.rs` "The last pane stays: a tab always shows one" |
| A tab is opened again (tab switch, reconnect) and one of its terminals finished meanwhile | herdr-core refuses the attach with `finished`; the pane goes, and a tab left without panes closes. A session left without tabs gets a new shell tab on connect | herdr-app `src/terminal.rs` "It ended between being listed and being attached: its pane goes" |
| The last tab is closed (Close Tab) | `Herdr 2 · No tabs`; the session stays connected and New Tab works | unchanged |
| The core stops or the link drops | The session is reported disconnected with the link's error, also when every pane has already ended | `Core::is_lost` checked each worker turn |

Before 2026-11-24 an ended pane stayed in its split, a tab whose only pane
ended marked the whole session disconnected (`live.connected` counted only
panes still running), and re-attaching a finished terminal ended the worker
(`herdr-core refused terminal.attach: t2 has finished`).

What the screen shows changes in two places only: a pane that ended in a
split disappears and its neighbour fills the tab, and the last pane of a tab
keeps its final screen with `ended: 1` in the status line instead of the
host turning disconnected. Colours, wording and other layout are unchanged.

## Real-core tests

`crates/yebisu/src/core/live_core_tests.rs` runs each case against a real
herdr-core started in a private `HOME`, `XDG_*` and `HC_SOCKET`, reached
through `hc connect` as the iPad reaches it over SSH. Each test stops only its
own core (`server stop` on its private socket) and the children it started.
The hidden-tab case (`live_core_pane_ended_in_a_hidden_tab_goes_on_return`)
ends a shell with `hc send <terminal> exit` while its tab is not shown, so
returning to the tab meets herdr-core's `finished` refusal. The always-run
fake-peer tests `pane_exit_in_a_split_closes_it_and_keeps_other_pane_live` and
`last_pane_exit_keeps_the_session_connected` guard the split and last-pane
cases in CI. The real-core tests are opt-in:

```sh
YEBISU_HERDR_CORE_TEST_BINARY=/path/to/herdr-core \
  cargo test --locked -p yebisu live_core -- --ignored --test-threads=1
```

Use the herdr-core version the fork pins (`scripts/release/herdr-core.json`
at the fork revision in `Cargo.toml`; 0.1.13 at herdr-gpui 9327f44).
