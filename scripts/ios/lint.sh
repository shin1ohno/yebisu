#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$root"
source scripts/ios/lib.sh
ios_export_deployment_target
for target in aarch64-apple-ios aarch64-apple-ios-sim; do
  cargo clippy --locked --workspace --all-targets --all-features --target "$target" -- -D warnings
done
