#!/usr/bin/env python3
"""Opt-in Simulator provenance. Never builds, installs, signs or acquires keys."""
import argparse
import hashlib
import importlib.util
import shutil
import json
import os
from pathlib import Path
import re
import subprocess
from urllib.parse import unquote, urlparse


def sibling(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + '.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


macho = sibling('macho_identity')
compiler = sibling('compiler_inputs')
session = sibling('provenance_session')


def bundle_files(app):
    paths = list(app.rglob('*'))
    if any(p.is_symlink() or not (p.is_file() or p.is_dir()) for p in paths):
        raise ValueError('bundle symlink/special file rejected')
    return {str(p.relative_to(app)): digest(p) for p in paths if p.is_file()}


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
    return dict(path=str(path), sha256=digest(path), size=path.stat().st_size, uuid=uuids(path), code=macho.code_identity(path.read_bytes()))


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
    parser.add_argument("phase", choices=["init", "metadata", "artifact", "copy", "signed", "preinstall", "installed"])
    parser.add_argument("--directory", required=True, type=Path)
    parser.add_argument("--source", type=Path)
    parser.add_argument("--executable", type=Path)
    parser.add_argument("--app", type=Path)
    parser.add_argument("--device")
    args = parser.parse_args()
    directory = args.directory.resolve(strict=True)
    receipt = directory / "binding.json"
    session.require_owner(directory)
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
        phases = ['init', 'metadata', 'artifact', 'copy', 'signed', 'preinstall', 'installed']
        if data['phases'] != phases[:phases.index(args.phase)]:
            raise ValueError('invalid provenance phase order')
        compiler.unchanged(data.get('packages', {}), data.get('compiler_inputs', []))
        inside(directory, root)
        if any(digest(source / p) != sha for p, sha in data["inputs"].items()):
            raise ValueError("source changed during bundle/install")
        if args.phase == 'metadata':
            raw = directory / 'metadata.json'
            metadata = json.loads(raw.read_text())
            data.update(metadata_sha256=digest(raw), packages=compiler.metadata_snapshot(metadata, source))
        elif args.phase == "artifact":
            raw = directory / "cargo.jsonl"
            messages = [json.loads(line) for line in raw.read_text().splitlines() if line.startswith('{')]
            item, executable = select_artifact(messages, source, Path(data["target"]))
            if executable != args.executable.resolve(strict=True):
                raise ValueError("bundle selected another executable")
            metadata = json.loads((directory / 'metadata.json').read_text())
            if digest(directory / 'metadata.json') != data['metadata_sha256']:
                raise ValueError('metadata changed after snapshot')
            records = compiler.validate_artifacts(messages, metadata, data['packages'], source,
                                                   Path(data['target']), directory)
            compiled = identity(executable)
            shutil.copyfile(executable, directory / 'unsigned.macho')
            same_binary(compiled, identity(directory / 'unsigned.macho'))
            data.update(compiler_artifact=item, cargo_json_sha256=digest(raw),
                        compiler_inputs=records, compiled=compiled)
        elif args.phase == "copy":
            app = inside(args.app, directory)
            copied = identity(app / "yebisu")
            same_binary(data["compiled"], copied)
            data.update(app=str(app), copied=copied)
        elif args.phase == "signed":
            app = inside(args.app, directory)
            if str(app) != data["app"]:
                raise ValueError("signed another bundle")
            signed = identity(app / "yebisu")
            same_binary(data['compiled'], identity(directory / 'unsigned.macho'))
            macho.same_code(data['compiled']['code'], signed['code'])
            shutil.copyfile(app / 'yebisu', directory / 'signed.macho')
            same_binary(signed, identity(directory / 'signed.macho'))
            data.update(signed=signed, bundle_files=bundle_files(app))
        elif args.phase == 'preinstall':
            app = inside(args.app, directory)
            if str(app) != data['app']:
                raise ValueError('install requested another bundle')
            same_binary(data['signed'], identity(app / 'yebisu'))
            if bundle_files(app) != data['bundle_files']:
                raise ValueError('bundle changed before install')
        else:
            if not args.device:
                raise ValueError("installed binding requires leased device UUID")
            app = args.app.resolve(strict=True)
            installed = identity(app / "yebisu")
            same_binary(data["signed"], installed)
            owner = json.loads((directory / 'owner.json').read_text())
            if args.device != owner['device']:
                raise ValueError('installed device differs from lease')
            same_binary(data['signed'], identity(Path(data['app']) / 'yebisu'))
            if bundle_files(app) != data['bundle_files'] or bundle_files(Path(data['app'])) != data['bundle_files']:
                raise ValueError('installed bundle content mismatch')
            shutil.copyfile(app / 'yebisu', directory / 'installed.macho')
            same_binary(installed, identity(directory / 'installed.macho'))
            data.update(installed=installed, device=args.device)
    data["phases"].append(args.phase)
    pending = directory / 'binding.pending'
    pending.write_text(json.dumps(data, indent=2) + '\n')
    pending.replace(receipt)


if __name__ == "__main__":
    main()
