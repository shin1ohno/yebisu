# P2 variants

No unresolved design fork affecting device feel was found in P2. Host trust is
an explicit fingerprint comparison; it has no automatic alternative. The
Developer: P0 rendering check link opens the retained baseline screen, and its
Keys and hosts link returns to P2, without rebuilding. It is a diagnostic
screen, not a competing user flow.

# Terminal display variants (P3)

The terminal toolbar's density control switches immediately between Standard
(14 pt font, 20 pt cells; upstream defaults) and Compact (12 pt, 17 pt cells).
No rebuild or reconnect is needed. Geometry is recomputed from the same metrics
used to paint and sent via the client resize API. The choice is session-local;
restart returns to Standard. Compare CJK legibility and how much output fits in
portrait and landscape. No alternatives were added for internal architecture.

# Keyboard and IME variants (P4)

Tap **Input options** in the terminal toolbar. Each button switches immediately;
no rebuild or reconnect is needed. Choices last until the process exits.

| Control | Default | Alternative | Compare on the device |
|---|---|---|---|
| Option | Native | Alt | Native lets UIKit produce layout characters/dead keys. Alt sends semantic Alt chords to the terminal. Try Option-P, Option-arrow and Japanese input. |
| Command | Local | Ctrl | Local reserves Command for the platform. Ctrl maps delivered Command chords to terminal Ctrl chords. OS-reserved chords can still be intercepted by iPadOS. Try Command-C and Command-A in a disposable terminal. |
| Candidate | Clause | Caret | Clause supplies the selected clause's rectangle; Caret supplies a cell-width rectangle at that clause's start. Both use the painted shaped line, including its right-edge shift. Compare a middle clause and a long line near the right edge. |

All three switches are visible together; changing options does not itself commit
marked text. A fresh tap or hardware key starts input again after a remote
target change cancelled a composition. Target changes never carry old text
into the next pane or popup. The Simulator-only QA mark/commit/delete/cancel buttons are
shown only under `YEBISU_INPUT_QA`; these exercise callbacks and do not emulate
Japanese candidate selection. They are absent from device builds.

# R1 review repairs

No new design fork affecting device feel was found. R1 only isolates the P0
input, refuses malformed/oversize protocol cells, bounds retained glyph data,
and tests the existing connection-generation guard. Existing P3 density and
P4 input switches remain the available on-device comparisons.

A cell symbol is limited to 128 UTF-8 bytes, enough for 32 four-byte scalars,
including practical ZWJ emoji, variation selectors and combining marks. Control
characters cannot be shaped as terminal glyphs. Excessively long combining
sequences are deliberately refused, not truncated. Cache retention is bounded
by 4,096 entries and a 16 MiB charged-byte budget. The byte charge includes two
text copies, ShapedLine/LineLayout, run/glyph vector capacities and conservative
per-entry map overhead. Fixed table headers and allocator metadata are outside
that charge but bounded by the entry cap; GPUI's own text-system cache is not
owned by this cache. Once full, valid cells still render without retention.

# R2 review repairs

No new feel-dependent design fork was found. The nonfunctional Esc alternative
was removed: pinned UIKit routes plain Esc to composition cancellation before
GPUI KeyDown and does not redispatch it. Esc cancels an active composition;
without composition it sends semantic Esc. Option, Command and candidate
switches remain available without rebuilding.

## P5 reconnect on return

SH1-155 option 1 は 2026-11-11 にオーナーが選択。Inactive（アプリスイッチャー、Stage Manager のフォーカス移動、通知/コントロールセンター、システムシート）では Herdr 1/2 の接続を維持し、Active に戻っても再接続しない。Background またはシーン切断で接続を閉じる。OS の suspension を防ぐ keep-alive は追加しない。通信が失われた場合は既存の切断表示と Reconnect を使う。新たな体感上の選択肢はない。

- Terminal の `Return: Automatic` が既定。背景で接続と入力を捨て、Active 復帰で一度だけ P2 の鍵検証から接続する。
- このラベルをタップして `Return: Tap reconnect` に切替。背景から戻った後は `Reconnect` をタップするまで接続しない。再ビルド不要、現在の起動中に保持。
- 両方とも入力を再送しない。ネットワーク切替で失敗した場合は理由を表示し、Reconnect または次回の背景復帰で再試行。連続 retry はしない。VersionMismatch は自動再接続しない。
- 保存ホストの内部構造には比較用 variant を作っていない。

## P6 status board density

Tap `Board: Dense` / `Board: Roomy` above the board to switch immediately, without a rebuild. Dense uses 4 pt vertical row padding and a 180 pt scrolling board; Roomy uses 12 pt padding and 260 pt. Both show the same snapshot states and textual harness marks. The board remains above the terminal; scroll it to reach every space and agent on every host. The switch is session-local. Compare tap accuracy, scan speed and remaining terminal space on M5 and at narrow width.

## R3 review repairs

No new design fork affecting device feel was found. R3 preserves the specified
stop-on-version-mismatch behavior, displays its retained reason and refreshes
the saved-host list. Existing reconnect, density and input switches remain.

## R4 review repairs

No new design fork affecting device feel was found. R4 retains the navigation
barrier contract and rejects stale rows. Existing board density and input
switches remain available without rebuilding.

## P7: shortcut precedence with Command-as-Control

Open **Input options** in the terminal toolbar. **Shortcuts: App priority** is the default: the catalog's Command chords (Command-T/D/B/1–9/Shift-P and pane navigation) perform app actions. Plain Control-B/D/T and all unmatched modifiers continue to the terminal. No prefix sequence is synthesized.

Tap that control to select **Shortcuts: Terminal priority**. With **Command: Ctrl** enabled beside it, all Command keys bypass app shortcuts and use the existing terminal Control mapping (e.g. Command-B becomes Control-B for a prefix, Command-D becomes Control-D). The tab strip, action buttons and palette button remain usable by touch. With **Command: Local**, both precedence modes keep the catalog bindings. Switch at runtime without rebuilding; this experimental preference is session-local. Compare both with your shell/editor and prefix configuration before choosing a default.

## R5 review repairs

No new design fork affecting device feel was found. Waiting for a coherent
post-move projection and observing the old reader finish are correctness
requirements. Existing density, input, reconnect-policy and shortcut switches
remain available without rebuilding. R5 fault injection is Simulator-only and
is absent from device builds.

## R6 review repairs

No new design fork affecting device feel was found. UIKit symbol normalization
and retaining a UI-consumed press until release repair the specified behavior.
Existing runtime shortcut-priority and input switches remain available. R6 QA
buttons are Simulator-only under YEBISU_R6_QA and absent from device builds.

## P8 handoff

SH1-151: on 2026-11-11 the owner chose the common iPad interaction on the
physical device: **Start: Drag** and **Copy: Manual**. Fresh installs and older
settings files without these fields use these defaults. Keep the strip switches
until the P11 settings sheet (SH1-133). No new interaction fork was introduced.

The terminal's handoff strip switches these immediately, without a rebuild:

- **Start: Select / double tap**: tap Select before dragging, or double
  tap to begin. **Start: Drag** (default): any terminal drag begins local selection; use
  Paste to leave selection. Compare with both finger and trackpad.
- **Copy: Manual** (default): press Copy or Command-C after selection.
  **Copy: On selection** copies when the drag ends. Both choices are saved through the P10 settings store and survive a restart.

Paste and Paste image are explicit commands, not alternative implementations.
A single preparation is allowed at a time to bound memory; busy pastes are refused.

P8 physical clipboard chords accept exactly Command-C, Command-V and
Command-Shift-V (image-only). Command-Shift-C and any added Option, Control or
Function fall through to the existing terminal path without reading or writing
the clipboard. Command: Ctrl plus Terminal priority still bypasses these physical
clipboard shortcuts; native editing actions remain independent of these settings.

## P9: Herdr 2 split direction

After **Open Herdr 2 → Split (2 panes)**, tap **Toggle split direction** to switch
between side-by-side (default) and stacked panes. No rebuild is needed. The
choice and active pane are saved per SSH target and default core session.
Compare both orientations with touch, hardware keyboard and rotation. Existing
Input options and display density switches also apply to Herdr 2.

## P10

No new feel-dependent design fork. Existing density, shortcut precedence and
board-visibility controls retain their behavior and now persist. Settings sheet
layout/motion remains P11; no speculative visual alternatives are added in P10.

## SH1-153

No new feel variant was found. Native Copy is enabled only for a nonempty terminal
selection; Paste is enabled for a focused, ready destination when no paste worker
is active, regardless of software/hardware keyboard mode. This is correctness
state, not a new preference. Cut keeps D3 copy-only semantics and Select All
remains unavailable because the terminal has no such operation.

## SH1-152

A claimed touch long press opens a native Copy/Paste menu without starting a
selection or synthesizing a tap. A secondary mouse-button press opens the same
menu; primary-button selection and dragging retain their existing behavior.
Focus moves to the terminal before availability is published. Failed presentation
still consumes the long press. Cancelled touches dismiss only their own ticket.

The window keeps a ticket containing the SDK token, destination stamp and exact
selection identity (frame allocation plus endpoints). Destination, selection,
focus and active-state changes revoke that ticket. SDK revocation is checked
before execution and every availability publication. Returning to an active
window never revives an old ticket. Native commands without a menu token retain
the existing Edit-menu path; physical Command-V continues through its separate
Command-to-Control and terminal-priority settings.

Overlays intersecting terminal geometry must either use `.occlude()` or wrap
their content with `front_layer`. The latter registers Normal hitboxes every
frame and rejects terminal long presses without blocking scroll propagation.
It also prevents a fallback overlay tap from starting terminal selection.

The baseline overlay survey found six syntax matches in four groups:

| Group | Source | Handling |
| --- | --- | --- |
| Settings error banner | `app.rs` | Explicit occlusion and a registered front layer |
| Input canvases on other screens | `app.rs`, `app_setup.rs` | Never layered over the terminal |
| Smoke paint receipt canvas | `app.rs` | Drawing observation only; no input listener |
| Settings sheet spike (two matches) | `settings_spike/sheet.rs` | Already occluded; `settings_spike.rs` confirms it is unmounted by `app::run` |

The SDK preflight selected UICommand items and an empty UIMenu for a revoked
owned configuration. Therefore S2c and E3i require zero children; UIAction S8 is
not applicable. An unrelated configuration may retain the two ordinary suggested
commands. Simulator tests and [device acceptance](device-check.md#sh1-152) remain
separate evidence.
