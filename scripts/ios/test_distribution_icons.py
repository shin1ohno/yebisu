"""Artwork contracts and actual macOS actool integration, without signing."""
import importlib.util
import json
from pathlib import Path
import plistlib
import struct
import sys
import tempfile
import unittest
import zipfile
import zlib

SPEC = importlib.util.spec_from_file_location('distribution', Path(__file__).with_name('distribution.py'))
d = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(d)
icons = d.load_helper('icons.py')


class IconTests(unittest.TestCase):
    def test_catalog_has_opaque_marketing_and_ipad_icons(self):
        with tempfile.TemporaryDirectory() as temporary:
            catalog = icons.catalog(Path(temporary))
            directory = catalog / 'AppIcon.appiconset'
            images = json.loads((directory / 'Contents.json').read_text())['images']
            self.assertTrue(any(i['idiom'] == 'ios-marketing' and i['size'] == '1024x1024' for i in images))
            self.assertTrue(any(i['size'] == '83.5x83.5' and i['scale'] == '2x' for i in images))
            for image in images:
                data = (directory / image['filename']).read_bytes()
                self.assertEqual(data[:8], b'\x89PNG\r\n\x1a\n')
                size = int(float(image['size'].split('x')[0]) * int(image['scale'][0]))
                self.assertEqual(struct.unpack('>IIBB', data[16:26]), (size, size, 8, 2))
                offset, compressed = 8, b''
                while offset < len(data):
                    length = struct.unpack('>I', data[offset:offset+4])[0]
                    kind = data[offset+4:offset+8]
                    payload = data[offset+8:offset+8+length]
                    self.assertEqual(zlib.crc32(kind + payload), struct.unpack('>I', data[offset+8+length:offset+12+length])[0])
                    if kind == b'IDAT':
                        compressed += payload
                    offset += length + 12
                self.assertEqual(len(zlib.decompress(compressed)), size * (1 + size * 3))

    @unittest.skipUnless(sys.platform == 'darwin', 'requires actual Apple actool')
    def test_real_actool_bundle_and_ipa_with_missing_icon_negatives(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            app = root / 'fixture.app'
            app.mkdir()
            info = plistlib.loads((d.ROOT / 'scripts/ios/Info.plist').read_bytes())
            info.update(CFBundleIdentifier=d.BUNDLE, CFBundleSupportedPlatforms=['iPhoneOS'],
                        CFBundleVersion='1', CFBundleShortVersionString='1.0')
            (app / 'Info.plist').write_bytes(plistlib.dumps(info))
            icons.compile_icons(app)
            compiled = plistlib.loads((app / 'Info.plist').read_bytes())
            self.assertEqual(compiled['CFBundleIdentifier'], d.BUNDLE)
            self.assertGreater((app / 'Assets.car').stat().st_size, 0)
            self.assertIn('CFBundleIcons~ipad', compiled)
            ipa = root / 'fixture.ipa'
            with zipfile.ZipFile(ipa, 'w') as archive:
                for file in app.iterdir():
                    if file.is_file():
                        archive.write(file, 'Payload/fixture.app/' + file.name)
            self.assertEqual(d.ipa_metadata(ipa)['CFBundleIdentifier'], d.BUNDLE)
            for missing in ('Assets.car', next(app.glob('AppIcon*.png')).name):
                names = {p.name for p in app.iterdir() if p.is_file()} - {missing}
                # Remove every scale of this icon for the PNG negative.
                if missing.endswith('.png'):
                    names = {n for n in names if not n.startswith('AppIcon')}
                with self.subTest(missing=missing), self.assertRaises(ValueError):
                    icons.validate(compiled, names)


if __name__ == '__main__':
    unittest.main()
