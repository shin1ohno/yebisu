#!/usr/bin/env python3
"""Aggregate actual native sinks; missing collection is a failure, never clean."""
import argparse
import hashlib
import json
from pathlib import Path
import re

SECRETS = (b"FAKE_PUBLIC_KEY_SENTINEL", b"FAKE_PRIVATE_SENTINEL",
           b"RkFLRV9QVUJMSUNfS0VZX1NFTlRJTkVM", b"RkFLRV9QMl9RQQ==")
REQUIRED = {"observation", "diagnostics", "native_stderr", "runner_stdout"}

def scan(sinks):
    if not REQUIRED <= set(sinks):
        raise ValueError("missing required native sink")
    for name, data in sinks.items():
        if not data:
            raise ValueError("empty sink: " + name)
        if any(secret in data for secret in SECRETS):
            raise ValueError("secret detected in " + name)
    observations=[json.loads(line) for line in sinks["observation"].splitlines()]
    if not any(r.get("kind")=="key-provider-preflight" and r.get("injected") is True for r in observations):
        raise ValueError("missing actual provider observation control")
    diagnostics=[json.loads(line) for line in sinks["diagnostics"].splitlines()]
    if not any(r.get("event")=="startup" for r in diagnostics):
        raise ValueError("missing diagnostic control")
    if b"key-provider-preflight injected=true" not in sinks["native_stderr"]:
        raise ValueError("missing native stderr control")
    if not any(m in sinks["runner_stdout"] for m in [b"P11 COMMAND", b"P2 COMMAND"]):
        raise ValueError("missing runner control")

def validate(manifest):
    if manifest.get("schema")!=1 or not re.fullmatch(r"[0-9a-f]{40}",manifest.get("head","")) or not manifest.get("run_id"):
        raise ValueError("missing exact-head run identity")
    if 'binding' in manifest or 'binding_sha256' in manifest:
        binding=manifest.get('binding',{})
        raw=json.dumps(binding,sort_keys=True,separators=(',',':')).encode()
        if hashlib.sha256(raw).hexdigest()!=manifest.get('binding_sha256'):raise ValueError('artifact binding digest mismatch')
        if binding.get('consumer_head')!=manifest['head'] or binding.get('sdk_head')!=manifest.get('sdk') or not manifest['run_id'].startswith(binding.get('run_uuid','')+'/'):raise ValueError('artifact binding identity mismatch')
        source=Path(binding['source_manifest_path']).read_bytes()
        if hashlib.sha256(source).hexdigest()!=binding['source_manifest_sha256'] or json.loads(source)['head']!=manifest['head']:raise ValueError('source manifest binding mismatch')
        for role in ['build','installed']:
            item=binding[role]
            if hashlib.sha256(Path(item['path']).read_bytes()).hexdigest()!=item['sha256']:raise ValueError('artifact bytes mismatch')
        if binding['build']['sha256']!=binding['installed']['sha256'] or binding['build']['uuids']!=binding['installed']['uuids']:raise ValueError('installed UUID/hash mismatch')
    if 'xctest_inputs' in manifest:
        item=manifest['xctest_inputs'];raw=Path(item['path']).read_bytes()
        if hashlib.sha256(raw).hexdigest()!=item['sha256'] or item['binding_sha256']!=manifest.get('binding_sha256'):raise ValueError('XCTest binding mismatch')
        root=Path(item['root']);actual={str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in root.rglob('*') if p.is_file()}
        if json.loads(raw)!=actual:raise ValueError('XCTest input drift')
    sinks={}
    for role, item in manifest["sinks"].items():
        data=Path(item["path"]).read_bytes()
        if hashlib.sha256(data).hexdigest()!=item["sha256"]:
            raise ValueError("sink digest mismatch")
        sinks[role]=data
    # The collector must inventory all log files, including rotated diagnostics.
    directory=Path(manifest["collection_directory"])
    actual={p.resolve() for p in directory.rglob("*") if p.is_file() and p.suffix in (".log",".jsonl")}
    supplied={Path(v["path"]).resolve() for v in manifest["sinks"].values()}
    if actual != supplied:
        raise ValueError("log inventory incomplete")
    scan(sinks)

if __name__=="__main__":
    parser=argparse.ArgumentParser();parser.add_argument("manifest",type=Path)
    args=parser.parse_args();validate(json.loads(args.manifest.read_text()))
    print("F24 collected sinks clean; this does not certify uncollected runs or real Keychain")
