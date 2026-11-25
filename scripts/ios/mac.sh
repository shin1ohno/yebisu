#!/usr/bin/env bash
# Sign, install, and launch yebisu on this Apple silicon Mac as an iPad app
# ("Designed for iPad"), then observe it: whether the process is still running
# after a wait, and a copy of the app's log file from its data container.
#
# Never uninstalls other apps, deletes data, or changes system settings. The
# only process it terminates is an earlier copy of com.shin1ohno.yebisu, and the
# only things it removes are earlier com.shin1ohno.yebisu wrappers in
# /Applications, which it asks Finder to move to the Trash (the owner
# authenticates), before installing. It never registers this Mac with the team unless asked to with
# --register-device (just mac-register, the owner's one-time step).
#
# Tests point YEBISU_MAC_APPLICATIONS (default /Applications) at a scratch
# directory and shorten YEBISU_MAC_TIMEOUT (seconds per wait, default 60).
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/ios/mac.sh --team TEAM [options]

Options:
  --team TEAM        Development team that signs the app (default: $HERDR_IOS_TEAM)
  --bundle-id ID     Bundle identifier; only com.shin1ohno.yebisu is accepted
  --dev              Build with the dev profile (default: release)
  --app PATH         Install this signed bundle instead of building one
  --wait SECONDS     How long the app runs before the check (default: 5)
  --out DIR          Directory for the summary, launch evidence and the copied
                     log (default: target/ios-mac/yebisu)
  --register-device  Let Xcode register this Mac with the team when no profile
                     lists it (outward-facing; the owner runs it once)
  --register-only    Stop after provisioning (just mac-register)
  --no-launch        Install only
  -h, --help         Show this help

Exits with status 2 for invalid options or a refused bundle identifier, 3 when
this Mac is not registered with the team (run just mac-register once) or the
owner did not authenticate Finder's move of an earlier wrapper to the Trash, 4 when
the app is no longer running after --wait, and 1 on any other failure. Every
failure ends with "error: just mac failed at step STEP: REASON".
EOF
}

fail() { # fail STEP STATUS REASON...
  local step=$1 status=$2
  shift 2
  echo "error: just mac failed at step $step: $*" >&2
  exit "$status"
}

need() { # need OPTION ARGUMENT-COUNT
  (($2 >= 2)) || fail options 2 "$1 needs a value"
}

team=${HERDR_IOS_TEAM:-}
bundle_id=com.shin1ohno.yebisu
profile_args=(--release)
app=
wait_seconds=5
out=
register_args=()
register_only=false
launch=true

while (($# > 0)); do
  case "$1" in
    --team) need "$1" $# && team=$2 && shift 2 ;;
    --bundle-id) need "$1" $# && bundle_id=$2 && shift 2 ;;
    --dev) profile_args=() && shift ;;
    --app) need "$1" $# && app=$2 && shift 2 ;;
    --wait) need "$1" $# && wait_seconds=$2 && shift 2 ;;
    --out) need "$1" $# && out=$2 && shift 2 ;;
    --register-device) register_args=(--register-device) && shift ;;
    --register-only) register_only=true && shift ;;
    --no-launch) launch=false && shift ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      usage >&2
      fail options 2 "unknown argument: $1"
      ;;
  esac
done

# gate:bundle-id begin
if [[ $bundle_id != com.shin1ohno.yebisu ]]; then
  fail options 2 "bundle identifier must be com.shin1ohno.yebisu, not '$bundle_id'"
fi
# gate:bundle-id end
[[ $team =~ ^[A-Z0-9]{10}$ ]] ||
  fail options 2 "--team needs a ten-character team ID (or HERDR_IOS_TEAM), not '$team'"
[[ $wait_seconds =~ ^[0-9]+$ ]] || fail options 2 "--wait takes whole seconds, not '$wait_seconds'"
if [[ -n $app ]]; then
  if [[ $register_only == true || ${#register_args[@]} -gt 0 ]]; then
    fail options 2 "--register-device and --register-only provision; they take no --app"
  fi
  [[ -f $app/Info.plist && -f $app/embedded.mobileprovision ]] ||
    fail options 2 "$app is not a signed bundle (no Info.plist or embedded.mobileprovision)"
  app=$(cd "$app" && pwd)
fi

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
root=$(cd "$here/../.." && pwd)
out=${out:-$root/target/ios-mac/yebisu}
mkdir -p "$out"
out=$(cd "$out" && pwd)
work=$(mktemp -d "${TMPDIR:-/tmp}/yebisu-mac.XXXXXX")
trap 'rm -rf -- "$work"' EXIT
mac_py=(python3 -I "$here/mac.py")
applications=${YEBISU_MAC_APPLICATIONS:-/Applications}
timeout_seconds=${YEBISU_MAC_TIMEOUT:-60}
[[ $timeout_seconds =~ ^[0-9]+$ ]] || fail options 2 "YEBISU_MAC_TIMEOUT takes whole seconds"

# mise's rust shim exports RUSTUP_TOOLCHAIN (1.100.0 here, macOS target only),
# which would override rust-toolchain.toml; clearing it lets the repository's
# pinned toolchain and its iOS targets apply, with no second copy of the pin.
if [[ -n ${RUSTUP_TOOLCHAIN:-} ]]; then
  echo "Clearing RUSTUP_TOOLCHAIN=$RUSTUP_TOOLCHAIN so rust-toolchain.toml applies" >&2
  unset RUSTUP_TOOLCHAIN
fi

[[ $(uname -m) == arm64 ]] || fail preflight 1 "this Mac is not Apple silicon"
udid=$(system_profiler SPHardwareDataType 2>"$work/system_profiler.err" | "${mac_py[@]}" udid) ||
  fail preflight 1 "could not read this Mac's Provisioning UDID"
# gate:identity begin
valid=$(security find-identity -v -p codesigning | "${mac_py[@]}" identities -) || valid=0
if ((valid == 0)); then
  listed=$(security find-identity -p codesigning | "${mac_py[@]}" identities -) || listed=0
  if ((listed > 0)); then
    fail preflight 1 "the Apple Development identity is not trusted; install Apple's" \
      "WWDR CA G3 intermediate certificate in the login keychain"
  fi
  fail preflight 1 "no Apple Development signing identity in the login keychain"
fi
# gate:identity end
xcrun --sdk iphoneos --show-sdk-version >/dev/null 2>&1 ||
  fail preflight 1 "no iphoneos SDK in the selected Xcode"
echo "Using this Mac (Provisioning UDID $udid)" >&2

if [[ -z $app ]]; then
  # The destination has spaces; it is one argument here and never passes
  # through just's re-split {{command}}.
  destination="platform=macOS,arch=arm64,variant=Designed for iPad,id=$udid"
  provision_status=0
  profile=$(bash "$here/provision.sh" --team "$team" --bundle-id "$bundle_id" \
    --device "$udid" --destination "$destination" \
    ${register_args[@]+"${register_args[@]}"} 2>"$work/provision.err" | tail -n 1) ||
    provision_status=$?
  cat "$work/provision.err" >&2
  if ((provision_status != 0)) || [[ -z $profile ]]; then
    # gate:registration begin
    if grep -q "isn't registered in your developer account" "$work/provision.err"; then
      fail register 3 "this Mac ($udid) is not registered with team $team; the owner runs" \
        "'just mac-register' once, then 'just mac' again"
    fi
    # gate:registration end
    fail provision 1 "no provisioning profile signs $team.$bundle_id for this Mac"
  fi
  if [[ $register_only == true ]]; then
    echo "profile: $profile"
    exit 0
  fi
  # gate:profile-udid begin
  "${mac_py[@]}" profile-lists --profile "$profile" --device "$udid" >&2 ||
    fail profile-check 1 "$profile does not list this Mac ($udid)"
  # gate:profile-udid end
  app=$(bash "$here/bundle.sh" --package yebisu --bin yebisu --no-default-features \
    --target aarch64-apple-ios ${profile_args[@]+"${profile_args[@]}"} \
    --bundle-id "$bundle_id" --team "$team" --profile "$profile" | tail -n 1) ||
    fail build 1 "could not build and sign the bundle"
  [[ -d $app ]] || fail build 1 "could not build and sign the bundle"
fi

# gate:bundle-id-built begin
built_id=$(plutil -extract CFBundleIdentifier raw -o - "$app/Info.plist" 2>/dev/null) || built_id=
signed_id=$(codesign -d --entitlements - --xml "$app" 2>/dev/null |
  "${mac_py[@]}" entitlement-app-id - 2>/dev/null) || signed_id=
if [[ $built_id != com.shin1ohno.yebisu ]]; then
  fail bundle-check 2 "the bundle's identifier is '$built_id', not com.shin1ohno.yebisu"
elif [[ $signed_id != "$team.com.shin1ohno.yebisu" ]]; then
  fail bundle-check 2 "the bundle's identifier is '$signed_id', not com.shin1ohno.yebisu"
fi
# gate:bundle-id-built end
# gate:embedded-profile begin
"${mac_py[@]}" profile-lists --profile "$app/embedded.mobileprovision" --device "$udid" >&2 ||
  fail bundle-check 1 "the bundle's embedded profile does not list this Mac"
# gate:embedded-profile end
codesign --verify --strict "$app" >&2 || fail bundle-check 1 "the bundle's signature does not verify"

# The signed .app does not run as it is ("incorrect executable format"). A zip
# whose root is Payload/yebisu.app is an .ipa; opening one has macOS install it
# as the wrapper $applications/yebisu.app/Wrapper/yebisu.app. macOS installs
# every opened .ipa as new: with a wrapper already there it adds a numbered one
# ("yebisu 2.app") rather than updating it, and only Xcode's own install
# service can install elsewhere. So every earlier wrapper goes to the Trash
# first, through Finder: it asks the owner to authenticate, and that dialog is
# the owner's only step. mac.sh never removes anything from /Applications itself.
wrappers() { # the installed wrappers of this bundle identifier, one per line
  local wrapper info
  for wrapper in "$applications"/*.app; do
    [[ -d $wrapper/Wrapper ]] || continue
    for info in "$wrapper"/Wrapper/*.app/Info.plist; do
      [[ -f $info ]] || continue
      # gate:wrapper-bundle-id begin
      [[ $(plutil -extract CFBundleIdentifier raw -o - "$info" 2>/dev/null) == "$bundle_id" ]] || continue
      # gate:wrapper-bundle-id end
      printf '%s\n' "$wrapper"
      break
    done
  done
}
# The path is an argument of the script, never part of its text, so spaces and
# quotes in it cannot change what Finder is asked to delete. The path becomes a
# file before Finder's tell block: inside it Finder evaluates `POSIX file` of
# the argument itself and cannot resolve it (-1728), and the variable is not
# named after a Finder term such as `target`, which inside the block is
# Finder's own property (both seen on macOS 26.7.3, 2026-11-25).
trash_wrapper() { # trash_wrapper PATH: one Finder delete; the owner authenticates
  # gate:trash-status begin
  osascript -e 'on run argv' \
    -e 'set wrapperFile to POSIX file (item 1 of argv)' \
    -e 'tell application "Finder" to delete wrapperFile' \
    -e 'end run' "$1" >&2 || return 1
  # gate:trash-status end
  return 0
}
installed=$applications/yebisu.app
installed_app=$installed/Wrapper/yebisu.app
earlier_wrappers=$(wrappers)
# Checked before anything is stopped, so a refused run leaves a running copy alone.
if [[ ( -e $installed || -L $installed ) && $earlier_wrappers != *"$installed"* ]]; then
  existing=$(plutil -extract CFBundleIdentifier raw -o - "$installed/Contents/Info.plist" 2>/dev/null ||
    plutil -extract CFBundleIdentifier raw -o - "$installed_app/Info.plist" 2>/dev/null ||
    echo "not a wrapped iPad app")
  fail install 1 "$installed is another app ($existing); not replacing it"
fi

# "PID PPID EXECUTABLE" for processes running an installed com.shin1ohno.yebisu
# bundle's own executable; never a process matched by its name alone.
app_processes() {
  ps -axww -o pid=,ppid=,comm= | "${mac_py[@]}" processes --bundle-id "$bundle_id" "$@"
}
# A copy can still run with its wrapper already in the Trash, and the installer
# would end it itself (installcoordinationd), so mac.sh stops it first.
mac_terminate() {
  local earlier old_pid tick
  earlier=$(app_processes) || return 1
  while read -r old_pid _; do
    if [[ -n $old_pid ]]; then
      echo "Terminating the earlier $bundle_id (pid $old_pid)" >&2
      kill -TERM "$old_pid" 2>/dev/null || true
    fi
  done <<<"$earlier"
  for ((tick = 0; tick < timeout_seconds * 5; tick++)); do
    earlier=$(app_processes) || return 1
    if [[ -z $earlier ]]; then return 0; fi
    sleep 0.2
  done
  echo "error: the earlier $bundle_id did not stop: $earlier" >&2
  return 1
}
mac_terminate || fail terminate 1 "could not stop the running $bundle_id"

while IFS= read -r wrapper; do
  [[ -n $wrapper ]] || continue
  echo "Moving $wrapper to the Trash; macOS asks you to authenticate" >&2
  trash_wrapper "$wrapper" ||
    fail install 3 "Finder did not move $wrapper to the Trash (authentication canceled or" \
      "Finder failed); nothing was installed, run 'just mac' again"
  # gate:trash-gone begin
  if [[ -e $wrapper || -L $wrapper ]]; then
    fail install 3 "$wrapper is still installed after Finder reported success; nothing was installed"
  fi
  # gate:trash-gone end
done <<<"$earlier_wrappers"

mac_install() {
  local found= wrapper= second
  mkdir -p "$work/ipa/Payload"
  ditto "$app" "$work/ipa/Payload/yebisu.app" && codesign --verify --strict "$work/ipa/Payload/yebisu.app" &&
    rm -f "$out/yebisu.ipa" &&
    ditto -c -k --sequesterRsrc --keepParent "$work/ipa/Payload" "$out/yebisu.ipa" &&
    open "$out/yebisu.ipa" || return 1
  # The installer finishes after open returns; the wrapper whose executable
  # is this build's is the one it wrote.
  for ((second = 0; second < timeout_seconds; second++)); do
    while IFS= read -r wrapper; do
      if [[ -n $wrapper ]] && cmp -s "$app/yebisu" "$wrapper/Wrapper/yebisu.app/yebisu"; then
        found=$wrapper
        break
      fi
    done <<<"$(wrappers)"
    if [[ -n $found ]]; then break; fi
    sleep 1
  done
  if [[ -z $found ]]; then
    echo "error: no installed wrapper became this build within ${timeout_seconds}s" >&2
    return 1
  fi
  if [[ $found != "$installed" ]]; then
    echo "error: macOS installed this build as $found, not $installed;" \
      "numbered wrappers are not used" >&2
    return 1
  fi
}
mac_install || fail install 1 "could not install $app on this Mac"
echo "Installed $bundle_id as $installed_app" >&2

mac_launch() {
  local tick
  # Opening the wrapper starts this copy even when LaunchServices still knows
  # other com.shin1ohno.yebisu bundles, where open -b could pick any. LaunchServices
  # starts it, so launchd (pid 1) is its parent and it does not end with this
  # terminal or the guard's process group.
  open "$installed" || return 1
  for ((tick = 0; tick < timeout_seconds; tick++)); do
    while read -r pid ppid executable; do
      if [[ -n $pid ]] && cmp -s "$app/yebisu" "$executable"; then
        printf 'pid %s\nparent pid %s\nexecutable %s\nlaunched %s\n' "$pid" "$ppid" "$executable" \
          "$(date '+%Y-%m-%dT%H:%M:%S%z')" >"$out/launch.txt"
        return 0
      fi
    done <<<"$(app_processes)"
    sleep 1
  done
  pid=
  return 1
}

pid=
status=0
step=
reason=
log_state="not copied (not launched)"
# Stale results from an earlier run must not stand in for this one's.
rm -f "$out/launch.txt" "$out/running.txt" "$out/summary.txt" "$out/yebisu.jsonl"
if [[ $launch != true ]]; then
  state="installed, not launched"
elif ! mac_launch; then
  state="launch failed" status=1 step=launch reason="could not launch $bundle_id"
else
  echo "Launched $bundle_id (pid $pid, parent pid $ppid); waiting ${wait_seconds}s" >&2
  sleep "$wait_seconds"
  if ! row=$(app_processes --pid "$pid"); then
    state="unknown (processes not listed)" status=1 step=running reason="could not list processes"
  elif [[ -z $row ]]; then
    state=exited status=4 step=running reason="the app exited within about ${wait_seconds}s"
  else
    printf '%s\n' "$row" >"$out/running.txt"
    read -r _ ppid _ <<<"$row"
    state="running (pid $pid)"
    if [[ $ppid != 1 ]]; then
      status=1 step=running
      reason="the app (pid $pid) has parent pid $ppid, not launchd (1); it may end with this terminal"
    fi
  fi
  # Best effort, whatever the outcome: the app's own record of its startup.
  log_source=$(cat "$root/crates/yebisu/src/log-location.txt")
  log_state="not copied: no data container for $bundle_id"
  for attempt in 1 2 3 4 5 6; do
    if container=$("${mac_py[@]}" container --containers "$HOME/Library/Containers" \
      --bundle-id "$bundle_id"); then
      if [[ -s $container/Data/$log_source ]] && cp "$container/Data/$log_source" "$out/yebisu.jsonl"; then
        log_state=$out/yebisu.jsonl
        grep -Eq "\"pid\":$pid[,}]" "$out/yebisu.jsonl" && break
        log_state="$out/yebisu.jsonl (no line from pid $pid yet)"
      else
        log_state="not copied: log file does not exist yet: $log_source"
      fi
    fi
    if ((attempt < 6)); then sleep 1; fi
  done
fi

cat <<EOF | tee "$out/summary.txt"
app:        $installed_app
bundle id:  $bundle_id
device:     this Mac ($udid)
state:      $state
log:        $log_state
EOF
if ((status != 0)); then fail "$step" "$status" "$reason"; fi
