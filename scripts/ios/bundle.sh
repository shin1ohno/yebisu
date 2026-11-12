#!/usr/bin/env bash
# Build a Cargo example or binary for iPadOS and assemble it into an .app with
# scripts/ios/Info.plist, linked for that template's MinimumOSVersion. A
# simulator bundle is ad-hoc signed, which is all `xcrun simctl install`
# needs. A device bundle is signed for a development team with its
# provisioning profile (scripts/ios/provision.sh), or left unsigned without one.
#
# The bundle's absolute path is the last line on stdout and everything else
# goes to stderr, so `app=$(scripts/ios/bundle.sh --example NAME)` works.
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/ios/bundle.sh [options] (--example NAME | --bin NAME)

Options:
  --package PKG    Cargo package with the example or binary (default: herdr-uikit)
  --manifest-path PATH
                   Cargo manifest (default: this repository's workspace;
                   relative paths are resolved from the caller's directory)
  --target TRIPLE  aarch64-apple-ios-sim, the simulator on Apple silicon
                   (default), or aarch64-apple-ios, a device
  --release        Build with the release profile (default: dev)
  --no-default-features
                   Disable the package's default Cargo features
  --features LIST  Enable the comma-separated Cargo features
  --bundle-id ID   CFBundleIdentifier (default: so.pen.herdr-gpui, with
                   .example.NAME appended for an example; for a device,
                   $HERDR_IOS_BUNDLE_ID replaces that default, and signing
                   needs one or the other, never the project's own)
  --team TEAM      Development team that signs a device bundle (default:
                   $HERDR_IOS_TEAM); finds or creates its profile with
                   scripts/ios/provision.sh
  --profile PATH   Provisioning profile that signs a device bundle, instead
                   of the one provision.sh finds
  -h, --help       Show this help

The bundle is written beside the built executable, as NAME.app.

For aarch64-apple-ios the bundle carries the profile as
embedded.mobileprovision and is signed with the keychain identity whose
certificate the profile lists, for entitlements derived from the profile.
Without --team or --profile it is assembled but not signed, and the script
exits with status 3.
EOF
}

package=yebisu
manifest_args=()
target=aarch64-apple-ios-sim
profile_args=()
feature_args=()
bundle_id=
team=${HERDR_IOS_TEAM:-}
signing_profile=
signing_flags=false
kind=
name=

while (($# > 0)); do
  case "$1" in
    --example | --bin)
      if [[ -n $kind ]]; then
        echo "error: give one --example or --bin" >&2
        exit 2
      fi
      kind=${1#--}
      name=${2:?"$1 needs a name"}
      shift 2
      ;;
    --package)
      package=${2:?"--package needs a name"}
      shift 2
      ;;
    --manifest-path)
      if [[ $# -lt 2 || -z $2 ]]; then
        echo "error: --manifest-path needs a path" >&2
        exit 2
      fi
      manifest_args=(--manifest-path "$2")
      shift 2
      ;;
    --target)
      target=${2:?"--target needs a triple"}
      shift 2
      ;;
    --release)
      profile_args=(--release)
      shift
      ;;
    --no-default-features)
      feature_args+=(--no-default-features)
      shift
      ;;
    --features)
      feature_args+=(--features "${2:?"--features needs a feature list"}")
      shift 2
      ;;
    --bundle-id)
      bundle_id=${2:?"--bundle-id needs an identifier"}
      shift 2
      ;;
    --team)
      team=${2:?"--team needs a team ID"}
      signing_flags=true
      shift 2
      ;;
    --profile)
      signing_profile=${2:?"--profile needs a path"}
      signing_flags=true
      shift 2
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ -z $kind ]]; then
  usage >&2
  exit 2
fi
if ((${#manifest_args[@]} > 0)); then
  manifest_path=${manifest_args[1]}
  if [[ ! -f $manifest_path ]]; then
    echo "error: no Cargo manifest at $manifest_path" >&2
    exit 2
  fi
  # Keep a caller-relative manifest valid after changing to this repository.
  manifest_path=$(cd "$(dirname "$manifest_path")" && pwd)/$(basename "$manifest_path")
  manifest_args=(--manifest-path "$manifest_path")
fi

case "$target" in
  aarch64-apple-ios-sim) platform=iPhoneSimulator ;;
  aarch64-apple-ios) platform=iPhoneOS ;;
  *)
    echo "error: unsupported target $target (aarch64-apple-ios-sim or aarch64-apple-ios)" >&2
    exit 2
    ;;
esac

# The simulator keeps the project's identifier whatever the environment says,
# so exporting HERDR_IOS_BUNDLE_ID for device work leaves its data container
# and Keychain group where they were.
implicit_bundle_id=false
if [[ -n $bundle_id && $bundle_id != com.shin1ohno.yebisu ]]; then
  echo "error: yebisu bundle identifier is fixed" >&2
  exit 2
fi
bundle_id=com.shin1ohno.yebisu
if [[ ! $bundle_id =~ ^[A-Za-z0-9][A-Za-z0-9.-]*$ ]]; then
  echo "error: '$bundle_id' is not a valid bundle identifier" >&2
  exit 2
fi
if [[ $platform == iPhoneSimulator && $signing_flags == true ]]; then
  echo "error: --team and --profile sign a device bundle (--target aarch64-apple-ios)" >&2
  exit 2
fi
if [[ $platform == iPhoneOS && -n $team && ! $team =~ ^[A-Z0-9]{10}$ ]]; then
  echo "error: '$team' is not a ten-character team ID" >&2
  exit 2
fi
# Provisioning registers the identifier as an App ID with the team, which
# for the project's own identifier would claim or collide with it.
if [[ $platform == iPhoneOS && (-n $team || -n $signing_profile) &&
  $implicit_bundle_id == true ]]; then
  echo "error: signing a device bundle needs your own bundle identifier: pass" \
    "--bundle-id ID or set HERDR_IOS_BUNDLE_ID (for example com.example.herdr-gpui)" >&2
  exit 2
fi
if [[ -n $signing_profile && ! -f $signing_profile ]]; then
  echo "error: no provisioning profile at $signing_profile" >&2
  exit 2
fi
if [[ -n $signing_profile ]]; then
  # Absolute before the cd below, so a path relative to the caller still works.
  signing_profile=$(cd "$(dirname "$signing_profile")" && pwd)/$(basename "$signing_profile")
fi

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$root"
# shellcheck source=scripts/ios/lib.sh
source scripts/ios/lib.sh
ios_export_deployment_target

# Entitlements are derived per build and never kept: a stale file could sign
# a bundle with another app's identifiers.
work=$(mktemp -d "${TMPDIR:-/tmp}/herdr-ios-bundle.XXXXXX")
trap 'rm -rf -- "$work"' EXIT
signing=false
if [[ $platform == iPhoneOS && (-n $team || -n $signing_profile) ]]; then
  signing=true
fi

if command -v rustup >/dev/null && ! rustup target list --installed | grep -qx "$target"; then
  echo "error: the Rust target $target is not installed: rustup target add $target" >&2
  exit 1
fi

# Cargo reports the built executable's path in its JSON messages, which
# follows CARGO_TARGET_DIR and profile settings this script need not know.
cargo_subcommand=build
link_args=()
if [[ $platform == iPhoneSimulator ]]; then
  # Simulator Security.framework reads this Mach-O section. A codesign-only
  # entitlement cannot grant Keychain access to an ad-hoc simulator binary.
  # Link only the final executable so dependencies retain their build cache.
  entitlements=$work/simulator.plist
  cat >"$entitlements" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>application-identifier</key><string>HERDRSIM.$bundle_id</string>
<key>keychain-access-groups</key><array><string>HERDRSIM.$bundle_id</string></array>
</dict></plist>
EOF
  cargo_subcommand=rustc
  link_args=(-- -C "link-arg=-Wl,-sectcreate,__TEXT,__entitlements,$entitlements")
elif [[ $signing == true ]]; then
  # Resolved before the build, so a missing profile or identity fails in
  # seconds rather than after a release compile.
  if [[ -z $signing_profile ]]; then
    signing_profile=$(bash scripts/ios/provision.sh --team "$team" --bundle-id "$bundle_id" | tail -n 1)
  fi
  entitlements=$work/device.plist
  python3 -I scripts/ios/profile.py entitlements --profile "$signing_profile" \
    --bundle-id "$bundle_id" ${team:+--team "$team"} --out "$entitlements"
  security find-identity -v -p codesigning >"$work/identities.txt"
  identity=$(python3 -I scripts/ios/profile.py identity --profile "$signing_profile" \
    --identities "$work/identities.txt")
fi
messages=$(cargo "$cargo_subcommand" --locked --package "$package" --target "$target" \
  ${manifest_args[@]+"${manifest_args[@]}"} \
  "--$kind" "$name" ${profile_args[@]+"${profile_args[@]}"} \
  ${feature_args[@]+"${feature_args[@]}"} --message-format=json-render-diagnostics \
  ${link_args[@]+"${link_args[@]}"})
artifact=$(python3 -I -c '
import json, sys

kind, name = sys.argv[1], sys.argv[2]
found = None
for line in sys.stdin:
    if not line.startswith("{"):
        continue
    message = json.loads(line)
    if message.get("reason") != "compiler-artifact" or not message.get("executable"):
        continue
    target = message["target"]
    if target["name"] == name and kind in target["kind"]:
        # A package ID ends in "#VERSION" or "#NAME@VERSION".
        version = message["package_id"].split("#", 1)[-1].rsplit("@", 1)[-1]
        found = (message["executable"], version)
if found is None:
    sys.exit(1)
print(*found, sep="\n")
' "$kind" "$name" <<<"$messages") || {
  echo "error: cargo built no $kind named $name in $package" >&2
  exit 1
}
executable=${artifact%$'\n'*}
version=${artifact##*$'\n'}

app="${executable%/*}/$name.app"
# Optional collector boundary must run BEFORE removing or creating a bundle.
if [[ -n ${HERDR_IOS_ARTIFACT_ROOT:-} ]]; then
  python3 -I -c '
import pathlib, sys
root, executable, app = [pathlib.Path(p).resolve() for p in sys.argv[1:]]
if not root.is_dir() or not executable.is_file() or any(not p.is_relative_to(root) or p == root for p in (executable, app)):
    sys.exit("error: cargo artifact escaped the requested isolated root")
' "$HERDR_IOS_ARTIFACT_ROOT" "$executable" "$app"
fi
# The whole bundle is this script's own output, rebuilt from scratch so a
# renamed executable leaves nothing stale behind.
rm -rf "$app"
mkdir -p "$app"
cp "$executable" "$app/$name"

plist="$app/Info.plist"
cp scripts/ios/Info.plist "$plist"
plutil -replace CFBundleExecutable -string "$name" "$plist"
plutil -replace CFBundleIdentifier -string "$bundle_id" "$plist"
plutil -replace CFBundleShortVersionString -string "$version" "$plist"
plutil -replace CFBundleVersion -string "$version" "$plist"
plutil -replace CFBundleSupportedPlatforms -json "[\"$platform\"]" "$plist"
# App-local fonts do not require access to fonts installed by other apps.
# UIAppFonts registers them before UIKit creates the first scene.
if [[ $package == yebisu && $kind == bin && $name == yebisu ]]; then
  font_source=assets/fonts/SauceCodePro
  mkdir -p "$app/Fonts"
  for face in Regular Bold Italic BoldItalic; do
    cp "$font_source/SauceCodeProNerdFontMono-$face.ttf" "$app/Fonts/"
  done
  cp "$font_source/LICENSE.txt" "$font_source/README.md" "$font_source/PROVENANCE.md" "$font_source/NERD-FONTS-LICENSE.txt" "$app/Fonts/"
  plutil -replace UIAppFonts -json '["Fonts/SauceCodeProNerdFontMono-Regular.ttf","Fonts/SauceCodeProNerdFontMono-Bold.ttf","Fonts/SauceCodeProNerdFontMono-Italic.ttf","Fonts/SauceCodeProNerdFontMono-BoldItalic.ttf"]' "$plist"
fi
plutil -lint "$plist" >&2
if grep -q '\$(' "$plist"; then
  echo "error: $plist still has an unsubstituted \$(...) value" >&2
  exit 1
fi

if [[ $platform == iPhoneOS && $signing != true ]]; then
  echo "Assembled an unsigned device bundle: $app" >&2
  echo "error: installing on an iPad needs a signed bundle: pass --team TEAM" \
    "(or set HERDR_IOS_TEAM) or --profile PATH" >&2
  printf '%s\n' "$app"
  exit 3
fi
if [[ $platform == iPhoneOS ]]; then
  # The profile is a sealed resource of the bundle, so it is copied first.
  cp "$signing_profile" "$app/embedded.mobileprovision"
  codesign --force --sign "$identity" --entitlements "$entitlements" \
    --timestamp=none "$app" >&2
  codesign --verify --strict "$app" >&2
  echo "Bundled and signed $bundle_id ($version) for a device with identity $identity" >&2
  printf '%s\n' "$app"
  exit 0
fi

# Signing the bundle, not just the linker-signed executable, binds Info.plist
# to the signature, as the simulator's installer expects.
codesign --force --sign - --timestamp=none "$app" >&2
codesign --verify --strict "$app" >&2
echo "Bundled $bundle_id ($version) for the simulator" >&2
printf '%s\n' "$app"
