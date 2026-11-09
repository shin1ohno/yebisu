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
