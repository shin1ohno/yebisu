#!/usr/bin/env python3
"""Move every declared Herdr fork dependency to one pinned revision."""

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FORK_URL = "https://github.com/shin1ohno/herdr-gpui"
REVISION = re.compile(r"[a-f0-9]{40}\Z")


def package_blocks(lock_text):
    for block in re.split(r"(?=\[\[package\]\])", lock_text):
        if 'source = "git+' + FORK_URL in block:
            name = re.search(r'^name = "([^"]+)"$', block, re.MULTILINE)
            if name:
                yield name.group(1)


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
    for path, raw in originals.items():
        text = raw.decode("utf-8")
        lines = text.splitlines(keepends=True)
        updated = []
        in_fork_table = False
        for line in lines:
            if line.lstrip().startswith("["):
                in_fork_table = False
            if FORK_URL in line:
                in_fork_table = True
            if in_fork_table and re.search(r"\brev\s*=", line):
                line = re.sub(r'(\brev\s*=\s*")[^"]+("\s*[,}])', r"\g<1>" + revision + r"\g<2>", line)
                in_fork_table = False
            updated.append(line)
        result = "".join(updated).encode("utf-8")
        if result != raw:
            replacements[path] = result

    if not replacements:
        if any(FORK_URL in raw.decode("utf-8") for raw in originals.values()):
            print("No Herdr fork revision fields found; refusing to change an ambiguous manifest.", file=sys.stderr)
            return 1
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
    except (OSError, subprocess.CalledProcessError):
        for path, content in originals.items():
            path.write_bytes(content)
        lock_path.write_bytes(lock_original)
        raise
    print("Updated {} manifest(s) and {} fork lockfile package(s) to {}".format(
        len(replacements), len(names), revision))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
