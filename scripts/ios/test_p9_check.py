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


class DocEvidenceTests(unittest.TestCase):
    """#144: the mock core's doc.set receipts sit beside the input and put receipts."""

    def receipt(self, version=1):
        return {"method": "doc.set", "name": "herdr-app", "version": version, "bytes": 120, "result": "ok"}

    def test_doc_receipts_pass_the_other_observers(self):
        paints, wire = ImageEvidenceTests().fixture()
        wire.insert(1, self.receipt())
        wire.insert(2, {"method": "terminal.run", "terminal": "t9",
                        "labels": {"herdr.space": "spa", "herdr.tab": "tba", "herdr.seen": "0"}})
        p9.validate(paints, wire)
        p9.validate_images(wire)

    def test_a_read_only_run_writes_no_document(self):
        _, wire = P9EvidenceTests().fixture()
        p9.validate_doc(wire)
        with self.assertRaisesRegex(ValueError, "expected 0 doc.set receipts, found 1"):
            p9.validate_doc(wire + [self.receipt()])

    def test_counts_and_schema(self):
        p9.validate_doc([self.receipt(1), self.receipt(2)], expected=2)
        with self.assertRaisesRegex(ValueError, "expected 1"):
            p9.validate_doc([], expected=1)
        for broken in ({"method": "doc.set", "bytes": 1}, {"method": "doc.set", "version": "1", "bytes": 1}):
            with self.subTest(broken=broken), self.assertRaisesRegex(ValueError, "malformed"):
                p9.validate_doc([broken], expected=1)


class FailureEvidenceTests(unittest.TestCase):
    def test_raw_receipts_survive_failed_native_attachment_export(self):
        import subprocess
        import tempfile
        from unittest.mock import patch
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            evidence = root / "evidence"
            evidence.mkdir()
            paints = root / "app.jsonl"
            wire = root / "core.jsonl"
            paints.write_text('{"kind":"p8-image-source","bytes":128}\n')
            wire.write_text('{"method":"terminal.put","terminal":"t3","decoded":100}\n')
            with patch.object(p9, "run", side_effect=subprocess.CalledProcessError(65, ["xcrun"])):
                p9.collect_evidence(evidence, root / "failed.xcresult", paints, wire)
            self.assertEqual((evidence / "paints.jsonl").read_bytes(), paints.read_bytes())
            self.assertEqual((evidence / "wire.jsonl").read_bytes(), wire.read_bytes())
