#!/usr/bin/env python3
"""Cargo-trusted provenance, including the actual rustc adapter invocation.

Registry cache, build-script execution/rerun policy, proc-macro behavior, sysroot,
linker and native system libraries remain Cargo/toolchain trust, not hermetic proof.
"""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tomllib
import uuid
from urllib.parse import urlsplit

sys.dont_write_bytecode = True
MAX_DEPFILE = 16 * 1024 * 1024


def sha(path):
    path = Path(path)
    if path.is_symlink() or not path.is_file():
        raise ValueError('expected regular compiler input/output: ' + str(path))
    return hashlib.sha256(path.read_bytes()).hexdigest()


def git(root, *args):
    return subprocess.check_output(['git', '-C', str(root), *args])


def sibling(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + '.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def make_words(line):
    """Restricted make tokens, not shell quoting. Reject expansions and recipes."""
    result, word, index = [], '', 0
    while index < len(line):
        c = line[index]
        if c == '\\':
            index += 1
            if index == len(line):
                raise ValueError('dangling make escape')
            if line[index] in ' \t#:$\\':
                word += line[index]
            else:
                word += '\\' + line[index]
        elif c == '$':
            if line[index:index + 2] != '$$':
                raise ValueError('make expansion unsupported')
            word += '$'; index += 1
        elif c == '#':
            break
        elif c == ':':
            if word:
                result.append(word); word = ''
            result.append(None)
        elif c.isspace():
            if word:
                result.append(word); word = ''
        elif c in ';|':
            raise ValueError('make recipe/order-only dependency unsupported')
        else:
            word += c
        index += 1
    if word:
        result.append(word)
    return result


def dep_rules(text, cwd):
    if len(text.encode()) > MAX_DEPFILE or '\0' in text:
        raise ValueError('depfile size/NUL limit')
    cwd = Path(cwd)
    if not cwd.is_absolute():
        raise ValueError('compiler cwd must be absolute')
    # Make joins a continuation with whitespace, never concatenates two names.
    text = re.sub(r'\\\r?\n[ \t]*', ' ', text)
    result = []
    def absolute(name):
        p = Path(name)
        return str((p if p.is_absolute() else cwd / p).resolve())
    for line in text.splitlines():
        if not line.strip() or line.lstrip().startswith('#'):
            continue
        words = make_words(line)
        if words.count(None) != 1 or words[0] is None:
            raise ValueError('invalid make rule outputs')
        split = words.index(None)
        result.append(dict(outputs=[absolute(n) for n in words[:split]],
                           inputs=[absolute(n) for n in words[split + 1:]]))
    if not result or not any(r['inputs'] for r in result):
        raise ValueError('missing nonempty compiler dependency rule')
    return result


def dependencies(path, cwd):
    return sorted({p for rule in dep_rules(Path(path).read_text(), cwd) for p in rule['inputs']})


def match_depfile(text, cwd, outputs, source, depfile):
    rules = dep_rules(text, cwd)
    bound = {out: set(r['inputs']) for r in rules if r['inputs'] for out in r['outputs']}
    wanted = set(map(str, outputs))
    if not wanted or not wanted.issubset(bound) or str(depfile) not in bound:
        raise ValueError('depfile outputs do not bind actual rustc outputs/dep-info')
    inputs = bound[next(iter(wanted))]
    if any(bound[p] != inputs for p in wanted | {str(depfile)}):
        raise ValueError('inconsistent compiler input rules')
    if str(source) not in inputs:
        raise ValueError('depfile lacks actual target source')
    return sorted(inputs), sorted(bound)


def metadata_snapshot(metadata, source, expected):
    source = Path(source).resolve()
    if Path(metadata['workspace_root']).resolve() != source or not metadata.get('resolve'):
        raise ValueError('foreign/unresolved metadata')
    lock = tomllib.loads((source / 'Cargo.lock').read_text())
    locked = {(p['name'], p['version'], p.get('source')) for p in lock['package']}
    nodes = {n['id']: n for n in metadata['resolve']['nodes']}
    all_packages = {p['id']: p for p in metadata['packages']}
    if len(nodes) != len(metadata['resolve']['nodes']) or not set(nodes).issubset(all_packages):
        raise ValueError('incomplete resolved metadata graph')
    for sdk in expected['sdk']:
        matches = [p for p in all_packages.values() if p['name'] == sdk['name'] and p['id'] in nodes]
        if len(matches) != 1:
            raise ValueError('expected SDK package missing/ambiguous')
        origin = matches[0].get('source') or ''
        if (not origin.startswith('git+') or origin.split('?', 1)[0].split('#', 1)[0][4:] != sdk['url'] or
                origin.rsplit('#', 1)[-1] != sdk['revision']):
            raise ValueError('expected SDK package/revision mismatch')
    packages, checkouts = {}, {}
    for package_id, node in nodes.items():
        package = all_packages[package_id]
        manifest = Path(package['manifest_path']).resolve(strict=True)
        origin = package.get('source')
        if origin and (package['name'], package['version'], origin) not in locked:
            raise ValueError('metadata dependency differs from Cargo.lock')
        deps = node.get('deps')
        if not isinstance(deps, list) or any(d['pkg'] not in nodes for d in deps):
            raise ValueError('incomplete resolved dependency edges')
        entry = dict(name=package['name'], manifest=str(manifest), manifest_sha256=sha(manifest),
                     source=origin, features=node['features'], targets=package['targets'], deps=deps)
        if origin and origin.startswith('git+'):
            revision = origin.rsplit('#', 1)[-1]
            top = Path(git(manifest.parent, 'rev-parse', '--show-toplevel').decode().strip())
            if git(top, 'rev-parse', 'HEAD').decode().strip() != revision:
                raise ValueError('SDK checkout revision mismatch')
            key = (str(top), revision)
            if key not in checkouts:
                blobs = {}
                for name in filter(None, git(top, 'ls-files', '-z').decode().split('\0')):
                    file = top / name
                    original = git(top, 'show', revision + ':' + name)
                    if file.is_symlink() or not file.is_file() or file.read_bytes() != original:
                        raise ValueError('SDK checkout blob mismatch: ' + name)
                    blobs[str(file)] = hashlib.sha256(original).hexdigest()
                checkouts[key] = blobs
            entry.update(git_root=str(top), revision=revision, blobs=checkouts[key])
        elif not origin and not manifest.is_relative_to(source):
            raise ValueError('external path dependency unsupported')
        elif origin and not origin.startswith('registry+'):
            raise ValueError('unsupported dependency source')
        packages[package_id] = entry
    return packages


def values(args, flag):
    result = []
    for i, arg in enumerate(args):
        if arg == flag:
            if i + 1 == len(args):
                raise ValueError('missing rustc option value')
            result.append(args[i + 1])
        elif arg.startswith(flag + '='):
            result.append(arg[len(flag) + 1:])
        elif flag == '-C' and arg.startswith('-C') and len(arg) > 2:
            result.append(arg[2:])
    return result


def one(args, flag, default=None):
    found = values(args, flag)
    if len(found) > 1:
        raise ValueError('ambiguous rustc option: ' + flag)
    return found[0] if found else default


def rust_profile(args):
    cg = {}
    for value in values(args, '-C'):
        key, _, val = value.partition('=')
        if key in cg:
            raise ValueError('duplicate rustc codegen option unsupported')
        cg[key] = val
    opt = cg.get('opt-level', '3' if '-O' in args else '0')
    def boolean(key, default):
        if key not in cg:
            return default
        if cg[key] not in ('yes', 'no', 'on', 'off', 'true', 'false', '1', '0'):
            raise ValueError('unsupported rustc boolean')
        return cg[key] in ('yes', 'on', 'true', '1')
    debug = boolean('debug-assertions', opt == '0')
    info = cg.get('debuginfo', '2' if '-g' in args else '0')
    info = {'none': 0, 'limited': 1, 'full': 2, '0': 0, '1': 1, '2': 2}.get(info)
    if info is None:
        raise ValueError('unsupported rustc debuginfo')
    return dict(opt_level=opt, debuginfo=info, debug_assertions=debug,
                overflow_checks=boolean('overflow-checks', debug), test='--test' in args)


def tree_files(root):
    root = Path(root)
    if not root.is_dir() or root.is_symlink():
        raise ValueError('invalid generated output directory')
    result, total = {}, 0
    for path in root.rglob('*'):
        if path.is_symlink() or not (path.is_file() or path.is_dir()):
            raise ValueError('generated symlink/special file unsupported')
        if path.is_file():
            total += path.stat().st_size
            if len(result) >= 100000 or total > 512 * 1024 * 1024:
                raise ValueError('generated output inventory limit')
            result[str(path)] = sha(path)
    return result


def rustc_adapter(argv):
    """Invoked by Cargo as RUSTC_WRAPPER; the first argument is the real rustc."""
    rustc, args = argv[0], argv[1:]
    if any(a in ('-vV', '-V', '--version') or a.startswith('--print') for a in args):
        return subprocess.run(argv).returncode
    s = sibling('provenance_session')
    directory = s.canonical(os.environ['YEBISU_BUNDLE_PROVENANCE_DIR'])
    owner = s.require_owner(directory)
    target = Path(owner['target']); cwd = Path.cwd().resolve()
    binding = json.loads((directory / 'binding.json').read_text())
    if binding['phases'] != ['init', 'metadata']:
        raise ValueError('compiler invocation outside metadata/build phase')
    if not any('dep-info' in v.split(',') for v in values(args, '--emit')):
        raise ValueError('compiler invocation lacks dep-info')
    outdir = s.child(one(args, '--out-dir'), target)
    sources = [Path(a) if Path(a).is_absolute() else cwd / a for a in args if a.endswith('.rs')]
    if len(sources) != 1:
        raise ValueError('requires one actual rustc source')
    source = sources[0].resolve(strict=True)
    manifest = Path(os.environ['CARGO_MANIFEST_DIR']) / 'Cargo.toml'
    packages = [p for p in binding['packages'].values() if p['manifest'] == str(manifest)]
    if len(packages) != 1 or not any(Path(t['src_path']).resolve() == source and
            t['name'].replace('-', '_') == one(args, '--crate-name') and
            sorted(t['crate_types']) == sorted(one(args, '--crate-type', '').split(','))
            for t in packages[0]['targets']):
        raise ValueError('rustc source/manifest absent from metadata')
    # Ask this same compiler for output filenames; do not guess adjacent .d stems.
    query = [rustc, *args, '--print=file-names']
    reported = subprocess.run(query, text=True, stdout=subprocess.PIPE, check=True).stdout
    names = reported.splitlines()
    if not names or any(not n or Path(n).name != n for n in names):
        raise ValueError('unsupported rustc output-name response')
    outputs = [s.child(outdir / n, target, exists=False) for n in names]
    if one(args, '-o') is not None:
        raise ValueError('explicit rustc -o unsupported')
    before = {str(p): (p.stat().st_ino, p.stat().st_mtime_ns, p.stat().st_size)
              for p in outdir.glob('*.d') if p.is_file() and not p.is_symlink()}
    externs = {}
    for value in values(args, '--extern'):
        name, sep, path = value.partition('=')
        if not sep:
            raise ValueError('unlocated rustc extern unsupported')
        path = s.child(path, target)
        externs[name] = dict(path=str(path), sha256=sha(path))
    generated_root = os.environ.get('OUT_DIR')
    generated = tree_files(s.child(generated_root, target)) if generated_root else {}
    known = {str(Path(binding['source']) / name): digest for name, digest in binding['inputs'].items()}
    for package in binding['packages'].values():
        known.update(package.get('blobs', {}))
    known.update(generated)
    invocation = dict(argv=argv, cwd=str(cwd), manifest=str(manifest), source=str(source),
                      target=one(args, '--target'), profile=rust_profile(args),
                      features=sorted(json.loads(c[len('feature='):]) for c in values(args, '--cfg')
                                      if c.startswith('feature=')),
                      output_query=dict(argv=query, stdout=reported), run_id=owner['expected']['run_id'],
                      environment_sha256={k: hashlib.sha256(v.encode()).hexdigest() for k, v in os.environ.items()},
                      generated_root=generated_root, generated=generated, externs=externs,
                      compiler_path=str(Path(rustc).resolve()), compiler_sha256=sha(Path(rustc).resolve()))
    ident = uuid.uuid4().hex
    unit = directory / 'units' / (ident + '.json')
    unit.write_text(json.dumps(dict(invocation, state='started'), indent=2) + '\n')
    result = subprocess.run(argv)
    if result.returncode:
        unit.write_text(json.dumps(dict(invocation, state='failed', returncode=result.returncode), indent=2) + '\n')
        return result.returncode
    candidates = []
    for path in outdir.glob('*.d'):
        if path.is_symlink() or not path.is_file():
            continue
        stat = path.stat()
        if before.get(str(path)) == (stat.st_ino, stat.st_mtime_ns, stat.st_size):
            continue
        raw = path.read_text()
        rules = dep_rules(raw, cwd)
        declared_outputs = {o for r in rules if r['inputs'] for o in r['outputs']}
        if set(map(str, outputs)).issubset(declared_outputs):
            names_in, all_outputs = match_depfile(raw, cwd, outputs, source, path)
            candidates.append((path, raw, names_in, all_outputs))
    if len(candidates) != 1:
        raise ValueError('missing/ambiguous freshly emitted depfile for actual rustc outputs')
    depfile, raw, inputs, declared_outputs = candidates[0]
    hashes = {name: sha(name) for name in inputs}
    registry = (packages[0]['source'] or '').startswith('registry+')
    registry_root = manifest.parent
    for name, digest in hashes.items():
        if known.get(name) == digest:
            continue
        if registry and Path(name).is_relative_to(registry_root):
            continue  # Explicit Cargo registry-source trust; not independent preimage proof.
        raise ValueError('compiler input changed or lacks pre-build snapshot: ' + name)
    for ext in externs.values():
        if sha(ext['path']) != ext['sha256']:
            raise ValueError('compiler extern changed during invocation')
    if generated_root and tree_files(generated_root) != generated:
        raise ValueError('generated input changed during invocation')
    actual_outputs = {}
    for path in declared_outputs:
        if path == str(depfile):
            continue
        s.child(path, target)
        actual_outputs[path] = sha(path)
    saved = directory / 'units' / (ident + '.d')
    saved.write_text(raw)
    invocation.update(state='complete', depfile=str(depfile), depfile_snapshot=str(saved),
                      depfile_sha256=sha(saved), inputs=hashes, outputs=actual_outputs,
                      reported_outputs=list(map(str, outputs)), depfile_fresh=True,
                      registry_source_trust=registry)
    unit.write_text(json.dumps(invocation, indent=2) + '\n')
    return 0


def validate_artifacts(messages, metadata, packages, source, target, directory, expected):
    source, target, directory = Path(source), Path(target), Path(directory)
    if messages[-1:] != [dict(reason='build-finished', success=True)]:
        raise ValueError('missing successful actual Cargo build-finished')
    units = [json.loads(p.read_text()) for p in sorted((directory / 'units').glob('*.json'))]
    if any(u.get('state') != 'complete' or u['run_id'] != expected['run_id'] for u in units):
        raise ValueError('incomplete/foreign compiler invocation')
    used, seen, records = set(), set(), []
    for item in messages:
        if item.get('reason') != 'compiler-artifact':
            continue
        package = packages.get(item['package_id'])
        if package is None:
            raise ValueError('artifact package missing from resolved metadata')
        manifest = Path(item['manifest_path']).resolve(strict=True)
        if str(manifest) != package['manifest'] or sha(manifest) != package['manifest_sha256']:
            raise ValueError('artifact manifest mismatch')
        src = str(Path(item['target']['src_path']).resolve(strict=True))
        if not any(all(item['target'].get(k) == t.get(k) for k in ('name', 'kind', 'crate_types')) and
                   src == str(Path(t['src_path']).resolve()) for t in package['targets']):
            raise ValueError('artifact source differs from metadata')
        if not set(item['features']).issubset(package['features']):
            raise ValueError('artifact features outside resolved feature union')
        app = item.get('executable') and item['target']['name'] == expected['binary'] and item['target']['kind'] == ['bin']
        if app and item['profile'] != expected['profile']:
            raise ValueError('compiler profile differs from expected app profile')
        files = {str(p): sha(p) for p in map(Path, item['filenames'])}
        if not files or any(not Path(p).is_relative_to(target) or Path(p).resolve() != Path(p) for p in files):
            raise ValueError('artifact outside canonical owned target')
        if app:
            profile_dir = 'debug' if expected['profile_name'] == 'dev' else 'release'
            final = target / expected['target'] / profile_dir / expected['binary']
            if item['executable'] != str(final) or str(final) not in files:
                raise ValueError('wrong selected executable output')
        registry_cached = item.get('fresh') is True and (package['source'] or '').startswith('registry+')
        if not registry_cached and item.get('fresh') is not False:
            raise ValueError('workspace/SDK compiler artifact must be freshly built')
        matched = []
        for n, unit in enumerate(units):
            if unit['manifest'] != str(manifest) or unit['source'] != src:
                continue
            if unit['features'] != sorted(item['features']) or unit['profile'] != item['profile']:
                continue
            is_host = unit['target'] is None
            if unit['target'] not in (None, expected['target']):
                continue
            if item['target']['kind'] in (['custom-build'], ['proc-macro']) and not is_host:
                continue
            if app and is_host:
                continue
            # Cargo may copy a bin/custom-build to its advertised final path. Bind
            # that path's bytes to the actual compiler-reported output, not a .d guess.
            copy_allowed = app or item['target']['kind'] == ['custom-build']
            if all(unit['outputs'].get(path) == digest or
                   (copy_allowed and digest in [unit['outputs'][p] for p in unit['reported_outputs']])
                   for path, digest in files.items()):
                matched.append((n, unit))
        if registry_cached:
            if matched:
                raise ValueError('cached registry artifact unexpectedly rebuilt')
            records.append(dict(artifact=item, outputs=files, inputs={}, externs={},
                                context='registry-cache-Cargo-trusted', generated={}))
        else:
            if len(matched) != 1 or matched[0][0] in used:
                raise ValueError('missing/ambiguous actual compiler invocation for artifact')
            n, unit = matched[0]; used.add(n)
            if sha(unit['depfile_snapshot']) != unit['depfile_sha256'] or not unit['depfile_fresh']:
                raise ValueError('depfile snapshot changed/stale')
            names, _ = match_depfile(Path(unit['depfile_snapshot']).read_text(), unit['cwd'],
                                    unit['reported_outputs'], unit['source'], unit['depfile'])
            if set(names) != set(unit['inputs']):
                raise ValueError('depfile inventory changed')
            records.append(dict(artifact=item, outputs=dict(unit['outputs'], **files), inputs=unit['inputs'],
                                externs=unit['externs'], generated=unit['generated'], generated_root=unit['generated_root'],
                                context='host' if unit['target'] is None else 'target', invocation=unit))
        seen.add(item['package_id'])
    if len(used) != len(units):
        raise ValueError('unmatched actual compiler invocation')
    required = {key for key, p in packages.items() if p['name'] in {s['name'] for s in expected['sdk']}}
    if not required.issubset(seen):
        raise ValueError('resolved SDK package lacks actual compiler artifact')
    if not any(r['artifact'].get('executable') for r in records):
        raise ValueError('missing executable compiler input binding')
    by_output = {}
    for r in records:
        for path, digest in r['outputs'].items():
            by_output.setdefault(path, []).append((r['artifact']['package_id'], digest))
    linked_edges = {}
    for r in records:
        package = packages[r['artifact']['package_id']]
        linked_edges.setdefault(r['artifact']['package_id'], set())
        for name, ext in r['externs'].items():
            allowed = {d['pkg'] for d in package['deps'] if d['name'].replace('-', '_') == name.split(':')[-1]}
            allowed.add(r['artifact']['package_id'])  # Package binary may use its own lib.
            edges = {pkg for pkg, digest in by_output.get(ext['path'], []) if pkg in allowed and digest == ext['sha256']}
            if not edges:
                raise ValueError('rustc extern not bound to resolved dependency output')
            linked_edges[r['artifact']['package_id']].update(edges)
    roots = {r['artifact']['package_id'] for r in records if r['artifact'].get('executable') and
             r['artifact']['target']['name'] == expected['binary']}
    reachable, pending = set(), list(roots)
    while pending:
        pkg = pending.pop()
        if pkg not in reachable:
            reachable.add(pkg); pending.extend(linked_edges.get(pkg, ()))
    if not required.issubset(reachable):
        raise ValueError('expected SDK not connected to actual executable dependency objects')
    scripts = []
    for item in messages:
        if item.get('reason') != 'build-script-executed':
            continue
        if item['package_id'] not in packages or not any(r['artifact']['package_id'] == item['package_id'] and
                                                        r['artifact']['target']['kind'] == ['custom-build'] for r in records):
            raise ValueError('build-script execution lacks host compiler artifact')
        root = sibling('provenance_session').child(item['out_dir'], target)
        scripts.append(dict(message=item, outputs=tree_files(root),
                            rerun_policy='Cargo-trusted; stable JSON does not attest rerun directives or external reads'))
    unchanged(packages, records)
    return records, scripts


def unchanged(packages, records):
    for p in packages.values():
        if sha(p['manifest']) != p['manifest_sha256']:
            raise ValueError('manifest changed during build/install')
        for path, digest in p.get('blobs', {}).items():
            if sha(path) != digest:
                raise ValueError('SDK source changed during build/install')
    for r in records:
        for kind in ('inputs', 'outputs', 'generated'):
            for path, digest in r.get(kind, {}).items():
                if sha(path) != digest:
                    raise ValueError('compiler ' + kind + ' changed during build/install')
        if r.get('generated_root') and tree_files(r['generated_root']) != r['generated']:
            raise ValueError('generated inventory changed during build/install')


def cargo_run(directory, argv):
    s = sibling('provenance_session'); directory = s.canonical(directory)
    owner = s.require_owner(directory); e = owner['expected']; source = Path(owner['source'])
    if Path.cwd() != source or s.configs(source, os.environ) != owner['config']:
        raise ValueError('build cwd/config changed since admission')
    prefix = ['cargo', 'rustc', '--locked', '--package', e['package'], '--target', e['target'],
              '--manifest-path', str(source / 'Cargo.toml'), '--bin', e['binary']]
    if e['profile_name'] == 'release': prefix.append('--release')
    features = []
    if e['no_default_features']: features.append('--no-default-features')
    if e['features']: features += ['--features', ','.join(e['features'])]
    prefix += features + ['--message-format=json-render-diagnostics', '--', '-C']
    if argv[:-1] != prefix or not argv[-1].startswith('link-arg=-Wl,-sectcreate,__TEXT,__entitlements,'):
        raise ValueError('actual Cargo argv differs from typed contract')
    entitlements = s.child(argv[-1].split(',', 4)[-1], directory)
    metadata_argv = ['cargo', 'metadata', '--locked', '--format-version', '1', '--filter-platform', e['target'],
                     '--manifest-path', str(source / 'Cargo.toml'), *features]
    invocation = dict(schema=s.SCHEMA, run_id=e['run_id'], cwd=str(source), argv=argv, metadata_argv=metadata_argv,
                      expected=e, config=owner['config'], link_input={str(entitlements): sha(entitlements)},
                      environment_sha256={k: hashlib.sha256(v.encode()).hexdigest() for k, v in os.environ.items()},
                      cargo_path=shutil.which('cargo'), rustc_wrapper=str(Path(__file__).resolve()))
    (directory / 'invocation.json').write_text(json.dumps(invocation, indent=2) + '\n')
    with (directory / 'metadata.json').open('x') as out:
        subprocess.run(metadata_argv, stdout=out, check=True)
    subprocess.run([sys.executable, '-I', '-B', str(Path(__file__).with_name('bundle-provenance.py')),
                    'metadata', '--directory', str(directory)], check=True)
    (directory / 'units').mkdir()
    env = dict(os.environ, RUSTC_WRAPPER=str(Path(__file__).resolve()), PYTHONDONTWRITEBYTECODE='1')
    with (directory / 'cargo.jsonl').open('x') as out:
        result = subprocess.run(argv, env=env, stdout=out)
    invocation.update(returncode=result.returncode, cargo_json_sha256=sha(directory / 'cargo.jsonl'),
                      metadata_sha256=sha(directory / 'metadata.json'))
    (directory / 'invocation.json').write_text(json.dumps(invocation, indent=2) + '\n')
    return result.returncode


if __name__ == '__main__':
    if len(sys.argv) > 3 and sys.argv[1] == 'cargo-run':
        raise SystemExit(cargo_run(sys.argv[2], sys.argv[3:]))
    raise SystemExit(rustc_adapter(sys.argv[1:]))
