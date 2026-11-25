#!/usr/bin/env python3
"""Lease-only real Appearance taps, painter, close, lifecycle and disk oracle.

Synthetic unit fixtures validate this checker only, never native acceptance.
"""
import argparse
import importlib.util
import os
import json
from pathlib import Path
import re
import shutil
import subprocess
import threading
import time
import tempfile

ROOT = Path(__file__).resolve().parents[2]
# drag_select stays False: with Drag on, a tap on the terminal starts a selection and fences
# the input (handoff_view.rs selection_down), so the FIFO positive controls would never be typed.
SEED = dict(version=1, language="en-US", theme="owner-default", compact=False,
            terminal_priority=False, board_hidden=False, drag_select=False,
            copy_on_select=False, terminal_font_size=None,
            terminal_font_family="SauceCodePro Nerd Font Mono",
            notifications=dict(route="toast", sound=False, volume=37, muted=True, quiet=[60, 120]),
            future=dict(preserve=42))


# The Theme section's button title (settings_pages/sections.rs) and its QA identifier
# (crates/yebisu/src/qa_ax_ids.rs, SETTINGS_PAGE_THEME). Since the UI test redesign S6
# SmokeQA presses the identifier; the title is no longer read off the screen.
APPEARANCE_PAGE = "Theme"
APPEARANCE_ID = "qa.settings.page.theme"
CONTROLS = ("P15-start", "P15-end")
# xcresulttool export attachments: <name>_<index>_<UUID><ext>
ATTACHMENT = re.compile(r"^(?P<base>.+)_(?P<index>\d+)_(?P<uuid>[0-9A-F]{8}-[0-9A-F]{4}-[0-9A-F]{4}-"
                        r"[0-9A-F]{4}-[0-9A-F]{12})(?P<ext>\.[A-Za-z0-9]+)?$")
STAGES = ["default", "alpha", "high-first", "owner-reset", "selected", "closed", "beta",
          "returned", "relaunch", "relaunch-host"]


def attachment_files(manifest, root):
    """Exactly one exported file per (base name, extension), matched exactly.

    XCTest exports `p15-ui.json` as `p15-ui_0_<UUID>.json`. A prefix match would let
    `p15-relaunch-host_0_...` satisfy `p15-relaunch`, so names are decomposed and
    compared whole. Names of any other shape are refused.
    """
    found = {}
    for test in manifest:
        for item in test["attachments"]:
            name = item["suggestedHumanReadableName"]
            match = ATTACHMENT.match(name)
            if not match:
                raise ValueError("attachment name has an unexpected shape: " + name)
            key = (match["base"], match["ext"] or "")
            if key in found:
                raise ValueError("duplicate attachment: " + match["base"] + key[1])
            found[key] = Path(root) / item["exportedFileName"]
    for stage in STAGES:
        if ("p15-" + stage, ".png") not in found:
            raise ValueError("missing native screenshot: " + stage)
    if ("p15-ui", ".json") not in found:
        raise ValueError("missing unique installed UI transcript")
    return found[("p15-ui", ".json")]


def check_page_label(rust_sections, rust_view, swift):
    """The section button, its QA identifier and SmokeQA's press agree (UI test redesign S6).

    SH1-157 L1: the sheet lists sections (settings_pages/sections.rs); the appearance page
    is the Theme section, whose button shows section.title() and carries the mark
    SETTINGS_PAGE_THEME. SmokeQA presses that identifier through tapAppearance, three times.
    """
    ids = re.findall(r'Self::Theme\s*=>\s*"([^"]+)"', rust_sections)
    if len(ids) != 1:
        raise ValueError("settings_pages: Theme section title not found exactly once")
    if not re.search(r'button\(\s*format!\("settings-page-\{\}",\s*page\.id\(\)\),\s*section\.title\(\)', rust_view):
        raise ValueError("settings_sheet: section button no longer shows section.title() as its label")
    if ids[0] != APPEARANCE_PAGE:
        raise ValueError("p15-check APPEARANCE_PAGE must equal the section title " + repr(ids[0]))
    if not re.search(r"Section::Theme\s*=>\s*crate::qa_ax::mark\(\s*ids::SETTINGS_PAGE_THEME\s*,\s*title\s*\)", rust_view):
        raise ValueError("settings_sheet: the Theme section button is not marked SETTINGS_PAGE_THEME")
    code = re.sub(r"//[^\n]*", "", swift)  # comments may quote the old label
    if re.search(r'"(?:Appearance|Theme)"', code):
        raise ValueError("SmokeQA names a literal Appearance/Theme label instead of the Theme page identifier")
    if (len(re.findall(r"qa\.press\(QAIds\.settingsPageTheme", code)) != 1
            or len(re.findall(r"tapAppearance\(qa", code)) != 3):
        raise ValueError("SmokeQA must press QAIds.settingsPageTheme through tapAppearance exactly three times")


def terminate(device):
    """Stops the app so none of its files is read while it may still write. Exit 3 is simctl's
    'nothing to terminate'."""
    result = subprocess.run(["xcrun", "simctl", "terminate", device, "com.shin1ohno.yebisu"], text=True, capture_output=True)
    print("P15 terminate before reading the app's files: exit", result.returncode, (result.stderr or "").strip(), flush=True)
    if result.returncode not in (0, 3):
        raise RuntimeError("could not terminate the app before reading its files (simctl exit %d)" % result.returncode)


def delivered_text(messages):
    events = []
    for message in messages:
        if "ClientShellPaneInput" in message:
            body = message["ClientShellPaneInput"]
            if body.get("pane_id") != "w1:p1":
                raise ValueError("wrong FIFO receiving pane")
            events.extend(body["events"])
    if any(set(event) != {"TextCommit"} for event in events):
        raise ValueError("unexpected terminal wire event")
    return "".join(event["TextCommit"] for event in events)


def modal_openings(records):
    openings, was_open = 0, False
    for record in records:
        if record.get("kind") == "terminal":
            now = record.get("settings_modal") is True
            openings += now and not was_open
            was_open = now
    return openings


def launch_segments(records):
    """(start, end, cutoff) per app launch.

    The cutoff is the first paint with an input destination in that launch. The app
    records `input-dropped not-connected` by itself while it connects after every launch
    (first start and relaunch), so a drop before its launch's cutoff is not typing.
    """
    starts = [i for i, r in enumerate(records) if r.get("kind") == "launch"]
    if not starts or starts[0] != 0:
        starts.insert(0, 0)
    segments = []
    for start, end in zip(starts, starts[1:] + [len(records)]):
        cutoff = next((i for i in range(start, end)
                       if records[i].get("kind") == "terminal" and records[i].get("input_pane")), end)
        segments.append((start, end, cutoff))
    return segments


def typing_windows(records):
    """Index ranges [start, end) in which the UI can be typing a positive control.

    The UI types P15-start after the first input paint and before the first modal opens,
    and P15-end after that modal closed and before the second modal opens (or the host
    switch that precedes it). Nothing is typed after the last control: a background
    return, a relaunch and a later host switch reconnect without typing, and the app
    drops nothing while the settings modal is open.
    """
    first = next((i for i, r in enumerate(records)
                  if r.get("kind") == "terminal" and r.get("input_pane")), None)
    if first is None:
        return []
    windows, current, openings, was_open = [], None, 0, False
    for i, record in enumerate(records):
        if record.get("kind") != "terminal":
            continue
        host, now = record.get("host"), record.get("settings_modal") is True
        if current is not None and current[1] is not None and host not in (None, current[1]):
            windows.append((current[0], i))
            current = None
        if now and not was_open:
            openings += 1
            if current is not None:
                windows.append((current[0], i))
                current = None
        if i == first and openings == 0:
            current = (i, host)
        elif was_open and not now and openings < len(CONTROLS):
            current = (i, host)
        was_open = now
    if current is not None:
        windows.append((current[0], len(records)))
    return windows


def typing_drops(records):
    """Reasons of the `input-dropped` records that can have lost a typed control."""
    segments, windows = launch_segments(records), typing_windows(records)
    reasons = []
    for i, record in enumerate(records):
        if record.get("kind") != "input-dropped":
            continue
        launch_time = any(start <= i < cutoff for start, _, cutoff in segments)
        if not launch_time and any(start <= i < end for start, end in windows):
            reasons.append(record.get("reason"))
    return reasons


def fifo_gate(records, messages):
    """Positive controls must reach the peer before the next phase starts.

    A control typed into the terminal is the only evidence that the later
    "nothing else arrived" observation could have seen a leak. Missing, dropped and
    leaked input fail with different messages.
    """
    dropped = typing_drops(records)
    if dropped:
        raise ValueError("input dropped by the app (" + ",".join(sorted({str(r) for r in dropped})) +
                         "): positive control not delivered")
    text = delivered_text(messages)
    opened = modal_openings(records)
    for needed, control in enumerate(CONTROLS, 1):
        if opened >= needed and control not in text:
            raise ValueError("positive control not delivered before the next phase: " + control)
    return text


def validate_fifo(records, messages):
    text = fifo_gate(records, messages)
    for control in CONTROLS:
        if control not in text:
            raise ValueError("positive control not delivered: " + control)
    if text != "".join(CONTROLS):
        if text == "".join(reversed(CONTROLS)):
            raise ValueError("positive controls arrived out of order")
        raise ValueError("terminal wire leak around the positive controls: " + repr(text))


def watch_fifo(process, records_path, previous, peer_path, peer_previous, grace=10.0, poll=1.0, clock=time.monotonic):
    """Poll the receiving files while XCTest runs and stop at the first missing control."""
    def lines(path, skip):
        if not path.exists():
            return []
        result = []
        for line in path.read_text().splitlines()[skip:]:
            try:
                result.append(json.loads(line))
            except json.JSONDecodeError:
                break  # a line still being written
        return result
    failing_since, problem = None, None
    while process.poll() is None:
        try:
            fifo_gate(lines(records_path, previous), lines(peer_path, peer_previous))
            failing_since = None
        except ValueError as error:
            problem = error
            failing_since = failing_since if failing_since is not None else clock()
            if clock() - failing_since >= grace:
                process.terminate()
                process.wait()
                raise ValueError("stopped XCTest: " + str(error)) from error
        time.sleep(poll)
    return problem


def run_watched(arguments, cwd, env, watch):
    """subprocess.run(capture_output) equivalent that also polls the FIFO gate."""
    print("P15 COMMAND", list(arguments), flush=True)
    process = subprocess.Popen(arguments, cwd=cwd, env=env, text=True, stdout=subprocess.PIPE,
                               stderr=subprocess.STDOUT)
    output = []
    reader = threading.Thread(target=lambda: output.extend(process.stdout), daemon=True)
    reader.start()
    try:
        watch(process)
    finally:
        reader.join(timeout=30)
        print("".join(output), flush=True)
    text = "".join(output)
    if process.returncode != 0:
        raise subprocess.CalledProcessError(process.returncode, arguments, output=text)
    return subprocess.CompletedProcess(arguments, 0, stdout=text, stderr="")


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
    marked = [r.get("count") for r in ui if r.get("kind") == "qa-match" and r.get("id") == APPEARANCE_ID]
    if marked != [1, 1, 1]:
        raise ValueError("the Theme page identifier must be marked exactly once on each press: " + repr(marked))
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
    for message in messages:
        if "ClientShellEndpointRequest" in message:
            request = json.loads(message["ClientShellEndpointRequest"]["request"])
            if request.get("method") != "client_shell.surface.set":
                raise ValueError("unexpected endpoint operation in theme-only fixture")
    validate_fifo(records, messages)


def build_owned(session, directory, source, device, environment, head):
    """Explicit P15 expectations; never infer the SDK identity from build output."""
    import uuid
    expected = dict(schema='cargo-provenance-v2', head=head, run_id=str(uuid.uuid4()),
                    package='yebisu', binary='yebisu', target='aarch64-apple-ios-sim',
                    features=[], no_default_features=False, profile_name='dev',
                    profile=dict(opt_level='0', debuginfo=0, debug_assertions=True,
                                 overflow_checks=True, test=False),
                    sdk=[dict(name='herdr-uikit', url='https://github.com/shin1ohno/herdr-uikit',
                              revision='1fab654acee82bda7cc43f8703826c151c6acc34'),
                         dict(name='herdr-ssh', url='https://github.com/shin1ohno/herdr-gpui',
                              revision='9327f4482faeb0b9ccfb086c912a300a98ebad61')])
    return session.build_install(directory, source, device,
                                 ['--package', 'yebisu', '--bin', 'yebisu'],
                                 environment, expected=expected)


def admit_owned(session, args, environment):
    """Read-only caller admission before evidence creation or native dispatch.

    The session still performs its full contract admission and lock acquisition.
    Like that API, this is not a fence against hostile same-UID races.
    """
    import re
    import subprocess
    import uuid
    if not args.expected_head or not args.simulator:
        raise ValueError("owned source, full expected head and leased simulator UUID required")
    if not re.fullmatch('[0-9a-f]{40}', args.expected_head):
        raise ValueError("full source head required")
    uuid.UUID(args.simulator)  # Never resolve a shared device by its default name.
    # Check the original argument: resolve() would erase a caller's symlink alias.
    own = session.canonical(args.own_root)
    source = session.child(ROOT, own)
    evidence = session.child(own / 'evidence', own, exists=False)
    if source.is_relative_to(evidence) or evidence.is_relative_to(source):
        raise ValueError('evidence overlaps source')
    paths = {own, source}
    paths.update(parent for parent in source.parents if parent.is_relative_to(own))
    if evidence.exists():
        paths.add(evidence)
    if any(not path.is_dir() for path in paths):
        raise ValueError('ownership paths must be directories')
    if any(path.stat().st_uid != os.getuid() for path in paths):
        raise ValueError('ownership uid mismatch')

    def git(*arguments):
        return subprocess.check_output(['git', '-C', str(source), *arguments], text=True,
                                       env=dict(environment, GIT_OPTIONAL_LOCKS='0')).strip()

    if git('rev-parse', '--show-toplevel') != str(source) or git('rev-parse', 'HEAD') != args.expected_head:
        raise ValueError('unexpected source checkout/head')
    if git('status', '--porcelain', '--untracked-files=no'):
        raise ValueError('tracked source is dirty')
    return own, evidence


def main():
    import sys
    # Admission must not create sibling bytecode before checking ownership.
    sys.dont_write_bytecode = True
    spec = importlib.util.spec_from_file_location("p11_runner", Path(__file__).with_name("p11-check.py"))
    runner = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(runner)
    run = runner.run
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--own-root", type=Path)
    parser.add_argument("--expected-head")
    parser.add_argument("--simulator")
    # The watchdog stops XCTest at the first FIFO marker the peer has not received after 10 s
    # (TODO.md: P15-start has not reached the board peer on the Mac mini). --no-watch lets the
    # test run to its end so its other steps are exercised; the oracle below still judges FIFO.
    parser.add_argument("--no-watch", action="store_true")
    args = parser.parse_args()
    environment = os.environ.copy()
    provenance = None
    if args.own_root:
        spec = importlib.util.spec_from_file_location('provenance_session', ROOT / 'scripts/ios/provenance_session.py')
        session = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(session)
        own, evidence = admit_owned(session, args, environment)
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
        tested = run_watched(
            ["xcodebuild", "test", "-project", str(project / "SmokeOracle.xcodeproj"),
             "-scheme", "SmokeOracle", "-destination", "id=" + device,
             "-derivedDataPath", str(derived),
             "-resultBundlePath", str(result), "CODE_SIGNING_ALLOWED=NO",
             "-only-testing:SmokeOracle/SmokeQA/testP15Theme"], ROOT, environment,
            (lambda process: process.wait()) if args.no_watch else
            (lambda process: watch_fifo(process, records_path, previous, peer_path, peer_previous)))
    finally:
        terminate(device)
        # The app is stopped first (UI test redesign S2, S6 for P15), so no file is read while it
        # may still write; even failed XCTest/oracle runs retain receiving files before validation.
        records = [json.loads(line) for line in records_path.read_text().splitlines()[previous:]] if records_path.exists() else []
        disk = json.loads(settings.read_text())
        messages = [json.loads(line) for line in peer_path.read_text().splitlines()[peer_previous:]] if peer_path.exists() else []
        (out / "peer.json").write_text(json.dumps(messages, indent=2))
        (out / "evidence.json").write_text(json.dumps(dict(records=records, disk=disk), indent=2))
    if "testP15Theme]' passed" not in tested.stdout:
        raise ValueError("P15 XCTest did not execute and pass")
    attachments = out / "attachments"
    run("xcrun", "xcresulttool", "export", "attachments", "--path", str(result), "--output-path", str(attachments))
    ui_file = attachment_files(json.loads((attachments / "manifest.json").read_text()), attachments)
    ui = json.loads(ui_file.read_text())
    (out / "ui.json").write_text(json.dumps(ui, indent=2))
    validate(records, disk)
    validate_ui_wire(records, messages, ui)
    print("P15 local-theme Simulator oracle passed; host read and physical gates remain unverified.")


if __name__ == "__main__":
    main()
