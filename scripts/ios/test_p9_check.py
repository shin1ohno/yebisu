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

class ImageEvidenceTests(unittest.TestCase):
    def fixture(self):
        paints, wire = P9EvidenceTests().fixture()
        for index in range(3):
            wire.append({"method": "terminal.put", "terminal": "t7", "name": "paste.png", "decoded": 100})
            wire.append({"channel": 1, "terminal": "t7", "bytes": list(f"/state\\ with\\ space/paste-{index}.png".encode())})
            if index == 1:
                wire.append({"channel": 1, "terminal": "t7", "bytes": list(b"P8-shadow")})
        return paints, wire

    def test_mixed_host_shaped_records_pass_both_observers(self):
        paints, wire = self.fixture()
        p9.validate(paints, wire)
        p9.validate_images(wire)

    def test_missing_duplicate_wrong_terminal_and_schema_fail(self):
        for mode in range(5):
            _, wire = self.fixture()
            if mode == 0:
                del wire[3]
            elif mode == 1:
                wire.insert(4, wire[3].copy())
            elif mode == 2:
                wire[3]["terminal"] = "wrong-terminal"
            elif mode == 3:
                wire[2]["decoded"] = "100"
            else:
                wire[3] = {"terminal": "t7"}
            with self.subTest(mode=mode), self.assertRaises(ValueError):
                p9.validate_images(wire)
