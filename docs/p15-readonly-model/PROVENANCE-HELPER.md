# Shared P11/P15 compiler-to-install provenance, API 2

Status: restricted source candidate, NOT admitted for native use or P11 adoption.
Actual compiler, signing and installation remain UNRUN. Independent phase1 review
blocks S3 at the P15 main caller's pre-admission mkdir/mkdtemp and I1 at the P11
package's default TMPDIR fallback. Neither is corrected by this Mach-O change.
Historical 808/d2bc/27-test records remain historical.
This is a NEW helper API/blob set, not an unchanged808 transfer.

Call `provenance_session.build_install(directory, source, device, bundle_args,
environment, expected=contract)`. All path arguments are canonical absolute paths;
symlinks, `..`, wrong roots, stale/nonempty run directories, overlapping source/run/
target paths, wrong source head and nested wrappers fail before the first owner,
lock or log write. Create an empty dedicated run directory and a dedicated target
under YEBISU_OWN_ROOT. A target inside the checkout is allowed only under .git.
Set YEBISU_EXPECTED_HEAD, CARGO_TARGET_DIR and YEBISU_BUNDLE_PROVENANCE_DIR consistently.
The session retains its flock through bundle/sign/preinstall/install/receiving checks.
Do not resume failed runs, install separately or sign separately. Same-UID hostile
writers and malicious build scripts are outside this cooperative ownership model.

The mandatory contract has exactly these fields (no inferred SDK/profile defaults):

```python
contract = {
    'schema': 'cargo-provenance-v2',
    'head': full_consumer_commit,
    'run_id': str(uuid.uuid4()),
    'package': 'yebisu', 'binary': 'yebisu',
    'target': 'aarch64-apple-ios-sim',
    'features': [], 'no_default_features': False,
    'profile_name': 'dev',
    'profile': {'opt_level': '0', 'debuginfo': 0, 'debug_assertions': True,
                'overflow_checks': True, 'test': False},
    'sdk': [{'name': 'herdr-uikit', 'url': 'https://github.com/shin1ohno/herdr-uikit',
             'revision': independently_expected_full_sdk_revision}],
}
```

The caller must use its own reviewed SDK: P15 b2fa7811fa9fad9592f47b06d851fc0c33facb16;
P11's separate 2dcd revision must be supplied by its source owner in full. Neither
consumer imports the other's Cargo manifests/lock or SettingsHost implementation.
Features are sorted requested Cargo features; artifact features are the actual
per-build-unit set, a subset of metadata's resolver feature union. The application
profile is exact; dependencies/host build units bind their own rustc options/profile.
Only the explicit yebisu Simulator binary is supported in provenance mode. Generic
non-provenance device/example/binary callers retain the existing bundle interface.
HERDR_IOS_ARTIFACT_ROOT retains its default pre-rm/mkdir containment guard. In owned
mode the explicit target root and separate run app root replace that default guard;
neither root is widened to admit an escaped artifact.

`compiler_inputs.py` is also the opt-in RUSTC_WRAPPER. The collector captures the
actual Cargo argv/cwd, full locked metadata and resolve edges, complete Cargo JSON,
config candidate hashes (including absence), environment value hashes, source head/
tracked bytes and independently expected SDK packages/revisions. Cargo includes,
nested wrappers and alternate build-dir configuration are currently unsupported.
Source/config additions or changes fail closed. Config bytes are not dumped into
logs; no credential files are read. Source snapshots do not attest toolchain purity.

For each actual rustc compilation the adapter records argv/cwd, compiler bytes,
context (target vs host), feature cfgs, profile, pre-compile extern object hashes
and generated OUT_DIR inventory. It asks that compiler `--print=file-names` for
output identities, then runs the original invocation. A freshly changed depfile
must have rules for those actual outputs AND its own dep-info output, with the
same inputs. Relative names resolve against that actual rustc cwd; make escaping,
continuations and literal dollar names are parsed without shell interpretation.
Unescaped ambiguous make syntax is unsupported. No adjacent filename guess or
main.rs substring match can establish binding. Rustc's current output formatter
itself does not escape every make-special output character; use ordinary paths.

Cargo final binary/custom-build copies must match a compiler-reported output's
bytes. Non-final artifact bytes are recorded too. Each --extern maps to a resolved
package edge and the captured object bytes; every explicitly expected SDK must
connect to the actual executable's dependency objects. Actual custom-build artifacts
may have executable=null; host/proc-macro contexts stay distinct from target builds.
Unused resolve nodes are preserved in full metadata, not mislabeled compiled objects.
Workspace/Git artifacts must be fresh. Registry cache objects retain Cargo trust.

All named workspace/SDK inputs must match their pre-build tracked snapshots.
Generated inputs must match the inventory captured BEFORE their rustc invocation;
changed/new/unowned inputs reject. Build-script-executed JSON and output inventories
are retained and rechecked. Cargo's stable JSON does not expose complete rerun-if
or external-read provenance: build-script execution/rerun policy, proc-macro reads,
registry source/cache, sysroot, linker/native system libraries remain explicit
Cargo/toolchain trust. This is NOT hermetic compiler-input closure or an assertion
that unreported external reads cannot occur. There is no new vendor/cache framework.

Compiled bytes equal the copied app executable. Signed bytes preserve the admitted
non-signature Mach-O identity; signed/installed binary and resource inventories
must then match exactly. Unsigned/signed/installed snapshots, raw metadata/Cargo
JSON/depfiles and rustc receipts remain in the run. Linker entitlements remain
hashed and immutable through install. Native `codesign --verify --strict` remains
mandatory. The existing nonzero-signature-tail pair is Unsupported, not native PASS;
linker-to-first-sign evidence needs a separately authorized Mac lease.

Mach-O identity schema `arm64-terminal-signature-v2` admits only thin little-endian
ARM64 subtype0 MH_EXECUTE with the explicitly enumerated mapping, section and
load-command forms in `macho_identity.py`. Every nonempty symbol/dyld/linkedit
payload and relocation mode is Unsupported, even if its outer range is in bounds.
Ordinary dynamic apps, including the retained real U1 pair, remain Unsupported.
This is a deliberately restricted structural subset, not actual yebisu readiness.

The terminal signature requires zero allocation tail, unique bounded slots, a
v0x20400 ad-hoc SHA256 CodeDirectory with codeLimit64=0, exact code-slot coverage
(page exponent0/12/14), and bounded identifier/hash arrays. Its executable segment
must equal TEXT; only empty requirements and the specified entitlement/ad-hoc CMS
metadata forms are admitted. Nonzero trailing padding is diagnosed before deferred
header/segment/section/payload subset exclusions. No padding rule is relaxed.

All bytes before the fixed signature offset remain hashed, including opaque
LINKEDIT bytes, UUID, entry point, VM mappings and command strings. Incremental
memoryview hashing virtually normalizes ONLY signature.datasize and LINKEDIT.filesize.
The latter becomes signature_offset minus LINKEDIT.fileoff. File size is checked
before reading (128MiB cap); command bytes/count, sections, signature bytes and slots
are capped at1MiB/4096/16384/16MiB/64. This is an allocation contract, not measured RSS.
Structural placeholder hashes in synthetic fixtures do not establish signature
authenticity, security policy equivalence, deployed kernel/dyld compatibility,
external dylib/OS identity or arbitrary code's behavior when reading its own file.
Native strict verification and actual receiving evidence remain separate gates.

P11 adoption: take the NEW frozen helper's nine paths only (five helper/producer
files, three test files, this document). The self-contained package is based on
its agreed P11 base with all other paths unchanged. P11 owns caller wiring and
must bind returned run_id/binding to case/XCTest/runtime-image receipts, preserving
privacy, observer/FIFO, feature and full26 assertions. Cherry-pick once only after
controller GO. P15 caller wiring is a separate follow-up commit. Helpers alone do
not satisfy P1-P5, physical/key/trust gates, SDK merge or native application checks.
