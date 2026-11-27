"""Report when xcodebuild lists a Simulator, and send `xcodebuild test` once more when it could not
find the device (issue #142 stage 4, class D).

`xcodebuild test -destination id=<udid>` can fail with "Unable to find a device matching the provided
destination specifier" (class D) for a Simulator created moments before [unproven: xcodebuild's list
lags behind simctl]. On this project's Mac a just-booted device is missing from
`xcodebuild -showdestinations` for 39-64 s while `xcodebuild test` already runs on it (issue #142
PR-4), so the listing is a report, not a gate (owner decision, 2026-11-25): every hosted check calls
`wait_for_destination` right before its `xcodebuild test`, which logs `SIM READY` or `SIM NOT LISTED`
and never fails, and runs it through `run_test`, which sends the same
`xcodebuild test` once more only when the device was not found and no test ran. That resend is a
`QA-RESEND destination-retry` line, counted in the job's budget (resend_budget.py); a second miss is
the check's failure. It never boots, erases or deletes a Simulator.

`watch_boot_tasks` (issue #256) reports, at fixed times after a device is first seen booted, which of the
boot-task daemons that lib.sh's ios_sim_quiet_boot_tasks keeps out are running. Also a report only.
"""
from datetime import datetime, timezone
import json
from pathlib import Path
import re
import subprocess
import threading
import time

LIMIT = 60.0
POLL = 3.0
MISSING = "Unable to find a device matching the provided destination specifier"
# The programs of lib.sh's IOS_QUIET_BOOT_TASKS; remotemanagementd starts the *Subscriber processes.
BOOT_TASK_PROGRAMS = ("remotemanagementd", "finhealthd", "DeviceConfigurationAgent", "healthcontentd", "maild",
                      "intelligencetasksd")
# Seconds after the device is first seen booted: before, inside and after the window (+270-500 s
# after boot on the xcode-27 runner) in which these daemons launched.
BOOT_TASK_CHECKPOINTS = (300, 480, 700)


def listed(output, udid):
    """Whether xcodebuild -showdestinations output names the device `udid`."""
    return any(f"id:{udid}" in line for line in output.splitlines())


def wait_for_destination(project, scheme, udid, limit=LIMIT, poll=POLL, run=subprocess.run, clock=time.monotonic,
                         sleep=time.sleep):
    """Seconds until `xcodebuild -showdestinations -project <project> -scheme <scheme>` named `udid`, or None.

    Prints `SIM READY <udid> after <s> s (<n> listings)`, or `SIM NOT LISTED <udid> after <s> s (<n> listings);
    last listing exit <code>: <tail>` once `limit` seconds passed. A report only: it never fails."""
    start = clock()
    listings = 0
    while True:
        listings += 1
        result = run(["xcodebuild", "-showdestinations", "-project", str(Path(project)), "-scheme", scheme],
                     capture_output=True, text=True)
        waited = clock() - start
        if listed(result.stdout or "", udid):
            print(f"SIM READY {udid} after {waited:.1f} s ({listings} listings)", flush=True)
            return waited
        if waited >= limit:
            last = " ".join(((result.stdout or "") + (result.stderr or "")).split())[-400:]
            print(f"SIM NOT LISTED {udid} after {waited:.1f} s ({listings} listings); last listing exit "
                  f"{result.returncode}: {last}", flush=True)
            return None
        sleep(poll)


def destination_missing(output):
    """Whether an `xcodebuild test` output says it found no device and ran no test."""
    text = output or ""
    return MISSING in text and not re.search(r"^Test (Suite|Case) '", text, re.M)


def destination(command):
    """The UDID of `-destination id=<udid>` in an xcodebuild command, or None."""
    words = [str(word) for word in command]
    for at, word in enumerate(words[:-1]):
        if word == "-destination" and words[at + 1].startswith("id="):
            return words[at + 1][3:]
    return None


def run_test(command, runner, wait=wait_for_destination):
    """`runner()`'s result (a CompletedProcess with text output) for the `xcodebuild test` `command`.

    When the device was not found and no test ran, waits for the listing once more (a report), prints a
    `QA-RESEND destination-retry` line, runs `runner()` once more and returns that result with the line in
    front of its stdout, so the check's resend record counts it. Anything else is returned unchanged; a
    second miss is the caller's failure."""
    result = runner()
    if not destination_missing((result.stdout or "") + (result.stderr or "")):
        return result
    words = [str(word) for word in command]
    project = words[words.index("-project") + 1] if "-project" in words else ""
    scheme = words[words.index("-scheme") + 1] if "-scheme" in words else ""
    udid = destination(words) or "?"
    wait(project, scheme, udid)
    record = {"kind": "destination-retry", "id": udid, "time": time.time(),
              "reason": f"xcodebuild test found no device ({MISSING}) and ran no test; sending it once more"}
    line = "QA-RESEND " + json.dumps(record, sort_keys=True)
    print(line, flush=True)
    again = runner()
    again.stdout = line + "\n" + (again.stdout or "")
    return again


def boot_task_processes(ps_output):
    """Sorted names of the Simulator processes in `ps -axo comm=` output that are boot-task daemons or
    remotemanagementd subscribers. A Simulator process runs from a path inside a runtime's RuntimeRoot."""
    names = set()
    for line in ps_output.splitlines():
        if "/RuntimeRoot/" not in line:
            continue
        name = line.strip().rsplit("/", 1)[-1]
        if name in BOOT_TASK_PROGRAMS or name.endswith("Subscriber"):
            names.add(name)
    return sorted(names)


def booted(udid, run=subprocess.run):
    """Whether `xcrun simctl list devices` shows the device `udid` booted."""
    result = run(["xcrun", "simctl", "list", "devices"], capture_output=True, text=True)
    return any(udid in line and "(Booted)" in line for line in (result.stdout or "").splitlines())


def watch_boot_tasks(udid, checkpoints=BOOT_TASK_CHECKPOINTS, run=subprocess.run, clock=time.monotonic,
                     sleep=time.sleep, stop=None, poll=5.0, limit=1800.0):
    """Waits until `udid` is booted (at most `limit` s), then at each checkpoint prints
    `SIM BOOT TASKS <udid> +<s> s after booted: running <n>: <names>`. A report only: it never fails, and it
    stops early when `stop` (a threading.Event) is set."""
    stop = stop or threading.Event()
    start = clock()
    while not booted(udid, run):
        if stop.is_set() or clock() - start >= limit:
            return
        sleep(poll)
    seen = clock()
    print(f"SIM BOOT TASKS {udid} booted seen at {datetime.now(timezone.utc).isoformat(timespec='seconds')}",
          flush=True)
    for checkpoint in checkpoints:
        while clock() - seen < checkpoint:
            if stop.is_set():
                return
            sleep(min(poll, checkpoint - (clock() - seen)))
        result = run(["ps", "-axo", "comm="], capture_output=True, text=True)
        names = boot_task_processes(result.stdout or "")
        print(f"SIM BOOT TASKS {udid} +{clock() - seen:.0f} s after booted: running {len(names)}: "
              f"{', '.join(names) or '-'}", flush=True)


def start_boot_task_watch(udid):
    """Runs watch_boot_tasks in a daemon thread; set the returned event to stop it."""
    stop = threading.Event()
    threading.Thread(target=watch_boot_tasks, args=(udid,), kwargs={"stop": stop}, daemon=True).start()
    return stop
