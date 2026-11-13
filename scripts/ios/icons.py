"""Render an opaque Y terminal icon and compile it with Apple's asset compiler."""
import json
from pathlib import Path
import plistlib
import struct
import subprocess
import tempfile
import zlib


def png(size):
    """Dependency-free RGB artwork: cream Y on a dark green square."""
    rows = bytearray()
    for y in range(size):
        rows.append(0)
        for x in range(size):
            u, v = (x + .5) / size, (y + .5) / size
            stem = .44 <= u <= .56 and .47 <= v <= .77
            arms = .23 <= v <= .53 and abs(abs(u - .5) - (.53 - v) * .65) <= .065
            rows.extend((244, 229, 187) if stem or arms else (18, 58, 49))

    def chunk(kind, data):
        return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))

    return (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', size, size, 8, 2, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress(bytes(rows))) + chunk(b'IEND', b''))


def catalog(root):
    assets = root / 'Icons.xcassets'
    icons = assets / 'AppIcon.appiconset'
    icons.mkdir(parents=True)
    images = []
    # Explicit iPad slots work with the minimum supported iPadOS 17 runtime.
    for points, scales in [('20', [1, 2]), ('29', [1, 2]), ('40', [1, 2]),
                           ('76', [1, 2]), ('83.5', [2]), ('1024', [1])]:
        for scale in scales:
            size = int(float(points) * scale)
            name = f'icon-{size}.png'
            (icons / name).write_bytes(png(size))
            images.append(dict(filename=name, size=f'{points}x{points}', scale=f'{scale}x',
                               idiom='ios-marketing' if points == '1024' else 'ipad'))
    info = dict(version=1, author='yebisu')
    (assets / 'Contents.json').write_text(json.dumps(dict(info=info)))
    (icons / 'Contents.json').write_text(json.dumps(dict(images=images, info=info)))
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


def compile_icons(app):
    info_path = app / 'Info.plist'
    info = plistlib.loads(info_path.read_bytes())
    with tempfile.TemporaryDirectory(prefix='yebisu-icons-', dir=app.parent) as temporary:
        root = Path(temporary)
        partial = root / 'icons.plist'
        subprocess.run(['xcrun', 'actool', str(catalog(root)), '--compile', str(app),
                        '--platform', 'iphoneos', '--minimum-deployment-target', info['MinimumOSVersion'],
                        '--target-device', 'ipad', '--app-icon', 'AppIcon',
                        '--output-partial-info-plist', str(partial)], check=True, capture_output=True)
        compiled = plistlib.loads(partial.read_bytes())
        # Only icon metadata is imported; identity/version stay with the bundle helper.
        for key in ('CFBundleIcons', 'CFBundleIcons~ipad'):
            if key in compiled:
                info[key] = compiled[key]
        validate(info, {path.name for path in app.iterdir() if path.is_file()})
        info_path.write_bytes(plistlib.dumps(info))
