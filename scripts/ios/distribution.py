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


# Values that must never reach a log, even when a tool echoes them.
SECRET_ENV = ("ASC_KEY_ID", "ASC_ISSUER_ID", "ASC_APP_ID", "ASC_PROVIDER_PUBLIC_ID",
              "ASC_KEY_FILE", "ASC_KEY_P8", "IOS_P12_FILE", "IOS_P12_PASSWORD",
              "IOS_P12_BASE64", "IOS_PROFILE_FILE", "IOS_PROFILE_BASE64")


def secret_values(*extra):
    values = set()
    for value in (*(os.environ.get(name) for name in SECRET_ENV), *extra):
        if not value:
            continue
        lines = [line.strip() for line in value.splitlines() if len(line.strip()) >= 8]
        for item in (value, value.strip(), *lines):
            # JSON output may escape slashes or other characters.
            values.update({item, item.replace("/", "\\/"), json.dumps(item)[1:-1]})
    return {value for value in values if value}


# Derived credentials never equal an injected value: a JWT signed with the .p8
# wraps the key ID and issuer in base64url. Bounded runs keep matching linear.
JWT = re.compile(r"eyJ[A-Za-z0-9_-]{1,4096}\.[A-Za-z0-9_-]{1,4096}\.[A-Za-z0-9_-]{0,4096}")
BEARER = re.compile(r"(?i)\b(bearer)(?:\s|\\[nrt]){1,64}[^\s\"',;]{1,4096}")


def redact(text, secrets):
    for value in sorted(secrets, key=len, reverse=True):
        text = text.replace(value, "[redacted]")
    text = JWT.sub("[redacted]", text)
    return BEARER.sub(r"\1 [redacted]", text)


# altool signs a JWT with the .p8, so its output is shown by allow-list rather
# than by matching every shape a token could take. A line is withheld when it
# names a token (eyJ, bearer) or carries a long base64url-like run, which is
# what the pieces of a JWT split across lines look like. UUIDs stay visible.
WITHHELD = "[withheld: line may carry a credential]"
TOKEN_WORD = re.compile(r"(?i)eyj|bearer")
LONG_RUN = re.compile(r"[A-Za-z0-9_.=+-]{32,}")
UUID_TEXT = re.compile(r"[0-9A-Fa-f]{8}(?:-[0-9A-Fa-f]{4}){3}-[0-9A-Fa-f]{12}")
ALTOOL_TOP = ("tool-version", "os-version", "success-message")
ALTOOL_ERROR = ("code", "message")
ALTOOL_USER_INFO = ("NSLocalizedDescription", "NSLocalizedFailureReason", "NSLocalizedRecoverySuggestion")


def safe_lines(text):
    """Keep lines that name no token, after turning escaped newlines into lines."""
    joined = re.sub(r"\\r\\n|\\[nr]", "\n", text.replace("\r\n", "\n"))
    kept = []
    for line in joined.split("\n"):
        unsafe = TOKEN_WORD.search(line) or any(
            not UUID_TEXT.fullmatch(run) for run in LONG_RUN.findall(line))
        if not unsafe:
            kept.append(line)
        elif not kept or kept[-1] != WITHHELD:
            kept.append(WITHHELD)
    return "\n".join(kept)


def altool_errors(errors, depth=0):
    view = []
    for entry in errors if isinstance(errors, list) and depth < 4 else []:
        if not isinstance(entry, dict):
            continue
        item = {key: entry[key] for key in ALTOOL_ERROR if isinstance(entry.get(key), (int, str))}
        info = entry.get("userInfo")
        if isinstance(info, dict):
            item.update({key: info[key] for key in ALTOOL_USER_INFO if isinstance(info.get(key), str)})
        nested = altool_errors(entry.get("underlying-errors"), depth + 1)
        if nested:
            item["underlying-errors"] = nested
        view.append({key: safe_lines(value) if isinstance(value, str) else value
                     for key, value in item.items()})
    return view


def altool_view(stream, text):
    """Reduce altool's JSON to the fields that diagnose an upload; filter the rest."""
    if stream == "stdout":
        try:
            body = json.loads(text)
        except ValueError:
            body = None
        if isinstance(body, dict):
            view = {key: safe_lines(body[key]) for key in ALTOOL_TOP if isinstance(body.get(key), str)}
            errors = altool_errors(body.get("product-errors"))
            if errors:
                view["product-errors"] = errors
            return json.dumps(view, indent=1, ensure_ascii=False)
    return safe_lines(text)


def shown(argv, secrets, view=None, **kwargs):
    """Run a tool and show its redacted stdout and stderr, never its arguments."""
    label = " ".join(argv[:2]) if argv[0] in {"security", "xcrun"} else argv[0]
    try:
        result = run(argv, capture_output=True, text=True, errors="replace", **kwargs)
    except subprocess.CalledProcessError as error:
        show_output(label, error.returncode, error.stdout, error.stderr, secrets, view)
        raise
    show_output(label, result.returncode, result.stdout, result.stderr, secrets, view)
    return result


def show_output(label, status, stdout, stderr, secrets, view=None):
    print(f"[{label}] exit status {status}", file=sys.stderr)
    for name, text in (("stdout", stdout), ("stderr", stderr)):
        if isinstance(text, bytes):
            text = text.decode(errors="replace")
        if text and text.strip():
            text = view(name, text) if view else text
            print(f"[{label}] {name}:\n{redact(text, secrets).rstrip()}", file=sys.stderr)
    sys.stderr.flush()


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


def load_helper(filename):
    spec = importlib.util.spec_from_file_location("ios_" + filename, ROOT / "scripts/ios" / filename)
    helper = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(helper)
    return helper


def profile_helper():
    return load_helper("profile.py")


# Xcode writes these into every bundle it builds. bundle.sh links with Cargo and
# copies Info.plist, so the distribution build stamps them from the Xcode in use.
# App Store Connect rejects a bundle without them (validate-app 90507 for
# DTPlatformName, 90534 for the SDK and Xcode) and a 64-bit binary whose
# UIRequiredDeviceCapabilities lacks arm64 (90502).
SDK_KEYS = ("BuildMachineOSBuild", "DTCompiler", "DTPlatformBuild", "DTPlatformName",
            "DTPlatformVersion", "DTSDKBuild", "DTSDKName", "DTXcode", "DTXcodeBuild")
APPLE_BUILD = r"[0-9]{1,3}[A-Z][0-9]{1,6}[a-z]?"
SDK_VERSION = r"[0-9]{1,3}\.[0-9]{1,2}(?:\.[0-9]{1,2})?"
XCODE_VERSION = rf"Xcode ([0-9]{{1,2}})\.([0-9])(?:\.([0-9]))?\nBuild version ({APPLE_BUILD})"


def probe(argv, pattern):
    """Read one value from Xcode's tools. An error names the probe, never its output."""
    label = " ".join(argv)
    try:
        output = run(argv, capture_output=True, text=True, errors="replace", timeout=120).stdout
    except (OSError, subprocess.SubprocessError) as error:
        raise ValueError(f"{label} failed") from error
    match = re.fullmatch(pattern, output.strip())
    if match is None:
        raise ValueError(f"{label} printed an unexpected value")
    return match


def sdk_metadata():
    """The Info.plist keys Xcode would write for the iphoneos SDK and this Mac."""
    sdk = probe(["xcrun", "--sdk", "iphoneos", "--show-sdk-version"], SDK_VERSION)[0]
    sdk_build = probe(["xcrun", "--sdk", "iphoneos", "--show-sdk-build-version"], APPLE_BUILD)[0]
    major, minor, patch, xcode_build = probe(["xcodebuild", "-version"], XCODE_VERSION).groups()
    machine = probe(["sw_vers", "-buildVersion"], APPLE_BUILD)[0]
    # DTXcode is four digits: the major version in two, then minor and patch.
    return {"BuildMachineOSBuild": machine, "DTCompiler": "com.apple.compilers.llvm.clang.1_0",
            "DTPlatformBuild": sdk_build, "DTPlatformName": "iphoneos", "DTPlatformVersion": sdk,
            "DTSDKBuild": sdk_build, "DTSDKName": "iphoneos" + sdk,
            "DTXcode": f"{int(major):02d}{minor}{patch or 0}", "DTXcodeBuild": xcode_build}


def require_store_metadata(info):
    """Refuse a bundle App Store Connect rejects before anything signs or sends it."""
    missing = [key for key in SDK_KEYS if not isinstance(info.get(key), str) or not info[key]]
    if missing:
        raise ValueError("bundle lacks Xcode SDK keys App Store Connect requires: " + ", ".join(missing))
    if (info["DTPlatformName"] != "iphoneos" or info["DTSDKName"] != "iphoneos" + info["DTPlatformVersion"]
            or not re.fullmatch(r"[0-9]{4}", info["DTXcode"])):
        raise ValueError("bundle Xcode SDK keys do not describe an iphoneos build")
    capabilities = info.get("UIRequiredDeviceCapabilities")
    if not (isinstance(capabilities, list) and "arm64" in capabilities
            or isinstance(capabilities, dict) and capabilities.get("arm64") is True):
        raise ValueError("UIRequiredDeviceCapabilities must include arm64 for App Store Connect")


def stamp_sdk_metadata(app, metadata):
    path = app / "Info.plist"
    info = plistlib.loads(path.read_bytes())
    if not isinstance(info, dict):
        raise ValueError("unsigned bundle Info.plist must be a dictionary")
    info.update(metadata)
    require_store_metadata(info)
    path.write_bytes(plistlib.dumps(info))


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
    require_store_metadata(info)
    if any(path.is_symlink() for path in app.rglob("*")):
        raise ValueError("bundle symlinks need an explicit packaging policy")
    if any(path.suffix in {".framework", ".dylib", ".appex", ".app", ".xpc"} for path in app.rglob("*")):
        raise ValueError("nested code needs an explicit signing policy; refusing this bundle")
    executable = info.get("CFBundleExecutable")
    if not isinstance(executable, str) or not executable or Path(executable).name != executable or executable in {".", ".."}:
        raise ValueError("bundle executable must be a local filename")
    if not (app / executable).is_file():
        raise ValueError("bundle executable is missing")
    load_helper("icons.py").validate(info, {path.name for path in app.iterdir() if path.is_file()})
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
        secrets = secret_values(password, key_password)
        search_list = shlex.split(shown(["security", "list-keychains", "-d", "user"], secrets).stdout)
        created = False
        try:
            # Creation may change the search list before reporting an error.
            created = True
            shown(["security", "create-keychain", "-p", key_password, str(keychain)], secrets)
            shown(["security", "unlock-keychain", "-p", key_password, str(keychain)], secrets)
            shown(["security", "import", str(certificate), "-k", str(keychain), "-P", password,
                   "-T", "/usr/bin/codesign"], secrets)
            shown(["security", "set-key-partition-list", "-S", "apple-tool:,apple:,codesign:",
                   "-s", "-k", key_password, str(keychain)], secrets)
            identities = shown(["security", "find-identity", "-v", "-p", "codesigning", str(keychain)],
                               secrets).stdout
            allowed = set(helper.certificate_hashes(profile))
            identity = next((item for item in helper.parse_identities(identities) if item in allowed), None)
            if identity is None:
                raise ValueError("dedicated keychain has no identity matching the profile")
            shown(["codesign", "--force", "--keychain", str(keychain), "--sign", identity,
                   "--entitlements", str(ent), "--timestamp=none", str(staged)], secrets)
            shown(["codesign", "--verify", "--strict", str(staged)], secrets)
            archive = work / "signed.ipa"
            shown(["ditto", "-c", "-k", "--keepParent", str(staged.parent), str(archive)], secrets)
            archive.chmod(0o600)
        finally:
            if created:
                try:
                    shown(["security", "delete-keychain", str(keychain)], secrets)
                finally:
                    shown(["security", "list-keychains", "-d", "user", "-s", *search_list], secrets)
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
        prefix = candidates[0].filename.removesuffix("Info.plist")
        load_helper("icons.py").validate(info, {item.filename.removeprefix(prefix)
            for item in archive.infolist() if item.file_size > 0 and item.filename.startswith(prefix)})
    if not isinstance(info, dict) or info.get("CFBundleIdentifier") != BUNDLE:
        raise ValueError("IPA bundle identity differs from the existing app")
    if info.get("CFBundleSupportedPlatforms") != ["iPhoneOS"]:
        raise ValueError("IPA must contain an iPhoneOS app")
    require_store_metadata(info)
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
    # altool prints JSON (--output-format json) on success and failure. Show
    # only its diagnostic fields (altool_view), with injected values redacted.
    shown(command, secret_values(key_file.read_text(errors="replace")), view=altool_view)
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
            # Probe Xcode before the long build, so a broken toolchain stops it early.
            metadata = sdk_metadata()
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
            stamp_sdk_metadata(app, metadata)
            load_helper("icons.py").compile_icons(app)
            print(app)
        elif args.stage == "package":
            package(args)
        else:
            upload_ipa(args.ipa)
    except (ValueError, OSError, subprocess.CalledProcessError, plistlib.InvalidFileException, zipfile.BadZipFile) as error:
        # Subprocess arguments may contain passwords. Never format the exception;
        # the tool's own redacted output was already shown above.
        detail = (f"external command failed with exit status {error.returncode}; its redacted output is shown above"
                  if isinstance(error, subprocess.CalledProcessError) else str(error))
        print(f"error: {detail}", file=sys.stderr)
        return 1
    finally:
        print(json.dumps({"stage": args.stage, "elapsed_seconds": round(time.monotonic() - start, 3)}), file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
