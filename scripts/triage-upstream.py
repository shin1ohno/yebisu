#!/usr/bin/env python3
"""List source changes for provenance rows, following full-diff renames."""

import csv
import os
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIELDS = (
    "kind", "destination", "source_repository", "source_commit",
    "source_path", "modified", "last_triaged", "last_triaged_at",
)


def git(repo, *args):
    return subprocess.check_output(["git", "-C", str(repo), *args])


def parse_name_status(data):
    """Parse `git diff-tree --name-status -z`, retaining rename pairs."""
    fields = data.decode("utf-8", "replace").split("\0")
    changes = []
    index = 0
    while index < len(fields) and fields[index]:
        status = fields[index]
        index += 1
        if status.startswith(("R", "C")):
            if index + 1 >= len(fields):
                raise ValueError("truncated rename/copy record")
            changes.append((status, fields[index], fields[index + 1]))
            index += 2
        else:
            if index >= len(fields):
                raise ValueError("truncated path record")
            changes.append((status, fields[index], None))
            index += 1
    return changes


def advance_path(path, changes):
    """Return the path after one commit and whether that file was touched."""
    touched = False
    deleted = False
    for status, old_path, new_path in changes:
        if status.startswith(("R", "C")) and old_path == path:
            path = new_path
            touched = True
            deleted = False
        elif old_path == path:
            touched = True
            deleted = status == "D"
    return path, touched, deleted


def commits_between(repo, base, head):
    return git(repo, "rev-list", "--first-parent", "--reverse", base + ".." + head).decode().splitlines()


def inspect_row(repo, row, head):
    path = row["source_path"]
    base = row["last_triaged"]
    events = []
    for commit in commits_between(repo, base, head):
        parents = git(repo, "rev-list", "--parents", "-n", "1", commit).decode().split()
        parent = parents[1] if len(parents) > 1 else "--root"
        if parent == "--root":
            diff = git(repo, "diff-tree", "--root", "--no-commit-id", "-r", "-M", "--name-status", "-z", commit)
        else:
            diff = git(repo, "diff-tree", "--no-commit-id", "-r", "-M", "--name-status", "-z", parent, commit)
        before = path
        path, touched, deleted = advance_path(path, parse_name_status(diff))
        if touched:
            subject = git(repo, "show", "-s", "--format=%s", commit).decode().strip()
            events.append((commit, subject, before, path, deleted))
    return path, events


def guarded(*command):
    subprocess.run([sys.executable, str(ROOT / "scripts/guarded.py"), *command], cwd=ROOT, check=True)


def source_checkout(url):
    configured = os.environ.get("HERDR_GPUI_REPO")
    if configured:
        return Path(configured).expanduser().resolve()
    checkout = ROOT / "target" / "provenance-herdr-gpui"
    if not (checkout / ".git").exists():
        guarded("git", "clone", "--filter=blob:none", "--no-checkout", "--single-branch", "--branch", "main", url, str(checkout))
    else:
        guarded("git", "-C", str(checkout), "fetch", "--filter=blob:none", "origin", "main")
    return checkout


def write_rows(rows):
    with (ROOT / "PROVENANCE.tsv").open("w", encoding="utf-8", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=FIELDS, delimiter="\t", lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def main():
    provenance = ROOT / "PROVENANCE.tsv"
    with provenance.open(encoding="utf-8", newline="") as stream:
        rows = list(csv.DictReader(stream, delimiter="\t"))
    by_url = {}
    for row in rows:
        by_url.setdefault(row["source_repository"], []).append(row)

    candidate_sections = []
    for url, source_rows in by_url.items():
        repo = source_checkout(url)
        head = git(repo, "rev-parse", "refs/remotes/origin/main").decode().strip()
        changed = {}
        for row in source_rows:
            new_path, events = inspect_row(repo, row, head)
            row["source_path"] = new_path
            row["last_triaged"] = head
            row["last_triaged_at"] = datetime.now(timezone.utc).date().isoformat()
            for commit, subject, before, after, deleted in events:
                entry = changed.setdefault(commit, {"subject": subject, "paths": set()})
                marker = " (deleted)" if deleted else ""
                entry["paths"].add("{}: {}{}".format(row["destination"], after, marker))
        if changed:
            candidate_sections.append((url, head, changed))

    if candidate_sections:
        with (ROOT / "docs/upstream-log.md").open("a", encoding="utf-8") as stream:
            for url, head, changed in candidate_sections:
                stamp = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M UTC")
                stream.write("\n## Scan {} — {}\n\nSource: {} at `{}`.\n\n".format(stamp, url, url, head))
                stream.write("Review each change and record `apply` or `skip` with a reason.\n\n")
                for commit in sorted(changed):
                    entry = changed[commit]
                    stream.write("- [ ] `{}` — {}\n".format(commit, entry["subject"]))
                    for path in sorted(entry["paths"]):
                        stream.write("  - {}\n".format(path))
                    stream.write("  - decision: pending review; reason: pending review\n")
        write_rows(rows)
        print("Wrote {} pending commit reviews to docs/upstream-log.md".format(
            sum(len(section[2]) for section in candidate_sections)))
    else:
        write_rows(rows)
        print("No provenance source changes since the last triage.")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, subprocess.CalledProcessError, ValueError) as error:
        print("triage-upstream: {}".format(error), file=sys.stderr)
        raise SystemExit(1)
