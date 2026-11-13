"""Validate CI input before building; never print injected values."""
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


def prepare(env, root):
    missing = [name for name in REQUIRED if not env.get(name)]
    if missing:
        raise ValueError("GitHub environment testflight is missing secrets: " + ", ".join(missing))
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
    if not env["ASC_KEY_P8"].strip().startswith("-----BEGIN PRIVATE KEY-----"):
        raise ValueError("ASC_KEY_P8 must contain the PEM private key, not base64")
    # Apple allows at most four digits in the first build-number component.
    if not re.fullmatch(r"[1-9][0-9]{0,3}", env.get("GITHUB_RUN_NUMBER", "")):
        raise ValueError("GITHUB_RUN_NUMBER must be 1..9999; migrate the build-number scheme before 10000")
    version = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version):
        raise ValueError("TestFlight requires a numeric major.minor.patch Cargo version")
    if env.get("GITHUB_REF_TYPE") == "tag" and env.get("GITHUB_REF_NAME") != "v" + version:
        raise ValueError("version tag must equal v followed by the workspace Cargo version")
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
    directory = Path(env["RUNNER_TEMP"]) / "yebisu-testflight"
    directory.mkdir(mode=0o700, exist_ok=False)
    for name, data in blobs.items():
        with (directory / name).open("xb") as stream:
            os.chmod(stream.name, 0o600)
            stream.write(data)


def main():
    try:
        prepare(os.environ, Path(__file__).resolve().parents[2])
    except (ValueError, OSError, KeyError):
        # ValueErrors above contain only our fixed labels; filesystem errors may
        # include paths, so suppress their details.
        error = sys.exception()
        print("error: " + (str(error) if isinstance(error, ValueError)
                           else "cannot prepare private TestFlight files"), file=sys.stderr)
        return 1
    print("TestFlight inputs validated; private files prepared")
    return 0


if __name__ == "__main__":
    sys.exit(main())
