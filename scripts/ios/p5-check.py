#!/usr/bin/env python3
"""Exercise lifecycle reconnect and saved hosts using only the private peer."""
import importlib.util
import json
import shutil
import subprocess
from pathlib import Path


def validate(records):
    connects = [r for r in records if r.get("kind") == "reconnect" and r.get("selected", True)]
    if len(connects) != 2 or [r["automatic"] for r in connects] != [True, False]:
        raise ValueError("expected exactly automatic-return then manual-tap reconnect")
    paints = [r for r in records if r.get("kind") == "terminal"]
    for connect in connects:
        if not any(p["generation"] == connect["generation"] and p["boot"] == "boot-v1" for p in paints):
            raise ValueError("new generation did not receive and paint private peer state")
    if connects[1]["generation"] <= connects[0]["generation"]:
        raise ValueError("connection generation reused")
    if not any(r.get("kind") == "setup" and {"qa@private-one", "qa@private-two"}.issubset(r.get("hosts", [])) for r in records):
        raise ValueError("saved host store did not reach the setup UI")
    if not any(r.get("kind") == "setup" and "Developer-injected save failure" in r.get("status", "") for r in records):
        raise ValueError("worker save failure did not reach UI")
    print("P5 PASS: automatic return, manual tap, new-generation paints, two saved hosts, visible save failure")


def main():
    root = Path(__file__).resolve().parents[2]
    out = root / "target/ios-p5"
    out.mkdir(exist_ok=True)
    def run(*args):
        print("P5 COMMAND", args, flush=True)
        result = subprocess.run(args, cwd=root, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        print(result.stdout, flush=True)
        result.check_returncode()
        return result.stdout
    built = run("bash", "scripts/ios/bundle.sh", "--package", "yebisu", "--bin", "yebisu")
    app = built.strip().splitlines()[-1]
    device = run("bash", "-c", 'source scripts/ios/lib.sh; ios_sim_boot "$HERDR_IOS_SIMULATOR"').strip().splitlines()[-1]
    run("xcrun", "simctl", "install", device, app)
    project = root / "scripts/ios/smoke-qa"
    run("xcodegen", "generate", "--spec", str(project / "project.yml"), "--project", str(project))
    result = out / "result.xcresult"
    if result.exists():
        shutil.rmtree(result)
    tested = run("xcodebuild", "test", "-project", str(project / "SmokeOracle.xcodeproj"), "-scheme", "SmokeOracle", "-only-testing:SmokeOracle/SmokeQA/testP5Reconnect", "-destination", "id=" + device, "-derivedDataPath", str(root / "target/ios-smoke/build"), "-resultBundlePath", str(result), "CODE_SIGNING_ALLOWED=NO")
    if "testP5Reconnect]' passed" not in tested:
        raise RuntimeError("P5 XCTest did not execute")
    export = out / "attachments"
    if export.exists():
        shutil.rmtree(export)
    run("xcrun", "xcresulttool", "export", "attachments", "--path", str(result), "--output-path", str(export))
    for test in json.loads((export / "manifest.json").read_text()):
        for attachment in test["attachments"]:
            stage = attachment["suggestedHumanReadableName"].split("_0_")[0]
            if stage.startswith("p5-"):
                shutil.copyfile(export / attachment["exportedFileName"], out / (stage + ".png"))
    container = run("xcrun", "simctl", "get_app_container", device, "com.shin1ohno.yebisu", "data").strip()
    shutil.copyfile(Path(container) / "Documents/p0-smoke.jsonl", out / "app.jsonl")
    records = [json.loads(line) for line in (out / "app.jsonl").read_text().splitlines()]
    validate(records)
    spec = importlib.util.spec_from_file_location("p3", root / "scripts/ios/p3-check.py")
    p3 = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(p3)
    spec = importlib.util.spec_from_file_location("colors", root / "scripts/ios/screenshot-color.py")
    colors = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(colors)
    for stage in ["p5-before", "p5-returned", "p5-tapped"]:
        _, _, channels, rows = colors.decode((out / (stage + ".png")).read_bytes())
        p3.validate_colors(colors.color_counts(rows, channels))
    print("P5 PASS: source-defined terminal colour oracle after reconnect")

if __name__ == "__main__":
    main()
