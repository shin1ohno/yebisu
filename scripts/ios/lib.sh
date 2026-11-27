# Shared by the iPadOS simulator scripts; source it, do not run it.
#
# HERDR_IOS_SIMULATOR names the simulator they use, by UDID or by device name.
# A name resolves to an available device of that name, preferring one already
# booted, then the newest iOS runtime, so the default works on any Mac whose
# Xcode has that iPad without committing one machine's UDID.

HERDR_IOS_SIMULATOR=${HERDR_IOS_SIMULATOR:-"iPad Pro 13-inch (M5)"}

# Exports IPHONEOS_DEPLOYMENT_TARGET as the Info.plist template's
# MinimumOSVersion, so the minimum OS the bundle declares and the one the
# executable is linked for (LC_BUILD_VERSION minos, objc2's `available!`) are
# one number. rustc tracks the variable, so every iOS build and lint sets it
# alike to share one set of build artifacts.
ios_export_deployment_target() {
  local plist
  plist="$(dirname "${BASH_SOURCE[0]}")/Info.plist"
  IPHONEOS_DEPLOYMENT_TARGET=$(plutil -extract MinimumOSVersion raw -o - "$plist") || {
    echo "error: $plist has no MinimumOSVersion" >&2
    return 1
  }
  export IPHONEOS_DEPLOYMENT_TARGET
}

# Prints "UDID STATE" for the simulator HERDR_IOS_SIMULATOR-style argument $1
# names, or fails when no available simulator matches.
ios_sim_resolve() {
  xcrun simctl list --json devices available | python3 -I -c '
import json, re, sys

wanted = sys.argv[1]
candidates = []
for runtime, devices in json.load(sys.stdin)["devices"].items():
    match = re.search(r"\.iOS-(\d+)-(\d+)(?:-(\d+))?$", runtime)
    if match is None:
        continue
    version = tuple(int(part or 0) for part in match.groups())
    for device in devices:
        if wanted in (device["udid"], device["name"]):
            candidates.append((device["state"] == "Booted", version, device["udid"], device["state"]))
if not candidates:
    sys.exit(1)
_, _, udid, state = max(candidates)
print(udid, state)
' "$1"
}

# Boots the simulator $1 names unless it is already booted, waits until it has
# finished booting, and prints its UDID. Never shuts down or erases one.
ios_sim_boot() {
  local wanted=$1 resolved udid state log
  if ! resolved=$(ios_sim_resolve "$wanted"); then
    echo "error: no available simulator is named or has the UDID '$wanted';" \
      "set HERDR_IOS_SIMULATOR to one from 'xcrun simctl list devices available'" >&2
    return 1
  fi
  read -r udid state <<<"$resolved"
  if [[ $state != Booted ]]; then
    echo "Booting simulator $udid ($wanted), currently $state; a first boot takes about a minute" >&2
    # -b boots a shut-down device; for one already booting it only waits. It
    # reports progress every second, so its output is kept for failures only.
    if ! log=$(xcrun simctl bootstatus "$udid" -b 2>&1); then
      printf '%s\n' "$log" >&2
      echo "error: simulator $udid did not finish booting" >&2
      return 1
    fi
    echo "Simulator $udid booted" >&2
    if [[ ${HERDR_IOS_QUIET_BOOT_TASKS:-} == 1 ]]; then
      ios_sim_quiet_boot_tasks "$udid"
    fi
  fi
  printf '%s\n' "$udid"
}

# Issue #256: an iOS 27.2 runtime schedules one-shot background tasks 120 s and
# 300 s after boot (the daemons' launchd plists, NonRepeatingTask ScheduleAfter).
# Their launch, remotemanagementd with its 15 subscriber processes among them,
# comes 270-500 s after boot on the xcode-27 runner, the same window in which
# the runner's free memory falls to 15-18 % and the guard probe stops for up to
# 250 s. None of the six is a part of the app the checks drive.
IOS_QUIET_BOOT_TASKS=(
  com.apple.remotemanagementd
  com.apple.finhealthd
  com.apple.DeviceConfigurationAgent
  com.apple.healthcontentd
  com.apple.email.maild
  com.apple.intelligencetasksd
)

# Keeps the six daemons above from launching in the just-booted simulator $1:
# on a local Mac, `launchctl disable system/<label>` alone left them launching
# at +276 s, while disable in both domains plus bootout kept them out past
# +480 s. Prints one line per label with each call's exit status, and never
# fails: the CI step reports whether they started (sim_ready.watch_boot_tasks).
# Only the hosted jobs' disposable devices set HERDR_IOS_QUIET_BOOT_TASKS=1,
# because a disabled service stays disabled on that device.
ios_sim_quiet_boot_tasks() {
  local udid=$1 label call line
  for label in "${IOS_QUIET_BOOT_TASKS[@]}"; do
    line="SIM BOOT TASK off $label"
    for call in "disable system/" "disable user/foreground/" "bootout system/"; do
      xcrun simctl spawn "$udid" launchctl "${call% *}" "${call#* }$label" >/dev/null 2>&1
      line+=" ${call% *}:${call#* }=$?"
    done
    echo "$line" >&2
  done
}
