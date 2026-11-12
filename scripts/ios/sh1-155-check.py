#!/usr/bin/env python3
"""Native Control Center / Home transitions against the private wire peer."""
import importlib.util
import json
import shutil
import subprocess
from pathlib import Path


def validate(records):
    phases = [(i, r) for i, r in enumerate(records) if r.get("kind") == "lifecycle"]
    kept = False
    dropped = False
    for offset, (i, row) in enumerate(phases):
        if row["phase"] == "Inactive" and offset + 1 < len(phases):
            _, following = phases[offset + 1]
            if following["phase"] == "Active":
                if not all(r.get("herdr1") and r.get("connected") and not r["suspended"] for r in [row, following]):
                    raise ValueError("inactive-only lost the Herdr 1 connection")
                if row["generation"] != following["generation"]:
                    raise ValueError("inactive-only reconnected")
                kept = True
        if row["phase"] == "Background":
            if row.get("herdr1") or row.get("connected") or not row["suspended"]:
                raise ValueError("background retained the connection")
            later = records[i+1:]
            reconnect = [r for r in later if r.get("kind") == "reconnect"]
            if not reconnect or reconnect[0]["generation"] <= row["generation"]:
                raise ValueError("background return did not reconnect")
            if not any(r.get("kind") == "terminal" and r["generation"] == reconnect[0]["generation"] for r in later):
                raise ValueError("background return did not paint the new peer")
            dropped = True
    if not kept or not dropped:
        raise ValueError("missing native inactive-only or background transition")
    print("SH1-155 PASS: native inactive-only same live connection; background disconnect and new-generation paint")


def main():
    root = Path(__file__).resolve().parents[2]
    out = root / "target/ios-sh1-155"
    out.mkdir(exist_ok=True)
    def run(*args):
        print("SH1-155 COMMAND", args, flush=True)
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
    tested = run("xcodebuild", "test", "-project", str(project / "SmokeOracle.xcodeproj"), "-scheme", "SmokeOracle", "-only-testing:SmokeOracle/SmokeQA/testSH1155Inactive", "-destination", "id=" + device, "-derivedDataPath", str(root / "target/ios-smoke/build"), "-resultBundlePath", str(result), "CODE_SIGNING_ALLOWED=NO")
    if "testSH1155Inactive]' passed" not in tested:
        raise RuntimeError("SH1-155 XCTest did not execute")
    export = out / "attachments"
    if export.exists():
        shutil.rmtree(export)
    run("xcrun", "xcresulttool", "export", "attachments", "--path", str(result), "--output-path", str(export))
    for test in json.loads((export / "manifest.json").read_text()):
        for attachment in test["attachments"]:
            stage = attachment["suggestedHumanReadableName"].split("_0_")[0]
            if stage.startswith("sh1-155-"):
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
    for stage in ["sh1-155-before", "sh1-155-returned", "sh1-155-background-return"]:
        _, _, channels, rows = colors.decode((out / (stage + ".png")).read_bytes())
        p3.validate_colors(colors.color_counts(rows, channels))
    print("SH1-155 PASS: source-defined terminal colour oracle after reconnect")

if __name__ == "__main__":
    main()
