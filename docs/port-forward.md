# Port forwarding (issue 140)

yebisu carries a port a program listens on on a saved host (its
`localhost:<port>`) to `127.0.0.1:<local>` on this iPad over SSH, so Safari can
open `http://127.0.0.1:<local>/`, a development server's page for example. It
works for Herdr 1 hosts, Herdr 2 hosts and hosts without Herdr: a forward has
its own SSH connection and needs only `sshd`.

## Using it

- **Forward port…** is in the Host menu of the menu bar (for the selected
  host), in a host row's long-press menu (for that row's host, selected or
  not), and in the Shortcuts list's APP group under Settings. It is disabled
  only while a sheet is open (`Close the sheet first`); the menu bar's and
  the Shortcuts list's need a selected host (`Select a host first`).
- The sheet asks for the **Remote port** (digits only, 1 to 65535). While it
  is open it reads the host's listening ports every 5 s and shows them as
  chips (`:3000`); a chip puts its number in the field. Only port numbers are
  shown; a host without `ss` and `lsof` shows no chips. **Forward** (or
  Return) starts the forward and closes the sheet; an input error keeps it
  open: `Port 3000 is already forwarded from this host.`, `At most 32 ports
  can be forwarded at once.`, `Enter a port number from 1 to 65535.`
- Each forward is a row under its host in the sidebar, also when the host is
  collapsed: `⇄ 3000 → 127.0.0.1:3000`, `⇄ 5173 · connecting…`,
  `⇄ 4000 · paused`, `⇄ 8080 · ended` with the reason below. A tap on a
  listening row opens its page in Safari; on any other row it opens the sheet.
  A long press offers Open and Stop (Dismiss once ended). The rail shows no
  forwards; the host long-press menu lists them under `Forwarded ports`.
- The status line says `Forwarding port 3000 to 127.0.0.1:3000`, adds
  ` · nothing listens on 3000 on {host} yet` when the host's port did not
  answer, says `Port 3000 now forwards to 127.0.0.1:52311 (3000 is in use on
  this iPad)` when the first-choice local port was taken, and
  `Port 3000 forward ended: {reason}`. The first Open of a process adds `Keep
  yebisu on screen beside Safari, or the forward pauses.`

The first-choice local port is the remote port, or the remote port + 10000
below 1024 (80 → 10080). Another app on the iPad holding it moves the forward
to a port the system chooses. Forwards are not saved: a new process starts
with none, as in Sheprd.

## The host's side

- The iPad's line in `~/.ssh/authorized_keys` must allow forwarding. The line
  the app shows starts with `restrict`, which refuses it; remove `restrict`
  from the line on a host you forward from, or write
  `restrict,port-forwarding,permitopen="localhost:*"` instead (as the fork's
  `docs/ios.md` says). The forward asks for the literal host `localhost`, so a
  `permitopen` must name `localhost`, not `127.0.0.1` [推測 from
  `herdr-ssh/src/forward/destination.rs`; the `*` port wildcard is 未検証 on
  the owner's `sshd`].
- `AllowTcpForwarding no` and `DisableForwarding yes` in `sshd_config` refuse
  forwarding too. All three end the forward with `{host} does not allow port
  forwarding for this iPad's key. Remove restrict from this iPad's line in
  ~/.ssh/authorized_keys on {host}.`
- Whether the line the app shows should drop `restrict` is undecided (issue
  140, Open questions); this change does not alter it.

## Lifecycle

When the app leaves the screen (Background or a disconnected scene) it asks
the system for background time (`Closing port forwards`) and each forward
closes its listener and its SSH connection after **5 s** and shows `paused`.
When the system grants no time, the forwards close at once. Back within the
5 s, nothing stops. Active again, a paused forward reopens on its previous
local port first; when that port was taken meanwhile the status line says so
and Safari's old tab needs a new Open. Inactive and Foreground (Safari beside
yebisu in Split View or Windowed Apps) change nothing. An ended forward never
restarts on its own. The grace is `PAUSE_GRACE` in `crates/yebisu/src/forwards.rs`
and may change after owner check O2.

The forward's own thread closes it at the deadline, so the pause does not
depend on the app's timers running in the background.

## Safety

- The listener binds `127.0.0.1` only: other devices on the network cannot
  reach it; other apps on this iPad can (loopback).
- The destination is always the host's `localhost` and the port the user
  typed or picked; nothing in terminal content or daemon messages chooses it.
- Remote forwarding is never requested, and the fork's handler refuses every
  channel the server opens.
- Reasons are built from typed errors only; nothing a host wrote reaches the
  screen, `qa.state` or a log line. Logs carry port numbers and states, never
  a URL.
- Limits: 32 forwards, 4 relay threads and 32 carried connections per forward,
  2 MiB unread per connection (`herdr_ssh::forward::UNREAD_LIMIT`).
- The status line is shared with notifications: the notification settings
  page shows the last forward message too.

## Checks

On any machine with this repository (`~/.cargo/bin/just guarded …`):

- `cargo test --locked -p yebisu forwards` — model, worker, lifecycle,
  reasons and the chip parser, with real loopback listeners and a fake SSH.
- `cargo test --locked -p yebisu shell::` — the host menus, the sheet, the
  sidebar rows, the Shortcuts row, the entry points and the input fence
  (`footer_tests::forward_keys`).
- Negative controls: each gate's test fails when the gate is removed; the
  records are in the pull requests' Test plans (PR-B: 15 mutations, PR-C:
  the input fence, the menu rules, the saved-host check and the Shortcuts
  row).

On this Mac, `just ios-forward` runs `scripts/ios/forward-check.py`: a private
`sshd` on `127.0.0.1`, `python3 -m http.server` with a known page, the app's
saved host and pinned key in a fresh Simulator, and the XCUITest
`SmokeQA/testForwardPort` (open the sheet from the host row's long press,
type the port, Forward, open the row in Safari and read the page, go home and
find the port closed, come back and find it on the same port). The Simulator
proves neither real suspension nor IME or hardware-keyboard behaviour.

### Owner checks on the iPad

Record each result here with the date and the build.

| ID | Check | Result |
|---|---|---|
| O1 | Remove `restrict` from the host's line; `python3 -m http.server 8000`; Forward port… → 8000; Safari beside yebisu; Open shows the page. Again with iCloud Private Relay on. | |
| O2 | Open into full-screen Safari: how far the page loads; compare N = 3…10 s and set `PAUSE_GRACE`. | |
| O3 | iPadOS 26.2+: yebisu in Slide Over pushed off screen: does the forward pause? | |
| O4 | Vite (HMR WebSocket) survives switching focus between Safari and yebisu. | |
| O5 | 80 → 10080; another app holding 3000 → a different port and the status text. | |
| O6 | A host with `restrict` kept → the `Prohibited` text. | |
| O7 | A Herdr 1 host, a Herdr 2 host and a host without Herdr behave the same. | |
| O8 | A 50 MB download over the forward while typing in the terminal: no visible lag. | |
| O9 | Quit from the App Switcher and relaunch: no forwards. | |
| O10 | The port field's keyboard (the default one; no number pad, Q1) and that digits typed with a hardware keyboard and the Japanese IME never reach the terminal. | |
