"""Tests for scripts/ios/screenshot-color.py's PNG decoding and color counting.

    python3 -I -B scripts/ios/test_screenshot_color.py

-B keeps the imported module's bytecode out of the tree; -I ignores
PYTHONDONTWRITEBYTECODE.
"""

import contextlib
import importlib.util
import io
import struct
import tempfile
import unittest
import zlib
from pathlib import Path

SPEC = importlib.util.spec_from_file_location(
    "screenshot_color", Path(__file__).with_name("screenshot-color.py")
)
screenshot_color = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(screenshot_color)


def chunk(kind, body):
    checksum = zlib.crc32(kind + body)
    return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", checksum)


def filter_row(kind, row, previous, channels):
    """Applies a PNG row filter the way an encoder does (PNG spec, section 9)."""
    out = bytearray()
    for i, value in enumerate(row):
        left = row[i - channels] if i >= channels else 0
        up = previous[i]
        up_left = previous[i - channels] if i >= channels else 0
        predictor = {
            0: 0,
            1: left,
            2: up,
            3: (left + up) >> 1,
            4: screenshot_color.paeth(left, up, up_left),
        }[kind]
        out.append((value - predictor) & 0xFF)
    return bytes([kind]) + bytes(out)


def encode(rows, width, channels, filters):
    color_type = {3: 2, 4: 6}[channels]
    raw = bytearray()
    previous = bytes(width * channels)
    for row, kind in zip(rows, filters):
        raw += filter_row(kind, row, previous, channels)
        previous = row
    header = struct.pack(">IIBBBBB", width, len(rows), 8, color_type, 0, 0, 0)
    return (
        screenshot_color.SIGNATURE
        + chunk(b"IHDR", header)
        + chunk(b"IDAT", zlib.compress(bytes(raw)))
        + chunk(b"IEND", b"")
    )


MAGENTA = bytes.fromhex("ff00ff")
WHITE = bytes.fromhex("ffffff")


class DecodeTests(unittest.TestCase):
    def test_every_row_filter_round_trips(self):
        width = 3
        rows = [
            bytes([10, 200, 30, 255, 0, 255, 7, 8, 9]),
            bytes([250, 1, 128, 255, 0, 255, 60, 61, 62]),
            bytes([0, 0, 0, 255, 255, 255, 128, 128, 128]),
            bytes([5, 250, 5, 255, 0, 255, 1, 2, 3]),
            bytes([99, 98, 97, 255, 0, 254, 200, 100, 50]),
        ]
        png = encode(rows, width, 3, filters=[0, 1, 2, 3, 4])

        decoded_width, height, channels, decoded = screenshot_color.decode(png)

        self.assertEqual((decoded_width, height, channels), (width, 5, 3))
        self.assertEqual([bytes(row) for row in decoded], rows)

    def test_alpha_is_ignored_when_counting(self):
        rows = [MAGENTA + b"\x80" + WHITE + b"\xff"]
        png = encode(rows, 2, 4, filters=[4])

        _, _, channels, decoded = screenshot_color.decode(png)
        counts = screenshot_color.color_counts(decoded, channels)

        self.assertEqual(counts, {MAGENTA: 1, WHITE: 1})

    def test_unsupported_and_corrupt_files_are_rejected(self):
        with self.assertRaises(screenshot_color.PngError):
            screenshot_color.decode(b"GIF89a")
        sixteen_bit = screenshot_color.SIGNATURE + chunk(
            b"IHDR", struct.pack(">IIBBBBB", 1, 1, 16, 2, 0, 0, 0)
        )
        with self.assertRaises(screenshot_color.PngError):
            screenshot_color.decode(sixteen_bit + chunk(b"IEND", b""))


class FractionTests(unittest.TestCase):
    def test_tolerance_bounds_each_channel(self):
        counts = {MAGENTA: 6, bytes.fromhex("f010f0"): 2, WHITE: 2}

        self.assertEqual(screenshot_color.fraction_near(counts, MAGENTA, 0), 0.6)
        self.assertEqual(screenshot_color.fraction_near(counts, MAGENTA, 16), 0.8)

    def test_min_sets_the_exit_status(self):
        rows = [MAGENTA * 3, MAGENTA + WHITE + MAGENTA]
        with tempfile.TemporaryDirectory() as directory:
            png = Path(directory, "screenshot.png")
            png.write_bytes(encode(rows, 3, 3, filters=[0, 2]))
            with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(
                io.StringIO()
            ):
                passing = screenshot_color.main([str(png), "ff00ff", "--min", "0.8"])
                failing = screenshot_color.main([str(png), "ff00ff", "--min", "0.9"])

        self.assertEqual((passing, failing), (0, 1))


if __name__ == "__main__":
    unittest.main()
