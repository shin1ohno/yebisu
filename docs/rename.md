# Rename in place (issue #143)

A Herdr 1 space, tab or pane is renamed where its name is drawn. herdr-uikit
lays one native `UITextField` over the name (`present_native_inline_field`,
herdr-uikit `docs/native-inline-field.md`); the field shares the window's
native modal gate with the search panel, so while it is up GPUI's views do not
take first responder and no key, composition, Paste or menu command reaches
the terminal. On Herdr 2 every Rename is listed and disabled with "Not
available on Herdr 2 yet" (the official app's machine document is a separate
issue).

## Where it starts

| Entry | Object | Field over |
|---|---|---|
| Long press of a sidebar space row → Rename… | that space (any space of the selected host) | the row's name |
| Long press of a sidebar agent row → Rename… | the agent's pane | the row's name |
| Long press of a rail space or agent → Rename… | that space or pane | the rail menu's header (the menu stays open) |
| Long press of a tab → Rename… (with Close Tab) | that tab | the tab's name after its number |
| Double tap of a tab's label (the first tap moves to it) | that tab | the tab's name |
| Double tap of a space row's name (the row moves once) | that space | the row's name |
| Palette or Terminal menu: Rename Tab, Rename Pane | the focused tab or pane | the tab's name, the pane's chip |
| Palette Rename Space, Space menu: Rename Space — name | the focused space | its sidebar row |

A rename is refused, with the reason in the status bar's detail, in this
order: a sheet is open ("Close the sheet first"), Find is open ("Close Find
first"), Herdr 2, another host ("Select this host first"), not connected or a
version mismatch ("Not connected"), the object is gone ("This … was closed."),
the method is not advertised ("This host can't rename"), and the same object's
last rename is still waiting ("Wait for the last rename").

## The field and the name

`shell/rename.rs` keeps one session: `Measuring` (the anchor has not been
drawn yet), `Presenting`, `Open`, `Restoring`. The anchor element draws a
slot canvas (`shell/rename/slot.rs`) that reports its rectangle and the
visible area; each frame `rename_frame_tick` presents the field once the
anchor was drawn, moves it when the rectangle changed (`set_frame`), scrolls
the sidebar list to show a row's field, and closes it when the object, the
connection or the boot went away ("This space was closed.", "Host
disconnected.") or the anchor was not drawn twice in a row ("Rename
cancelled"; never drawn: "Rename unavailable here"). The SDK is called only
from deferred work, never inside a render.

Return sends one request in herdr-gpui's own shape: `workspace.rename
{workspace_id, label}`, `tab.rename {tab_id, label}`, `pane.rename {pane_id,
label}`. The name is normalized (`shell/rename/normalize.rs`: control
characters to spaces, trimmed, at most 160 characters); a tab or space is
never sent empty, a pane's empty name gives it back its automatic name, and an
unchanged name sends nothing. The result is matched by request id in
`live.rename` (`RenameTicket`), never in navigation's `live.pending_request`,
so moves and palette commands go on while it waits.

Until the host answers, the sent name is drawn in place of the host's at the
eight places a name shows (sidebar space and agent rows, rail item and its
menu header, tab row, space switcher and list, pane chip, Space menu), muted
(`#acb2bb`); accepted, it is drawn in the normal color until a snapshot newer
than the one it was sent at names it. An error or rejection shows "Rename
failed: <reason>"; no answer within 10 s shows "Rename not confirmed; showing
the host's name"; a disconnect or boot change drops it. The snapshot is never
rewritten.

The terminal gives up what it was composing before the field opens
(`terminal.input.detach`), a Paste being prepared is cancelled, the key band
is hidden, and no native text retirement is submitted while the field holds
first responder (`native_field_open`, shared with the search panel). The SDK
handle is dropped only after the commit was sent; the next key after
`InputRestored` reaches the terminal once.

## Tests

```sh
just guarded cargo test --locked -p yebisu -- shell::rename::tests shell::host_menus::tests
just guarded python3 scripts/ios/rename-mutations.py      # each guard's named test fails without it
python3 -I -B -m unittest scripts/ios/test_rename_check.py
just guarded python3 scripts/ios/rename-check.py --out /abs/dir --label run   # Simulator, HERDR_IOS_SIMULATOR
just guarded python3 scripts/ios/rename-check.py --out /abs/dir2 --label more --suite more     # S7, S8
just guarded python3 scripts/ios/rename-check.py --out /abs/dir3 --label h2 --suite herdr2     # S10
just guarded python3 scripts/ios/rename-check.py --out /abs/dir4 --label menus --suite menus   # S9
```

`rename-check.py` runs `SmokeQA/testRenameInPlace`: R1 a space's long press,
a new name and Return (one `workspace.rename`); R2 a tab's double tap (one
`tab.rename`); R3 an outside tap (nothing sent); R4 keys, Command-V,
Command-B and the field's long press Paste while the field is up (the pasted
word is in the field's text; nothing reaches the terminal); R5 Japanese
romaji after Control-Space (the input source and any composition are
recorded, not required).

`--suite more` runs `SmokeQA/testRenameMore`: S7 opens the field over the
focused pane's agent row, then the wire peer (`YEBISU_RENAME_QA_INSERT`,
`mock_peer.rs`) sends a snapshot with an agent above it; the row's slot must
move and the field's frame stay within 1 pt of the rectangle over it, also
after the window turns to portrait and back. S8 renames the current space
from the rail's long-press menu (one `workspace.rename`). `--suite herdr2`
runs `SmokeQA/testRenameHerdr2` on the Herdr 2 mock: S10, the palette's
Rename Space row and the tab menu's Rename… are disabled with "Only herdr-app
spaces and tabs can be renamed" (the mock serves no `herdr-app` document;
#144 D3 renames a document's spaces and tabs, and Rename Pane stays "Not
available on Herdr 2 yet") and no field opens. `--suite menus` runs `P12QA/testRenameMenus`
(scripts/ios/p12-qa): S9, the OS menu bar's Rename Tab and Space › Rename
Space — name open the field over the tab and the space row. The menu bar is
revealed by P12's swipe, which can flake (issue #142).

## Simulator and device

| Behaviour | Where it is proved |
|---|---|
| Field first responder, GPUI views off its chain, GPUI keeps drawing | herdr-uikit probe; R1-R5 `rename-state` records |
| Field frame over the label (rectangle) | herdr-uikit probe `p1-frame-matches-label-within-1pt`; S7 |
| Field text on the label's baseline | device D1, D4 (a UIKit capture in the probe does not see GPUI's Metal layer) |
| Return and Escape while composing keep the field | herdr-uikit probe (marked text set directly); device D2, D3 |
| Japanese IME conversion | device D1-D3 (the Simulator's XCUITest typing did not compose) |
| Escape from a hardware keyboard | device D3 (XCUITest's Escape never reaches a presented field) |
| Return held while the field closes, released 1 s later | device D12 (XCUITest's `typeKey` presses and releases at once; it cannot hold a key) |
| The field's long press Paste | R4 (the word in the field, none at the peer) |
| Outside tap with a finger, Pencil or trackpad | R3 (finger); device D5 |
| Software keyboard covering a row | device D11 |
| Stage Manager resize, app switch | device D13 |
