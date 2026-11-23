"""Build yebisu's app icon asset catalog and compile it into a bundle with Apple's actool.

The artwork lives in assets/app-icon/: the SVG sources, the 1024 px PNGs rendered from
them, and render.py, which regenerates the PNGs. This module copies the PNGs into a
single-size iOS app icon set with the three appearances iPadOS 18 and later use (the
opaque light icon, the dark icon on a transparent background, and the grayscale tinted
icon) and compiles it. scripts/ios/bundle.sh runs it for every yebisu bundle before
signing, so development and distribution builds carry the same icon.

Usage: python3 -I scripts/ios/icons.py --app PATH --platform iphoneos|iphonesimulator [--scratch DIR]
"""
import argparse
import json
from pathlib import Path
import plistlib
import shutil
import struct
import subprocess
import sys
import tempfile
import zlib

ROOT = Path(__file__).resolve().parents[2]
ARTWORK = ROOT / 'assets/app-icon'
SIZE = 1024
PLATFORMS = ('iphoneos', 'iphonesimulator')
# Appearance, the asset catalog luminosity value (None for the default, light icon), and
# the PNG color type: 2 RGB without alpha (opaque), 6 RGBA, 4 gray with alpha.
APPEARANCES = (('light', None, 2), ('dark', 'dark', 6), ('tinted', 'tinted', 4))


def png_form(data):
    """Return (width, height, bit depth, color type, has tRNS) of PNG bytes, checking CRCs."""
    if data[:8] != b'\x89PNG\r\n\x1a\n':
        raise ValueError('app icon artwork is not a PNG file')
    offset, header, transparency = 8, None, False
    while offset < len(data):
        length, kind = struct.unpack('>I4s', data[offset:offset + 8])
        payload = data[offset + 8:offset + 8 + length]
        if zlib.crc32(kind + payload) != struct.unpack('>I', data[offset + 8 + length:offset + 12 + length])[0]:
            raise ValueError('app icon artwork has a corrupt PNG chunk')
        if kind == b'IHDR':
            header = struct.unpack('>IIBB', payload[:10])
        transparency |= kind == b'tRNS'
        offset += length + 12
    if header is None:
        raise ValueError('app icon artwork has no PNG header')
    return (*header, transparency)


def check_artwork(artwork=ARTWORK):
    """Require each appearance's 1024 px PNG in the form Apple asks for."""
    for appearance, _, color in APPEARANCES:
        path = artwork / f'AppIcon-{appearance}.png'
        width, height, depth, actual, transparency = png_form(path.read_bytes())
        if (width, height, depth) != (SIZE, SIZE, 8):
            raise ValueError(f'{path.name} must be an 8-bit {SIZE}x{SIZE} PNG')
        if actual != color or transparency:
            raise ValueError(f'{path.name} must have PNG color type {color} and no tRNS chunk')


def icon_name(info):
    """The primary icon name the Info.plist template declares for the catalog."""
    group = info.get('CFBundleIcons~ipad') if isinstance(info, dict) else None
    primary = group.get('CFBundlePrimaryIcon') if isinstance(group, dict) else None
    name = primary.get('CFBundleIconName') if isinstance(primary, dict) else None
    if not isinstance(name, str) or not name:
        raise ValueError('Info.plist must declare CFBundleIcons~ipad CFBundlePrimaryIcon CFBundleIconName')
    return name


def catalog(root, artwork=ARTWORK, name='AppIcon'):
    """Write Icons.xcassets with one single-size 1024 icon in light, dark and tinted."""
    check_artwork(artwork)
    assets = root / 'Icons.xcassets'
    icons = assets / f'{name}.appiconset'
    icons.mkdir(parents=True)
    images = []
    for appearance, luminosity, _ in APPEARANCES:
        filename = f'AppIcon-{appearance}.png'
        shutil.copyfile(artwork / filename, icons / filename)
        image = dict(filename=filename, idiom='universal', platform='ios', size=f'{SIZE}x{SIZE}')
        if luminosity:
            image['appearances'] = [dict(appearance='luminosity', value=luminosity)]
        images.append(image)
    info = dict(version=1, author='yebisu')
    (assets / 'Contents.json').write_text(json.dumps(dict(info=info)))
    (icons / 'Contents.json').write_text(json.dumps(dict(images=images, info=info), indent=2))
    return assets


def icon_files(info):
    if not isinstance(info, dict):
        raise ValueError('compiled AppIcon plist must be a dictionary')
    files = []
    for key in ('CFBundleIcons', 'CFBundleIcons~ipad'):
        group = info.get(key, {})
        primary = group.get('CFBundlePrimaryIcon', {}) if isinstance(group, dict) else {}
        if isinstance(primary, dict) and primary.get('CFBundleIconName') == 'AppIcon':
            entries = primary.get('CFBundleIconFiles', [])
            if not isinstance(entries, list):
                raise ValueError('compiled AppIcon file entries must be a list')
            files.extend(entries)
    if not files or any(not isinstance(name, str) or Path(name).name != name for name in files):
        raise ValueError('compiled AppIcon plist entries are missing')
    return files


def validate(info, names):
    """Require the compiler's catalog and each referenced icon in a bundle/IPA."""
    if 'Assets.car' not in names:
        raise ValueError('compiled AppIcon Assets.car is missing')
    for name in icon_files(info):
        stem = name.removesuffix('.png')
        if not any(item == stem + '.png' or item.startswith(stem + '@') and item.endswith('.png')
                   for item in names):
            raise ValueError('compiled AppIcon PNG is missing')


def check_bundle(app):
    """Require the icon bundle.sh compiled into an assembled bundle."""
    info = plistlib.loads((app / 'Info.plist').read_bytes())
    icon_name(info)
    validate(info, {path.name for path in app.iterdir() if path.is_file()})


def compile_icons(app, platform='iphoneos', scratch=None):
    if platform not in PLATFORMS:
        raise ValueError(f'unknown actool platform {platform}')
    info_path = app / 'Info.plist'
    info = plistlib.loads(info_path.read_bytes())
    name = icon_name(info)
    # The catalog and actool's partial plist go to the caller's scratch directory (bundle.sh
    # passes its own), never into the bundle.
    with tempfile.TemporaryDirectory(prefix='yebisu-icons-', dir=scratch or app.parent) as temporary:
        root = Path(temporary)
        partial = root / 'icons.plist'
        command = ['xcrun', 'actool', str(catalog(root, name=name)), '--compile', str(app),
                   '--platform', platform, '--minimum-deployment-target', info['MinimumOSVersion'],
                   '--target-device', 'ipad', '--app-icon', name,
                   '--output-partial-info-plist', str(partial)]
        result = subprocess.run(command, capture_output=True, text=True, check=False)
        if result.returncode:
            # actool reports its errors on stdout as a plist; keep stdout free for callers.
            sys.stderr.write(result.stdout + result.stderr)
            raise subprocess.CalledProcessError(result.returncode, command)
        compiled = plistlib.loads(partial.read_bytes())
        # Only icon metadata is imported; identity/version stay with the bundle helper.
        for key in ('CFBundleIcons', 'CFBundleIcons~ipad'):
            if key in compiled:
                info[key] = compiled[key]
        validate(info, {path.name for path in app.iterdir() if path.is_file()})
        info_path.write_bytes(plistlib.dumps(info))


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--app', type=Path, required=True, help='the .app bundle to add the icon to')
    parser.add_argument('--platform', choices=PLATFORMS, required=True)
    parser.add_argument('--scratch', type=Path, help='directory for the catalog (default: beside the app)')
    args = parser.parse_args(argv)
    try:
        compile_icons(args.app, args.platform, args.scratch)
    except (ValueError, OSError, KeyError, subprocess.CalledProcessError, plistlib.InvalidFileException) as error:
        print(f'error: app icon: {error}', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
