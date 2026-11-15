"""Icon fixture data for offline distribution tests; Assets.car is a stub."""
from pathlib import Path

ICON_INFO = {'CFBundleIcons~ipad': {'CFBundlePrimaryIcon': {
    'CFBundleIconName': 'AppIcon', 'CFBundleIconFiles': ['AppIcon76x76']}}}
ICON_FILES = {'Assets.car': b'fixture catalog, not compiled',
              'AppIcon76x76@2x~ipad.png': b'fixture icon, not rendered'}


def add_icons(app):
    for name, content in ICON_FILES.items():
        (Path(app) / name).write_bytes(content)
