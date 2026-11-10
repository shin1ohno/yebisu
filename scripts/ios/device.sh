#!/usr/bin/env bash
# Sign, install, and launch Sheprd on a connected iPad, then observe it:
# whether the process is still running after a wait, and a copy of the app's
# log file from its data container.
#
# Never uninstalls an app, erases data, or changes the device's settings. The
# only process it terminates is an earlier copy of this app, when launching.
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/ios/device.sh [options]

Options:
  --device NAME|ID  iPad to use, by name, CoreDevice identifier, or hardware
                    UDID (default: $HERDR_IOS_DEVICE, else the only paired iPad)
  --team TEAM       Development team that signs the app (default: $HERDR_IOS_TEAM)
  --bundle-id ID    Bundle identifier, one under a domain you control
                    (default: $HERDR_IOS_BUNDLE_ID; required when building);
                    the team needs an App ID for it, which
                    scripts/ios/provision.sh asks Xcode to create
  --dev             Build with the dev profile (default: release)
  --app PATH        Install this signed bundle instead of building one
  --wait SECONDS    How long the app runs before the check (default: 5)
  --out DIR         Directory for the copied log and devicectl's JSON
                    (default: target/ios-device/yebisu)
  --no-launch       Install only
  -h, --help        Show this help

The iPad must be paired with this Mac, have Developer Mode enabled, and be
connected (USB or the same network) and unlocked. Exits with status 2 for
invalid options, 3 when the iPad is not ready (unreachable, unpaired,
without Developer Mode, or reporting no UDID), 4 when the app is no longer running after --wait,
and 1 on any other failure. Once the app is installed, the app's log is
copied to OUT/yebisu.jsonl and the summary printed whatever the outcome.
EOF
}

device=${HERDR_IOS_DEVICE:-}
team=${HERDR_IOS_TEAM:-}
bundle_id=com.shin1ohno.yebisu
profile_args=(--release)
app=
wait_seconds=5
out=
launch=true

while (($# > 0)); do
  case "$1" in
    --device)
      device=${2:?"--device needs a name or identifier"}
      shift 2
      ;;
    --team)
      team=${2:?"--team needs a team ID"}
      shift 2
      ;;
    --bundle-id)
      bundle_id=${2:?"--bundle-id needs an identifier"}
      shift 2
      ;;
    --dev)
      profile_args=()
      shift
      ;;
    --app)
      app=${2:?"--app needs a path"}
      shift 2
      ;;
    --wait)
      wait_seconds=${2:?"--wait needs seconds"}
      shift 2
      ;;
    --out)
      out=${2:?"--out needs a directory"}
      shift 2
      ;;
    --no-launch)
      launch=false
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

if [[ ! $wait_seconds =~ ^[0-9]+$ ]]; then
  echo "error: --wait takes whole seconds, not '$wait_seconds'" >&2
  exit 2
fi
if [[ -n $app ]]; then
  if [[ ! -f $app/Info.plist || ! -f $app/embedded.mobileprovision ]]; then
    echo "error: $app is not a signed device bundle (no Info.plist or embedded.mobileprovision)" >&2
    exit 2
  fi
  app=$(cd "$app" && pwd)
  bundle_id=$(plutil -extract CFBundleIdentifier raw -o - "$app/Info.plist")
elif [[ ! $team =~ ^[A-Z0-9]{10}$ ]]; then
  echo "error: signing for a device needs a team: pass --team TEAM or set HERDR_IOS_TEAM" >&2
  exit 2
elif [[ -z $bundle_id ]]; then
  # Provisioning registers the identifier as an App ID with the team, so the
  # project's own so.pen.herdr-gpui is never used implicitly.
  echo "error: signing for a device needs your own bundle identifier: pass" \
    "--bundle-id ID or set HERDR_IOS_BUNDLE_ID (for example com.example.herdr-gpui)" >&2
  exit 2
fi
if [[ $bundle_id != com.shin1ohno.yebisu ]]; then
  echo "error: yebisu bundle identifier is fixed" >&2
  exit 2
fi
if [[ ! $bundle_id =~ ^[A-Za-z0-9][A-Za-z0-9.-]*$ ]]; then
  echo "error: '$bundle_id' is not a valid bundle identifier" >&2
  exit 2
fi

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
out=${out:-$root/target/ios-device/yebisu}
mkdir -p "$out"
out=$(cd "$out" && pwd)
work=$(mktemp -d "${TMPDIR:-/tmp}/herdr-ios-device.XXXXXX")
trap 'rm -rf -- "$work"' EXIT
devicectl_py=(python3 -I "$root/scripts/ios/devicectl.py")

if ! xcrun devicectl list devices --json-output "$work/devices.json" >"$work/list.log" 2>&1; then
  cat "$work/list.log" >&2
  echo "error: xcrun devicectl could not list devices" >&2
  exit 1
fi
# resolve exits 1 (no or several matches) or 3 (not ready), passed through.
resolved=$("${devicectl_py[@]}" resolve "$work/devices.json" "$device")
IFS=$'\x1f' read -r identifier udid tunnel name <<<"$resolved"
if [[ $tunnel == unavailable ]]; then
  echo "error: $name ($identifier) is unreachable; connect it by USB or to this" \
    "Mac's network, unlock it, and run this again" >&2
  exit 3
fi
echo "Using $name ($identifier, UDID $udid, $tunnel)" >&2

if [[ -z $app ]]; then
  # Provisioning for this device first: the profile must list it, and a
  # missing App ID or registration fails before the long build.
  profile=$(bash "$root/scripts/ios/provision.sh" --team "$team" --bundle-id "$bundle_id" \
    --device "$udid" | tail -n 1) || {
    echo "error: no provisioning profile signs $team.$bundle_id for $name" >&2
    exit 1
  }
  app=$(bash "$root/scripts/ios/bundle.sh" --package yebisu --bin yebisu \
    --no-default-features --target aarch64-apple-ios ${profile_args[@]+"${profile_args[@]}"} \
    --bundle-id "$bundle_id" --team "$team" --profile "$profile" | tail -n 1) || {
    echo "error: could not build and sign the bundle for $name" >&2
    exit 1
  }
fi

# CoreDevice error 3002 is a dropped transfer connection, which a second
# attempt usually gets past; any other failure is reported as it is.
install_app() {
  xcrun devicectl device install app --device "$identifier" \
    --json-output "$work/install.json" "$app" >"$work/install.log" 2>&1
}
if ! install_app; then
  code=$("${devicectl_py[@]}" error-code "$work/install.json" 2>/dev/null || true)
  if [[ $code == 3002 ]] || grep -q 'error 3002' "$work/install.log"; then
    echo "Install failed with CoreDevice error 3002; retrying once" >&2
    if ! install_app; then
      cat "$work/install.log" >&2
      echo "error: could not install $app on $name" >&2
      exit 1
    fi
  else
    cat "$work/install.log" >&2
    echo "error: could not install $app on $name" >&2
    exit 1
  fi
fi
echo "Installed $bundle_id on $name" >&2

if [[ $launch != true ]]; then
  cat <<EOF
app:        $app
bundle id:  $bundle_id
device:     $name ($identifier)
state:      installed, not launched
EOF
  exit 0
fi

# Stale results from an earlier run must not stand in for this one's.
rm -f "$out/launch.json" "$out/processes.json"
status=0
if ! xcrun devicectl device process launch --device "$identifier" --terminate-existing \
  --json-output "$out/launch.json" "$bundle_id" >&2; then
  echo "error: devicectl could not launch $bundle_id on $name" >&2
  state="launch failed"
  status=1
elif ! pid=$("${devicectl_py[@]}" launched-pid "$out/launch.json"); then
  echo "error: devicectl reported no process for $bundle_id; see $out/launch.json" >&2
  state="launch failed"
  status=1
else
  echo "Launched $bundle_id (pid $pid); waiting ${wait_seconds}s" >&2
  sleep "$wait_seconds"
  if ! xcrun devicectl device info processes --device "$identifier" \
    --json-output "$out/processes.json" >/dev/null; then
    echo "error: devicectl could not list the processes on $name" >&2
    state="unknown (processes not listed)"
    status=1
  elif "${devicectl_py[@]}" running "$out/processes.json" "$pid"; then
    state="running (pid $pid)"
  else
    state=exited
    status=4
  fi
fi

# The log is the app's own record of startup; copied whether or not it still
# runs, as it matters most after a crash.
log=$out/yebisu.jsonl
rm -f "$log"
# Shared with Rust include_str!: never maintain a second container path.
log_source=$(cat "$root/crates/yebisu/src/log-location.txt")
log_state="not copied"
for attempt in 1 2 3 4 5 6; do
  rm -f "$work/copy.json"
  if xcrun devicectl device copy from --device "$identifier" \
    --domain-type appDataContainer --domain-identifier "$bundle_id" \
    --timeout 5 --json-output "$work/copy.json" --source "$log_source" \
    --destination "$log" >"$work/copy.log" 2>&1; then
    log_state=$log
    break
  fi
  # A failed copy must never leave a stale/partial log reported as this run's.
  rm -f "$log"
  code=$("${devicectl_py[@]}" error-code "$work/copy.json" 2>/dev/null || true)
  if [[ -z $code ]] && grep -q "CoreDeviceError error 7000" "$work/copy.log"; then
    code=7000
  fi
  if [[ $code != 7000 ]]; then
    cat "$work/copy.log" >&2
    log_state="not copied: device log transfer failed"
    break
  fi
  log_state="log file does not exist yet: $log_source (6 attempts; 5s command timeout, 1s retry interval)"
  if ((attempt < 6)); then sleep 1; fi
done

cat <<EOF
app:        $app
bundle id:  $bundle_id
device:     $name ($identifier)
state:      $state
log:        $log_state
EOF

if ((status == 4)); then
  echo "error: the app exited within about ${wait_seconds}s; check the log and" \
    "the device's crash reports (Xcode > Window > Devices and Simulators)" >&2
fi
exit "$status"
