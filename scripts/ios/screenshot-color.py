#!/usr/bin/env python3
"""Report what fraction of a PNG's pixels are close to one color.

Checks simulator screenshots from scripts/ios/sim.sh with the standard library
alone (no Pillow): 8-bit RGB and RGBA PNGs are decoded with zlib.

    python3 -I scripts/ios/screenshot-color.py SCREENSHOT.png ff00ff --min 0.5

Prints the fraction and the most common colors; exits 1 when --min is given
and the fraction is below it.
"""

import argparse
import struct
import sys
import zlib
from collections import Counter

SIGNATURE = b"\x89PNG\r\n\x1a\n"
# Color type to channels, for the 8-bit types screenshots use.
CHANNELS = {2: 3, 6: 4}


class PngError(ValueError):
    pass


def read_chunks(data):
    if not data.startswith(SIGNATURE):
        raise PngError("not a PNG file")
    position = len(SIGNATURE)
    while position < len(data):
        (length,) = struct.unpack(">I", data[position : position + 4])
        kind = data[position + 4 : position + 8]
        yield kind, data[position + 8 : position + 8 + length]
        position += 12 + length
        if kind == b"IEND":
            return
    raise PngError("no IEND chunk")


def paeth(left, up, up_left):
    estimate = left + up - up_left
    distances = (abs(estimate - left), abs(estimate - up), abs(estimate - up_left))
    if distances[0] <= distances[1] and distances[0] <= distances[2]:
        return left
    return up if distances[1] <= distances[2] else up_left


def unfilter(data, width, height, channels):
    """Undoes the per-row filters (PNG spec, section 9) into raw rows."""
    stride = width * channels
    if len(data) < height * (stride + 1):
        raise PngError("image data is shorter than its header says")
    previous = bytearray(stride)
    rows = []
    position = 0
    for _ in range(height):
        kind = data[position]
        row = bytearray(data[position + 1 : position + 1 + stride])
        position += 1 + stride
        if kind == 1:
            for i in range(channels, stride):
                row[i] = (row[i] + row[i - channels]) & 0xFF
        elif kind == 2:
            row = bytearray((a + b) & 0xFF for a, b in zip(row, previous))
        elif kind == 3:
            for i in range(stride):
                left = row[i - channels] if i >= channels else 0
                row[i] = (row[i] + ((left + previous[i]) >> 1)) & 0xFF
        elif kind == 4:
            for i in range(stride):
                left = row[i - channels] if i >= channels else 0
                up_left = previous[i - channels] if i >= channels else 0
                row[i] = (row[i] + paeth(left, previous[i], up_left)) & 0xFF
        elif kind != 0:
            raise PngError(f"unknown row filter {kind}")
        rows.append(row)
        previous = row
    return rows


def decode(data):
    """Returns display-oriented (width, height, channels, rows) of a PNG."""
    header = None
    orientation = 1
    compressed = bytearray()
    for kind, body in read_chunks(data):
        if kind == b"IHDR":
            header = struct.unpack(">IIBBBBB", body)
        elif kind == b"IDAT":
            compressed += body
        elif kind == b"eXIf":
            orientation = exif_orientation(body)
    if header is None:
        raise PngError("no IHDR chunk")
    width, height, depth, color_type, _, _, interlace = header
    if depth != 8 or color_type not in CHANNELS or interlace != 0:
        raise PngError(
            f"unsupported PNG: bit depth {depth}, color type {color_type}, "
            f"interlace {interlace} (need 8-bit RGB or RGBA, not interlaced)"
        )
    channels = CHANNELS[color_type]
    rows = unfilter(zlib.decompress(bytes(compressed)), width, height, channels)
    # XCTest landscape captures may retain portrait IHDR dimensions and express
    # rotation solely in eXIf. Paint receipts use the displayed viewport.
    if orientation in (2, 3, 4):
        if orientation in (3, 4):
            rows = rows[::-1]
        if orientation in (2, 3):
            rows = [bytearray().join(row[x:x+channels] for x in range(
                (width-1)*channels, -1, -channels)) for row in rows]
    elif orientation in (5, 6, 7, 8):
        source = rows if orientation in (5, 8) else rows[::-1]
        columns = range(width) if orientation in (5, 6) else range(width-1, -1, -1)
        rows = [bytearray().join(row[x*channels:(x+1)*channels] for row in source)
                for x in columns]
        width, height = height, width
    return width, height, channels, rows


def exif_orientation(body):
    """Read the TIFF IFD0 orientation from PNG's eXIf chunk; fail on corruption."""
    try:
        endian = {b"MM": ">", b"II": "<"}[body[:2]]
        if struct.unpack_from(endian + "H", body, 2)[0] != 42:
            raise PngError("invalid eXIf TIFF header")
        offset = struct.unpack_from(endian + "I", body, 4)[0]
        count = struct.unpack_from(endian + "H", body, offset)[0]
        for index in range(count):
            entry = offset + 2 + index * 12
            tag, kind, size = struct.unpack_from(endian + "HHI", body, entry)
            if tag == 0x0112:
                if kind != 3 or size != 1:
                    raise PngError("invalid eXIf orientation field")
                orientation = struct.unpack_from(endian + "H", body, entry + 8)[0]
                if orientation not in range(1, 9):
                    raise PngError("invalid eXIf orientation value")
                return orientation
    except (KeyError, struct.error) as error:
        raise PngError("invalid eXIf data") from error
    return 1


def color_counts(rows, channels):
    counts = Counter()
    for row in rows:
        for i in range(0, len(row), channels):
            counts[bytes(row[i : i + 3])] += 1
    return counts


def fraction_near(counts, color, tolerance):
    total = sum(counts.values())
    near = sum(
        count
        for pixel, count in counts.items()
        if all(abs(a - b) <= tolerance for a, b in zip(pixel, color))
    )
    return near / total if total else 0.0


def parse_color(text):
    text = text.removeprefix("#")
    if len(text) != 6:
        raise argparse.ArgumentTypeError(f"'{text}' is not an RRGGBB color")
    try:
        return bytes.fromhex(text)
    except ValueError as error:
        raise argparse.ArgumentTypeError(f"'{text}' is not an RRGGBB color") from error


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("png")
    parser.add_argument("color", type=parse_color, help="RRGGBB, for example ff00ff")
    parser.add_argument(
        "--tolerance",
        type=int,
        default=24,
        help="largest per-channel difference that still matches (default 24)",
    )
    parser.add_argument("--min", type=float, help="fail below this fraction")
    arguments = parser.parse_args(argv)

    with open(arguments.png, "rb") as file:
        width, height, channels, rows = decode(file.read())
    counts = color_counts(rows, channels)
    fraction = fraction_near(counts, arguments.color, arguments.tolerance)
    total = width * height
    print(
        f"{fraction:.4f} of {width}x{height} pixels are within {arguments.tolerance} "
        f"of #{arguments.color.hex()}"
    )
    for pixel, count in counts.most_common(3):
        print(f"  #{pixel.hex()}: {count / total:.4f}")
    if arguments.min is not None and fraction < arguments.min:
        print(f"error: below the required {arguments.min}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
