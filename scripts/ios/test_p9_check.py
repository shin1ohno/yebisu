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
    """#144: the mock core's doc.set and labelled terminal.run receipts sit beside the input and
    put receipts."""

    def receipt(self, version=1, result="ok", size=120):
        return {"method": "doc.set", "name": "herdr-app", "version": version, "bytes": size, "result": result}

    def run_receipt(self, terminal="t3"):
        return {"method": "terminal.run", "terminal": terminal,
                "labels": {"herdr.space": "spa", "herdr.tab": "tba", "herdr.seen": "0"}}

    def written(self):
        # S5's save, then S3's stale write and the write again over the newer version; then
        # testP9HerdrDocRename's (#144 D3, a new mock core per launch): S2's rename, and S3's
        # stale rename and the rename again. A rename starts no terminal.
        return [self.receipt(1), self.run_receipt("t3"), self.receipt(1, "conflict", 120),
                self.receipt(2, "ok", 135), self.run_receipt("t3"),
                self.receipt(1, "ok", 121), self.receipt(1, "conflict", 121), self.receipt(2, "ok", 136)]

    def test_doc_receipts_pass_the_other_observers(self):
        paints, wire = ImageEvidenceTests().fixture()
        wire.insert(1, self.receipt())
        wire.insert(2, self.run_receipt("t9"))
        p9.validate(paints, wire)
        p9.validate_images(wire)

    def test_the_herdr_doc_writes_pass(self):
        _, wire = P9EvidenceTests().fixture()
        p9.validate_doc(wire + self.written())

    def test_a_read_only_run_writes_no_document(self):
        _, wire = P9EvidenceTests().fixture()
        p9.validate_doc(wire, expected=(), labelled=0)
        with self.assertRaisesRegex(ValueError, "expected doc.set results"):
            p9.validate_doc(wire + [self.receipt()], expected=(), labelled=0)

    def test_missing_extra_or_reordered_writes_fail(self):
        written = self.written()
        for broken in (written[:-2], written + [self.receipt(3)], [written[2], written[0]] + written[3:]):
            with self.subTest(broken=broken), self.assertRaisesRegex(ValueError, "expected doc.set results"):
                p9.validate_doc(broken)

    def test_a_stale_write_sent_again_unchanged_fails(self):
        for again in (self.receipt(1, "ok", 135), self.receipt(2, "ok", 120)):
            written = self.written()
            written[3] = again
            with self.subTest(again=again), self.assertRaisesRegex(ValueError, "sent again unchanged"):
                p9.validate_doc(written)
        # D3: a stale rename sent again unchanged fails the same way.
        written = self.written()
        written[-1] = self.receipt(2, "ok", 121)
        with self.assertRaisesRegex(ValueError, "sent again unchanged"):
            p9.validate_doc(written)

    def test_terminals_need_the_official_labels(self):
        for labels in ({"herdr.space": "spa", "herdr.tab": "tba"}, {"herdr.space": "spa", "herdr.tab": "tba",
                                                                     "herdr.seen": "1"}, None):
            written = self.written()
            written[1] = {"method": "terminal.run", "terminal": "t3", "labels": labels}
            with self.subTest(labels=labels), self.assertRaisesRegex(ValueError, "official labels"):
                p9.validate_doc(written)
        # S3's New tab terminal (the last labelled run) missing.
        with self.assertRaisesRegex(ValueError, "labelled terminal.run"):
            p9.validate_doc([r for i, r in enumerate(self.written()) if i != 4])

    def test_schema(self):
        for broken in ({"method": "doc.set", "bytes": 1, "result": "ok"},
                       {"method": "doc.set", "version": "1", "bytes": 1, "result": "ok"}):
            with self.subTest(broken=broken), self.assertRaisesRegex(ValueError, "malformed"):
                p9.validate_doc([broken], expected=("ok",), labelled=0)


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
