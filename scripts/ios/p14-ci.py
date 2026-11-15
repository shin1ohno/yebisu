#!/usr/bin/env python3
"""Run the P14 SDK native tests and mutants on a disposable hosted Simulator.

Hosted counterpart of the exclusive-lease run in docs/p14-native-qa.md: it clones
the exact herdr-uikit revision pinned in Cargo.toml, creates one disposable iPad
Simulator, and runs scripts/ios/p14-sdk-check.py unchanged. The tests are ordinary
Simulator test processes. They do not request notification permission and are not
actual OS banner, sound, tap or physical-device evidence.
"""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[2]
SDK_URL = "https://github.com/shin1ohno/herdr-uikit"


def main():
    if os.environ.get("GITHUB_ACTIONS") != "true":
        raise SystemExit("Use p14-sdk-check.py under an explicit exclusive Mac lease outside hosted CI")
    rev = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["dependencies"]["herdr-uikit"]["rev"]
    sdk = ROOT / "target/p14-sdk-source"
    sdk.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(["git", "clone", "--filter=blob:none", "--no-checkout", SDK_URL, str(sdk)], check=True)
    subprocess.run(["git", "-C", str(sdk), "checkout", "--detach", rev], check=True)
    head = subprocess.check_output(["git", "-C", str(sdk), "rev-parse", "HEAD"], text=True).strip()
    if head != rev:
        raise SystemExit(f"SDK checkout {head} is not the pinned revision {rev}")
    spec = importlib.util.spec_from_file_location("ci_smoke", Path(__file__).with_name("ci-smoke.py"))
    smoke = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(smoke)
    listing = subprocess.check_output(["xcrun", "simctl", "list", "--json", "devices", "available"], text=True)
    device_type, runtime = smoke.select_template(json.loads(listing)["devices"])
    device = subprocess.check_output(["xcrun", "simctl", "create", "yebisu P14 CI", device_type, runtime], text=True).strip()
    lease = "hosted-ci-{}-{}".format(os.environ.get("GITHUB_RUN_ID", "local"), os.environ.get("GITHUB_RUN_ATTEMPT", "0"))
    print(f"P14 CI simulator={device} runtime={runtime} sdk={head} lease={lease}", flush=True)
    try:
        subprocess.run([sys.executable, str(Path(__file__).with_name("p14-sdk-check.py")),
                        "--sdk-root", str(sdk), "--lease-id", lease],
                       env={**os.environ, "HERDR_IOS_SIMULATOR": device}, check=True)
    finally:
        subprocess.run(["xcrun", "simctl", "shutdown", device], check=False)
        subprocess.run(["xcrun", "simctl", "delete", device], check=True)
        print(f"P14 CI cleanup simulator={device} deleted", flush=True)


if __name__ == "__main__":
    main()
