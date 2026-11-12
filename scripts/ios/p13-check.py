#!/usr/bin/env python3
"""P13 Simulator receiver/renderer/native-selector gate; never physical IME proof.

Run only under the controller's exclusive Mac lease and `just guarded`.
Offline validation accepts an existing evidence directory; --run builds and
collects one Simulator observation using a task-specific generated XCTest target.
"""
import argparse
import json
import platform
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[2]
SWIFT = r'''
import XCTest
import Vision
final class P13QA: XCTestCase {
    func testSearch() throws {
        let app = XCUIApplication(bundleIdentifier: "com.shin1ohno.yebisu")
        app.terminate()
        app.launchEnvironment["YEBISU_TERMINAL_QA"] = "1"
        app.launchEnvironment["YEBISU_P13_QA"] = "1"
        app.launchEnvironment["YEBISU_SMOKE_OBSERVE"] = "1"
        app.launch()
        sleep(3)
        for step in 0..<14 {
            try tap("P13 Next", app)
            sleep(2)
            let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
            attachment.name = "p13-step-\(step)"
            attachment.lifetime = .keepAlways
            add(attachment)
        }
        XCTAssertEqual(app.state, .runningForeground)
    }
    private func tap(_ label: String, _ app: XCUIApplication) throws {
        for _ in 0..<3 {
            let image = try XCTUnwrap(XCUIScreen.main.screenshot().image.cgImage)
            let request = VNRecognizeTextRequest()
            request.recognitionLevel = .accurate
            request.customWords = [label]
            try VNImageRequestHandler(cgImage: image).perform([request])
            let rows = (request.results ?? []).sorted { $0.boundingBox.midY > $1.boundingBox.midY }
            for row in rows {
                for candidate in row.topCandidates(5) {
                    if let range = candidate.string.range(of: label),
                       let observation = try candidate.boundingBox(for: range) {
                        let box = observation.boundingBox
                        app.coordinate(withNormalizedOffset: CGVector(dx: box.midX, dy: 1-box.midY)).tap()
                        return
                    }
                }
            }
            sleep(1)
        }
        XCTFail("Missing P13 source-defined control")
        throw NSError(domain: "P13QA.MissingControl", code: 1)
    }
}
'''
PROJECT = '''name: P13Oracle
options:
  deploymentTarget:
    iOS: "17.0"
targets:
  P13Oracle:
    type: bundle.ui-testing
    platform: iOS
    sources: [Tests]
    settings:
      base:
        PRODUCT_BUNDLE_IDENTIFIER: com.shin1ohno.yebisu.p13oracle
        GENERATE_INFOPLIST_FILE: YES
        CODE_SIGNING_ALLOWED: NO
schemes:
  P13Oracle:
    build:
      targets:
        P13Oracle: [test]
    test:
      targets: [P13Oracle]
'''


def validate(records, messages):
    requests = [json.loads(m["ClientShellEndpointRequest"]["request"])
                for m in messages if "ClientShellEndpointRequest" in m]
    searches = [r["params"] for r in requests if r["method"] == "pane.copy_search"]
    queries = [r["query"] for r in searches]
    if not queries or queries[0] != "needle" or "日本語" not in queries or queries[-1] != "beyond" or queries.count("needle") < 2:
        raise ValueError("missing ordered ASCII/Japanese/native-paste receiving queries")
    if any(q not in {"needle", "日本語", "beyond"} for q in queries):
        raise ValueError("composing or unrelated text reached search wire")
    if not {"forward", "backward"}.issubset({r["direction"] for r in searches}):
        raise ValueError("next/previous receiving requests missing")
    if any(r["pane_id"] != "w1:p1" or r["content_revision"] != 4 for r in searches):
        raise ValueError("search used wrong pane/content")
    for action in ("paste", "cut"):
        if not any(r.get("kind") == "p13-native-selector" and r.get("action") == action and r.get("scheduled") is True for r in records):
            raise ValueError("real UIKit " + action + " selector was not scheduled")
    closes = [r for r in records if r.get("kind") == "p13-close"]
    if len(closes) != 1 or not closes[0].get("focused") or closes[0].get("active"):
        raise ValueError("close did not restore current terminal focus")
    steps = {r["step"]: r for r in records if r.get("kind") == "p13-step"}
    if set(steps) != set(range(14)) or not steps[3].get("active") or steps[3].get("marked") is not None:
        raise ValueError("composition cancellation/step coverage missing")
    cuts = [r for r in records if r.get("kind") == "p13-cut"]
    if len(cuts) != 2 or {r.get("action") for r in cuts} != {"nativeCut", "commandX"}:
        raise ValueError("D4 nativeCut/commandX callback coverage missing")
    for cut in cuts:
        if (cut.get("text"), cut.get("marked"), cut.get("selected"), cut.get("clipboard")) != ("pre:変中:post", {"start": 4, "end": 6}, [5, 5], "😀"):
            raise ValueError("D4 Cut deleted unselected marked text or copied outside selection")
    cancels = [r for r in records if r.get("kind") == "p13-cut-cancel"]
    if len(cancels) != 2 or {r.get("step") for r in cancels} != {8, 9} or any(r.get("text") != "pre::post" or r.get("marked") is not None for r in cancels) or not all(steps[s].get("active") for s in (8, 9)):
        raise ValueError("D4 Escape after Cut failed to cancel or closed search")
    if not any(r.get("kind") == "p13-ime-none" and r.get("text") == "pre:確定:post" and r.get("marked") is None for r in records):
        raise ValueError("D4 IME None contract changed")
    if not any(r.get("kind") == "p13-partial-mark" and r.get("retained") is True for r in records):
        raise ValueError("native partial selection deletion did not retain marked text")
    if not any(r["method"] == "pane.scroll" and 0 <= r["params"].get("offset_from_bottom", 100) < 100 for r in requests):
        raise ValueError("offscreen search never reached receiving pane.scroll")
    terminal = [event for m in messages if "ClientShellPaneInput" in m
                for event in m["ClientShellPaneInput"]["events"]]
    if terminal != [{"TextCommit": "P13-fresh"}]:
        raise ValueError("search leaked terminal bytes or fresh commit count differs: " + repr(terminal))
    if any("ClientShellPopupInput" in m for m in messages):
        raise ValueError("search leaked popup input")
    paints = [r for r in records if r.get("kind") == "terminal" and r.get("p13_search")]
    for query in ("needle", "日本語", "beyond"):
        matched = [r for r in paints if r["p13_search"].get("query") == query
                   and r["p13_search"].get("state") == "Matches"
                   and r.get("p13_highlights", 0) > 0
                   and r["p13_search"].get("result_revision") == 4
                   and r["p13_search"].get("pane") == "w1:p1"
                   and r.get("boot") == "boot-v1" and r.get("projection") == 7]
        if not matched:
            raise ValueError("no coherent renderer highlight receipt for " + query)
    print("P13 receipt validation PASS; verify artifact provenance separately; physical IME/hardware input UNVERIFIED", flush=True)


def run(args, directory, name):
    result = subprocess.run(args, cwd=ROOT, text=True, capture_output=True, timeout=1200)
    (directory / (name + ".stdout")).write_text(result.stdout)
    (directory / (name + ".stderr")).write_text(result.stderr)
    (directory / (name + ".exit")).write_text(str(result.returncode) + "\n")
    result.check_returncode()
    return result.stdout


def collect(directory, expected_head):
    if platform.system() != "Darwin":
        raise RuntimeError("P13 --run requires leased Mac checkout; Linux builds are forbidden")
    head = subprocess.check_output(["git", "-C", str(ROOT), "rev-parse", "HEAD"], text=True).strip()
    dirty = subprocess.check_output(["git", "-C", str(ROOT), "status", "--porcelain"], text=True)
    if dirty:
        raise RuntimeError("P13 exact-head run requires a clean source checkout")
    if head != expected_head:
        raise RuntimeError("P13 exact-head mismatch")
    directory.mkdir(parents=True, exist_ok=False)
    (directory / "head.txt").write_text(head + "\n")
    bundle = run(["bash", "scripts/ios/bundle.sh", "--package", "yebisu", "--bin", "yebisu"], directory, "bundle").strip().splitlines()[-1]
    device = run(["bash", "-c", 'source scripts/ios/lib.sh; ios_sim_boot "$HERDR_IOS_SIMULATOR"'], directory, "device").strip().splitlines()[-1]
    run(["xcrun", "simctl", "install", device, bundle], directory, "install")
    project = directory / "oracle"
    (project / "Tests").mkdir(parents=True)
    (project / "Tests/P13QA.swift").write_text(SWIFT)
    (project / "project.yml").write_text(PROJECT)
    run(["xcodegen", "generate", "--spec", str(project / "project.yml"), "--project", str(project)], directory, "generate")
    result = directory / "result.xcresult"
    stdout = run(["xcodebuild", "test", "-project", str(project / "P13Oracle.xcodeproj"), "-scheme", "P13Oracle", "-destination", "id=" + device, "-derivedDataPath", str(ROOT / "target/ios-p13-build"), "-resultBundlePath", str(result), "CODE_SIGNING_ALLOWED=NO", "-only-testing:P13Oracle/P13QA/testSearch"], directory, "xctest")
    if "testSearch]' passed" not in stdout:
        raise RuntimeError("P13 XCTest did not execute and pass")
    run(["xcrun", "xcresulttool", "export", "attachments", "--path", str(result), "--output-path", str(directory / "attachments")], directory, "attachments")
    container = Path(run(["xcrun", "simctl", "get_app_container", device, "com.shin1ohno.yebisu", "data"], directory, "container").strip())
    for source, name in [("p0-smoke.jsonl", "app.jsonl"), ("p13-peer.jsonl", "peer.jsonl")]:
        shutil.copyfile(container / "Documents" / source, directory / name)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--run", action="store_true")
    parser.add_argument("--expected-head")
    args = parser.parse_args()
    directory = args.directory.resolve()
    if args.run:
        if not args.expected_head:
            parser.error("--run requires --expected-head")
        collect(directory, args.expected_head)
    records = [json.loads(line) for line in (directory / "app.jsonl").read_text().splitlines()]
    messages = [json.loads(line) for line in (directory / "peer.jsonl").read_text().splitlines()]
    validate(records, messages)


if __name__ == "__main__":
    main()
