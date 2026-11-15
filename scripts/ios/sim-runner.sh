#!/usr/bin/env bash
# Cargo runner for aarch64-apple-ios-sim: runs a test binary inside the iPad
# simulator with `xcrun simctl spawn`, which exits with the binary's status,
# so `cargo test --target aarch64-apple-ios-sim` works (`just test-ios`).
#
#   CARGO_TARGET_AARCH64_APPLE_IOS_SIM_RUNNER=/absolute/path/to/scripts/ios/sim-runner.sh
#
# Boots the simulator HERDR_IOS_SIMULATOR names (scripts/ios/lib.sh) when it
# is not running, and never shuts it down. The binary runs as a plain process,
# outside any app bundle, so a test that needs UIApplicationMain cannot run here.
set -euo pipefail

if (($# == 0)); then
  echo "usage: sim-runner.sh BINARY [ARGS...] (set as Cargo's target runner)" >&2
  exit 2
fi

# shellcheck source=scripts/ios/lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

binary=$1
shift
# simctl searches the device's PATH for a bare name; give it the host path.
if [[ $binary != /* ]]; then binary=$PWD/$binary; fi

udid=$(ios_sim_boot "$HERDR_IOS_SIMULATOR")

# Bash cannot enumerate OS environment names containing hyphens. Keep that
# boundary in Python, which reads the raw environment without evaluating it.
environment_helper=$(dirname "${BASH_SOURCE[0]}")/sim-runner-env.py
if [[ ! -f $environment_helper ]]; then
  echo "sim-runner.sh: missing environment helper: $environment_helper" >&2
  exit 1
fi

# Cargo binary names can contain hyphens: valid OS environment names but not
# shell identifiers. env preserves those names and literal values without eval.
if [[ ${binary##*/} == ios_clipboard-* ]]; then
  exec python3 "$environment_helper" -- bash "$(dirname "${BASH_SOURCE[0]}")/clipboard-test-runner.sh" "$udid" "$binary" "$@"
fi
exec python3 "$environment_helper" -- xcrun simctl spawn "$udid" "$binary" "$@"
