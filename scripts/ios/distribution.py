"""Opt-in Rust iPad distribution stages; plan never builds, signs, or uploads."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import shlex
import subprocess
import sys
import tempfile
import time
import tomllib
import uuid
import zipfile

ROOT = Path(__file__).resolve().parents[2]
BUNDLE = "com.shin1ohno.yebisu"
TEAM = "AL9567565U"
TARGET = "aarch64-apple-ios"


def toolchain():
    return tomllib.loads((ROOT / "rust-toolchain.toml").read_text())["toolchain"]["channel"]


def cache_key(sdk, xcode):
    inputs = [toolchain(), sdk, xcode, TARGET, "release", "default-features",
              hashlib.sha256((ROOT / "Cargo.lock").read_bytes()).hexdigest()]
    return "ipad-" + hashlib.sha256("\n".join(inputs).encode()).hexdigest()


def run(args, **kwargs):
    return subprocess.run(args, check=True, **kwargs)


def required_file(variable):
    value = os.environ.get(variable)
    if not value or not Path(value).is_file():
        raise ValueError(f"{variable} must name an existing secret file")
    return Path(value).resolve()


def distribution_entitlements(profile, method, helper):
    derived = helper.entitlements(profile, BUNDLE, TEAM)
    if derived.get("get-task-allow") is not False:
        raise ValueError("a distribution profile must explicitly disable get-task-allow")
    if profile.get("ProvisionsAllDevices"):
        raise ValueError("enterprise profiles are not supported")
    devices = profile.get("ProvisionedDevices")
    if method == "testflight" and devices is not None:
        raise ValueError("TestFlight needs an App Store distribution profile")
    if method == "adhoc" and (not isinstance(devices, list) or not devices):
        raise ValueError("Ad Hoc needs registered devices in its profile")
    return derived


def profile_helper():
    spec = importlib.util.spec_from_file_location("ios_profile", ROOT / "scripts/ios/profile.py")
    helper = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(helper)
    return helper


def package(args):
    # Files are injected by the CI secret provider. No login keychain is opened.
    certificate = required_file("IOS_P12_FILE")
    profile_path = required_file("IOS_PROFILE_FILE")
    password = os.environ.get("IOS_P12_PASSWORD")
    if password is None:
        raise ValueError("IOS_P12_PASSWORD is required (empty is allowed)")
    if not re.fullmatch(r"[1-9][0-9]{0,3}(\.[0-9]{1,2}){0,2}", args.build_number):
        raise ValueError("build number must satisfy Apple's numeric version format")
    app = args.app.resolve()
    info = plistlib.loads((app / "Info.plist").read_bytes())
    if not isinstance(info, dict) or info.get("CFBundleIdentifier") != BUNDLE or info.get("CFBundleSupportedPlatforms") != ["iPhoneOS"]:
        raise ValueError("expected the yebisu iPhoneOS bundle identity")
    if any(path.is_symlink() for path in app.rglob("*")):
        raise ValueError("bundle symlinks need an explicit packaging policy")
    if any(path.suffix in {".framework", ".dylib", ".appex", ".app", ".xpc"} for path in app.rglob("*")):
        raise ValueError("nested code needs an explicit signing policy; refusing this bundle")
    executable = info.get("CFBundleExecutable")
    if not isinstance(executable, str) or not executable or Path(executable).name != executable or executable in {".", ".."}:
        raise ValueError("bundle executable must be a local filename")
    if not (app / executable).is_file():
        raise ValueError("bundle executable is missing")
    helper = profile_helper()
    try:
        profile = helper.decode(profile_path)
        entitlements = distribution_entitlements(profile, args.method, helper)
    except helper.ProfileError as error:
        raise ValueError("profile validation failed") from error
    if entitlements["application-identifier"] != f"{TEAM}.{BUNDLE}":
        raise ValueError("App ID prefix differs from the verified existing app")
    output = args.output.absolute()
    if output.resolve().is_relative_to(app):
        raise ValueError("IPA output must be outside the input bundle")
    if args.output.exists() or args.output.is_symlink():
        raise ValueError("output already exists; choose a fresh IPA path")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="yebisu-ci-sign-", dir=output.parent) as temporary:
        work = Path(temporary)
        work.chmod(0o700)
        keychain = work / "signing.keychain-db"
        key_password = os.urandom(32).hex()
        staged = work / "Payload" / app.name
        staged.parent.mkdir()
        shutil.copytree(app, staged)
        info["CFBundleVersion"] = args.build_number
        (staged / "Info.plist").write_bytes(plistlib.dumps(info))
        shutil.copyfile(profile_path, staged / "embedded.mobileprovision")
        ent = work / "entitlements.plist"
        ent.write_bytes(plistlib.dumps(entitlements))
        search_list = shlex.split(run(["security", "list-keychains", "-d", "user"],
                                      capture_output=True, text=True).stdout)
        created = False
        try:
            # Creation may change the search list before reporting an error.
            created = True
            run(["security", "create-keychain", "-p", key_password, str(keychain)], capture_output=True)
            run(["security", "unlock-keychain", "-p", key_password, str(keychain)], capture_output=True)
            run(["security", "import", str(certificate), "-k", str(keychain), "-P", password,
                 "-T", "/usr/bin/codesign"], capture_output=True)
            run(["security", "set-key-partition-list", "-S", "apple-tool:,apple:,codesign:",
                 "-s", "-k", key_password, str(keychain)], capture_output=True)
            identities = run(["security", "find-identity", "-v", "-p", "codesigning", str(keychain)],
                             capture_output=True, text=True).stdout
            allowed = set(helper.certificate_hashes(profile))
            identity = next((item for item in helper.parse_identities(identities) if item in allowed), None)
            if identity is None:
                raise ValueError("dedicated keychain has no identity matching the profile")
            run(["codesign", "--force", "--keychain", str(keychain), "--sign", identity,
                 "--entitlements", str(ent), "--timestamp=none", str(staged)], capture_output=True)
            run(["codesign", "--verify", "--strict", str(staged)], capture_output=True)
            archive = work / "signed.ipa"
            run(["ditto", "-c", "-k", "--keepParent", str(staged.parent), str(archive)])
            archive.chmod(0o600)
        finally:
            if created:
                try:
                    run(["security", "delete-keychain", str(keychain)], capture_output=True)
                finally:
                    run(["security", "list-keychains", "-d", "user", "-s", *search_list],
                        capture_output=True)
        # Publish only after cleanup succeeds. Same-filesystem link never overwrites.
        os.link(archive, output)
    print("IPA packaged; App Store acceptance and device update remain unverified")


def ipa_metadata(ipa):
    """Read bounded metadata without extracting files or claiming ASC acceptance."""
    with zipfile.ZipFile(ipa) as archive:
        candidates = [item for item in archive.infolist()
                      if re.fullmatch(r"Payload/[^/]+\.app/Info\.plist", item.filename)]
        if len(candidates) != 1 or candidates[0].file_size > 1_048_576:
            raise ValueError("IPA must have exactly one bounded top-level app Info.plist")
        with archive.open(candidates[0]) as stream:
            info = plistlib.loads(stream.read(1_048_577))
    if not isinstance(info, dict) or info.get("CFBundleIdentifier") != BUNDLE:
        raise ValueError("IPA bundle identity differs from the existing app")
    if info.get("CFBundleSupportedPlatforms") != ["iPhoneOS"]:
        raise ValueError("IPA must contain an iPhoneOS app")
    for field in ("CFBundleVersion", "CFBundleShortVersionString"):
        if not isinstance(info.get(field), str) or not re.fullmatch(r"[0-9]+(?:\.[0-9]+){0,2}", info[field]):
            raise ValueError(f"IPA is missing valid {field}")
    return info


def upload_command(ipa, metadata, key_file, key_id, issuer, apple_id, provider=None):
    if not re.fullmatch(r"[A-Z0-9]+", key_id):
        raise ValueError("ASC_KEY_ID is required")
    try:
        uuid.UUID(issuer)
        if provider:
            uuid.UUID(provider)
    except (ValueError, AttributeError) as error:
        raise ValueError("ASC issuer/provider must be UUIDs") from error
    if not apple_id or not re.fullmatch(r"[1-9][0-9]*", apple_id):
        raise ValueError("ASC_APP_ID must be the app record's numeric Apple ID")
    command = ["xcrun", "altool", "--upload-package", str(ipa.resolve()),
               "--api-key", key_id, "--api-issuer", issuer,
               "--p8-file-path", str(key_file), "--output-format", "json"]
    if provider:
        command += ["--provider-public-id", provider]
    return command


def upload_ipa(ipa):
    if not ipa.is_file():
        raise ValueError("IPA is missing")
    metadata = ipa_metadata(ipa)
    key_file = required_file("ASC_KEY_FILE")
    command = upload_command(ipa, metadata, key_file, os.environ.get("ASC_KEY_ID", ""),
        os.environ.get("ASC_ISSUER_ID", ""), os.environ.get("ASC_APP_ID", ""),
        os.environ.get("ASC_PROVIDER_PUBLIC_ID"))
    # An explicit file path avoids searching shared directories or copying keys.
    run(command, capture_output=True)
    print("Upload accepted by tool; processing, group delivery and device update are pending")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="stage", required=True)
    sub.add_parser("plan")
    key = sub.add_parser("cache-key")
    key.add_argument("--sdk", required=True)
    key.add_argument("--xcode", required=True)
    sub.add_parser("build")
    pack = sub.add_parser("package")
    pack.add_argument("--app", type=Path, required=True)
    pack.add_argument("--output", type=Path, required=True)
    pack.add_argument("--build-number", required=True)
    pack.add_argument("--method", choices=["testflight", "adhoc"], required=True)
    upload = sub.add_parser("upload")
    upload.add_argument("--ipa", type=Path, required=True)
    args = parser.parse_args()
    start = time.monotonic()
    try:
        if args.stage == "plan":
            print(json.dumps({"execution": False, "toolchain": toolchain(), "target": TARGET,
                              "bundle": BUNDLE, "team": TEAM, "profile": "release",
                              "features": "default", "adoption": "compare hosted, reserved Mac, or self-hosted examples"}))
        elif args.stage == "cache-key":
            print(cache_key(args.sdk, args.xcode))
        elif args.stage == "build":
            if not os.environ.get("CARGO_TARGET_DIR"):
                raise ValueError("CARGO_TARGET_DIR must name this job's isolated target directory")
            env = dict(os.environ, HERDR_IOS_TEAM="", HERDR_IOS_BUNDLE_ID=BUNDLE)
            result = subprocess.run(["bash", "scripts/ios/bundle.sh", "--package", "yebisu",
                                     "--bin", "yebisu", "--target", TARGET, "--release",
                                     "--bundle-id", BUNDLE],
                                    cwd=ROOT, env=env, text=True, stdout=subprocess.PIPE, check=False)
            # bundle.sh documents status 3 for an assembled unsigned device bundle.
            if result.returncode != 3:
                raise ValueError(f"expected unsigned bundle status 3, received {result.returncode}")
            app = Path(result.stdout.strip().splitlines()[-1])
            if not (app / "Info.plist").is_file():
                raise ValueError("unsigned bundle output is missing")
            print(app)
        elif args.stage == "package":
            package(args)
        else:
            upload_ipa(args.ipa)
    except (ValueError, OSError, subprocess.CalledProcessError, plistlib.InvalidFileException, zipfile.BadZipFile) as error:
        # Subprocess arguments may contain passwords. Never format the exception.
        detail = "external command failed; output withheld to protect injected secrets" if isinstance(error, subprocess.CalledProcessError) else str(error)
        print(f"error: {detail}", file=sys.stderr)
        return 1
    finally:
        print(json.dumps({"stage": args.stage, "elapsed_seconds": round(time.monotonic() - start, 3)}), file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
