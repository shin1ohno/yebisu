"""Actual macOS actool integration, without signing (artwork contracts: test_icons.py)."""
import importlib.util
import json
from pathlib import Path
import plistlib
import sys
import tempfile
import unittest
import zipfile

SPEC = importlib.util.spec_from_file_location('distribution', Path(__file__).with_name('distribution.py'))
d = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(d)
icons = d.load_helper('icons.py')


class IconTests(unittest.TestCase):
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
            # The real Xcode probes, in build order: SDK keys, then icons.
            d.stamp_sdk_metadata(app, d.sdk_metadata())
            icons.compile_icons(app)
            compiled = plistlib.loads((app / 'Info.plist').read_bytes())
            self.assertEqual(compiled['CFBundleIdentifier'], d.BUNDLE)
            self.assertEqual(compiled['DTPlatformName'], 'iphoneos')
            self.assertEqual(compiled['DTSDKName'], 'iphoneos' + compiled['DTPlatformVersion'])
            self.assertRegex(compiled['DTXcode'], r'\A[0-9]{4}\Z')
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
