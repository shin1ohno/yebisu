# yebisu app icon

D1a "deep-hook thick rod" (the owner's pick, 2026-11-23): a shepherd's crook that is also
a fishing rod, with its line, a red float, and a herd of three dots. Colors are the
canvas palette only; there is no text.

| Appearance | Source | Committed PNG | Form (Apple, iPadOS 18 and later) |
|---|---|---|---|
| light (default) | `AppIcon-light.svg` | `AppIcon-light.png` | opaque 1024 px, 8-bit RGB without alpha; the background fills every pixel and iPadOS rounds the corners |
| dark | `AppIcon-dark.svg` | `AppIcon-dark.png` | 1024 px RGBA with a transparent background, so the system-provided dark background shows through; same shapes and colors as light |
| tinted | `AppIcon-tinted.svg` | `AppIcon-tinted.png` | 1024 px gray + alpha with a transparent background; the system tints the grayscale (lighter gray, stronger tint) |

Apple: "Provide your tinted app icon as a grayscale image. Provide your dark app icon
with a transparent background so the system-provided background can show through."
([Configuring your app icon](https://developer.apple.com/documentation/xcode/configuring-your-app-icon)).
The system derives the iPadOS 26 clear appearance and every smaller size from these.

## Regenerate the PNGs

Edit an SVG, then run, from the repository root:

    python3 -I assets/app-icon/render.py [--chrome PATH]

It needs Chrome or chrome-headless-shell (it looks for the Playwright headless shell,
then Google Chrome or Chromium on macOS, then `PATH`) and only the Python standard
library. It writes the three PNGs and `manifest.json` (the SVG and PNG SHA-256 and the
renderer version); commit them together. `scripts/ios/test_icons.py` fails when an SVG
no longer matches the manifest or a PNG is not in the form above.

## How builds use it

`scripts/ios/icons.py` copies the PNGs into a single-size `AppIcon` set with the
`dark` and `tinted` luminosity appearances and compiles it with `xcrun actool` into
`Assets.car` and the PNGs older iPadOS versions read (`CFBundleIconFiles`).
`scripts/ios/bundle.sh` runs it for every yebisu bundle before signing, so Simulator,
device development and distribution builds carry the same icon. `scripts/ios/Info.plist`
names the icon (`CFBundleIcons~ipad` / `CFBundlePrimaryIcon` / `CFBundleIconName`);
actool adds the file list.
`scripts/ios/test_distribution_icons.py` reads the compiled `Assets.car` with
`xcrun assetutil --info` on macOS and requires the 1024 icon in the default (opaque),
`UIAppearanceDark` and `ISAppearanceTintable` (grayscale) appearances.

## Home Screen screenshots

`docs/screenshots/w5-app-icon/` holds the iPadOS 26.5 Simulator Home Screen in the
Default (light), Dark and Tinted icon styles, and the design canvas render. The bundle
in them was assembled by `scripts/ios/bundle.sh` with the real `Info.plist`, artwork and
actool step but a stub executable, so they show the icon, not a running app.
