#!/usr/bin/env python3
"""Check copied files and model references have complete provenance."""
import csv
from datetime import date
import re
import subprocess
from pathlib import Path

FIELDS = (
    "kind", "destination", "source_repository", "source_commit",
    "source_path", "modified", "last_triaged", "last_triaged_at",
)
SHA = re.compile(r"[a-f0-9]{40}\Z")
REQUIRED_COPIES = (
    "scripts/check-file-size.sh",
    "scripts/check-sorted.py",
    "scripts/release/check-commit-messages.py",
)
REQUIRED_IOS_COPIES = (
    "scripts/ios/Info.plist",
    "scripts/ios/bundle.sh",
    "scripts/ios/device.sh",
    "scripts/ios/devicectl.py",
    "scripts/ios/lib.sh",
    "scripts/ios/lint.sh",
    "scripts/ios/profile.py",
    "scripts/ios/provision.sh",
    "scripts/ios/screenshot-color.py",
    "scripts/ios/sim-runner.sh",
    "scripts/ios/sim.sh",
    "scripts/ios/test_bundle.py",
    "scripts/ios/test_device.py",
    "scripts/ios/test_lint.py",
    "scripts/ios/test_profile.py",
    "scripts/ios/test_screenshot_color.py",
    "scripts/ios/test_sim_runner.py",
)


def validate(root=Path(".")):
    errors = []
    provenance = root / "PROVENANCE.tsv"
    with provenance.open(encoding="utf-8", newline="") as stream:
        reader = csv.DictReader(stream, delimiter="\t")
        if tuple(reader.fieldnames or ()) != FIELDS:
            return ["PROVENANCE.tsv header must be: " + "\t".join(FIELDS)], []
        rows = list(reader)

    seen = set()
    for number, row in enumerate(rows, 2):
        label = "PROVENANCE.tsv:{}".format(number)
        if row["kind"] not in {"copy", "model"}:
            errors.append("{}: kind must be copy or model".format(label))
        if not SHA.fullmatch(row["source_commit"]):
            errors.append("{}: source_commit must be a full commit hash".format(label))
        if not SHA.fullmatch(row["last_triaged"]):
            errors.append("{}: last_triaged must be a full commit hash".format(label))
        try:
            date.fromisoformat(row["last_triaged_at"])
        except ValueError:
            errors.append("{}: last_triaged_at must be YYYY-MM-DD".format(label))
        if not row["source_repository"].startswith("https://github.com/"):
            errors.append("{}: source_repository must be a GitHub URL".format(label))
        destination = Path(row["destination"])
        if destination.is_absolute() or ".." in destination.parts:
            errors.append("{}: destination must stay inside the repository".format(label))
        elif not (root / destination).is_file():
            errors.append("{}: destination does not exist: {}".format(label, destination))
        if not row["source_path"] or row["source_path"].startswith("/"):
            errors.append("{}: source_path must be repository-relative".format(label))
        if row["modified"] not in {"yes", "no"}:
            errors.append("{}: modified must be yes or no".format(label))
        key = (row["kind"], row["destination"], row["source_repository"], row["source_path"])
        if key in seen:
            errors.append("{}: duplicate provenance row".format(label))
        seen.add(key)

    tracked = subprocess.check_output(
        ["git", "-C", str(root.resolve()), "ls-files"], text=True
    ).splitlines()
    copied = {row["destination"] for row in rows if row["kind"] == "copy"}
    for path in tracked:
        if path.startswith(("assets/fonts/", "upstream/")):
            if path not in copied:
                errors.append("unregistered copy: {}".format(path))
    for path in REQUIRED_COPIES + REQUIRED_IOS_COPIES:
        if path in tracked and path not in copied:
            errors.append("unregistered copy: {}".format(path))

    kinds = {row["kind"] for row in rows}
    if "copy" not in kinds:
        errors.append("PROVENANCE.tsv must contain at least one copy row")
    if "model" not in kinds:
        errors.append("PROVENANCE.tsv must contain at least one model row")
    if not any(row["destination"] == "scripts/ios/profile.py"
               and row["kind"] == "copy" and row["modified"] == "yes" for row in rows):
        errors.append("scripts/ios/profile.py must be recorded as a modified copy")
    return errors, rows


def main():
    errors, rows = validate()
    for error in errors:
        print("FAIL: " + error)
    if errors:
        return 1
    today = date.today()
    stale = [row for row in rows
             if (today - date.fromisoformat(row["last_triaged_at"])).days > 14]
    for row in stale:
        print("WARN: {} last triaged {} days ago".format(
            row["destination"],
            (today - date.fromisoformat(row["last_triaged_at"])).days))
    print("Provenance: {} copy/model rows checked".format(len(rows)))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
