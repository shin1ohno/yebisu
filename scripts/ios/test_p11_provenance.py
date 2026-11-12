"""Local path/parser adversaries only; no Cargo, Xcode, Mach-O or native run."""
import importlib.util
import tempfile
from pathlib import Path
import unittest
spec=importlib.util.spec_from_file_location('p',Path(__file__).with_name('p11-provenance.py'))
p=importlib.util.module_from_spec(spec);spec.loader.exec_module(p)
HEAD='a'*40
RUN='f1c579db-a0d4-4c3f-a6b5-aa709ef3f908'
class ProvenanceTests(unittest.TestCase):
    def test_owned_layout_and_inherited_output_rejection(self):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d);paths=p.layout(root,root/'target/p11-native'/HEAD/RUN,HEAD)
            env=p.environment({'PATH':'/tools'},paths)
            self.assertEqual(env['TMPDIR'],str(paths['tmp']))
            for key in p.OVERRIDES | {'CARGO_BUILD_RUSTFLAGS','CARGO_BUILD_RUSTC_WRAPPER','CARGO_TARGET_AARCH64_APPLE_IOS_SIM_LINKER'}:
                with self.assertRaisesRegex(ValueError,'inherited build overrides'):
                    p.environment({key:'/shared'},paths)
            with self.assertRaisesRegex(ValueError,'checkout-owned'):p.layout(root,root/'shared'/RUN,HEAD)
    def test_symlink_and_reused_root_rejected(self):
        with tempfile.TemporaryDirectory() as d,tempfile.TemporaryDirectory() as other:
            root=Path(d);(root/'target').symlink_to(other,target_is_directory=True)
            with self.assertRaisesRegex(ValueError,'checkout-owned'):p.layout(root,root/'target/p11-native'/HEAD/RUN,HEAD)
            (root/'target').unlink();candidate=root/'target/p11-native'/HEAD/RUN;candidate.mkdir(parents=True)
            with self.assertRaisesRegex(ValueError,'fresh'):p.layout(root,candidate,HEAD)
    def test_actual_tool_shape_all_architectures_and_malformed_output(self):
        value=p.uuids('UUID: F1C579DB-A0D4-4C3F-A6B5-AA709EF3F908 (arm64) /app/yebisu\nUUID: AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE (x86_64) /app/yebisu\n')
        self.assertEqual([x['arch'] for x in value],['arm64','x86_64'])
        for bad in ['', 'UUID: not-a-uuid (arm64) /app', 'success']:
            with self.assertRaises(ValueError):p.uuids(bad)
    def test_cargo_ancestor_configuration_is_not_silently_inherited(self):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d)/'checkout';(root/'.cargo').mkdir(parents=True)
            config=root/'.cargo/config.toml';config.write_text('[net]\ngit-fetch-with-cli=true\n[env]\nIPHONEOS_DEPLOYMENT_TARGET="17.0"\n')
            self.assertEqual(p.cargo_configuration(root),p.digest(config.read_bytes()))
            ancestor=Path(d)/'.cargo';ancestor.mkdir();(ancestor/'config.toml').write_text('[build]\nrustc-wrapper="external"\n')
            with self.assertRaisesRegex(ValueError,'ancestor Cargo config'):p.cargo_configuration(root)
    def test_source_build_installed_binding_does_not_accept_wrong_binary(self):
        source={'head':HEAD,'files':{'Cargo.lock':'locked'}}
        build={'path':'/owned/app','sha256':'1'*64,'uuids':[{'arch':'arm64','uuid':RUN}]}
        installed=dict(build,path='/installed/app')
        binding,sha=p.bind(source,build,installed,RUN,'2'*40)
        self.assertEqual(sha,p.digest(p.encoded(binding)))
        for changed in [dict(installed,sha256='3'*64),dict(installed,uuids=[])]:
            with self.assertRaisesRegex(ValueError,'identity differs'):p.bind(source,build,changed,RUN,'2'*40)
if __name__=='__main__':unittest.main()
