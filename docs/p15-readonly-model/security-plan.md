# P15 read-only security plan (network implementation blocked pending review)

## Scope and authority
The owner D5 override (evidence/A.md:10,14; D.md:21 in task 71c4ec6ae3c444a6)
forbids host writes. This task implements pure parsing/request identity/local palette
models only, with no SSH, command construction, credentials, file IO or UI wiring.
Controller must obtain a separate adversarial review before any network/auth code.
Reference remote.rs at herdr-gpui 398c132a has output64KiB/idle20s (:14-27),
but save (:82-109) is forbidden and read_script (:149-165) is NOT a safe copy:
its lstat-then-cat has a race and it does not reject symlinked parent components.

## Required future adapter contract (proposal, not implemented or approved)
- Resolve only the selected saved host identity, exact transport connection generation
  and application-issued monotonically increasing request ID. Bind credentials and
  already-approved trust to that same immutable identity; never select by display name.
  Changed/missing host key fails closed; no trust prompts, fallback host or new keys.
- One request in flight per selected view; bounded queue capacity1 and latest-state
  completion mailbox. Cancel on switch/disconnect; check identity again before publish.
- Path provenance must be explicitly established. SSH login HERDR_CONFIG_PATH is not
  daemon environment proof (reference remote.rs:23-27). No default/guessed daemon path.
  Show "file settings; daemon effective state unverified". Unknown path is diagnostic.
- No shell interpolation, eval, arbitrary prefix, host-provided commands, stdin payload
  or inherited agent forwarding. Prefer a reviewed read-only subsystem using separate
  argument data. Absolute path only, reject traversal/control/metacharacter paths.
- Read regular files only; reject final and parent symlinks atomically. Component-wise
  directory handles plus no-follow open and fstat are required if peer API lacks this.
  A shell test followed by cat is insufficient. Missing/access/unsafe are distinct.
- Output payload maximum65536 bytes, metadata separately bounded to4096-byte path,
  idle20s plus absolute20s (stricter total cap); cancel closes channel and releases
  buffers. Incremental byte cap BEFORE accumulation; no unlimited retries.
- Never create directories, touch/rename/write files, reload daemon or carry save/CAS.
  Do not log config contents, arbitrary parse messages, private keys or credentials.
  Parser retains allowlisted known fields only and returns fixed error variants.

## Required receiving-side proof before integration acceptance
A controlled two-host peer must record actual outbound operations plus file-open flags
for success, missing, malformed, permission failure, oversized/slow output, cancellation,
malicious path, parent/final symlink and replacement races. Assert no write-open,
create, rename, reload, shell expansion or cross-host credential use; verify before/after
file bytes and daemon state independently. Mutate a test adapter to introduce a write
and prove this receiving assertion fails. Pure reducer/static fixtures are NOT proof.
Native path: exact-head guarded check/lint-ios/test, real settings persistence through
background/relaunch, selected-host switches and owner device gates remain unverified.
No SSH or Mac commands until controller grants isolated checkout lease AND scope.

## Review request
Review this plan before implementing any adapter, especially path capability feasibility,
TOCTOU protection, credential scoping, cancellation and metadata/body byte accounting.
If the available transport cannot enforce the contract, keep the feature unavailable;
do not silently weaken it. This task does not implement a security boundary.
