#!/usr/bin/env python3
"""Lease-only provisional SDK11 collection. Never declares full P1-P5 admission."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

ROOT=Path(__file__).resolve().parents[2]
def module(name):
    spec=importlib.util.spec_from_file_location(name,Path(__file__).with_name(name+'.py'))
    result=importlib.util.module_from_spec(spec);spec.loader.exec_module(result);return result
oracle=module('p11-observer-oracle');privacy=module('p11-privacy-check');provenance=module('p11-provenance')

def collect(container,out,head,run_id,runner,binding,binding_sha,xctest):
    """Copy every launch and rotated log before any oracle may fail."""
    sinks=out/'sinks';sinks.mkdir()
    sources=[*container.joinpath('Documents').rglob('*.jsonl'),
             *container.joinpath('Documents/p11-native').glob('*.log'),
             *container.joinpath('Library/Application Support/yebisu/logs').glob('*')]
    paths={}
    for source in sources:
        if not source.is_file():continue
        target=sinks/source.relative_to(container).as_posix().replace('/','__')
        shutil.copy2(source,target);paths[source.relative_to(container).as_posix()]=target
    native=[p for k,p in paths.items() if k.startswith('Documents/p11-native/')]
    if not native:raise ValueError('missing actual launch stderr')
    # Every app launch must contain its own actual selected-provider receipt.
    for path in native:
        data=path.read_bytes()
        if b'p11-native-capture pid=' not in data or b'key-provider-preflight injected=true' not in data or ('binding='+binding_sha).encode() not in data or ('image_uuid='+binding['installed']['uuids'][0]['uuid']).encode() not in data:
            raise ValueError('unverified provider or missing launch capture')
    observation=paths.get('Documents/p0-smoke.jsonl')
    diagnostics=paths.get('Library/Application Support/yebisu/logs/yebisu.jsonl')
    if observation is None or diagnostics is None:raise ValueError('missing native observation/diagnostic file')
    combined=sinks/'native-all.log';combined.write_bytes(b'\n'.join(p.read_bytes() for p in sorted(native)))
    output=sinks/'runner.log';output.write_bytes(runner)
    roles={'observation':observation,'diagnostics':diagnostics,'native_stderr':combined,'runner_stdout':output}
    roles.update({f'collected-{i}':p for i,p in enumerate(paths.values()) if p not in roles.values()})
    manifest={'schema':1,'head':head,'sdk':oracle.SDK,'run_id':run_id,'binding_sha256':binding_sha,'binding':binding,'xctest_inputs':xctest,'collection_directory':str(sinks.resolve()),
              'sinks':{k:{'path':str(p.resolve()),'sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for k,p in roles.items()}}
    (out/'privacy-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
    privacy.validate(manifest)
    records=[json.loads(line) for line in observation.read_text().splitlines()]
    messages=[json.loads(line) for line in paths['Documents/p4-peer.jsonl'].read_text().splitlines()]
    launches=[r for r in records if r.get('kind')=='launch']
    if len(launches)!=len(native):raise ValueError('launch/capture inventory mismatch')
    return records,messages

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--lease',required=True,help='Controller exact lease receipt path; no permission expansion')
    parser.add_argument('--head',required=True)
    parser.add_argument('--device',required=True)
    parser.add_argument('--target-dir',type=Path,required=True,help='Fresh, exclusive per-head Cargo target directory')
    args=parser.parse_args()
    if sys.platform!='darwin':raise SystemExit('Mac-only; Linux product builds forbidden')
    lease=json.loads(Path(args.lease).read_text())
    if lease.get('consumer_head')!=args.head or lease.get('sdk_head')!=oracle.SDK or lease.get('device')!=args.device or lease.get('exclusive') is not True:raise SystemExit('lease exact-head mismatch')
    if subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()!=args.head:raise SystemExit('wrong source head')
    if subprocess.check_output(['git','status','--porcelain'],cwd=ROOT,text=True).strip():raise SystemExit('dirty checkout')
    if args.target_dir.exists():raise SystemExit('target directory must be fresh, not Runner/shared cache')
    paths=provenance.layout(ROOT,args.target_dir,args.head)
    env=provenance.environment(os.environ,paths)
    provenance.cargo_configuration(ROOT)
    if lease.get('checkout')!=str(ROOT.resolve()) or lease.get('run_root')!=str(args.target_dir.resolve()):raise SystemExit('lease checkout/run-root mismatch')
    for path in paths.values():path.mkdir(parents=True)
    out=paths['artifacts'];run_uuid=args.target_dir.name
    source_state=provenance.source_manifest(ROOT,args.head)
    (out/'source-manifest.json').write_bytes(provenance.encoded(source_state))
    transcript=bytearray()
    def run(*command):
        transcript.extend(('P11 COMMAND '+json.dumps(command)+'\n').encode())
        result=subprocess.run(command,cwd=ROOT,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
        transcript.extend(result.stdout);(out/'runner.log').write_bytes(transcript)
        result.check_returncode();return result.stdout.decode()
    run('rustc','-Vv');run('cargo','-V')
    bundle=Path(run('bash','scripts/ios/bundle.sh','--package','yebisu','--bin','yebisu','--features','test-support').strip().splitlines()[-1])
    if not bundle.resolve().is_relative_to(args.target_dir.resolve()):raise RuntimeError('bundle escaped isolated target directory')
    executable=bundle/'yebisu'
    build={'path':str(executable.resolve()),'sha256':provenance.digest(executable.read_bytes()),
           'uuids':provenance.uuids(run('xcrun','dwarfdump','--uuid',str(executable)))}
    if build['uuids'][0]['arch']!='arm64' or len(build['uuids'])!=1:raise RuntimeError('collector requires thin arm64 Simulator executable')
    if provenance.source_manifest(ROOT,args.head)!=source_state:raise RuntimeError('source changed during build')
    run('xcrun','simctl','install',args.device,str(bundle))
    installed_path=Path(run('xcrun','simctl','get_app_container',args.device,'com.shin1ohno.yebisu','app').strip())/'yebisu'
    installed={'path':str(installed_path),'sha256':provenance.digest(installed_path.read_bytes()),
               'uuids':provenance.uuids(run('xcrun','dwarfdump','--uuid',str(installed_path)))}
    installed_copy=out/'installed-yebisu'
    shutil.copy2(installed_path,installed_copy)
    installed['original_path']=installed['path'];installed['path']=str(installed_copy)
    binding,binding_sha=provenance.bind(source_state,build,installed,run_uuid,oracle.SDK)
    binding['source_manifest_path']=str(out/'source-manifest.json')
    binding_sha=provenance.digest(provenance.encoded(binding))
    (out/'artifact-binding.json').write_bytes(provenance.encoded(binding))
    container=Path(run('xcrun','simctl','get_app_container',args.device,'com.shin1ohno.yebisu','data').strip())
    project=paths['project']
    shutil.copy2(ROOT/'scripts/ios/smoke-qa/project.yml',project/'project.yml')
    shutil.copytree(ROOT/'scripts/ios/smoke-qa/Tests',project/'Tests')
    swift=project/'Tests/SmokeQA.swift'
    original=swift.read_text()
    if original.count('P11_BINDING_UNSET')!=1:raise RuntimeError('missing unique XCTest binding seam')
    swift.write_text(original.replace('P11_BINDING_UNSET',binding_sha))
    run('xcodegen','generate','--spec',str(project/'project.yml'),'--project',str(project))
    xctest_inputs=provenance.tree_manifest(project)
    xctest_path=out/'xctest-inputs.json';xctest_path.write_bytes(provenance.encoded(xctest_inputs))
    xctest={'path':str(xctest_path),'sha256':provenance.digest(xctest_path.read_bytes()),'root':str(project),'binding_sha256':binding_sha}
    seen_runs=set();results=[]
    for name,modal,drawing,disabled in [('Text',False,False,False),('Drawing',False,True,False),('Modal',True,False,False),('Disabled',True,False,True)]:
        if provenance.tree_manifest(project)!=xctest_inputs:raise RuntimeError('XCTest inputs changed before case')
        case=out/name;case.mkdir()
        # Preserve old bytes first; do not mix historical launches into this run.
        old=case/'before';old.mkdir()
        for source in [container/'Documents/p0-smoke.jsonl',container/'Documents/p4-peer.jsonl',container/'Documents/p11-native',container/'Library/Application Support/yebisu/logs']:
            if source.is_dir():shutil.copytree(source,old/source.name);shutil.rmtree(source)
            elif source.exists():shutil.copy2(source,old/source.name);source.unlink()
        for source in container.rglob('p11-qa/yebisu-settings.json'):
            shutil.copy2(source,old/'p11-settings.json');source.unlink()
        method='testP11Observer'+name
        failure=None
        try:
            output=run('xcodebuild','test','-project',str(project/'SmokeOracle.xcodeproj'),'-scheme','SmokeOracle','-destination','id='+args.device,'-derivedDataPath',str(paths['xcode']),'-resultBundlePath',str(case/'result.xcresult'),'CODE_SIGNING_ALLOWED=NO','SYMROOT='+str(paths['xcode']/'products'),'OBJROOT='+str(paths['xcode']/'intermediates'),'SHARED_PRECOMPS_DIR='+str(paths['xcode']/'precompiled'),'CLANG_MODULE_CACHE_PATH='+str(paths['clang-modules']),'SWIFT_MODULE_CACHE_PATH='+str(paths['swift-modules']),'-clonedSourcePackagesDirPath',str(paths['swift-packages']),'-only-testing:SmokeOracle/SmokeQA/'+method)
            if method+"]' passed" not in output:raise RuntimeError('XCTest did not execute')
        except Exception as error:failure=error
        finally:
            # Actual process termination must precede final transcript collection.
            stopped=subprocess.run(['xcrun','simctl','terminate',args.device,'com.shin1ohno.yebisu'],capture_output=True)
            (case/'termination.json').write_text(json.dumps({'returncode':stopped.returncode,'xctest_termination_assertion':'required; inspect result bundle'}))
            records,messages=collect(container,case,args.head,run_uuid+"/"+name,bytes(transcript),binding,binding_sha,xctest)
            (case/'evidence.json').write_text(json.dumps({'records':records,'messages':messages},indent=2))
        if provenance.source_manifest(ROOT,args.head)!=source_state:raise RuntimeError('source changed during native collection')
        if provenance.tree_manifest(project)!=xctest_inputs:raise RuntimeError('XCTest inputs changed during case')
        if failure:raise failure
        # Disabled is accepted only as an intended missing-pair negative AFTER real positives.
        if disabled:
            try:oracle.validate(records,modal=True,disabled=True)
            except ValueError as error:
                if str(error)!='missing native pair':raise
            else:raise ValueError('observer-disabled negative survived')
        identity=oracle.validate(records,modal=modal,drawing=drawing,disabled=disabled,allow_disabled=disabled)
        if identity in seen_runs:raise ValueError('process run reused')
        seen_runs.add(identity);oracle.fifo(messages,modal)
        if not modal:oracle.positive(records,messages)
        else:
            settings=list(container.rglob('p11-qa/yebisu-settings.json'))
            if len(settings)!=1:raise ValueError('missing unique settings file')
            disk=json.loads(settings[0].read_text());(case/'settings.json').write_text(json.dumps(disk,indent=2))
            oracle.settings(records,messages,disk)
        results.append({'case':name,'observed_controls':True})
        (out/'source-candidate-controls.json').write_text(json.dumps(results,indent=2))
    (out/'acceptance.json').write_text(json.dumps({'accepted':False,'reason':'inactive/no-responder and pinned transport final-drain gate unsupported; independent code/native/full26 gates remain separate','legacy_0_of_4':'historical failure retained'},indent=2))
    raise SystemExit('Collected supported controls only; full native acceptance remains CLOSED')
if __name__=='__main__':main()
