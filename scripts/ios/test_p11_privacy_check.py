import importlib.util
from pathlib import Path
import unittest
import tempfile
import json
import hashlib
spec=importlib.util.spec_from_file_location("privacy",Path(__file__).with_name("p11-privacy-check.py"))
privacy=importlib.util.module_from_spec(spec);spec.loader.exec_module(privacy)
class PrivacyOracle(unittest.TestCase):
    def fixture(self):
        return {"observation":b'{"kind":"key-provider-preflight","injected":true}\n',
                "diagnostics":b'{"event":"startup"}\n',
                "native_stderr":b'key-provider-preflight injected=true\n',
                "runner_stdout":b'P11 COMMAND native probe\n'}
    def test_controls(self):privacy.scan(self.fixture())
    def test_each_sink_and_encoded_key_negative(self):
        # Independent expected bytes: never derive witnesses from detector SECRETS.
        witnesses = (b"FAKE_PUBLIC_KEY_SENTINEL", b"FAKE_PRIVATE_SENTINEL",
                     b"RkFLRV9QVUJMSUNfS0VZX1NFTlRJTkVM",
                     b"RkFLRV9QUklWQVRFX1NFTlRJTkVM",
                     b"FAKE_P2_QA", b"RkFLRV9QMl9RQQ==")
        roles = (*self.fixture(), "native_stdout", "rotated_diagnostics", "guard", "peer")
        for role in roles:
            for secret in witnesses:
                with self.subTest(role=role, secret=secret):
                    sinks = self.fixture()
                    # Valid JSONL even in structured sinks: syntax must not mask a miss.
                    sinks[role] = sinks.get(role, b"nonsecret collection control\n") + json.dumps(
                        {"fixture": secret.decode("ascii")}).encode() + b"\n"
                    with self.assertRaisesRegex(ValueError, "^secret detected in " + role + "$" ):
                        privacy.scan(sinks)

    def test_additional_clean_sinks(self):
        sinks = self.fixture()
        sinks.update(native_stdout=b"capture positive\n", rotated_diagnostics=b'{"event":"startup"}\n',
                     guard=b"disk guard positive\n", peer=b"peer transcript positive\n")
        privacy.scan(sinks)

    def test_no_stderr_is_not_clean(self):
        sinks=self.fixture();del sinks['native_stderr']
        with self.assertRaisesRegex(ValueError,"missing required"):privacy.scan(sinks)
    def test_empty_sink_fails(self):
        sinks=self.fixture();sinks['native_stderr']=b''
        with self.assertRaisesRegex(ValueError,"empty sink"):privacy.scan(sinks)
    def test_wrong_provider_fails(self):
        sinks=self.fixture();sinks['observation']=b'{"kind":"key-provider-preflight","injected":false}\n'
        with self.assertRaisesRegex(ValueError,"provider observation"):privacy.scan(sinks)
    def test_manifest_inventory_and_digest(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);items={}
            for role,data in self.fixture().items():
                path=root/(role+".log");path.write_bytes(data)
                items[role]={"path":str(path),"sha256":hashlib.sha256(data).hexdigest()}
            manifest={"schema":1,"head":"a"*40,"run_id":"fixture-only","collection_directory":str(root),"sinks":items}
            privacy.validate(manifest)
            rotated=root/"diagnostic.1.jsonl";rotated.write_text('{"event":"startup"}\n')
            with self.assertRaisesRegex(ValueError,"inventory incomplete"):privacy.validate(manifest)
            rotated.unlink()
            (root/"native_stderr.log").write_bytes(b"changed")
            with self.assertRaisesRegex(ValueError,"digest mismatch"):privacy.validate(manifest)
    def test_bound_source_and_binary_tamper_fail(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);logs=root/"logs";logs.mkdir();items={}
            for role,data in self.fixture().items():
                path=logs/(role+".log");path.write_bytes(data)
                items[role]={"path":str(path),"sha256":hashlib.sha256(data).hexdigest()}
            source=root/"source.json";source.write_text(json.dumps({"head":"a"*40}))
            binary=root/"app";binary.write_bytes(b"fixture executable, not Mach-O proof")
            artifact={"path":str(binary),"sha256":hashlib.sha256(binary.read_bytes()).hexdigest(),"uuids":[{"arch":"arm64","uuid":"fixture-only"}]}
            binding={"consumer_head":"a"*40,"sdk_head":"b"*40,"run_uuid":"fixture-run", "source_manifest_path":str(source),"source_manifest_sha256":hashlib.sha256(source.read_bytes()).hexdigest(),"build":artifact,"installed":artifact}
            manifest={"schema":1,"head":"a"*40,"sdk":"b"*40,"run_id":"fixture-run/Text","collection_directory":str(logs),"sinks":items,"binding":binding,"binding_sha256":hashlib.sha256(json.dumps(binding,sort_keys=True,separators=(',',':')).encode()).hexdigest()}
            project=root/'project';project.mkdir();swift=project/'Tests.swift';swift.write_text('fixture XCTest input')
            inputs=root/'inputs.json';inputs.write_text(json.dumps({'Tests.swift':hashlib.sha256(swift.read_bytes()).hexdigest()}))
            manifest['xctest_inputs']={'path':str(inputs),'sha256':hashlib.sha256(inputs.read_bytes()).hexdigest(),'root':str(project),'binding_sha256':manifest['binding_sha256']}
            privacy.validate(manifest)
            swift.write_text('edited input')
            with self.assertRaisesRegex(ValueError,'XCTest input drift'):privacy.validate(manifest)
            swift.write_text('fixture XCTest input')
            binary.write_bytes(b"wrong installed app")
            with self.assertRaisesRegex(ValueError,"artifact bytes mismatch"):privacy.validate(manifest)
            binary.write_bytes(b"fixture executable, not Mach-O proof")
            source.write_text(json.dumps({"head":"c"*40}))
            with self.assertRaisesRegex(ValueError,"source manifest binding mismatch"):privacy.validate(manifest)
if __name__=='__main__':unittest.main()
