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
        for role in self.fixture():
            for secret in privacy.SECRETS:
                sinks=self.fixture();sinks[role]+=secret
                with self.assertRaisesRegex(ValueError,"secret detected"):privacy.scan(sinks)
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
if __name__=='__main__':unittest.main()
