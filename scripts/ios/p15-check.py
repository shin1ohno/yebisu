#!/usr/bin/env python3
"""Lease-only real Appearance taps, painter, close, lifecycle and disk oracle.

Synthetic unit fixtures validate this checker only, never native acceptance.
"""
import argparse
import importlib.util
import os
import json
from pathlib import Path
import shutil
import time
import tempfile

ROOT = Path(__file__).resolve().parents[2]
SEED = dict(version=1, language="en-US", theme="owner-default", compact=False,
            terminal_priority=False, board_hidden=False, drag_select=True,
            copy_on_select=False, terminal_font_size=None,
            terminal_font_family="SauceCodePro Nerd Font Mono",
            notifications=dict(route="toast", sound=False, volume=37, muted=True, quiet=[60, 120]),
            future=dict(preserve=42))


def validate(records, disk):
    expected = dict(SEED, theme="high-contrast")
    if disk != expected:
        raise ValueError("exact saved schema/theme/other fields mismatch")
    loads = [i for i, r in enumerate(records) if r.get("kind") == "settings-loaded"]
    if len(loads) != 2 or [records[i].get("theme") for i in loads] != ["owner-default", "high-contrast"]:
        raise ValueError("two real process loads must restore the selected theme")
    clicks = [i for i, r in enumerate(records) if r.get("kind") == "theme-selected"]
    if [records[i].get("theme") for i in clicks] != ["high-contrast", "owner-default", "high-contrast"]:
        raise ValueError("missing ordered real theme choices")
    selected = clicks[-1]
    lifecycle = [(i, r.get("phase")) for i, r in enumerate(records)
                 if r.get("kind") == "settings-lifecycle" and selected < i < loads[1]]
    background = next((i for i, phase in lifecycle if phase == "Background"), None)
    active = next((i for i, phase in lifecycle if phase == "Active" and background is not None and i > background), None)
    if background is None or active is None:
        raise ValueError("missing background/return lifecycle")
    def painted(start, end, theme, modal):
        return any(r.get("kind") == "terminal" and r.get("theme") == theme
                   and r.get("settings_modal") is modal for r in records[start + 1:end])
    if not loads[0] < selected < background < active < loads[1]:
        raise ValueError("click/lifecycle/relaunch ordering mismatch")
    for index, end, theme in zip(clicks, clicks[1:] + [background],
                                 ["high-contrast", "owner-default", "high-contrast"]):
        if not painted(index, end, theme, True):
            raise ValueError("missing immediate modal painter receipt")
    if not painted(loads[0], clicks[0], "owner-default", False):
        raise ValueError("missing initial owner palette paint")
    if not painted(selected, background, "high-contrast", True):
        raise ValueError("missing immediate modal painter receipt")
    if not painted(selected, background, "high-contrast", False):
        raise ValueError("missing closed-sheet painter receipt")
    if not painted(active, loads[1], "high-contrast", False):
        raise ValueError("missing background return paint")
    if not painted(loads[1], len(records), "high-contrast", False):
        raise ValueError("missing relaunched painter receipt")


def validate_ui_wire(records, messages, ui):
    """Installed GUI + private peer evidence; not remote-settings/SSH proof."""
    picks = [r.get("label") for r in ui if r.get("kind") == "theme-click"]
    if picks != ["High contrast", "Owner default", "High contrast"]:
        raise ValueError("actual GUI theme choices missing")
    hosts = [r for r in ui if r.get("kind") == "host-screen"]
    if [(r.get("stage"), r.get("host")) for r in hosts] != [
            ("alpha", "qa-alpha"), ("beta", "qa-beta"), ("relaunch-host", "qa-alpha")]:
        raise ValueError("actual current-host screen sequence missing")
    for screen in hosts:
        text = "\n".join(screen.get("lines", []))
        if ("Host: " + screen["host"] not in text or
                "Host: " + screen["stale"] in text or
                not all(value in text for value in ["File path unavailable", "Unavailable",
                                                    "daemon-effective state unverified"])):
            raise ValueError("stale host or fabricated available settings screen")
        # Require the renderer's most recent selected host, not a stale frame from
        # earlier in the run, and a nearby paint from the same modal.
        paints = [r for r in records if r.get("kind") == "terminal"
                  and r.get("time", 0) <= screen["time"]]
        if not paints or paints[-1].get("host") != screen["host"] or not paints[-1].get("settings_modal"):
            raise ValueError("host screen disagrees with current renderer")
    events = []
    for message in messages:
        if "ClientShellEndpointRequest" in message:
            request = json.loads(message["ClientShellEndpointRequest"]["request"])
            if request.get("method") != "client_shell.surface.set":
                raise ValueError("unexpected endpoint operation in theme-only fixture")
        if "ClientShellPaneInput" in message:
            body = message["ClientShellPaneInput"]
            if body.get("pane_id") != "w1:p1":
                raise ValueError("wrong FIFO receiving pane")
            events.extend(body["events"])
    if any(set(event) != {"TextCommit"} for event in events):
        raise ValueError("unexpected terminal wire event")
    if "".join(event["TextCommit"] for event in events) != "P15-startP15-end":
        raise ValueError("missing FIFO positive controls or terminal wire leak")


def build_owned(session, directory, source, device, environment, head):
    """Explicit P15 expectations; never infer the SDK identity from build output."""
    import uuid
    expected = dict(schema='cargo-provenance-v2', head=head, run_id=str(uuid.uuid4()),
                    package='yebisu', binary='yebisu', target='aarch64-apple-ios-sim',
                    features=[], no_default_features=False, profile_name='dev',
                    profile=dict(opt_level='0', debuginfo=0, debug_assertions=True,
                                 overflow_checks=True, test=False),
                    sdk=[dict(name='herdr-uikit', url='https://github.com/shin1ohno/herdr-uikit',
                              revision='b2fa7811fa9fad9592f47b06d851fc0c33facb16'),
                         dict(name='herdr-ssh', url='https://github.com/shin1ohno/herdr-gpui',
                              revision='398c132a06723a0cc1494842a34666d173ec0200')])
    return session.build_install(directory, source, device,
                                 ['--package', 'yebisu', '--bin', 'yebisu'],
                                 environment, expected=expected)


def main():
    spec = importlib.util.spec_from_file_location("p11_runner", Path(__file__).with_name("p11-check.py"))
    runner = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(runner)
    run = runner.run
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--own-root", type=Path)
    parser.add_argument("--expected-head")
    parser.add_argument("--simulator")
    args = parser.parse_args()
    environment = os.environ.copy()
    provenance = None
    if args.own_root:
        own = args.own_root.resolve(strict=True)
        if not ROOT.is_relative_to(own) or not args.expected_head or not args.simulator:
            raise ValueError("owned source, full expected head and leased simulator UUID required")
        if len(args.expected_head) != 40:
            raise ValueError("full source head required")
        import uuid
        uuid.UUID(args.simulator)  # Never resolve a shared device by its default name.
        evidence = own / "evidence"
        evidence.mkdir(exist_ok=True)
        out = Path(tempfile.mkdtemp(prefix="p15-", dir=evidence))
        provenance = out / "provenance"
        provenance.mkdir()
        target = out / "cargo-target"
        target.mkdir(exist_ok=True)
        if not target.resolve().is_relative_to(own):
            raise ValueError("shared Cargo target symlink")
        environment.update(CARGO_TARGET_DIR=str(target.resolve()),
                           YEBISU_OWN_ROOT=str(own), YEBISU_EXPECTED_HEAD=args.expected_head,
                           YEBISU_BUNDLE_PROVENANCE_DIR=str(provenance),
                           HERDR_IOS_SIMULATOR=args.simulator,
                           PYTHONDONTWRITEBYTECODE="1")
        environment.pop("HERDR_IOS_TEAM", None)
        project = out / "smoke-qa"
        shutil.copytree(ROOT / "scripts/ios/smoke-qa", project,
                        ignore=shutil.ignore_patterns("*.xcodeproj", "__pycache__"))
        derived = out / "DerivedData"
    else:
        if args.expected_head or args.simulator:
            raise ValueError("provenance arguments require --own-root")
        out = ROOT / "target/ios-p15" / time.strftime("%Y%m%d-%H%M%S")
        out.mkdir(parents=True)
        project = ROOT / "scripts/ios/smoke-qa"
        derived = ROOT / "target/ios-smoke/build"
    original_run = run
    def run(*arguments, **kwargs):
        return original_run(*arguments, env=environment, **kwargs)
    device = run("bash", "-c", 'source scripts/ios/lib.sh; ios_sim_boot "$HERDR_IOS_SIMULATOR"').stdout.split()[0]
    if provenance:
        spec = importlib.util.spec_from_file_location('provenance_session', ROOT / 'scripts/ios/provenance_session.py')
        session = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(session)
        binding = build_owned(session, provenance, ROOT, device, environment, args.expected_head)
        installed = binding['installed']
    else:
        bundle = run("bash", "scripts/ios/bundle.sh", "--package", "yebisu", "--bin", "yebisu")
        run("xcrun", "simctl", "install", device, bundle.stdout.strip().splitlines()[-1])
    container = Path(run("xcrun", "simctl", "get_app_container", device, "com.shin1ohno.yebisu", "data").stdout.strip())
    if provenance:
        (out / "execution-paths.json").write_text(json.dumps(dict(
            source=str(ROOT), head=args.expected_head, target=environment["CARGO_TARGET_DIR"],
            evidence=str(out.resolve()), project=str(project.resolve()), derived=str(derived.resolve()),
            device=device, app=installed, data=str(container.resolve()),
            fixture=str(container / "Library/Application Support/herdr/gpui/p15-qa")), indent=2))
    # Only this test namespace is seeded; preserve every pre-existing byte as evidence.
    settings = container / "Library/Application Support/herdr/gpui/p15-qa/yebisu-settings.json"
    settings.parent.mkdir(parents=True, exist_ok=True)
    if settings.exists():
        shutil.copyfile(settings, out / "previous-settings.json")
    settings.write_text(json.dumps(SEED))
    (out / "seed.json").write_text(json.dumps(SEED))
    records_path = container / "Documents/p0-smoke.jsonl"
    peer_path = container / "Documents/p4-peer.jsonl"
    peer_previous = len(peer_path.read_text().splitlines()) if peer_path.exists() else 0
    previous = len(records_path.read_text().splitlines()) if records_path.exists() else 0
    run("xcodegen", "generate", "--spec", str(project / "project.yml"), "--project", str(project))
    result = out / "result.xcresult"
    try:
        tested = run("xcodebuild", "test", "-project", str(project / "SmokeOracle.xcodeproj"),
                     "-scheme", "SmokeOracle", "-destination", "id=" + device,
                     "-derivedDataPath", str(derived),
                     "-resultBundlePath", str(result), "CODE_SIGNING_ALLOWED=NO",
                     "-only-testing:SmokeOracle/SmokeQA/testP15Theme")
    finally:
        # Even failed XCTest/oracle runs retain receiving files before validation.
        records = [json.loads(line) for line in records_path.read_text().splitlines()[previous:]] if records_path.exists() else []
        disk = json.loads(settings.read_text())
        messages = [json.loads(line) for line in peer_path.read_text().splitlines()[peer_previous:]] if peer_path.exists() else []
        (out / "peer.json").write_text(json.dumps(messages, indent=2))
        (out / "evidence.json").write_text(json.dumps(dict(records=records, disk=disk), indent=2))
    if "testP15Theme]' passed" not in tested.stdout:
        raise ValueError("P15 XCTest did not execute and pass")
    attachments = out / "attachments"
    run("xcrun", "xcresulttool", "export", "attachments", "--path", str(result), "--output-path", str(attachments))
    names = [a["suggestedHumanReadableName"] for test in json.loads((attachments / "manifest.json").read_text()) for a in test["attachments"]]
    for stage in ["default", "alpha", "high-first", "owner-reset", "selected", "closed", "beta", "returned", "relaunch", "relaunch-host"]:
        if not any(name.startswith("p15-" + stage) for name in names):
            raise ValueError("missing native screenshot: " + stage)
    ui_files = [attachments / a["exportedFileName"]
                for test in json.loads((attachments / "manifest.json").read_text())
                for a in test["attachments"] if a["suggestedHumanReadableName"].startswith("p15-ui.json")]
    if len(ui_files) != 1:
        raise ValueError("missing unique installed UI transcript")
    ui = json.loads(ui_files[0].read_text())
    (out / "ui.json").write_text(json.dumps(ui, indent=2))
    validate(records, disk)
    validate_ui_wire(records, messages, ui)
    print("P15 local-theme Simulator oracle passed; host read and physical gates remain unverified.")


if __name__ == "__main__":
    main()
