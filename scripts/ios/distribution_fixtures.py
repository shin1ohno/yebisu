"""Fixture data for offline distribution tests: icons (Assets.car is a stub) and Xcode SDK keys."""
from pathlib import Path

ICON_INFO = {'CFBundleIcons~ipad': {'CFBundlePrimaryIcon': {
    'CFBundleIconName': 'AppIcon', 'CFBundleIconFiles': ['AppIcon76x76']}}}
ICON_FILES = {'Assets.car': b'fixture catalog, not compiled',
              'AppIcon76x76@2x~ipad.png': b'fixture icon, not rendered'}

# What each Xcode probe prints on the Mac mini that passed validate-app, and the
# keys distribution.py build stamps from them (Xcode 27.2, iphoneos SDK 27.2).
PROBE_OUTPUT = {('xcrun', '--sdk', 'iphoneos', '--show-sdk-version'): '27.2\n',
                ('xcrun', '--sdk', 'iphoneos', '--show-sdk-build-version'): '24B8118\n',
                ('xcodebuild', '-version'): 'Xcode 27.2\nBuild version 27B52\n',
                ('sw_vers', '-buildVersion'): '26B5091g\n'}
SDK_INFO = {'BuildMachineOSBuild': '26B5091g', 'DTCompiler': 'com.apple.compilers.llvm.clang.1_0',
            'DTPlatformBuild': '24B8118', 'DTPlatformName': 'iphoneos', 'DTPlatformVersion': '27.2',
            'DTSDKBuild': '24B8118', 'DTSDKName': 'iphoneos27.2', 'DTXcode': '2720',
            'DTXcodeBuild': '27B52', 'UIRequiredDeviceCapabilities': ['arm64']}


def add_icons(app):
    for name, content in ICON_FILES.items():
        (Path(app) / name).write_bytes(content)
