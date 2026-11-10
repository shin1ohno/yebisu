#!/usr/bin/env bash
# Find a development provisioning profile that signs TEAM.BUNDLE_ID (for a
# device when given) with an identity in the login keychain, and print its
# path as the last line on stdout; diagnostics go to stderr.
#
# When no installed profile qualifies, this builds a throwaway one-file app
# with that bundle identifier and automatic signing, so Xcode registers the
# App ID with the team and downloads a profile, then looks again. That talks
# to Apple's developer services with the account signed in to Xcode.
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/ios/provision.sh --team TEAM --bundle-id ID [options]

Options:
  --team TEAM         Apple development team ID, ten characters
  --bundle-id ID      Bundle identifier the profile must cover
  --device UDID       Hardware UDID the profile must list
                      (xcrun devicectl list devices shows the device)
  --profiles-dir DIR  Where to look; repeatable (default: Xcode's
                      ~/Library/Developer/Xcode/UserData/Provisioning Profiles
                      and ~/Library/MobileDevice/Provisioning Profiles)
  --no-create         Fail instead of asking Xcode for a profile
  -h, --help          Show this help

Exits with status 2 for invalid options and 1 when no profile can be found
or created. Xcode is asked only when the local search ran and found nothing,
never after it failed.
EOF
}

team=
bundle_id=
device=
create=true
profile_dirs=()

while (($# > 0)); do
  case "$1" in
    --team)
      team=${2:?"--team needs a team ID"}
      shift 2
      ;;
    --bundle-id)
      bundle_id=${2:?"--bundle-id needs an identifier"}
      shift 2
      ;;
    --device)
      device=${2:?"--device needs a UDID"}
      shift 2
      ;;
    --profiles-dir)
      profile_dirs+=("${2:?"--profiles-dir needs a directory"}")
      shift 2
      ;;
    --no-create)
      create=false
      shift
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

if [[ ! $team =~ ^[A-Z0-9]{10}$ ]]; then
  echo "error: --team needs a ten-character team ID, not '$team'" >&2
  exit 2
fi
if [[ ! $bundle_id =~ ^[A-Za-z0-9][A-Za-z0-9.-]*$ ]]; then
  echo "error: '$bundle_id' is not a valid bundle identifier" >&2
  exit 2
fi
if ((${#profile_dirs[@]} == 0)); then
  profile_dirs=(
    "$HOME/Library/Developer/Xcode/UserData/Provisioning Profiles"
    "$HOME/Library/MobileDevice/Provisioning Profiles"
  )
fi

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
work=$(mktemp -d "${TMPDIR:-/tmp}/herdr-ios-provision.XXXXXX")
trap 'rm -rf -- "$work"' EXIT

security find-identity -v -p codesigning >"$work/identities.txt"

select_args=(--app-id "$team.$bundle_id" --identities "$work/identities.txt")
for dir in "${profile_dirs[@]}"; do select_args+=(--profiles-dir "$dir"); done
if [[ -n $device ]]; then select_args+=(--device "$device"); fi

# profile.py select exits 3 when it ran and nothing qualifies; any other
# failure is a broken search, which must not lead to a request to Apple.
none_qualifies=3
find_profile() {
  local status=0
  found=$(python3 -I "$here/profile.py" select "$@") || status=$?
  if ((status != 0 && status != none_qualifies)); then
    echo "error: searching the installed provisioning profiles failed (status $status)" >&2
    exit 1
  fi
  ((status == 0))
}

if find_profile "${select_args[@]}"; then
  echo "Using provisioning profile $found" >&2
  printf '%s\n' "$found"
  exit 0
fi

if [[ $create != true ]]; then
  echo "error: no installed provisioning profile covers $team.$bundle_id" >&2
  exit 1
fi

# shellcheck source=scripts/ios/lib.sh
source "$here/lib.sh"
ios_export_deployment_target

stub=$work/stub
mkdir -p "$stub/Sources"
cat >"$stub/Sources/App.swift" <<'EOF'
import SwiftUI

@main
struct ProvisioningStub: App {
    var body: some Scene {
        WindowGroup { EmptyView() }
    }
}
EOF
# XcodeGen expands ${...} from the environment, so the values are written
# literally and --no-env keeps them that way.
cat >"$stub/project.yml" <<EOF
name: HerdrProvisioning
targets:
  HerdrProvisioning:
    type: application
    platform: iOS
    deploymentTarget: "$IPHONEOS_DEPLOYMENT_TARGET"
    sources: [Sources]
    scheme: {}
    settings:
      base:
        PRODUCT_BUNDLE_IDENTIFIER: "$bundle_id"
        DEVELOPMENT_TEAM: "$team"
        CODE_SIGN_STYLE: Automatic
        TARGETED_DEVICE_FAMILY: "2"
        GENERATE_INFOPLIST_FILE: "YES"
        INFOPLIST_KEY_UILaunchScreen_Generation: "YES"
        MARKETING_VERSION: "1.0"
        CURRENT_PROJECT_VERSION: "1"
        SWIFT_VERSION: "5.0"
EOF

echo "No installed profile covers $team.$bundle_id; asking Xcode to register" \
  "the App ID and download one with a throwaway app" >&2
xcodegen generate --no-env --quiet --spec "$stub/project.yml" --project "$stub" >&2
build_log=$work/xcodebuild.log
if ! xcodebuild -project "$stub/HerdrProvisioning.xcodeproj" -scheme HerdrProvisioning \
  -destination generic/platform=iOS -derivedDataPath "$work/DerivedData" \
  -allowProvisioningUpdates build >"$build_log" 2>&1; then
  grep -E 'error:' "$build_log" | tail -n 20 >&2 || tail -n 20 "$build_log" >&2
  echo "error: Xcode could not provision $team.$bundle_id. Sign in to Xcode" \
    "(Settings > Accounts) with an account in team $team, and register the" \
    "device with the team." >&2
  exit 1
fi

if find_profile "${select_args[@]}"; then
  echo "Xcode provisioned $team.$bundle_id: $found" >&2
  printf '%s\n' "$found"
  exit 0
fi
# The stub builds for a generic device, so Xcode may keep a profile made
# before the iPad was registered rather than regenerate it with the iPad.
if [[ -n $device ]]; then
  without_device=()
  for arg in "${select_args[@]}"; do
    if [[ $arg == --device ]]; then break; fi
    without_device+=("$arg")
  done
  if find_profile "${without_device[@]}"; then
    echo "error: $found covers $team.$bundle_id but does not list device $device." \
      "If the device is registered with team $team, delete that profile and run" \
      "this again so Xcode downloads a current one." >&2
    exit 1
  fi
fi
echo "error: Xcode built the throwaway app, yet no installed profile covers" \
  "$team.$bundle_id${device:+ for device $device}. Register the device with team" \
  "$team (Xcode > Window > Devices and Simulators), then run this again." >&2
exit 1
