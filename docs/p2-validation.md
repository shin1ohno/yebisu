# P2 validation

Mac mini, interactive `zsh -il`, pinned Rust 1.99.1, one build at a time.
All fetches/builds use the existing running 6 GiB guard. The initial data-volume
probe reported 13 GiB available. No old app targets or other files were deleted.

The host suite includes six device-comment cases from pinned
`ssh/ipad/tests.rs:107-112`, arbitrary-comment rejection, untrusted/changed-key
authentication gates, real KnownHosts pin-write failure and refusal to overwrite,
endpoint-save failure preserving the previous endpoint, status error visibility,
and expected SHA256 identity after reopening the store. Expected digest comes
from pinned `herdr-ssh/src/host_key/tests.rs`, not a screenshot.

Simulator P2 uses native controls through XCTest/OCR, records the application's
state across independent process launches, checks exact restricted-key syntax,
and compares the receiving Simulator clipboard with the complete formatted
public key. Missing launch receipts and missing copy completion are negative
controls using actual app-shaped receipts. The final screenshot is exported
from the foreground XCTest attachment before teardown and checked against the
source background constant #2e3440. The P0 oracle retains its separate test
selector so adding P2 does not change the old gate's initial mode.

Measured one-line change: adding the trust-binding source comment to
`app_setup.rs` and rebuilding the Simulator bundle took **4.52 s wall time**
(non-incremental dev; Cargo, bundling and ad-hoc signing included). Rust source:
**955 production lines + 238 test lines = 1,193 physical lines**. Target sizes at
measurement: host 1.6 GiB, device 1.5 GiB, Simulator 2.1 GiB. Final free disk and
exact-head gate stdout/exit codes are supplied in the controller's report and
check artifact, separately from the owner-only physical checks.

Dependency additions: sheprd-client supplies the existing iOS scan/trust,
Keychain-backed public-key and authenticated probe APIs. herdr-ssh supplies
KnownHosts reads so fingerprints are derived from actual saved pins and the
same reader is exercised on the build host. Both use the specified fork rev.
Its lock supplies newly introduced package versions, with all earlier P0/P1
package identities retained: 168 packages added, none removed. Five existing
names gain additional versions required by the SSH tree (base64 0.23.1,
num-bigint 0.5.1, rand 0.10.3, rand_core 0.10.1, windows-sys 0.52.0); existing
versions are not upgraded. GPUI remains exactly 0.3.12; no new runtime is added
by yebisu (the pinned SSH crate owns its runtime).

There is no real feel-related design fork in P2; see variants.md. Physical
Secure Enclave generation, one host authentication, Local Network permission,
signed entitlement comparison and old-app connectivity remain owner steps in
device-check.md and the repository TODO. Simulator evidence does not complete
them. Independent Codex review is handled by the controller, as the brief
forbids another worker here. Weekly quota before/after is unmeasured in this
worker session; no fabricated usage percentage is reported.
