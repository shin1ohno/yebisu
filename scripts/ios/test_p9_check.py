"""The P9 observer must reject no-op XCTest and duplicate terminal identities."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("p9_check", Path(__file__).with_name("p9-check.py"))
p9 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p9)


class P9EvidenceTests(unittest.TestCase):
    def fixture(self):
        paints = [{"kind": "terminal", "boot": "core-1", "generation": n} for n in (1, 3)]
        wire = [{"terminal": "t3", "bytes": list("P9-nativeかな😀変換P9-pane1".encode())},
                {"terminal": "t4", "bytes": list(b"P9-pane2")}]
        return paints, wire

    def test_distinct_terminals_and_reattachment(self):
        p9.validate(*self.fixture())

    def test_duplicate_id_is_not_a_split(self):
        paints, wire = self.fixture()
        wire[1]["terminal"] = "t3"
        with self.assertRaisesRegex(ValueError, "distinct terminal IDs"):
            p9.validate(paints, wire)

    def test_no_reattachment_frame(self):
        paints, wire = self.fixture()
        with self.assertRaisesRegex(ValueError, "reattachment"):
            p9.validate(paints[:1], wire)

    def test_no_ime_receipt(self):
        paints, wire = self.fixture()
        wire[0]["bytes"] = list(b"P9-nativeP9-pane1")
        with self.assertRaisesRegex(ValueError, "IME commit"):
            p9.validate(paints, wire)
