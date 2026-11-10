#!/usr/bin/env bash
# Run an iPadOS .app in the simulator and observe it: boot the simulator when
# it is not running, install and launch the app with its stdout and stderr in
# files, wait, take a screenshot, and report whether the app is still running.
#
# Never erases, deletes, or shuts down a simulator. The only app it ever
# terminates is the one it launches: a running copy when relaunching it, and
# that app after the screenshot with --terminate.
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/ios/sim.sh [options] APP [-- ARGS...]

Runs APP (an .app built by scripts/ios/bundle.sh) with ARGS.

Options:
  --simulator NAME|UDID  Simulator to use (default: $HERDR_IOS_SIMULATOR, else
                         "iPad Pro 13-inch (M5)")
  --wait SECONDS         How long the app runs before the screenshot (default: 5)
  --out DIR              Directory for stdout.log, stderr.log, and
                         screenshot.png (default: target/ios-sim/NAME)
  --terminate            Terminate the app after the screenshot (default:
                         leave it running to use in the simulator)
  --show                 Open Simulator.app on the device
  --expect-color RRGGBB  Fail (status 5) unless at least --min of the
                         screenshot is near this color
  --min FRACTION         Share --expect-color needs (default: 0.4)
  -h, --help             Show this help

RUST_BACKTRACE (default 1) reaches the app, and RUST_LOG is passed on for
code that reads it. Yebisu's own allowlisted JSONL path is defined in
crates/yebisu/src/log-location.txt, relative to its data container. It exits with status 4 when the app is no longer running at the
screenshot, or exits before --terminate stops it; the summary of log paths is
printed either way. A running app is not proof that it drew anything: check
the screenshot (scripts/ios/screenshot-color.py).
EOF
}

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
# shellcheck source=scripts/ios/lib.sh
source "$root/scripts/ios/lib.sh"

simulator=$HERDR_IOS_SIMULATOR
wait_seconds=5
out=
terminate=false
show=false
expect_color=
min_fraction=0.4
app=
app_args=()

while (($# > 0)); do
  case "$1" in
    --simulator)
      simulator=${2:?"--simulator needs a name or UDID"}
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
    --terminate)
      terminate=true
      shift
      ;;
    --show)
      show=true
      shift
      ;;
    --expect-color)
      expect_color=${2:?"--expect-color needs an RRGGBB color"}
      shift 2
      ;;
    --min)
      min_fraction=${2:?"--min needs a fraction"}
      shift 2
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    --)
      shift
      app_args=("$@")
      break
      ;;
    -*)
      echo "error: unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
    *)
      if [[ -n $app ]]; then
        echo "error: give one APP; pass the app's arguments after --" >&2
        exit 2
      fi
      app=$1
      shift
      ;;
  esac
done

if [[ -z $app ]]; then
  usage >&2
  exit 2
fi
if [[ ! -f $app/Info.plist ]]; then
  echo "error: $app is not an app bundle (no Info.plist)" >&2
  exit 2
fi
if [[ ! $wait_seconds =~ ^[0-9]+$ ]]; then
  echo "error: --wait takes whole seconds, not '$wait_seconds'" >&2
  exit 2
fi

app=$(cd "$app" && pwd)
name=$(basename "$app" .app)
bundle_id=$(plutil -extract CFBundleIdentifier raw -o - "$app/Info.plist")
out=${out:-$root/target/ios-sim/$name}
mkdir -p "$out"
out=$(cd "$out" && pwd)
stdout_log=$out/stdout.log
stderr_log=$out/stderr.log
screenshot=$out/screenshot.png

udid=$(ios_sim_boot "$simulator")
if [[ $show == true ]]; then
  open -a Simulator --args -CurrentDeviceUDID "$udid"
fi

xcrun simctl install "$udid" "$app"

# simctl hands the app only SIMCTL_CHILD_-prefixed variables.
export SIMCTL_CHILD_RUST_BACKTRACE=${RUST_BACKTRACE:-1}
if [[ -n ${RUST_LOG:-} ]]; then export SIMCTL_CHILD_RUST_LOG=$RUST_LOG; fi

# Separate files: the app opens each path on its own, so one shared file
# would have stderr overwrite stdout. Emptied so a run shows only its output.
: >"$stdout_log"
: >"$stderr_log"
# Prints "BUNDLE_ID: PID"; the PID is a host process, visible to ps.
launched=$(xcrun simctl launch --terminate-running-process \
  --stdout="$stdout_log" --stderr="$stderr_log" \
  "$udid" "$bundle_id" ${app_args[@]+"${app_args[@]}"})
pid=${launched##*: }
if [[ ! $pid =~ ^[0-9]+$ ]]; then
  echo "error: unexpected launch output: $launched" >&2
  exit 1
fi
echo "Launched $bundle_id (pid $pid); waiting ${wait_seconds}s" >&2
sleep "$wait_seconds"

running=false
if ps -p "$pid" >/dev/null 2>&1; then running=true; fi
xcrun simctl io "$udid" screenshot "$screenshot" >&2

state="running (pid $pid)"
terminate_failed=false
if [[ $running != true ]]; then
  state=exited
elif [[ $terminate == true ]]; then
  # The app can exit between the probe and here, which fails simctl; the
  # summary below must still print, as the logs matter most in that crash.
  if xcrun simctl terminate "$udid" "$bundle_id"; then
    state="terminated after the screenshot (pid $pid)"
  elif ps -p "$pid" >/dev/null 2>&1; then
    state="running (pid $pid); terminating it failed"
    terminate_failed=true
  else
    running=false
    state="exited before it could be terminated"
  fi
fi

cat <<EOF
app:        $app
bundle id:  $bundle_id
simulator:  $udid
state:      $state
stdout:     $stdout_log
stderr:     $stderr_log
screenshot: $screenshot
EOF

if [[ $running != true ]]; then
  echo "error: the app exited within about ${wait_seconds}s; the end of its stderr:" >&2
  tail -n 20 "$stderr_log" >&2
  exit 4
fi
if [[ $terminate_failed == true ]]; then
  echo "error: could not terminate $bundle_id (pid $pid)" >&2
  exit 1
fi
if [[ -n $expect_color ]] &&
  ! python3 -I "$root/scripts/ios/screenshot-color.py" "$screenshot" "$expect_color" --min "$min_fraction"; then
  echo "error: less than $min_fraction of the screenshot is near #$expect_color" >&2
  exit 5
fi
