"""Rasterize the app icon SVGs into the committed 1024 px PNGs and manifest.json.

Usage: python3 -I assets/app-icon/render.py [--chrome PATH]

Chrome (or chrome-headless-shell) draws each AppIcon-<appearance>.svg on a transparent
page; this script then writes the PNG form each appearance needs (see README.md):

  light   8-bit RGB, no alpha channel: the opaque icon App Store Connect accepts
  dark    8-bit RGBA: a transparent background for the system's dark background
  tinted  8-bit gray + alpha: a grayscale image the system tints

Only the Python standard library is used. scripts/ios/icons.py copies the PNGs into the
asset catalog at build time and scripts/ios/test_icons.py checks them against the SVGs'
hashes in manifest.json, so edit an SVG, run this script, and commit all three files.
"""
import argparse
import glob
import hashlib
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import zlib

HERE = Path(__file__).resolve().parent
SIZE = 1024
APPEARANCES = ("light", "dark", "tinted")
CHROME_CANDIDATES = (
    os.path.expanduser("~/.cache/ms-playwright/chromium_headless_shell-*/chrome-headless-shell-linux64/chrome-headless-shell"),
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
)
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"
CHANNELS = {0: 1, 2: 3, 4: 2, 6: 4}


def find_chrome(explicit):
    if explicit:
        return explicit
    for pattern in CHROME_CANDIDATES:
        matches = sorted(glob.glob(pattern))
        if matches:
            return matches[-1]
    for name in ("chrome-headless-shell", "chromium", "google-chrome"):
        if found := shutil.which(name):
            return found
    raise SystemExit("error: no Chrome found; pass --chrome PATH")


def chunks(data):
    if data[:8] != PNG_SIGNATURE:
        raise ValueError("not a PNG file")
    offset = 8
    while offset < len(data):
        length, kind = struct.unpack(">I4s", data[offset:offset + 8])
        payload = data[offset + 8:offset + 8 + length]
        (crc,) = struct.unpack(">I", data[offset + 8 + length:offset + 12 + length])
        if zlib.crc32(kind + payload) != crc:
            raise ValueError(f"bad CRC in {kind!r}")
        yield kind, payload
        offset += length + 12


def decode(data):
    """Return (width, height, channels, rows) for an 8-bit, non-interlaced PNG."""
    header, compressed = None, b""
    for kind, payload in chunks(data):
        if kind == b"IHDR":
            header = struct.unpack(">IIBBBBB", payload)
        elif kind == b"IDAT":
            compressed += payload
    width, height, depth, color, _, _, interlace = header
    if depth != 8 or interlace or color not in CHANNELS:
        raise ValueError(f"unsupported PNG form depth={depth} color={color} interlace={interlace}")
    bpp = CHANNELS[color]
    raw = zlib.decompress(compressed)
    stride = width * bpp
    rows, previous = [], bytearray(stride)
    for y in range(height):
        start = y * (stride + 1)
        kind, line = raw[start], bytearray(raw[start + 1:start + 1 + stride])
        for i in range(stride):
            a = line[i - bpp] if i >= bpp else 0
            b = previous[i]
            c = previous[i - bpp] if i >= bpp else 0
            if kind == 1:
                line[i] = (line[i] + a) & 255
            elif kind == 2:
                line[i] = (line[i] + b) & 255
            elif kind == 3:
                line[i] = (line[i] + (a + b) // 2) & 255
            elif kind == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                line[i] = (line[i] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
            elif kind != 0:
                raise ValueError(f"unknown PNG filter {kind}")
        rows.append(line)
        previous = line
    return width, height, bpp, rows


def encode(width, height, color, rows):
    def chunk(kind, payload):
        return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", zlib.crc32(kind + payload))

    raw = b"".join(b"\x00" + bytes(row) for row in rows)
    return (PNG_SIGNATURE + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, color, 0, 0, 0))
            + chunk(b"sRGB", b"\x00") + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))


def convert(appearance, rgba_rows):
    """Turn Chrome's RGBA rows into the PNG form the appearance needs."""
    out = []
    for row in rgba_rows:
        pixels = [row[i:i + 4] for i in range(0, len(row), 4)]
        if appearance == "light":
            if any(p[3] != 255 for p in pixels):
                raise ValueError("the light icon must cover every pixel (no transparency)")
            out.append(bytearray(v for p in pixels for v in p[:3]))
        elif appearance == "dark":
            out.append(bytearray(row))
        else:
            line = bytearray()
            for r, g, b, a in pixels:
                if a and max(r, g, b) - min(r, g, b) > 1:
                    raise ValueError("the tinted icon must be grayscale")
                line += bytes((round((r + g + b) / 3) if a else 0, a))
            out.append(line)
    return {"light": 2, "dark": 6, "tinted": 4}[appearance], out


def screenshot(chrome, svg, work):
    target = work / (svg.stem + ".png")
    subprocess.run([chrome, "--headless", "--disable-gpu", "--no-sandbox", "--hide-scrollbars",
                    "--force-device-scale-factor=1", f"--window-size={SIZE},{SIZE}",
                    "--default-background-color=00000000", f"--screenshot={target}", svg.as_uri()],
                   check=True, capture_output=True)
    return target.read_bytes()


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--chrome", help="Chrome or chrome-headless-shell executable")
    args = parser.parse_args()
    chrome = find_chrome(args.chrome)
    version = subprocess.run([chrome, "--version"], check=True, capture_output=True, text=True).stdout.strip()
    manifest = {"size": SIZE, "renderer": version, "images": {}}
    with tempfile.TemporaryDirectory(prefix="yebisu-app-icon-") as temporary:
        for appearance in APPEARANCES:
            svg = HERE / f"AppIcon-{appearance}.svg"
            width, height, bpp, rows = decode(screenshot(chrome, svg, Path(temporary)))
            if (width, height) != (SIZE, SIZE) or bpp not in (3, 4):
                raise ValueError(f"{svg.name}: Chrome returned {width}x{height} with {bpp} channels")
            if bpp == 3:
                # Chrome drops the alpha channel when every pixel is opaque.
                rows = [bytearray(v for i in range(0, len(row), 3) for v in (*row[i:i + 3], 255))
                        for row in rows]
            color, converted = convert(appearance, rows)
            png = encode(SIZE, SIZE, color, converted)
            (HERE / f"AppIcon-{appearance}.png").write_bytes(png)
            manifest["images"][appearance] = {
                "svg": svg.name, "svg_sha256": hashlib.sha256(svg.read_bytes()).hexdigest(),
                "png": f"AppIcon-{appearance}.png", "png_sha256": hashlib.sha256(png).hexdigest()}
            print(f"{appearance}: {svg.name} -> AppIcon-{appearance}.png ({len(png)} bytes)")
    (HERE / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
