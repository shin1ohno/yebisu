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

test:
    python3 scripts/guarded.py cargo test --locked --workspace
    python3 -I -B -m unittest discover -s scripts/tests -p 'test_*.py'
    python3 -I -B -m unittest discover -s scripts/ios -p 'test_*.py'

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

graphs:
    mkdir -p target/graphs
    cargo tree --locked --target aarch64-apple-ios -e normal,build > target/graphs/device.txt
    cargo tree --locked --target aarch64-apple-ios-sim -e normal,build > target/graphs/simulator.txt
    cargo tree --locked -e normal,build,dev > target/graphs/host.txt

# Private wire peer, density switch, orientation attempt and colour oracle.
ios-p3:
    python3 scripts/guarded.py python3 scripts/ios/p3-check.py
