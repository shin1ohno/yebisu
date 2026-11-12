#!/usr/bin/env python3
"""Opt-in Simulator provenance. Never builds, installs, signs or acquires keys."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
from urllib.parse import unquote, urlparse


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def inside(path, root):
    path, root = Path(path).resolve(), Path(root).resolve()
    if not path.is_relative_to(root):
        raise ValueError(f"path outside owned root: {path}")
    return path


def command(*args):
    return subprocess.check_output(args, text=True).strip()


def uuids(path):
    raw = command("xcrun", "dwarfdump", "--uuid", str(path))
    values = re.findall(r"UUID: ([0-9A-Fa-f-]{36}) \(([^)]+)\)", raw)
    if not values:
        raise ValueError("missing Mach-O UUID")
    return dict(raw=raw, values=sorted([u.upper(), arch] for u, arch in values))


def identity(path):
    path = Path(path).resolve(strict=True)
    return dict(path=str(path), sha256=digest(path), size=path.stat().st_size, uuid=uuids(path))


def same_binary(left, right):
    if left["sha256"] != right["sha256"] or left["uuid"]["values"] != right["uuid"]["values"]:
        raise ValueError("binary hash/UUID mismatch")


def select_artifact(messages, source, target_root):
    candidates = [m for m in messages if m.get("reason") == "compiler-artifact"
                  and m.get("executable") and m.get("target", {}).get("name") == "yebisu"
                  and "bin" in m.get("target", {}).get("kind", [])]
    if len(candidates) != 1:
        raise ValueError("expected one actual yebisu compiler artifact")
    item = candidates[0]
    if Path(item["manifest_path"]).resolve() != source / "crates/yebisu/Cargo.toml":
        raise ValueError("foreign manifest")
    if Path(item["target"]["src_path"]).resolve() != source / "crates/yebisu/src/main.rs":
        raise ValueError("foreign source")
    package_url = item["package_id"].split('#', 1)[0]
    if not package_url.startswith('path+file:') or Path(unquote(urlparse(package_url[5:]).path)).resolve() != source / "crates/yebisu":
        raise ValueError("foreign package identity")
    if item["profile"]["test"]:
        raise ValueError("test artifact cannot be the installed app")
    executable = inside(item["executable"], target_root)
    if "aarch64-apple-ios-sim" not in executable.parts:
        raise ValueError("artifact is not Simulator target")
    return item, executable


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("phase", choices=["init", "artifact", "copy", "signed", "installed"])
    parser.add_argument("--directory", required=True, type=Path)
    parser.add_argument("--source", type=Path)
    parser.add_argument("--executable", type=Path)
    parser.add_argument("--app", type=Path)
    parser.add_argument("--device")
    args = parser.parse_args()
    directory = args.directory.resolve(strict=True)
    receipt = directory / "binding.json"
    if args.phase == "init":
        if receipt.exists():
            raise ValueError("refuse to overwrite a prior build binding")
        root = Path(os.environ["YEBISU_OWN_ROOT"]).resolve(strict=True)
        inside(directory, root)
        source = inside(args.source, root)
        target = inside(os.environ["CARGO_TARGET_DIR"], root)
        if target == source or target == root:
            raise ValueError("target must be a dedicated child directory")
        head = command("git", "-C", str(source), "rev-parse", "HEAD")
        if head != os.environ["YEBISU_EXPECTED_HEAD"]:
            raise ValueError("unexpected source head")
        if command("git", "-C", str(source), "status", "--porcelain", "--untracked-files=no"):
            raise ValueError("tracked source is dirty")
        paths = subprocess.check_output(["git", "-C", str(source), "ls-files", "-z"]).decode().split('\0')
        inputs = {p: digest(source / p) for p in paths if p and (source / p).is_file()}
        data = dict(head=head, own_root=str(root), source=str(source), target=str(target),
                    inputs=inputs, signing="Simulator ad-hoc identity '-' only", phases=[])
    else:
        data = json.loads(receipt.read_text())
        root, source = Path(data["own_root"]), Path(data["source"])
        inside(directory, root)
        if any(digest(source / p) != sha for p, sha in data["inputs"].items()):
            raise ValueError("source changed during bundle/install")
        if args.phase == "artifact":
            raw = directory / "cargo.jsonl"
            messages = [json.loads(line) for line in raw.read_text().splitlines() if line.startswith('{')]
            item, executable = select_artifact(messages, source, Path(data["target"]))
            if executable != args.executable.resolve(strict=True):
                raise ValueError("bundle selected another executable")
            depfile = executable.with_suffix('.d')
            deps = depfile.read_text().replace('\\ ', ' ')
            for relative in ["crates/yebisu/src/main.rs", "crates/yebisu/src/p15_settings_view.rs"]:
                if str(source / relative) not in deps:
                    raise ValueError("depfile lacks this checkout's source")
            (directory / "app.d").write_bytes(depfile.read_bytes())
            data.update(compiler_artifact=item, cargo_json_sha256=digest(raw),
                        depfile=dict(path=str(depfile), sha256=digest(depfile)), compiled=identity(executable))
        elif args.phase == "copy":
            app = inside(args.app, data["target"])
            copied = identity(app / "yebisu")
            same_binary(data["compiled"], copied)
            data.update(app=str(app), copied=copied)
        elif args.phase == "signed":
            app = inside(args.app, data["target"])
            if str(app) != data["app"]:
                raise ValueError("signed another bundle")
            signed = identity(app / "yebisu")
            if data["copied"]["uuid"]["values"] != signed["uuid"]["values"]:
                raise ValueError("signing changed Mach-O UUID")
            data.update(signed=signed, bundle_files={str(p.relative_to(app)): digest(p)
                        for p in app.rglob('*') if p.is_file()})
        else:
            if not args.device:
                raise ValueError("installed binding requires leased device UUID")
            app = args.app.resolve(strict=True)
            installed = identity(app / "yebisu")
            same_binary(data["signed"], installed)
            for name, sha in data["bundle_files"].items():
                if digest(app / name) != sha:
                    raise ValueError("installed bundle content mismatch")
            data.update(installed=installed, device=args.device)
    data["phases"].append(args.phase)
    receipt.write_text(json.dumps(data, indent=2) + '\n')


if __name__ == "__main__":
    main()
