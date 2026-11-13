"""Bind Cargo's actual artifact messages and depfiles to locked resolved sources."""
import hashlib
import json
from pathlib import Path
import shlex
import subprocess
import tomllib


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def git(root, *args):
    return subprocess.check_output(['git', '-C', str(root), *args])


def dependencies(path):
    # rustc emits one primary make rule followed by empty source rules/env comments.
    text = Path(path).read_text().replace('\\\n', '')
    first = next((line for line in text.splitlines() if line and not line.startswith('#')), '')
    if ': ' not in first:
        raise ValueError('invalid depfile primary rule')
    names = shlex.split(first.split(': ', 1)[1], posix=True)
    if not names or any(not Path(p).is_absolute() for p in names):
        raise ValueError('requires nonempty absolute compiler inputs')
    return sorted(set(names))


def metadata_snapshot(metadata, source):
    source = Path(source).resolve()
    if Path(metadata['workspace_root']).resolve() != source or not metadata.get('resolve'):
        raise ValueError('foreign/unresolved metadata')
    lock = tomllib.loads((source / 'Cargo.lock').read_text())
    locked = {(p['name'], p['version'], p.get('source')) for p in lock['package']}
    nodes = {n['id']: n for n in metadata['resolve']['nodes']}
    packages = {}
    for package in metadata['packages']:
        if package['id'] not in nodes:
            continue
        manifest = Path(package['manifest_path']).resolve(strict=True)
        origin = package.get('source')
        if origin and (package['name'], package['version'], origin) not in locked:
            raise ValueError('metadata dependency differs from Cargo.lock')
        entry = dict(manifest=str(manifest), manifest_sha256=sha(manifest),
                     source=origin, features=nodes[package['id']]['features'],
                     targets=package['targets'])
        if origin and origin.startswith('git+'):
            expected = origin.rsplit('#', 1)[-1]
            top = Path(git(manifest.parent, 'rev-parse', '--show-toplevel').decode().strip())
            if git(top, 'rev-parse', 'HEAD').decode().strip() != expected:
                raise ValueError('SDK checkout revision mismatch')
            files = git(top, 'ls-files', '-z').decode().split('\0')
            blobs = {}
            for name in filter(None, files):
                file = top / name
                original = git(top, 'show', expected + ':' + name)
                if file.is_symlink() or file.read_bytes() != original:
                    raise ValueError('SDK checkout blob mismatch: ' + name)
                blobs[str(file)] = hashlib.sha256(original).hexdigest()
            entry.update(git_root=str(top), revision=expected, blobs=blobs)
        packages[package['id']] = entry
    if not any(p.get('source', '').startswith('git+') for p in packages.values() if p.get('source')):
        raise ValueError('missing pinned SDK bindings')
    return packages


def validate_artifacts(messages, metadata, packages, source, target, directory):
    directory, source, target = Path(directory), Path(source), Path(target)
    seen, records = set(), []
    for item in messages:
        if item.get('reason') != 'compiler-artifact':
            continue
        package = packages.get(item['package_id'])
        if package is None:
            raise ValueError('artifact package missing from resolved metadata')
        manifest = Path(item['manifest_path']).resolve(strict=True)
        if str(manifest) != package['manifest'] or sha(manifest) != package['manifest_sha256']:
            raise ValueError('artifact manifest mismatch')
        declared = package['targets']
        if not any(item['target']['name'] == t['name'] and item['target']['kind'] == t['kind']
                   and Path(item['target']['src_path']).resolve() == Path(t['src_path']).resolve()
                   for t in declared):
            raise ValueError('artifact source differs from metadata')
        if sorted(item['features']) != sorted(package['features']):
            raise ValueError('artifact features differ from resolved metadata')
        if not isinstance(item.get('profile'), dict):
            raise ValueError('missing compiler profile')
        # Require rebuilt workspace/SDK objects; old shared-target cache is not proof.
        if (not package['source'] or package['source'].startswith('git+')) and item.get('fresh') is not False:
            raise ValueError('workspace/SDK compiler artifact must be freshly built')
        candidates = set()
        for filename in item['filenames']:
            artifact = Path(filename).resolve(strict=True)
            if not artifact.is_relative_to(target.resolve()):
                raise ValueError('artifact outside owned target')
            stem = artifact.stem
            for base in (stem, stem.removeprefix('lib')):
                dep = artifact.with_name(base + '.d')
                if dep.is_file():
                    candidates.add(dep)
        if item.get('executable'):
            dep = Path(item['executable']).with_suffix('.d')
            if dep.is_file():
                candidates.add(dep)
        if not candidates:
            raise ValueError('missing actual compiler depfile')
        all_inputs = {}
        for dep in sorted(candidates):
            names = dependencies(dep)
            hashes = {name: sha(name) for name in names}
            src = str(Path(item['target']['src_path']).resolve())
            if src not in hashes:
                raise ValueError('depfile lacks actual target source')
            if package.get('blobs'):
                for name, digest in hashes.items():
                    if Path(name).is_relative_to(package['git_root']) and package['blobs'].get(name) != digest:
                        raise ValueError('SDK depfile input blob mismatch')
            all_inputs.update(hashes)
            saved = directory / ('depfile-' + str(len(records)) + '-' + str(len(all_inputs)) + '.d')
            saved.write_bytes(dep.read_bytes())
        records.append(dict(artifact=item, depfiles=[str(p) for p in sorted(candidates)], inputs=all_inputs))
        seen.add(item['package_id'])
    required = {key for key, p in packages.items() if p.get('source') and p['source'].startswith('git+')}
    if not required.issubset(seen):
        raise ValueError('resolved SDK package lacks actual compiler artifact')
    if not any(r['artifact'].get('executable') for r in records):
        raise ValueError('missing executable compiler input binding')
    return records


def unchanged(packages, records):
    for p in packages.values():
        if sha(p['manifest']) != p['manifest_sha256']:
            raise ValueError('manifest changed during build/install')
        for path, digest in p.get('blobs', {}).items():
            if sha(path) != digest:
                raise ValueError('SDK source changed during build/install')
    for r in records:
        for path, digest in r['inputs'].items():
            if sha(path) != digest:
                raise ValueError('compiler input changed during build/install')
