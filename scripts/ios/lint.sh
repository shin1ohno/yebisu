#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$root"
source scripts/ios/lib.sh
ios_export_deployment_target
for target in aarch64-apple-ios aarch64-apple-ios-sim; do
  cargo clippy --locked --workspace --all-targets --target "$target" -- -D warnings
done
# P13's opt-in native query probe is Simulator-only; lint it at the exact head
# so a feature-gated QA compile failure is not first found under a Mac lease.
cargo clippy --locked --package yebisu --all-targets --target aarch64-apple-ios-sim \
  --features p13-native-probe -- -D warnings
