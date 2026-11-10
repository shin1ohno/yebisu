"""Choose an iOS provisioning profile and derive what signing needs from it.

Standard library only; run with `python3 -I`. Profiles are CMS-signed plists
decoded by `security cms -D -i`, found on PATH so tests can substitute it.

    select --profiles-dir DIR... --app-id TEAM.BUNDLE [--device UDID]
           [--identities FILE] [--now ISO]
        Print the best profile's path: not expired, for iOS, its
        application-identifier exactly TEAM.BUNDLE before a wildcard (the more
        specific wildcard first), listing the device unless it provisions all
        devices, holding a certificate among the identities when given, and
        the latest expiry among equals. A profile with fields of the wrong
        type is skipped with its reason. Exit 3 when none qualifies; any
        other failure exits 1 or 2, so a caller can tell "nothing installed"
        from a broken run.
    entitlements --profile FILE --bundle-id ID [--team TEAM] [--out FILE]
                 [--now ISO]
        Write the entitlements the app is signed with: application-identifier,
        team-identifier, keychain-access-groups narrowed to the app's own
        group, and get-task-allow as the profile has it. Nothing else. Fails
        for a profile that is expired or not for iOS.
    identity --profile FILE [--identities FILE]
        Print the SHA-1 of the first signing identity whose certificate the
        profile lists.
    certificates --profile FILE
        Print the SHA-1 of each certificate in the profile.

--identities reads `security find-identity -v -p codesigning` output, from
stdin when FILE is `-`. Nothing here prints a profile beyond its name, UUID,
and certificate hashes; certificates are public, and no key is ever read.
"""

import argparse
from datetime import datetime, timezone
import hashlib
from pathlib import Path
import plistlib
import re
import subprocess
import sys


# select's status when it ran correctly and found nothing; provision.sh asks
# Xcode for a profile only then, never after a crash.
NONE_QUALIFIES = 3


class ProfileError(Exception):
    pass


def decode(path):
    """Return the profile at `path` as a dict, verified and decoded by security."""
    result = subprocess.run(
        ["security", "cms", "-D", "-i", str(path)],
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        raise ProfileError(f"security could not decode {path}")
    try:
        profile = plistlib.loads(result.stdout)
    except plistlib.InvalidFileException as error:
        raise ProfileError(f"{path} is not a property list") from error
    if not isinstance(profile, dict):
        raise ProfileError(f"{path} is not a provisioning profile")
    return profile


def pattern_matches(pattern, app_id):
    """Whether an entitlement value such as TEAM.* or TEAM.ID covers app_id."""
    if pattern.endswith("*"):
        return app_id.startswith(pattern[:-1])
    return pattern == app_id


def as_utc(moment):
    """plistlib returns naive datetimes in UTC; compare them as aware ones."""
    if moment.tzinfo is None:
        return moment.replace(tzinfo=timezone.utc)
    return moment.astimezone(timezone.utc)


def certificate_hashes(profile):
    return [
        hashlib.sha1(certificate).hexdigest().upper()
        for certificate in profile.get("DeveloperCertificates", [])
        if isinstance(certificate, bytes)
    ]


def parse_identities(text):
    """SHA-1s of the valid identities in `security find-identity -v` output."""
    return re.findall(r"^\s*\d+\)\s+([0-9A-F]{40})\s", text, flags=re.MULTILINE)


# Fields read from a profile, with the type each must have. A profile is
# untrusted input: one with an odd shape is skipped, never a crash that would
# end the scan of every other installed profile.
FIELD_TYPES = {
    "Platform": list,
    "Entitlements": dict,
    "ProvisionedDevices": list,
    "DeveloperCertificates": list,
    "ApplicationIdentifierPrefix": list,
    "TeamIdentifier": list,
}


def malformed(profile):
    """The first field with the wrong type, or None."""
    for field, kind in FIELD_TYPES.items():
        if field in profile and not isinstance(profile[field], kind):
            return f"{field} is not a {kind.__name__}"
    pattern = profile.get("Entitlements", {}).get("application-identifier", "")
    if not isinstance(pattern, str):
        return "application-identifier is not a string"
    return None


def unusable(profile, now):
    """Why the profile signs nothing today, or None."""
    problem = malformed(profile)
    if problem is not None:
        return f"malformed: {problem}"
    if "iOS" not in profile.get("Platform", []):
        return "not an iOS profile"
    expiry = profile.get("ExpirationDate")
    if not isinstance(expiry, datetime) or as_utc(expiry) <= now:
        return "expired"
    return None


def rejection(profile, app_id, device, identities, now):
    """Why the profile cannot sign app_id for device, or None when it can."""
    reason = unusable(profile, now)
    if reason is not None:
        return reason
    pattern = profile.get("Entitlements", {}).get("application-identifier", "")
    if not pattern_matches(pattern, app_id):
        return f"for {pattern or 'no app ID'}"
    if (
        device is not None
        and not profile.get("ProvisionsAllDevices", False)
        and device not in profile.get("ProvisionedDevices", [])
    ):
        return "does not list the device"
    if identities is not None and not set(certificate_hashes(profile)) & set(identities):
        return "no certificate matches a signing identity in the keychain"
    return None


def rank(profile):
    """Higher is better: exact app IDs, then specific wildcards, then expiry."""
    pattern = profile["Entitlements"]["application-identifier"]
    exact = not pattern.endswith("*")
    return (exact, len(pattern), as_utc(profile["ExpirationDate"]))


def select(paths, app_id, device=None, identities=None, now=None, load=decode, report=None):
    """The path of the best profile among `paths`, or None."""
    now = now or datetime.now(timezone.utc)
    best = None
    for path in paths:
        try:
            profile = load(path)
        except ProfileError as error:
            if report:
                report(f"skipped {path.name}: {error}")
            continue
        reason = rejection(profile, app_id, device, identities, now)
        if reason is not None:
            if report:
                report(f"skipped {profile.get('Name', path.name)}: {reason}")
            continue
        if best is None or rank(profile) > rank(best[1]):
            best = (path, profile)
    return None if best is None else best[0]


def entitlements(profile, bundle_id, team=None, now=None):
    """The entitlements to sign bundle_id with, limited to what the app uses."""
    # An explicitly chosen profile gets the checks select() applies, so an
    # expired or macOS profile fails here rather than at install time.
    reason = unusable(profile, now or datetime.now(timezone.utc))
    if reason is not None:
        raise ProfileError(f"the profile cannot sign: {reason}")
    granted = profile.get("Entitlements", {})
    prefixes = profile.get("ApplicationIdentifierPrefix") or []
    pattern = granted.get("application-identifier", "")
    prefix = prefixes[0] if prefixes else pattern.split(".", 1)[0]
    if not prefix:
        raise ProfileError("the profile has no application identifier prefix")
    app_id = f"{prefix}.{bundle_id}"
    if not pattern_matches(pattern, app_id):
        raise ProfileError(f"the profile is for {pattern or 'no app ID'}, not {app_id}")
    profile_team = granted.get("com.apple.developer.team-identifier") or next(
        iter(profile.get("TeamIdentifier") or []), None
    )
    if not profile_team:
        raise ProfileError("the profile names no team")
    if team is not None and team != profile_team:
        raise ProfileError(f"the profile belongs to team {profile_team}, not {team}")
    derived = {
        "application-identifier": app_id,
        "com.apple.developer.team-identifier": profile_team,
    }
    # Only the app's own group: a wildcard grant would let this build read
    # every Keychain item the team's other apps share.
    groups = granted.get("keychain-access-groups") or []
    if isinstance(groups, list) and any(
        isinstance(group, str) and pattern_matches(group, app_id) for group in groups
    ):
        derived["keychain-access-groups"] = [app_id]
    if "get-task-allow" in granted:
        derived["get-task-allow"] = bool(granted["get-task-allow"])
    return derived


def profile_paths(directories):
    paths = []
    for directory in directories:
        directory = Path(directory)
        if directory.is_dir():
            paths.extend(
                sorted(
                    path
                    for path in directory.iterdir()
                    if path.suffix in (".mobileprovision", ".provisionprofile")
                )
            )
    return paths


def read_identities(name):
    if name is None:
        return None
    text = sys.stdin.read() if name == "-" else Path(name).read_text()
    return parse_identities(text)


def parse_now(text):
    moment = datetime.fromisoformat(text)
    return as_utc(moment)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    commands = parser.add_subparsers(dest="command", required=True)

    chooser = commands.add_parser("select")
    chooser.add_argument("--profiles-dir", action="append", required=True)
    chooser.add_argument("--app-id", required=True)
    chooser.add_argument("--device")
    chooser.add_argument("--identities")
    chooser.add_argument("--now", type=parse_now)

    deriver = commands.add_parser("entitlements")
    deriver.add_argument("--profile", required=True)
    deriver.add_argument("--bundle-id", required=True)
    deriver.add_argument("--team")
    deriver.add_argument("--out")
    deriver.add_argument("--now", type=parse_now)

    signer = commands.add_parser("identity")
    signer.add_argument("--profile", required=True)
    signer.add_argument("--identities", default="-")

    lister = commands.add_parser("certificates")
    lister.add_argument("--profile", required=True)

    args = parser.parse_args(argv)
    try:
        if args.command == "select":
            chosen = select(
                profile_paths(args.profiles_dir),
                args.app_id,
                device=args.device,
                identities=read_identities(args.identities),
                now=args.now,
                report=lambda line: print(line, file=sys.stderr),
            )
            if chosen is None:
                print(f"no usable provisioning profile for {args.app_id}", file=sys.stderr)
                return NONE_QUALIFIES
            print(chosen)
        elif args.command == "entitlements":
            derived = entitlements(decode(args.profile), args.bundle_id, args.team, args.now)
            data = plistlib.dumps(derived)
            if args.out:
                Path(args.out).write_bytes(data)
            else:
                sys.stdout.buffer.write(data)
        elif args.command == "identity":
            available = read_identities(args.identities)
            listed = set(certificate_hashes(decode(args.profile)))
            match = next((sha for sha in available if sha in listed), None)
            if match is None:
                print(
                    f"no signing identity in the keychain has a certificate in {args.profile}",
                    file=sys.stderr,
                )
                return 1
            print(match)
        else:
            print(*certificate_hashes(decode(args.profile)), sep="\n")
    except ProfileError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
