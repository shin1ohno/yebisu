"""Read the JSON files `xcrun devicectl ... --json-output FILE` writes.

Standard library only; run with `python3 -I`.

    resolve FILE [WANTED]
        From `devicectl list devices` output, print IDENTIFIER, UDID,
        TUNNEL_STATE, and NAME separated by the unit separator (0x1F) for the
        physical iOS device WANTED names by name, CoreDevice identifier, or
        hardware UDID, or for the only physical iPad when WANTED is empty.
        Exit 1 when none or several match, and 3 when it is unpaired, without
        Developer Mode, or reports no identifier or hardware UDID.
    error-code FILE
        Print the CoreDevice error code a failed command recorded, if any.
    launched-pid FILE
        Print the process identifier `device process launch` reported.
    running FILE PID
        Exit 0 when `device info processes` lists PID, else 1.

devicectl's result shapes vary between Xcode releases beyond the device list,
so the process readers look for `processIdentifier` anywhere in the result
rather than at one key path.
"""

import json
import sys

# Not a tab: bash's `read` merges runs of whitespace delimiters, so an empty
# field would shift every later one into the wrong variable.
SEPARATOR = "\x1f"


def load(path):
    with open(path, encoding="utf-8") as file:
        return json.load(file)


def physical_ios(device):
    hardware = device.get("hardwareProperties", {})
    return hardware.get("reality") == "physical" and hardware.get("platform") == "iOS"


def text(value):
    """A string field, or "" when devicectl left it out or gave another type."""
    return value if isinstance(value, str) else ""


def describe(device):
    hardware = device.get("hardwareProperties", {})
    return {
        "identifier": text(device.get("identifier")),
        "udid": text(hardware.get("udid")),
        "name": text(device.get("deviceProperties", {}).get("name")),
        "type": text(hardware.get("deviceType")),
        "tunnel": text(device.get("connectionProperties", {}).get("tunnelState")),
        "pairing": text(device.get("connectionProperties", {}).get("pairingState")),
        "developer_mode": text(device.get("deviceProperties", {}).get("developerModeStatus")),
    }


class Unresolved(Exception):
    def __init__(self, message, status=1):
        super().__init__(message)
        self.status = status


def resolve(document, wanted=""):
    devices = [
        describe(device)
        for device in document.get("result", {}).get("devices", [])
        if physical_ios(device)
    ]
    if wanted:
        matches = [
            device
            for device in devices
            if wanted in (device["identifier"], device["udid"], device["name"])
        ]
        if not matches:
            raise Unresolved(f"no paired iOS device is named or has the identifier '{wanted}'")
    else:
        matches = [device for device in devices if device["type"] == "iPad"]
        if not matches:
            raise Unresolved("no iPad has been paired with this Mac")
    if len(matches) > 1:
        names = ", ".join(f"{device['name']} ({device['identifier']})" for device in matches)
        raise Unresolved(f"several devices match; choose one with --device: {names}")
    device = matches[0]
    if device["pairing"] != "paired":
        raise Unresolved(f"{device['name']} is not paired with this Mac", status=3)
    if device["developer_mode"] != "enabled":
        raise Unresolved(
            f"{device['name']} does not have Developer Mode enabled"
            " (Settings > Privacy & Security > Developer Mode)",
            status=3,
        )
    for field, label in (("identifier", "CoreDevice identifier"), ("udid", "hardware UDID")):
        if not device[field] or SEPARATOR in device[field]:
            raise Unresolved(
                f"{device['name'] or 'the device'} reports no {label}; connect it and unlock it",
                status=3,
            )
    return device


def find_values(node, key):
    if isinstance(node, dict):
        for name, value in node.items():
            if name == key:
                yield value
            yield from find_values(value, key)
    elif isinstance(node, list):
        for item in node:
            yield from find_values(item, key)


def error_code(document):
    error = document.get("error")
    if isinstance(error, dict) and isinstance(error.get("code"), int):
        return error["code"]
    return None


def launched_pid(document):
    return next(
        (value for value in find_values(document.get("result", {}), "processIdentifier")
         if isinstance(value, int)),
        None,
    )


def running(document, pid):
    return pid in find_values(document.get("result", {}), "processIdentifier")


def main(argv):
    if len(argv) < 2:
        print(__doc__, file=sys.stderr)
        return 2
    command, path, rest = argv[0], argv[1], argv[2:]
    document = load(path)
    if command == "resolve":
        try:
            device = resolve(document, rest[0] if rest else "")
        except Unresolved as error:
            print(f"error: {error}", file=sys.stderr)
            return error.status
        fields = (device["identifier"], device["udid"], device["tunnel"], device["name"])
        print(*(field.replace(SEPARATOR, " ") for field in fields), sep=SEPARATOR)
    elif command == "error-code":
        code = error_code(document)
        if code is not None:
            print(code)
    elif command == "launched-pid":
        pid = launched_pid(document)
        if pid is None:
            return 1
        print(pid)
    elif command == "running" and rest and rest[0].isdigit():
        return 0 if running(document, int(rest[0])) else 1
    else:
        print(__doc__, file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
