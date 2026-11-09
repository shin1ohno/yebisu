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
