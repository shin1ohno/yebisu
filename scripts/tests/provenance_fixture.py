"""Project real fork blobs into a small Git history for offline CI tests."""

import json
from pathlib import Path
import subprocess

FIXTURE = Path(__file__).parent / "fixtures/provenance-history.json"


def git(repo, *args):
    return subprocess.check_output(["git", "-C", str(repo), *args], text=True).strip()


def history(root):
    data = json.loads(FIXTURE.read_text())
    root.mkdir()
    git(root, "init", "-q", "-b", "main")
    commits = {}

    def commit(label, subject):
        git(root, "add", ".")
        tree = git(root, "write-tree")
        args = ["-c", "user.name=Provenance fixture", "-c", "user.email=fixture@example.invalid",
                "commit-tree", tree, "-m", subject]
        if commits:
            args += ["-p", next(reversed(commits.values()))]
        digest = git(root, *args)
        git(root, "update-ref", "refs/heads/main", digest)
        commits[label] = digest

    old = root / data["old_path"]
    new = root / data["new_path"]
    terminal = root / data["terminal_path"]
    old.parent.mkdir(parents=True)
    old.write_text(data["panes_before"])
    terminal.write_text(data["terminal_before"])
    commit("base", "fixture parent " + data["base_commit"])
    new.parent.mkdir(parents=True)
    old.rename(new)
    commit("8c1719924", " ".join(data["commits"]["8c1719924"]))
    new.write_text(data["panes_after"])
    commit("9c32a9270", " ".join(data["commits"]["9c32a9270"]))
    terminal.write_text(data["terminal_after"])
    commit("3a1b61753", " ".join(data["commits"]["3a1b61753"]))
    # Upstream main receives the terminal change through a merge. A
    # first-parent-only walk must fail to enumerate its original commit.
    tree = git(root, "write-tree")
    merged = git(root, "-c", "user.name=Provenance fixture", "-c", "user.email=fixture@example.invalid",
                 "commit-tree", tree, "-p", commits["9c32a9270"], "-p", commits["3a1b61753"],
                 "-m", "fixture upstream main merges terminal links")
    git(root, "update-ref", "refs/heads/main", merged)
    commits["head"] = merged
    return data, commits
