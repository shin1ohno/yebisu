# P15 read-only adapter plan, revision 2

Status: PROPOSED / awaiting independent plan review. No network/auth implementation.
This revision addresses fc78cac79b124149/report-1.md and contract-1.md; their T2
actual receiving proof remains UNVERIFIED. A future plan PASS is not IO proof.
Original security-plan-1.md is immutable, SHA256:
098dc4709d4470dfc333dbef7dc6ef33c2cca0a05d17e2b9378f077db705e702.
Owner D5 still forbids host writes, save/CAS/reload and hidden write paths
(task71c4ec6ae3c444a6/evidence/A.md:10,14; D.md:21).
No host installation, Keychain creation/recovery/replacement, trust change, SSH,
Mac build or SDK edit is authorized by this revision. Pure parser/model work remains
within the existing scope. All algorithm descriptions below are future design only.

## Primary source basis (re-read, not inferred from the review)
F = /home/shin1ohno/ManagedProjects/herdr-gpui at398c132a06723a0cc1494842a34666d173ec0200.
Y = this task's sources/yebisu at a8070ba3fb049d8db480597bf14499037d04842e.
Exact excerpts and file hashes: primary-evidence-2.txt; revision hashes: plan-hashes-2.json.
- F/crates/herdr-ssh/src/tunnel.rs:165-191: exec accepts Target/ClientKey/Pins and
  a command STRING; copies request state. Pins::None/empty is refused before connect.
- F/crates/herdr-ssh/src/client_key.rs:35-66,130-149: ClientKey is cloneable, holds
  an external signing closure; signing uses spawn_blocking. No read-only key lookup
  is implied by this type, and a blocking signer is not made cancellable by a timeout.
- F/crates/sheprd-client/src/ssh/in_process.rs:126-178,272-294: convenience runners
  resolve pins/key again; client_key calls DeviceKey::load_or_create. authorized_key
  uses the same creating path. F/crates/sheprd-credentials/src/device_key.rs:47-58
  includes recovery/retag/create; private find is at91-98. Never call these from P15.
- Y/crates/yebisu/src/setup.rs:84-88 returns a public authorized line, not a retained
  signing handle. Y/crates/yebisu/src/core.rs:88-99 uses exec_ssh_stream. These specific
  entry points do NOT satisfy approvedClientKey injection; do not parse a public line
  into a pretend signer. Missing injection returns CredentialUnavailable before exec.
- F/crates/herdr-ssh/src/target.rs:31-76: normalization includes user/host/port,
  whereas pin lookup name omits user. Y/crates/yebisu/src/hosts.rs:79-82 reads pins.
- F/crates/herdr-ssh/src/exec.rs:99-128: public-key auth only, command-string exec,
  no PTY/environment request. handler.rs:54-86,89-135 refuses mismatched pins and
  forwarded/unknown channels. Do not infer arbitrary remote login-shell behavior.
- F/crates/sheprd-client/src/ssh/exec_output.rs:15-33 caps TOTAL stdout before append;
  ssh/in_process.rs:133-163 waits for exit as well as bytes. F/crates/herdr-ssh/src/
  exec.rs:162-195 waits beyond SSH EOF; :240-265 and tunnel.rs:96-108 retain stderr.
- F/crates/herdr-ssh/src/tunnel.rs:138-152 exposes wait_exit and abort-on-drop;
  exec.rs:45-59 records normal exit after tunnel completion, not an abort receipt.
- F/crates/sheprd-gpui/src/herdr_settings/remote.rs:23-27 documents SSH-login vs
  daemon path mismatch; :150-158 uses shell predicates then cat, NOT proven lstat
  calls or atomic no-follow lookup. This reader is not copied.
- F/crates/sheprd-client/src/script.rs:117-121 excludes iOS; ssh.rs:521-545 describes
  inherited desktop forwarding/sharing. Neither is a fallback for this plan.
No claim is made that all SSH libraries lack subsystems, or that any host lacks/has
Python. This design selects the verified existing exec API, not an imagined argv API.

## 1. Immutable authority and exact identity
Proposed adapter input: ApprovedReadContext, issued by the existing auth/selection
owner under its serialized authority, containing savedProfileId, normalized Target,
consumer connectionGeneration, process-lifetime viewEpoch, monotonic requestId,
approvedClientKey (retained ClientKey clone), opaque credentialId AND credentialRevision,
exact immutable Pins::Keys set AND monotonic trustRevision, validated FILE path and
path provenance, approved readerProfileId/version, acceptedAt monotonic timestamp.
The SSH read is a NEW connection; connectionGeneration is a publication fence,
not evidence that exec reuses the daemon's authenticated session.

The auth owner must supply the already-authorized signing handle without acquisition
inside P15. No preflight existence check followed by load_or_create; no scan, trust,
replacement, authorized_key convenience call or SDK-private access. If the current
owner cannot supply it, CredentialUnavailable is the concrete supported outcome.
Supplying this handle requires a separately owned/reviewed auth integration change;
this plan does not assert the current consumer already retains it. A retained handle
may become unusable after deletion: return CredentialUnavailable/AuthRefused, never
recover/create/retry. The authorized signing closure must not create keys or log raw
error content. Future live authentication requires its own explicit execution scope.

Before dispatch AND before publish compare the entire context against owner state.
Selection/path/profile/user/port/generation/credential/trust changes invalidate first,
then resolve replacement. Trust revoke/re-add increments revision even with identical
pins. Pin read alone is not a revision subscription: if the auth owner cannot provide
a coherent snapshot plus invalidation events, return UnsupportedIdentityFence before
exec. On revocation no NEW dispatch is allowed; already accepted auth/channel work
is cancelled and its results discarded (do not promise retroactive remote revocation).
Never reset request/view epochs while old callbacks can arrive; checked counter
overflow fails closed. Extend the current pure model identity or wrap it in this
adapter context before wiring it: current host/generation/request/path fields alone
are insufficient for credential/trust/view epochs.

## 2. Fixed reader selection, path provenance and command bytes
Choose ONE future audited ReaderProfile: exact absolute `/usr/bin/python3`, Python3
isolated/no-site/no-bytecode mode `-I -S -B -c`, and a compiled-in fixed program literal.
This is a conditional profile, not an assertion that this path exists on any host.
Interpreter/version/stdlib trust and flags, supported OS semantics and remote exec
login shell startup must be approved on controlled peers first. No PATH search,
installer, alternate interpreter, remote module, temporary script or test/cat fallback.
Missing/unapproved profile yields UnsupportedReader before exec; failure to launch the
exact profile (including exit126/127) also maps to UnsupportedReader, never fallback.
An unverified login-shell/startup environment yields UnsupportedLaunchEnvironment;
no claim of no-host-write follows just from the reader's own source.

Command shape (specification, NOT implemented or executed):
`exec /usr/bin/python3 -I -S -B -c '<fixed audited literal>' '<lowercase hex path bytes>'`
The program literal is fixed/escaped at build time and has a recorded digest. Path
is validated before encoding (absolute, <=4096 UTF-8 bytes, no empty/dot/dotdot/control
components; this consumer currently also restricts to ASCII). Hex alphabet [0-9a-f]
contains no shell syntax; bounded <=8192 hex chars, one single-quoted data argument.
No environment expansion, eval, command from config, prefix, stdin payload or dynamic
code fragment. Set a whole-command cap16384 bytes; if the audited fixed program plus
encoded path exceeds it, reject CommandTooLarge, never truncate. Reader revalidates
hex length, decoded path and components. This allows one fixed SSH exec and interpreter
launch, NOT zero exec. Actual exec bytes must be matched by receiving fixtures.

Path is independently known FILE data with provenance, including a previously verified
SSH-login-derived path. No environment-discovery command is added here. Unknown FILE
path returns UnknownPath; known file may be shown while daemon-effective path/state
is unknown, labelled "file settings; daemon-effective state unverified". Do not guess
an effective daemon path, require daemon equality for file display, or execute the path.

## 3. Atomic no-follow reader algorithm (conditional capability)
The fixed program checks required os.open dir_fd support, O_DIRECTORY/O_NOFOLLOW/
O_NONBLOCK and fstat capability before config lookup; missing functionality returns
UnsupportedReader. The exact platform behavior still requires receiving evidence.
Open `/` with RDONLY|DIRECTORY|NOFOLLOW. Walk parents relative to the held directory
fd with RDONLY|DIRECTORY|NOFOLLOW, retaining/closing descriptors deterministically.
Open final leaf relative to its parent with RDONLY|NOFOLLOW|NONBLOCK; fstat it and
reject every nonregular file BEFORE read. NOFOLLOW protects lookup, not a prior test;
NONBLOCK avoids FIFO-open waiting. Platform errors that cannot distinguish a symlink
from a nondirectory component map to UnsafePath, not a fabricated Symlink certainty.
No CREATE/TRUNCATE/APPEND/write-open, mkdir, rename, chmod, lockfile or reload.

Read at most65537 bytes into a bounded buffer; the extra byte detects TooLarge.
Compare pre/post fstat (device/inode/type/size/mtime_ns/ctime_ns), discard observed
change as ChangedDuringRead. Optionally recheck parent-relative leaf identity using
no-follow stat for detectable replacement; this is diagnostic, not the safety barrier.
A descriptor can still point to a renamed ancestor; in-place concurrent writes can
escape timestamp observation. State explicitly: bounded FILE observation, not frozen
namespace or atomic multiwriter snapshot. Do not add a write/lock for stronger claims.
Only after checks emit raw file bytes to stdout. All fds close in finally on every
ordinary branch. No stdout traceback/config diagnostics; no raw stderr diagnostics.
Uncaught interpreter/system failures are fixed UnknownReaderFailure at the client.
The program has no side effect except reading and writing its SSH stdout stream.
Host interpreter/shell side effects remain a receiving-proof gate, not assumed absent.

## 4. Wire result and bounds (one protocol, no alternative envelope)
TOTAL stdout cap =65536 bytes, INCLUDING ANY metadata. Chosen protocol has NO stdout
metadata: path/identity/provenance stay in the local immutable request. No 65536+4096
allowance. Empty stdout+exit0 is an empty file; missing is a distinct fixed status.
Proposed exit table:0 present;40 missing;41 unsafe/not-regular;42 permission denied;
43 unsupported reader;44 observed change;45 too large;46 reader IO failure.
All nonzero exits discard bytes; unexpected status, signal, absent status or invalid
status/body combination is ProtocolError/UnknownReaderFailure, never Missing/success.
Raw stderr is never decoded, interpolated, serialized or logged. Pattern-match
TunnelExit with `stderr_tail: _`; drop the tail. Never Debug/log full TunnelExit,
SDK errors carrying host text, stdout, TOML parser errors or config bytes. Diagnostics
use fixed enums and bounded opaque request IDs; exact path is shown only in its UI.
SDK currently retains a4KiB stderr tail (F/exec.rs:32,240-265); that is not log permission.

Use herdr_ssh::exec(&target,&approvedClientKey,&pins,&fixedCommand) directly and own
the bounded collector off UI. shutdown(Write) immediately: no stdin data. Reads have
<=50ms poll slices, checking cancellation and acceptedAt+20s before/after each read,
before append and before publish. Read min(4096,remaining+1); reject overflow before
append; never accept a prefix. EOF alone is insufficient; require wait_exit in <=50ms
slices and an exit0 within deadline for success. Stderr/trickle cannot extend total20s;
this total cap is stricter than an idle20s cap. Queue time counts in acceptedAt+20s.
Do not start slow synchronous key lookup at acceptance; required handle is injected.
Existing SDK signer may block despite cancellation (F/client_key.rs:141-149): neither
20s publication cutoff nor dropping Tunnel proves immediate signer or remote teardown.

## 5. Cancellation, teardown and concurrency
Use one app-owned P15 worker/authority service, shared across views; capacity1 latest
pending slot and capacity1 latest-result mailbox. No thread/task per click. Before any
replacement or timeout, invalidate publication; set cancel flag, shutdown(Both)/drop
local stream, drop Tunnel to schedule SDK abort, clear buffer and discard result.
The publication deadline is20s. Proposed local cancel processing budget is<=100ms
(two poll slices), and receiving channel closure target<=1s after cancel; these are
future measurements, NOT established runtime guarantees.

Existing Tunnel exposes no abort-and-join receipt: drop aborts the private task and
normal wait_exit publication may never run after abort (F/tunnel.rs:138-152;
F/exec.rs:45-59). Therefore no repeated new exec may follow an unacknowledged abort.
Quarantine the shared P15 service as UnsupportedTeardownFence, drop queued requests,
and keep a single unresolved-attempt token; view close/reopen must NOT reset it.
Do not infer remote process death from local stream closure. A separately reviewed
SDK termination receipt or actual receiving-side lifecycle oracle is required before
lifting quarantine and supporting repeated cancel/switch reads. Current production
APIs alone do not discharge that condition. No polling with another SSH channel to
inspect/kill the reader and no automatic retries. Signing-task lifetime also needs
review/measurement before claiming a global bound on unresolved signing work.

A normal wait_exit plus stream completion is a local completion observation; verify
channel/reader process lifetime independently in fixtures. If the service cannot
provide its lifetime/fencing contract, return UnsupportedTeardownFence before sending
further reads. This deliberately limits availability rather than weakening no-write
or claiming a receipt the SDK cannot furnish.

## 6. Separate review gates and actual receiving proof
G-plan: independent reviewer evaluates this exact plan hash and primary citations.
T2/G-IO: UNVERIFIED until a later separately authorized actual adapter run. A plan PASS
permits neither an assertion of IO proof nor SSH/build/key/trust permission expansion.
Before implementation, resolve/refuse each missing owner handle/fence/profile; keeping
precise Unsupported diagnostics is a feasible source path, not full P15 acceptance.

Adopt review task fc78cac79b124149/receiving-fixtures-1.json R01-R16, with this revision's
raw-body-only TOTAL cap. Execute the exact adapter+reader under a disposable receiver
that captures accepted command bytes, normalized target, credential/pin identity,
open flags, ALL write/create/rename/reload attempts, closure times, live channel and
signer counts. Capture independent before/after tree bytes and daemon state. Never
install helpers on user hosts; a prearranged instrumented test peer is a separate scope.
Test two hosts, missing/empty/error/oversize, malicious paths and parent/final symlinks,
FIFO and replacement races, cancel/slow/stderr flooding/EOF-before-status, old view and
trust revoke/re-add, key disappearance, and retained-key failure without creation.
Mutate the ACTUAL adapter/reader to attempt writes (including identical-byte writes),
rename/reload or second arbitrary exec; the SAME receiver oracle must fail. Restore
exact candidate and re-run. Static checker/mocked bytes/synthetic receipts cannot pass T2.
Report plan verdict, local source tests, receiving IO, native and owner QA separately.
