"""Check TestFlight CI input, then write the private files; never print values.

`validate` runs before any build or third-party code and writes nothing.
`materialize` runs after the build and before signing, and writes the .p12,
profile and .p8 into a fresh private directory under RUNNER_TEMP.
"""
import base64
import binascii
import os
from pathlib import Path
import re
import sys
import tomllib
import uuid

REQUIRED = (
    "ASC_KEY_ID", "ASC_ISSUER_ID", "ASC_KEY_P8", "ASC_APP_ID", "APPLE_TEAM_ID",
    "IOS_P12_BASE64", "IOS_P12_PASSWORD", "IOS_PROFILE_BASE64",
)
# The only inputs the materialize step receives.
FILE_INPUTS = ("ASC_KEY_P8", "IOS_P12_BASE64", "IOS_PROFILE_BASE64")
DIRECTORY = "yebisu-testflight"


def require(env, names):
    missing = [name for name in names if not env.get(name)]
    if missing:
        raise ValueError("GitHub environment testflight is missing secrets: " + ", ".join(missing))


def decode_files(env):
    """Return the private file contents by filename, checked but not written."""
    require(env, FILE_INPUTS)
    if not env["ASC_KEY_P8"].strip().startswith("-----BEGIN PRIVATE KEY-----"):
        raise ValueError("ASC_KEY_P8 must contain the PEM private key, not base64")
    blobs = {}
    for name, filename in (("IOS_P12_BASE64", "distribution.p12"),
                           ("IOS_PROFILE_BASE64", "app.mobileprovision")):
        try:
            blobs[filename] = base64.b64decode("".join(env[name].split()), validate=True)
        except (ValueError, binascii.Error):
            raise ValueError(name + " must be valid base64") from None
        if not blobs[filename]:
            raise ValueError(name + " must decode to a nonempty file")
    blobs["AuthKey.p8"] = env["ASC_KEY_P8"].encode()
    return blobs


def validate(env, root):
    """Check every input without touching the filesystem except Cargo.toml."""
    require(env, REQUIRED)
    if env.get("GITHUB_REF") != "refs/heads/main":
        raise ValueError("TestFlight runs only from refs/heads/main")
    if env["APPLE_TEAM_ID"] != "AL9567565U":
        raise ValueError("APPLE_TEAM_ID must match yebisu's existing signing team")
    if not re.fullmatch(r"[A-Z0-9]+", env["ASC_KEY_ID"]):
        raise ValueError("ASC_KEY_ID must be an alphanumeric key identifier")
    try:
        uuid.UUID(env["ASC_ISSUER_ID"])
    except ValueError:
        raise ValueError("ASC_ISSUER_ID must be a UUID") from None
    if not re.fullmatch(r"[1-9][0-9]*", env["ASC_APP_ID"]):
        raise ValueError("ASC_APP_ID must be the numeric Apple ID of the ASC app record")
    # Apple allows at most four digits in the first build-number component.
    if not re.fullmatch(r"[1-9][0-9]{0,3}", env.get("GITHUB_RUN_NUMBER", "")):
        raise ValueError("GITHUB_RUN_NUMBER must be 1..9999; migrate the build-number scheme before 10000")
    version = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version):
        raise ValueError("TestFlight requires a numeric major.minor.patch Cargo version")
    decode_files(env)


def materialize(env):
    """Write the private files; refuse if the directory already exists."""
    blobs = decode_files(env)
    if not env.get("RUNNER_TEMP"):
        raise ValueError("RUNNER_TEMP must name the job's temporary directory")
    directory = Path(env["RUNNER_TEMP"]) / DIRECTORY
    # exist_ok=False also refuses a directory or symlink planted by the build.
    directory.mkdir(mode=0o700, exist_ok=False)
    for name, data in blobs.items():
        with (directory / name).open("xb") as stream:
            os.chmod(stream.name, 0o600)
            stream.write(data)


def main(argv=None):
    args = sys.argv[1:] if argv is None else argv
    if args not in (["validate"], ["materialize"]):
        print("usage: testflight-secrets.py validate|materialize", file=sys.stderr)
        return 2
    try:
        if args == ["validate"]:
            validate(os.environ, Path(__file__).resolve().parents[2])
        else:
            materialize(os.environ)
    except (ValueError, OSError, KeyError):
        # ValueErrors above contain only our fixed labels; filesystem errors may
        # include paths, so suppress their details.
        error = sys.exception()
        print("error: " + (str(error) if isinstance(error, ValueError)
                           else "cannot " + args[0] + " TestFlight inputs"), file=sys.stderr)
        return 1
    print("TestFlight inputs validated; no files written" if args == ["validate"]
          else "TestFlight private files written")
    return 0


if __name__ == "__main__":
    sys.exit(main())
