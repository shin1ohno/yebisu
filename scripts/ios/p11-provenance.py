"""Collector provenance helpers. Parsing tests are not binary/native proof."""
import hashlib
import json
from pathlib import Path
import re
import subprocess
import uuid
import tomllib

OVERRIDES = {'CARGO_TARGET_DIR','CARGO_BUILD_TARGET_DIR','CARGO_BUILD_BUILD_DIR',
             'RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS',
             'RUSTDOCFLAGS','XCODE_XCCONFIG_FILE','SYMROOT','OBJROOT','DSTROOT',
             'CONFIGURATION_BUILD_DIR','CONFIGURATION_TEMP_DIR','DERIVED_DATA_DIR',
             'SHARED_PRECOMPS_DIR','CLANG_MODULE_CACHE_PATH','SWIFT_MODULE_CACHE_PATH',
             'HERDR_IOS_ARTIFACT_ROOT','RUSTC','RUSTDOC','RUSTUP_TOOLCHAIN','CARGO_ENCODED_RUSTDOCFLAGS'}

def digest(data):return hashlib.sha256(data).hexdigest()
def encoded(value):return json.dumps(value,sort_keys=True,separators=(',',':')).encode()

def layout(checkout, requested, head):
    checkout=Path(checkout).resolve();requested=Path(requested).absolute()
    # Resolve both sides but anchor expected output in the REAL checkout.
    expected=checkout/'target'/'p11-native'/head
    if requested.parent!=expected or requested.resolve().parent!=expected:
        raise ValueError('outputs must be in checkout-owned target/p11-native/HEAD/run-UUID')
    run=str(uuid.UUID(requested.name))
    if requested.name!=run or requested.exists():raise ValueError('run root must be fresh canonical UUID')
    if not re.fullmatch('[0-9a-f]{40}',head):raise ValueError('invalid source head')
    return {name:requested/name for name in ['cargo','cargo-build','cargo-home','xcode','project','tmp','artifacts','clang-modules','swift-modules','swift-packages']}

def environment(original, paths):
    forbidden=[k for k,v in original.items() if v and (k in OVERRIDES or k.startswith(('CARGO_BUILD_','CARGO_TARGET_')))]
    if forbidden:raise ValueError('inherited build overrides: '+','.join(sorted(forbidden)))
    result=dict(original)
    result.update(CARGO_HOME=str(paths['cargo-home']),CARGO_TARGET_DIR=str(paths['cargo']),CARGO_BUILD_BUILD_DIR=str(paths['cargo-build']),
                  HERDR_IOS_ARTIFACT_ROOT=str(paths['cargo']),TMPDIR=str(paths['tmp']),
                  CLANG_MODULE_CACHE_PATH=str(paths['clang-modules']),SWIFT_MODULE_CACHE_PATH=str(paths['swift-modules']))
    return result

def source_manifest(root, head):
    actual=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()
    if actual!=head or subprocess.check_output(['git','status','--porcelain'],cwd=root,text=True).strip():raise ValueError('source changed or dirty')
    names=subprocess.check_output(['git','ls-files','-z'],cwd=root).decode().split('\0')
    files={name:digest((root/name).read_bytes()) for name in names if name}
    return {'head':head,'checkout':str(root.resolve()),'files':files}

def uuids(output):
    values=[]
    for line in output.splitlines():
        match=re.fullmatch(r'UUID: ([0-9A-Fa-f-]{36}) \(([^)]+)\) .+',line)
        if not match:raise ValueError('unrecognized Mach-O UUID output')
        values.append({'arch':match[2],'uuid':str(uuid.UUID(match[1]))})
    if not values or len({v['arch'] for v in values})!=len(values):raise ValueError('missing/duplicate architecture UUID')
    return sorted(values,key=lambda v:v['arch'])

def bind(source, build, installed, run, sdk):
    if build['sha256']!=installed['sha256'] or build['uuids']!=installed['uuids']:raise ValueError('installed artifact identity differs')
    value={'schema':1,'source_manifest_sha256':digest(encoded(source)),
           'consumer_head':source['head'],'sdk_head':sdk,'run_uuid':run,
           'build':build,'installed':installed}
    return value,digest(encoded(value))


def cargo_configuration(root):
    root=Path(root).resolve()
    allowed=root/'.cargo/config.toml'
    for directory in [root,*root.parents]:
        for name in ['config','config.toml']:
            path=directory/'.cargo'/name
            if path.exists() and path!=allowed:raise ValueError('ancestor Cargo config requires separate review: '+str(path))
    if tomllib.loads(allowed.read_text())!={'net':{'git-fetch-with-cli':True},'env':{'IPHONEOS_DEPLOYMENT_TARGET':'17.0'}}:
        raise ValueError('checkout Cargo config changed; review effective build settings')
    # User Cargo configuration is excluded by the fresh checkout-owned CARGO_HOME.
    return digest(allowed.read_bytes())

def tree_manifest(root):
    return {str(p.relative_to(root)):digest(p.read_bytes()) for p in root.rglob('*') if p.is_file()}
