export PATH := env_var("HOME") + "/.cargo/bin:" + env_var("PATH")

set shell := ["bash", "-euo", "pipefail", "-c"]

# All fetch and build commands pass a running 6 GiB guard.
guarded +command:
    python3 scripts/guarded.py {{command}}

disk:
    python3 scripts/guarded.py df -g /System/Volumes/Data

check:
    cargo fmt --check --all
    bash scripts/check-file-size.sh
    python3 scripts/check-sorted.py
    python3 scripts/check-provenance.py

lint-ios:
    python3 scripts/guarded.py bash scripts/ios/lint.sh
    # P13's opt-in Simulator query probe, so its QA code compiles at the exact head.
    python3 scripts/guarded.py bash -c 'source scripts/ios/lib.sh && ios_export_deployment_target && cargo clippy --locked --package yebisu --all-targets --target aarch64-apple-ios-sim --features p13-native-probe -- -D warnings'

test:
    python3 scripts/guarded.py cargo test --locked --workspace
    # The same unittest discovery, one process per test module in parallel.
    python3 -I -B scripts/run-python-tests.py scripts/tests scripts/ios

bump-fork REV:
    python3 scripts/bump-fork.py {{REV}}

triage-upstream:
    python3 scripts/triage-upstream.py

ios-sim wait="5":
    python3 scripts/guarded.py python3 scripts/ios/smoke-check.py --wait {{wait}}

ios-p2:
    python3 scripts/guarded.py python3 scripts/ios/p2-check.py

ipad:
    python3 scripts/guarded.py env CARGO_INCREMENTAL=1 bash scripts/ios/device.sh --device M5 --bundle-id com.shin1ohno.yebisu --team AL9567565U

# This Mac as an iPad app ("Designed for iPad"): provision, build, sign, launch.
mac wait="5":
    python3 scripts/guarded.py env CARGO_INCREMENTAL=1 bash scripts/ios/mac.sh --bundle-id com.shin1ohno.yebisu --team AL9567565U --wait {{wait}}

# The owner's one-time registration of this Mac with team AL9567565U; no build.
mac-register:
    python3 scripts/guarded.py bash scripts/ios/mac.sh --bundle-id com.shin1ohno.yebisu --team AL9567565U --register-device --register-only

graphs:
    mkdir -p target/graphs
    cargo tree --locked --target aarch64-apple-ios -e normal,build > target/graphs/device.txt
    cargo tree --locked --target aarch64-apple-ios-sim -e normal,build > target/graphs/simulator.txt
    cargo tree --locked -e normal,build,dev > target/graphs/host.txt

# Private wire peer, density switch, orientation attempt and colour oracle.
ios-p3:
    python3 scripts/guarded.py python3 scripts/ios/p3-check.py

# Private Herdr 2 wire, UIKit callbacks, split identities and reattachment.
ios-p9:
    python3 scripts/guarded.py python3 scripts/ios/p9-check.py

# Native lifecycle, atomic settings persistence and process restart.
ios-p10:
    python3 scripts/guarded.py python3 scripts/ios/p10-check.py

# Input mode indicator at the terminal cursor (#139); Simulator only, not IME proof.
ios-input-mode:
    python3 scripts/guarded.py python3 scripts/ios/input-mode-check.py

# Sidebar native search: panel, gate, filter, result taps, terminal restore.
ios-search:
    python3 scripts/guarded.py python3 scripts/ios/search-check.py --out target/ios-search --label local

# Validate this branch against the release changelog rules.
check-commits:
    python3 scripts/release/check-commit-messages.py range origin/main..HEAD
