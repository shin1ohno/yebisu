# P0 validation after review corrections

D1: app.rs now converts source RGBA pixels to BGRA at the RenderImage upload boundary. The independent RGB expectations remain #ebcb8b and #b48ead from the source constants. Earlier acceptance of #ad8eb4 was wrong and is superseded by the regional gate.

D2: `just ios-sim` builds and launches the GPUI app, runs XCTest input/Home/rotation/idle actions, exports screenshots, copies the app-written Documents/p0-smoke.jsonl receipt log, and runs smoke_oracle.py. The app records actual painted bounds, viewport, rendered title/input/marked state and counter values. The bounded writer performs file I/O on a background thread; recording is enabled only for the smoke run. No accessibility text is assumed for Metal content.

Each screenshot is cropped using its paint receipt's bounds. Text requires #eceff4 in its own region, the quad requires #88c0d0, and the image requires both source RGB constants. The landscape keyboard can obscure lower content: the image's first two source rows are sampled there, and image-clear checks the full image after the keyboard is dismissed. Input pixels are also checked in portrait; receiving-side input strings and committed marked state are required at every stage. Counter progression is asserted across both Home return and at least 60 seconds idle. A stale/missing paint receipt fails the check.

`python3 scripts/ios/smoke-negatives.py POSITIVE_DIR OUTPUT_DIR` blanks the text, quad and image independently in captured evidence, drops received input, and freezes the counter separately across backgrounding and idle. Each actual oracle invocation must return 1 with its specific diagnostic. These artifact mutations never switch faults on in product code. Unit tests exercise the same positive/negative contracts on host-shaped fixtures.

D3: scripts/guarded.py uses nonblocking selector-based output rather than a forwarding thread. After SIGTERM it watches the process group independently of the leader and sends SIGKILL to remaining members after the grace period. The grandchild test ignores SIGTERM, retains stdout and writes a heartbeat; the fixed guard exits 86 and the writer stops. Its PID/heartbeat/elapsed evidence is in the worker output.

## Resource measurements

Measurement started 2026-11-09T17:59:00.957690+00:00 on the Mac mini. One build at a time; all builds used the 6 GiB guard. Raw command output and the exact one-line diff are retained in [p0-resource-3.txt](p0-resource-3.txt) and the execution report's check-3.txt.

- Rust line count from `wc -l crates/yebisu/src/*.rs`: **433**, including test files.
- Cold build: **39.15 s** in the initially absent target/p0-cold-3.
- One-line rebuild: **1.23 s**, after adding one explanatory app.rs comment; runtime behavior stayed identical.
- Both commands: pinned Rust 1.99.1, development debug=0, CARGO_INCREMENTAL=0. Registry/toolchain caches already existed.

```sh
just guarded env CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target/p0-cold-3 \
  /usr/bin/time -lp cargo build --locked --workspace --target aarch64-apple-ios
```

| Measured directory | du -sk (KiB) |
|---|---:|
| target/aarch64-apple-ios | 1523516 |
| target/aarch64-apple-ios-sim | 1304880 |
| target/debug | 1295244 |
| target/p0-cold-3 | 985588 |

The device, simulator and host rows describe their measured target directories, not an extrapolation from time. The additional cold target is reported separately. No old app targets were removed by this worker.

## Physical device status

Physical M5 signing/install/launch and signed-entitlement comparison remain pending the owner under the explicit worker restriction. `just ipad` remains the single provision/build/sign/install/launch command. Simulator results do not prove physical IME/hardware keyboard acceptance. The P0 field still does not supply candidate geometry; completing that editor is outside these review corrections.

Dependency graphs in dependency-graphs/ are regenerated with the updated manifest. GPUI remains exactly 0.3.12 and herdr-uikit remains pinned to a573fb3d90cec084e6211bc84473f2f0906f1731 (unmerged PR #6). The only lock change is adding the already resolved serde_json as a direct iOS dependency for D2's structured receipts.
