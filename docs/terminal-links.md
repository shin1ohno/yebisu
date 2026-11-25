# Opening web addresses from the terminal (L66)

With a hardware keyboard, hold Command and tap, or click with the pointer, on a
web address in a terminal pane: yebisu opens it in the iPad's default browser.
This works on Herdr 1 and Herdr 2 panes.

## What opens

| Under the press | Result |
|---|---|
| A plain `http://` or `https://` address in the text | Opens, exactly as shown |
| An address that meets a row's edge: it may have been wrapped into the next row, or the line may have ended there (see below) | Herdr 1: opens what Herdr says the address covers, the whole wrapped address shown in the status line (`Opening https://…`). Herdr 2: nothing; `Not opened: the address meets the line edge and may go on · Herdr cannot say where it ends` |
| An explicit OSC 8 hyperlink whose target is `http` or `https` | Opens its target |
| Another scheme (`mailto:`, `ssh://`, `javascript:` ...), as text or as an OSC 8 target | Nothing; the status line says `Not opened: <scheme>: link · only http and https addresses open` |
| A file path (`/etc/hosts`, `~/a`, `./b`, `src/c.rs:12`) or a `file:` address | Nothing; `Not opened: a file path is on the host · only web addresses open` |
| An OSC 8 hyperlink with no readable target | Nothing; `Not opened: the link names no web address` |
| Anything else | The press is an ordinary tap or click |

The reason is the terminal status, which the status bar shows in its
detail, behind the ▸ after the connection state, where other terminal
status such as `Paste queued` appears.

Command counts as held only as the keyboard last reported it. A key
released while another app is in front, as after an open puts Safari there,
never reaches yebisu, so when the window becomes active again the held
Command it remembers is not believed until the keyboard reports a modifier
again (review F2). Until then a tap is an ordinary tap.

A tap or click without Command is unchanged: it focuses the terminal, starts a
selection or asks for the keyboard as before. A press with Command over text
that is no link is an ordinary press too. A press that opens or refuses a link
reaches nothing else: no focus change, no selection, no keyboard request and no
byte for the terminal.

The status line names only the host of an opened address (`Opening example.com
in the browser`), since a path or query may hold a token.

## The Command underline

While Command is held and the pointer rests on a web address that would
open, the address is underlined: one point, solid, in each cell's text
colour, two points above the row's bottom. That is where an explicit hyperlink's dashes sit at rest
(4 points on, 3 off), so on an OSC 8 link the line replaces the dashes. It is
the fork's hovered-link line, `TerminalPainter::paint_link`. Links that open
nothing, an address at a row edge whose end Herdr must still say, a drag,
and a finger (which has no hover) show no underline. The
owner delegated this design (2026-11-24); the artboard `OpenUrl` records it.

## Safety

Terminal content is untrusted (AGENTS.md):

- Only `http` and `https` addresses with a host open. Anything else is refused
  by name, never handed to the system, which could open another app for it.
- The address opened is the text the pane shows, or the target the hyperlink
  names, byte for byte. Nothing is resolved, completed, unescaped or run, and
  no shell is involved. Blanks and control characters end an address; an
  address with one inside it is not opened.
- The open is GPUI's platform open. On iPadOS that is UIKit's
  `openURL:options:completionHandler:` on the main thread (herdr-uikit
  `platform/system.rs`), which returns at once, so rendering never waits.
- At most 16 rows and 32 KiB of text are read around a press.

## How the address is found

`crates/yebisu/src/terminal_links.rs` ports the fork's
`crates/sheprd-gpui/src/terminal/links.rs` (`pane_link_at`,
`pane_hyperlink_at`, `frame_link`, `plain_url`) and `links/wrap.rs`:

- A link never leaves its pane. While a popup is shown, no link is read.
- An explicit hyperlink is authoritative and covers its run of cells, wide
  glyph continuations included.
- Plain text is read as the fork reads it: an address starts wherever
  `http://` or `https://` stands and ends at a blank, a control character or
  one of `<>"'` and the backtick; prose punctuation after it (`.,;:!?`) and
  closing brackets it did not open are left out.
- Wide characters are read by their lead cell, with either continuation shape
  Herdr 1 sends (a blank or an empty cell, `skip` false) and Herdr 2's.

Differences from the fork:

- Herdr 1's surface carries no soft-wrap flag, so there a line the
  terminal wrapped and a line that happened to end at the right edge look
  the same (review F1: an address ending at the edge was opened joined with
  the next line's first word, an address the screen never shows). An
  address is only a candidate (`Link::unproven`) when it is read across
  rows, when it runs into the right edge of a row whose next row is not
  certainly a new line (a next row starting with a blank is one, as a
  wrapped address would go on with its next character there), or when its
  rows run past 16. A candidate is never opened as read. As the fork does,
  yebisu asks the Herdr 1 daemon (`pane.link.resolve`,
  `terminal_links/resolve.rs`), whose terminal knows its soft wraps, and
  opens what the cells it names show: the whole wrapped address, which the
  status line shows whole as it opens, or only the row's address when the
  line ended at the edge. A stale screen, an error or no answer within 5 s
  opens nothing and says so.
- Herdr 2's terminal is emulated in the app (`sheprd-terminal`), whose frames
  carry the emulator's `WRAPLINE` as `CellData::modifier` bit 9 (`SOFT_WRAP`,
  fork `sheprd-terminal` `cells.rs`, #141) on the last cell of a row it
  wrapped. The mark proves both a wrap and a line end, so a wrapped address
  opens whole (status `Opening {whole}`), and an address ending at the edge
  above a new line opens as shown. Only what the marks cannot settle stays a
  candidate, which opens nothing and says why (Herdr 2 has no
  `pane.link.resolve`): an address starting at the pane's top-left cell
  (it may go on from a row the frame does not show; it is read as Herdr 1
  reads it), one read past 16 rows, and a pane whose emulator frame is not
  yet the pane's width (the composer marks it `UNMARKED`, a yebisu-only bit
  never on the wire, as after a reconnect its first frames are 80 x 24).
  Herdr 1 frames never set bit 9, and yebisu reads it only on Herdr 2.
- A wide glyph that does not fit in a row's last cell goes to the next row,
  and both terminals leave a blank in that cell (Herdr 1: ghostty's spacer
  head, `src/pane/terminal.rs` writes `" "`; Herdr 2: alacritty's leading
  wide-char spacer, `sheprd-terminal` `cells.rs:102`). Such a row may go on
  too (review F3): text reaching the cell before the blank, under a row that
  starts with a wide glyph, is a candidate, and the blank is no part of it.
  On Herdr 2 the spacer carries the wrap mark, which proves the row goes on;
  a blank without it is a real line end. A wide glyph that fills the row
  carries the mark on its second (`skip`) cell, where alacritty puts it.
- The fork opens files and some other schemes on the machine that runs the
  pane. The iPad does not run the pane, so yebisu refuses them.

Herdr 2 terminals number their OSC 8 hyperlinks into their own tables. The
composed Herdr 2 frame (`core/screen.rs`) now carries every pane's table and
moves each pane's cell indexes past the panes before it, so a hyperlink keeps
its own pane's target.

## Checks

- Host: `cargo test --locked -p yebisu -- terminal_links core::screen::tests`.
  `terminal_links_tests/tap.rs` presses through GPUI's real mouse and touch
  dispatch on the Herdr 1 wire peer and reads GPUI's platform open and the
  peer's received input. Herdr 2's marks: `terminal_links::tests::marked`
  (hand-built marked frames), `terminal_links::tests::core_open` (the same
  dispatch on a view switched to Herdr 2) and `core::screen::tests::wrap`
  (composition keeps each pane's marks, `UNMARKED`, and the pinned
  `sheprd-terminal` emulator's own marks on the Herdr 2 fixture).
- The Herdr 1 wire peer answers `pane.link.resolve` as a terminal that knows
  its wraps (`qa::mock_regions`): the fixture's wrapped address spans its
  rows, the address ending at the edge of row 18 (`EDGE`, above `tail` on
  row 19) only its own.
- Simulator: `SmokeQA/testUrlCommandTapHerdr1` and
  `SmokeQA/testUrlCommandTapHerdr2` (`scripts/ios/smoke-qa/Tests/SmokeQA+Url.swift`)
  launch with `YEBISU_URL_TAP_QA`, press each fixture link with the
  Simulator's hardware Command held, and check the app's own open receipt in
  `qa.state` (`url_open`, `url_open_last`), Safari in front, and the peer's
  input count; on Herdr 2 `core_wrapped`, which the emulator wraps, opens
  whole (#141). `SmokeQA/testUrlCommandUnderlineShot` launches with
  `YEBISU_URL_TAP_QA=hover`, which takes the pointer to rest on the plain
  address with Command held, and saves a screenshot of the underline; XCTest
  cannot hover a pointer on iPad. None of them is in the CI job selection.
- Screenshots (`docs/screenshots/w6-open-url/`, iPadOS 26.5 Simulator,
  landscape): `herdr1-fixture.png` and `herdr2-fixture.png` (the fixtures),
  `plain-safari.png`, `edge-safari.png`, `wrapped-safari.png`,
  `osc8-safari.png`, `core-osc8-safari.png` (Safari in front after each
  Command + tap), `mailto-refused.png`, `path-refused.png` and
  `core-wrapped-refused.png` (yebisu still in front after a refused press;
  the `core-wrapped` one predates #141, which opens that address),
  `command-underline.png` (the Command underline on the plain address) and
  `artboard-open-url.png` (the design record).
- Back from Safari, a Herdr 2 session reconnects with a new boot and its
  first frames are 80 x 24 until the resize, so the test presses a link only
  once its point and frame (`url_targets.surface`) held for two seconds, and
  `qa.state` `url_press` records the cell and row text each press read.

## Device checks (owner)

Hardware keyboard and trackpad on the owner's iPad; Simulator checks do not
prove them (AGENTS.md). Herdr 2 checks print the address below the pane's
first row, as an address from its top-left cell stays refused.

| Check | What | Date | iPad model | iPadOS | Herdr | Result |
|---|---|---|---|---|---|---|
| O1 | A URL longer than the row: Command + finger tap and Command + pointer click on each row open the whole URL in Safari; status `Opening {whole}` | | | | 2 | not run |
| O2 | An address exactly the row width, then another word on the next line: Command + tap opens only the address | | | | 2 | not run |
| O3 | Rotate the iPad (width change), then O1 and O2 again | | | | 2 | not run |
| O4 | An address with Japanese (wide) characters that meets the row edge opens whole, with no stray space | | | | 2 | not run |
| O5 | Back from Safari, a tap without Command opens nothing (focus and selection as before) | | | | 2 | not run |
| O6 | A Herdr 1 pane behaves as before (the daemon's `pane.link.resolve`) | | | | 1 | not run |
