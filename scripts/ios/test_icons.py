"""The app icon catalog: one 1024 px icon in light, dark and tinted, in the form Apple asks for.

Apple's "Configuring your app icon" (Xcode documentation) for iOS and iPadOS 18 and later:
the default (light) icon is opaque, the dark icon has a transparent background so the
system-provided background shows through, and the tinted icon is a grayscale image.
"""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import plistlib
import struct
import subprocess
import sys
import tempfile
import textwrap
import unittest
import zlib


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
ARTWORK = ROOT / 'assets/app-icon'
icons = load('ios_icons', HERE / 'icons.py')
render = load('app_icon_render', ARTWORK / 'render.py')


def pixels(path):
    """Decoded rows and channel count of a committed artwork PNG."""
    width, height, channels, rows = render.decode(path.read_bytes())
    return width, height, channels, rows


class CatalogTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix='yebisu-icons-test-')
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)

    def images(self):
        directory = icons.catalog(self.root) / 'AppIcon.appiconset'
        return directory, json.loads((directory / 'Contents.json').read_text())['images']

    def test_catalog_is_one_1024_icon_in_light_dark_and_tinted(self):
        directory, images = self.images()
        self.assertEqual(len(images), 3, images)
        appearances = set()
        for image in images:
            self.assertEqual((image['idiom'], image['platform'], image['size']), ('universal', 'ios', '1024x1024'))
            values = image.get('appearances', [])
            self.assertLessEqual(len(values), 1)
            if values:
                self.assertEqual(values[0]['appearance'], 'luminosity')
            appearances.add(values[0]['value'] if values else 'light')
            width, height, depth, _, _ = icons.png_form((directory / image['filename']).read_bytes())
            self.assertEqual((width, height, depth), (1024, 1024, 8))
        self.assertEqual(appearances, {'light', 'dark', 'tinted'})

    def test_light_icon_is_opaque_rgb_without_alpha(self):
        directory, images = self.images()
        light = next(i for i in images if 'appearances' not in i)
        _, _, _, color, transparency = icons.png_form((directory / light['filename']).read_bytes())
        self.assertEqual((color, transparency), (2, False), 'RGB without an alpha channel or tRNS')
        _, _, channels, rows = pixels(directory / light['filename'])
        self.assertEqual(channels, 3)
        # The background fills the corners (the system rounds them), in the canvas gradient.
        self.assertEqual(tuple(rows[0][:3]), (28, 28, 34))
        self.assertEqual(tuple(rows[-1][-3:]), (16, 20, 25))

    def test_dark_icon_keeps_the_shapes_on_a_transparent_background(self):
        _, _, channels, dark = pixels(ARTWORK / 'AppIcon-dark.png')
        _, _, _, light = pixels(ARTWORK / 'AppIcon-light.png')
        self.assertEqual(channels, 4)
        self.assertEqual(dark[0][3], 0, 'top-left corner is transparent')
        self.assertEqual(dark[512][512 * 4 + 3], 0, 'the space inside the crook is transparent')
        for x, y in [(462, 600), (806, 600), (270, 560), (806, 772)]:
            self.assertEqual(dark[y][x * 4 + 3], 255)
            self.assertEqual(dark[y][x * 4:x * 4 + 3], light[y][x * 3:x * 3 + 3], 'same colors as light')

    def test_tinted_icon_is_grayscale_with_the_rod_brightest(self):
        _, _, channels, tinted = pixels(ARTWORK / 'AppIcon-tinted.png')
        self.assertEqual(channels, 2, 'gray + alpha')
        self.assertEqual(tinted[0][1], 0, 'transparent background')
        rod = tinted[600][462 * 2]
        others = [tinted[y][x * 2] for x, y in [(806, 600), (270, 560), (158, 742), (326, 786), (806, 772)]]
        self.assertEqual(rod, 255)
        self.assertTrue(all(0 < value < rod for value in others), others)
        self.assertEqual(len({tinted[y][x * 2] for x, y in [(270, 560), (158, 742), (326, 786)]}), 3,
                         'the three herd dots stay distinguishable')

    def test_committed_pngs_match_their_svg_sources(self):
        manifest = json.loads((ARTWORK / 'manifest.json').read_text())
        self.assertEqual(set(manifest['images']), {'light', 'dark', 'tinted'})
        for appearance, entry in manifest['images'].items():
            with self.subTest(appearance=appearance):
                self.assertEqual(hashlib.sha256((ARTWORK / entry['svg']).read_bytes()).hexdigest(),
                                 entry['svg_sha256'], 'SVG changed: run assets/app-icon/render.py')
                self.assertEqual(hashlib.sha256((ARTWORK / entry['png']).read_bytes()).hexdigest(),
                                 entry['png_sha256'], 'PNG differs from what render.py wrote')

    def test_wrong_artwork_forms_are_refused(self):
        def png(color, size=1024, transparency=False):
            channels = render.CHANNELS[color]
            data = render.encode(size, size, color, [bytearray(size * channels)] * size)
            if transparency:
                # A valid tRNS chunk (gray/RGB key color 0) before IEND.
                payload = bytes(6)
                chunk = (struct.pack('>I', len(payload)) + b'tRNS' + payload
                         + struct.pack('>I', zlib.crc32(b'tRNS' + payload)))
                data = data[:-12] + chunk + data[-12:]
                self.assertEqual(icons.png_form(data)[4], True)
            return data

        good = {'light': png(2), 'dark': png(6), 'tinted': png(4)}
        bad = {'light with alpha': ('light', png(6)), 'light with tRNS': ('light', png(2, transparency=True)),
               'light at 512': ('light', png(2, 512)), 'dark without alpha': ('dark', png(2)),
               'tinted in color': ('tinted', png(6))}
        for label, (appearance, data) in bad.items():
            artwork = self.root / label
            artwork.mkdir()
            for name, content in {**good, appearance: data}.items():
                (artwork / f'AppIcon-{name}.png').write_bytes(content)
            with self.subTest(label), self.assertRaises(ValueError):
                icons.check_artwork(artwork)
        artwork = self.root / 'good'
        artwork.mkdir()
        for name, content in good.items():
            (artwork / f'AppIcon-{name}.png').write_bytes(content)
        icons.check_artwork(artwork)


class InfoTests(unittest.TestCase):
    def test_info_template_names_the_catalog_icon_for_ipad(self):
        template = plistlib.loads((HERE / 'Info.plist').read_bytes())
        self.assertEqual(icons.icon_name(template), 'AppIcon')
        for broken in [{}, {'CFBundleIcons~ipad': []}, {'CFBundleIcons~ipad': {'CFBundlePrimaryIcon': {}}}]:
            with self.subTest(broken=broken), self.assertRaises(ValueError):
                icons.icon_name(broken)

    def test_check_bundle_requires_the_compiled_icon(self):
        with tempfile.TemporaryDirectory() as temporary:
            app = Path(temporary) / 'fixture.app'
            app.mkdir()
            info = {'CFBundleIcons~ipad': {'CFBundlePrimaryIcon': {
                'CFBundleIconName': 'AppIcon', 'CFBundleIconFiles': ['AppIcon76x76']}}}
            (app / 'Info.plist').write_bytes(plistlib.dumps(info))
            with self.assertRaisesRegex(ValueError, 'Assets.car'):
                icons.check_bundle(app)
            (app / 'Assets.car').write_bytes(b'fixture')
            with self.assertRaisesRegex(ValueError, 'PNG'):
                icons.check_bundle(app)
            (app / 'AppIcon76x76@2x~ipad.png').write_bytes(b'fixture')
            icons.check_bundle(app)


@unittest.skipUnless(sys.platform == 'darwin', 'bundling uses macOS plutil and actool')
class BundleIconTests(unittest.TestCase):
    """bundle.sh compiles the icon into every yebisu bundle, with fake cargo and codesign."""

    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix='yebisu-icon-bundle-')
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        tools = self.root / 'tools'
        tools.mkdir()
        self.executable = self.root / 'build' / 'yebisu'
        self.executable.parent.mkdir()
        self.executable.write_bytes(b'isolated test executable')
        fakes = {
            'cargo': '''
                import json, os, sys
                args = sys.argv[1:]
                kind = "example" if "--example" in args else "bin"
                print(json.dumps({"reason": "compiler-artifact", "executable": os.environ["YEBISU_ICON_TEST_BINARY"],
                                  "target": {"name": args[args.index("--" + kind) + 1], "kind": [kind]},
                                  "package_id": "path+file:///isolated#yebisu@0.1.0"}))
            ''',
            'codesign': 'pass\n',
        }
        for name, body in fakes.items():
            path = tools / name
            path.write_text('#!' + sys.executable + '\n' + textwrap.dedent(body))
            path.chmod(0o755)
        self.env = {key: value for key, value in os.environ.items() if not key.startswith('HERDR_IOS_')}
        self.env.update(PATH=f'{tools}{os.pathsep}{os.environ["PATH"]}', TMPDIR=str(self.root),
                        YEBISU_ICON_TEST_BINARY=str(self.executable))

    def bundle(self, *args):
        return subprocess.run(['bash', str(HERE / 'bundle.sh'), *args], cwd=ROOT, env=self.env,
                              capture_output=True, text=True, check=False)

    def test_development_and_distribution_bundles_carry_the_icon(self):
        for target, status in [('aarch64-apple-ios-sim', 0), ('aarch64-apple-ios', 3)]:
            with self.subTest(target=target):
                result = self.bundle('--package', 'yebisu', '--bin', 'yebisu', '--target', target)
                self.assertEqual(result.returncode, status, result.stderr)
                app = Path(result.stdout.strip().splitlines()[-1])
                info = plistlib.loads((app / 'Info.plist').read_bytes())
                self.assertGreater((app / 'Assets.car').stat().st_size, 0)
                self.assertTrue(icons.icon_files(info))
                icons.check_bundle(app)

    def test_examples_do_not_name_an_icon(self):
        result = self.bundle('--example', 'uikit_smoke')
        self.assertEqual(result.returncode, 0, result.stderr)
        app = Path(result.stdout.strip().splitlines()[-1])
        self.assertNotIn('CFBundleIcons~ipad', plistlib.loads((app / 'Info.plist').read_bytes()))
        self.assertFalse((app / 'Assets.car').exists())


if __name__ == '__main__':
    unittest.main()
