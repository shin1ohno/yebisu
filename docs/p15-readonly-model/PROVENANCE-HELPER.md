# Shared P11/P15 compiler-to-install provenance

Status: source candidate; native and independent review gates are UNRUN.

Use `provenance_session.py:build_install(directory, source, device, bundle_args,
environment)` from either collector. `directory` must be a newly created run-owned
directory below YEBISU_OWN_ROOT. The source checkout and CARGO_TARGET_DIR must also
be below that root; YEBISU_EXPECTED_HEAD must be its full clean commit. Pass the
leased Simulator UUID, never a device name. The return value contains the run app,
installed app and binding.json paths. Do not separately install or re-sign them.
The session owns an exclusive lock from bundle construction through receiving
verification. A failed/abandoned run cannot be resumed using the same directory.
Only cooperative writers are in scope: no defense against malicious same-UID
processes rewriting validators, locks or evidence. One Mini build remains required.

`bundle.sh` captures the full actual `cargo rustc` JSON and `cargo metadata --locked`
with the same feature/target arguments. Metadata is snapshotted before compilation.
All resolved Git dependency revisions and tracked blobs are matched to Git objects;
all compiler artifacts bind package, manifest, target source, features, profile and
parsed depfile input hashes. Workspace and Git SDK artifacts must be fresh; cached
artifacts fail closed. Prepare a fresh owned target or a separately reviewed cache
invalidation under the controller lease. Never remove another task's target/cache.
Registry caches retain Cargo's trust model; this is not hermetic compiler attestation.
Generated inputs are recorded and rechecked, not claimed to be tracked source.
Unsupported depfile/metadata layouts fail closed. Full metadata/JSON and depfiles
remain on disk along with unsigned, signed and installed executable copies.

The code identity accepts only little-endian thin ARM64 MH_EXECUTE with exactly one
LC_UUID, one pre-existing LC_CODE_SIGNATURE and one terminal __LINKEDIT. It hashes
EVERY byte before the fixed aligned signature offset, normalizing only the signature
command datasize and __LINKEDIT filesize. A bounded terminal signature SuperBlob
may change and grow/shrink. All other load commands, section data, non-signature
linkedit bytes, UUID, VM sizes/protection, offsets and padding remain bound. An
added signature command, moved range, changed VM size, universal binary or malformed
layout is rejected. This conservative contract can reject legitimate signing layouts;
it must not silently fall back to UUID/text-only checks. `codesign --verify --strict`
remains mandatory. Fixture SuperBlobs model allowed structure, not cryptographic
validity. An actual linker/ad-hoc pair must be observed under a separate Mini lease.

The copied executable must equal the compiler output by SHA/UUID; signed output
must equal its non-signature code identity. Preinstall checks the signed executable
and exact resource inventory. Installed bytes/UUID and exact resources must equal
the signed bundle, and the source bundle is rechecked after installation. Symlinks,
special files, run reuse and phase reordering are rejected. Bundle/temp/receipt
paths live in the unique run directory, never the executable-adjacent fixed app.
The temporary linker entitlements stay as evidence through install.

P11 adoption: take only the helper commit's bundle.sh, bundle-provenance.py,
macho_identity.py, compiler_inputs.py, provenance_session.py and their tests/doc.
Adapt the collector to `build_install`; retain P11 feature arguments, QA config,
privacy sinks, observer/FIFO gates and its final installed verification. Do not take
P15 settings/store/Cargo pins wholesale. This helper cannot admit SDK changes.
P15/P11 source manifests alone are not compiler-input proof; acceptance additionally
needs exact-head real Mac execution and independent receiving-system review.
