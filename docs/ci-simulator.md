# Simulator CI

The `simulator-smoke` job runs on GitHub-hosted `macos-26` for pull requests,
pushes to main and manual dispatch, with read-only contents permission and a
40-minute timeout. It installs XcodeGen and just, then uses the pinned Rust
toolchain and the continuously monitored 6 GiB disk guard.

`scripts/ios/ci-smoke.py` selects an available iPad Pro 13-inch device type on
the newest installed iOS runtime and creates a disposable Simulator. It calls
the same `scripts/ios/smoke-check.py` used by `just ios-sim`, without changing
the oracle: bundle/build, boot, install, launch, XCTest input and lifecycle
actions, screenshots and receiving-side paint/input/counter validation all
must succeed. It shuts down and deletes its own Simulator on success or failure.

Unlike the local default, CI supplies a fresh Simulator UDID rather than relying
on a pre-existing M5 Simulator name or saved app data. The simulator bundle uses
ad-hoc signing; the XCTest bundle disables signing. No developer identity,
unlocked login keychain, secrets or owner's hosts are required. The P0 smoke
screen is local to the app. The existing check job remains separate.

Run the same CI entry point locally with
`just guarded python3 scripts/ios/ci-smoke.py`. Evidence remains in
`target/ios-smoke/`, including the XCTest result, screenshots and app receipts;
`latest.json` identifies the last successful capture. This checks Simulator
rendering and lifecycle behavior, not physical IME or hardware keyboard behavior.
