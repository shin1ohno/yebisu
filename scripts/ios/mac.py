"""Read what scripts/ios/mac.sh checks about this Mac, its profile and its app.

Standard library only; run with `python3 -I`. Profiles are decoded by
profile.py (`security cms -D -i`, found on PATH so tests can substitute it).

    udid                  Provisioning UDID from `system_profiler
                          SPHardwareDataType` text on stdin
    identities FILE|-     count of Apple Development identities in
                          `security find-identity` output
    profile-lists --profile FILE --device UDID
                          exit 0 only when the profile's ProvisionedDevices
                          lists UDID; ProvisionsAllDevices does not count
    entitlement-app-id FILE|-
                          application-identifier from `codesign -d
                          --entitlements - --xml` output
    container --containers DIR --bundle-id ID
                          the data container whose metadata names ID
    processes --bundle-id ID [--pid PID]
                          from `ps -axww -o pid=,ppid=,comm=` on stdin, print
                          "PID PPID EXECUTABLE" for each process running the
                          executable of an installed (Wrapper/NAME.app) bundle
                          whose CFBundleIdentifier is ID
"""

import argparse
import importlib.util
from pathlib import Path
import plistlib
import re
import sys
from xml.parsers.expat import ExpatError

SPEC = importlib.util.spec_from_file_location("profile", Path(__file__).with_name("profile.py"))
profile = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(profile)

UDID = re.compile(r"^[0-9A-F]{8}-[0-9A-F]{16}$")
IDENTITY = re.compile(r'^\s*\d+\)\s+[0-9A-F]{40}\s+"Apple Development:', re.MULTILINE)
METADATA = ".com.apple.containermanagerd.metadata.plist"
PLIST_ERRORS = (plistlib.InvalidFileException, ValueError, ExpatError)


def read(name):
    return sys.stdin.read() if name == "-" else Path(name).read_text()


def provisioning_udid(text):
    for line in text.splitlines():
        key, _, value = line.strip().partition(":")
        if key == "Provisioning UDID" and UDID.match(value.strip()):
            return value.strip()
    return None


def lists_device(document, device):
    """None when the profile lists the device by its UDID, else the reason."""
    problem = profile.malformed(document)
    if problem is not None:
        return f"is malformed: {problem}"
    if device not in document.get("ProvisionedDevices", []):
        return f"does not list device {device}"
    return None


def container(directory, bundle_id):
    for metadata in sorted(Path(directory).glob(f"*/{METADATA}")):
        try:
            record = plistlib.loads(metadata.read_bytes())
        except (OSError, *PLIST_ERRORS):
            continue
        if isinstance(record, dict) and record.get("MCMMetadataIdentifier") == bundle_id:
            return metadata.parent
    return None


def bundle_of(executable, bundle_id):
    """Whether `executable` is the CFBundleExecutable of an installed bundle with that ID."""
    path = Path(executable)
    if path.parent.suffix != ".app" or path.parent.parent.name != "Wrapper":
        return False
    try:
        info = plistlib.loads((path.parent / "Info.plist").read_bytes())
    except (OSError, *PLIST_ERRORS):
        return False
    return (isinstance(info, dict) and info.get("CFBundleIdentifier") == bundle_id
            and info.get("CFBundleExecutable") == path.name)


def processes(text, bundle_id, pid=None):
    found = []
    for line in text.splitlines():
        fields = line.split(None, 2)
        if len(fields) != 3 or not fields[0].isdigit() or not fields[1].isdigit():
            continue
        if pid is not None and int(fields[0]) != pid:
            continue
        if bundle_of(fields[2], bundle_id):
            found.append((int(fields[0]), int(fields[1]), fields[2]))
    return found


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("udid")
    commands.add_parser("identities").add_argument("file")
    lister = commands.add_parser("profile-lists")
    lister.add_argument("--profile", required=True)
    lister.add_argument("--device", required=True)
    commands.add_parser("entitlement-app-id").add_argument("file")
    finder = commands.add_parser("container")
    finder.add_argument("--containers", required=True)
    finder.add_argument("--bundle-id", required=True)
    runner = commands.add_parser("processes")
    runner.add_argument("--bundle-id", required=True)
    runner.add_argument("--pid", type=int)
    args = parser.parse_args(argv)

    if args.command == "udid":
        udid = provisioning_udid(sys.stdin.read())
        if udid is None:
            print("error: no Provisioning UDID in the hardware overview", file=sys.stderr)
            return 1
        print(udid)
    elif args.command == "identities":
        print(len(IDENTITY.findall(read(args.file))))
    elif args.command == "profile-lists":
        try:
            document = profile.decode(args.profile)
        except profile.ProfileError as error:
            print(f"error: {error}", file=sys.stderr)
            return 1
        reason = lists_device(document, args.device)
        if reason is not None:
            print(f"error: {document.get('Name', args.profile)} {reason}", file=sys.stderr)
            return 1
    elif args.command == "entitlement-app-id":
        try:
            granted = plistlib.loads(read(args.file).encode())
        except PLIST_ERRORS:
            granted = None
        value = granted.get("application-identifier") if isinstance(granted, dict) else None
        if not isinstance(value, str):
            print("error: the signature has no application-identifier", file=sys.stderr)
            return 1
        print(value)
    elif args.command == "container":
        found = container(args.containers, args.bundle_id)
        if found is None:
            return 1
        print(found)
    else:
        for pid, ppid, executable in processes(sys.stdin.read(), args.bundle_id, args.pid):
            print(pid, ppid, executable)
    return 0


if __name__ == "__main__":
    sys.exit(main())
