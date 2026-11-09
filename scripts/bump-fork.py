#!/usr/bin/env python3
"""Move every declared Herdr fork dependency to one pinned revision."""

import re
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from fork_manifest import FORK_URL, fork_dependencies, update_manifest

ROOT = Path(__file__).resolve().parents[1]
REVISION = re.compile(r"[a-f0-9]{40}\Z")


def package_blocks(lock_text):
    for block in re.split(r"(?=\[\[package\]\])", lock_text):
        if 'source = "git+' + FORK_URL in block:
            name = re.search(r'^name = "([^"]+)"$', block, re.MULTILINE)
            if name:
                yield name.group(1)


def verify_lock(text, revision):
    for block in re.split(r"(?=\[\[package\]\])", text):
        if 'source = "git+' + FORK_URL in block:
            source = re.search(r'^source = "([^"]+)"$', block, re.MULTILINE)
            if not source or not source.group(1).endswith("#" + revision):
                raise ValueError("fork lockfile revision mismatch")


def guarded(*args):
    subprocess.run([sys.executable, str(ROOT / "scripts/guarded.py"), *args], cwd=ROOT, check=True)


def main(argv):
    if len(argv) != 2 or not REVISION.fullmatch(argv[1]):
        print("usage: bump-fork.py FULL_40_CHARACTER_REVISION", file=sys.stderr)
        return 2
    revision = argv[1]
    manifests = [ROOT / "Cargo.toml"] + sorted((ROOT / "crates").glob("*/Cargo.toml"))
    originals = {path: path.read_bytes() for path in manifests if path.is_file()}
    replacements = {}
    dependency_count = 0
    for path, raw in originals.items():
        updated, count = update_manifest(raw.decode("utf-8"), revision)
        dependency_count += count
        if updated.encode("utf-8") != raw:
            replacements[path] = updated.encode("utf-8")

    if not dependency_count:
        print("No Herdr fork dependencies are pinned in this phase; nothing to bump.")
        return 0

    lock_path = ROOT / "Cargo.lock"
    lock_original = lock_path.read_bytes()
    names = sorted(set(package_blocks(lock_original.decode("utf-8"))))
    try:
        for path, content in replacements.items():
            path.write_bytes(content)
        for name in names:
            guarded("cargo", "update", "-p", name, "--precise", revision)
        guarded("cargo", "metadata", "--locked", "--format-version", "1")
        for path in originals:
            if any(dep.get("rev") != revision for dep in fork_dependencies(path.read_text())):
                raise ValueError("fork dependency revision mismatch after Cargo update")
        verify_lock(lock_path.read_text(), revision)
    except (OSError, subprocess.CalledProcessError, ValueError):
        for path, content in originals.items():
            path.write_bytes(content)
        lock_path.write_bytes(lock_original)
        raise
    print("Updated {} manifest(s) and {} fork lockfile package(s) to {}".format(
        len(replacements), len(names), revision))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
