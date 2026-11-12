import importlib.util
from pathlib import Path
import unittest
spec = importlib.util.spec_from_file_location("p2", Path(__file__).with_name("p2-check.py"))
p2 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p2)

class ObservationTests(unittest.TestCase):
    def records(self):
        return [{"kind":"launch"},{"kind":"setup","key_ready":True,"status":"Copied restricted public key."},{"kind":"launch"},{"kind":"setup","key_ready":True}]
    def test_actual_nonsecret_setup_shape(self):
        self.assertEqual(p2.validate_records(self.records()), "restrict ssh-ed25519 RkFLRV9QMl9RQQ== yebisu-injected-qa")
    def test_negative_public_key_in_receipt(self):
        records=self.records();records[1]["key_line"]="FAKE_PUBLIC_KEY_SENTINEL"
        with self.assertRaisesRegex(RuntimeError,"public key leaked"):p2.validate_records(records)
    def test_negative_missing_second_launch_readiness(self):
        records=self.records();records[-1]["key_ready"]=False
        with self.assertRaisesRegex(RuntimeError,"not ready"):p2.validate_records(records)
    def test_negative_missing_copy(self):
        records=self.records();records[1]["status"]="ready"
        with self.assertRaisesRegex(RuntimeError,"copy action"):p2.validate_records(records)
if __name__ == "__main__":unittest.main()
