# P3 validation

P3 displays a read-only Herdr 1 terminal through the P2 host-trust flow.
The private Simulator peer uses the actual wire protocol, client worker,
mailbox and painter, without opening a network endpoint or touching any real
host key or authorized_keys file. Device builds exclude the peer entirely.

- Rust: 77 tests passed, including all P3 negative cases and copied painter,
  image and OSC52 tests. The client event queue capacity is 8 at the pinned
  revision; the congestion fixture delivers 600 complete surfaces without UI
  polling and observes surface 600 in the receiving mailbox.
- Seven deliberate mutations were detected by assertion failures: future
  surface retention, metadata-only projection preservation, old boot rejection,
  startup failure visibility, inbox replacement, paint-generation fencing and
  busy-UI/congestion state progress. Original files were restored after each.
- Python: 16 repository-script and 81 iOS-script tests passed. The P3 oracle
  rejects a blank dark screenshot, missing glyph colours, missing density
  transitions and (in strict rotation mode) an unchanged portrait viewport.
- Simulator: iPad Pro 13-inch (M5), iOS 26.5. Standard and Compact were tapped
  on screen and the wire peer returned changed dimensions. The four saved
  stages pass the exact source-colour oracle. See screenshots/p3-terminal.png.
- Rotation is **unverified**, not passed: XCTest recorded the requested device
  orientation but the app viewport stayed portrait. `--require-rotation`
  correctly fails this result. See TODO.md and device-check.md.
- One-line dev rebuild: 2.008 seconds wall time, including the disk guard;
  Cargo reported 1.93 seconds, Simulator target, deployment target 17.0,
  incremental disabled by the workspace profile. A baseline build preceded the
  temporary one-line comment; the source was restored afterward. This is a
  build measurement, not device install or signing time.

The original painter closure was recounted at 2,067 production lines plus
2,231 test lines. The other unpruned sources were terminal.rs 728, osc52.rs
122 (including inline tests), font_weight.rs 98; copied protocol fixtures add
57 lines. The port removes integration-test performance branches and splits
OSC52 tests into their own file. Every source file remains at most 1,000 lines.

Additional direct dependency: unicode-width 0.2.3, used by the copied terminal
painter's cell shaping and the private peer. Cargo.lock adds only that package
and the yebisu dependency edge. No fork revision or GPUI version changed.

The final PR report contains exact-head check/lint/test output, repository line
counts and disk measurements. Physical signing, Secure Enclave authentication,
CJK/font judgement and rotation remain owner checks. No password was requested.
