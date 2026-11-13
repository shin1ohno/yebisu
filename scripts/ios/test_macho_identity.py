"""Independent binary fixtures. They are not Apple codesign/runtime evidence."""
import importlib.util
from pathlib import Path
import struct
import unittest

spec = importlib.util.spec_from_file_location('macho', Path(__file__).with_name('macho_identity.py'))
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)


def image(signature_payload=b'linker CodeDirectory fixture'):
    # CodeDirectory slot in a bounded SuperBlob; actual codesign validity is a native gate.
    entry = struct.pack('>II', 0xfade0c02, 8 + len(signature_payload)) + signature_payload
    signature = struct.pack('>IIIII', 0xfade0cc0, 20 + len(entry), 1, 0, 20) + entry
    signature += bytes((-len(signature)) % 16)
    def segment(name, fileoff, filesize):
        return struct.pack('<II16sQQQQIIII', 0x19, 72, name, fileoff, 4096,
                           fileoff, filesize, 7, 5, 0, 0)
    commands = segment(b'__TEXT', 0, 256) + segment(b'__LINKEDIT', 256, 64 + len(signature))
    commands += struct.pack('<II16s', 0x1b, 24, bytes(range(16)))
    commands += struct.pack('<IIII', 0x1d, 16, 320, len(signature))
    header = struct.pack('<8I', 0xfeedfacf, 0x100000c, 0, 2, 4, len(commands), 0, 0)
    body = header + commands
    return body + b'X' * (256 - len(body)) + b'L' * 64 + signature


class CodeIdentity(unittest.TestCase):
    def test_signature_only_resize_preserves_all_non_signature_bytes(self):
        before = image()
        after = image(b'bundle CodeDirectory and resources fixture' * 4)
        self.assertNotEqual(before, after)
        m.same_code(m.code_identity(before), m.code_identity(after))

    def test_same_uuid_changed_text_data_linkedit_commands_rejected(self):
        baseline = m.code_identity(image())
        for index in (240, 250, 280, 24, 112):
            altered = bytearray(image())
            altered[index] ^= 1
            with self.subTest(byte=index), self.assertRaises(ValueError):
                m.same_code(baseline, m.code_identity(altered))

    def test_forged_signature_range_and_trailing_data_rejected(self):
        for offset, value in ((208, 240), (212, 1), (200, 0x2), (328, 999)):
            data = bytearray(image())
            struct.pack_into('<I', data, offset, value)
            with self.subTest(offset=offset), self.assertRaises(ValueError):
                m.code_identity(data)
        with self.assertRaises(ValueError):
            m.code_identity(image() + b'payload')

    def test_non_macho_and_changed_uuid_fail_closed(self):
        with self.assertRaises(ValueError):
            m.code_identity(b'compiled')
        changed = bytearray(image()); changed[184] ^= 1
        with self.assertRaises(ValueError):
            m.same_code(m.code_identity(image()), m.code_identity(changed))


if __name__ == '__main__':
    unittest.main()
