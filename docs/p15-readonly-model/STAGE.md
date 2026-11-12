# P15 read-only historical f83d7be stage, not product acceptance

Current integration status is in INTEGRATED-CANDIDATE.md. The text below records
the earlier unmounted source stage and is not the current mount status.

Model origin: PR31 at329fbbb6e05085ed76c8065e93ce5539b843c4f1.
Security contract: security-plan-2.md, SHA256
603e65c8f6dc6ded11103db9d4c5633d092036d6625f80201be089f0e5e39ea3.
Independent plan admission: taskfc78cac79b124149/report-plan2-1.md.

The independent modules are not yet mounted into the application. P11's current
dd8128be175a1b394b8362f2078641c57938f406 is a reference, NOT a frozen integration head.
The concrete integration patch is a task artifact and must be rebased onto the
author's frozen head before any runtime or persistence claim.

## Available source

- `host_settings/availability.rs` refuses the current provider before any SSH or
  credential operation. It has no transport, acquisition, callback or retry path.
- `host_settings/reader.py` is the exact fixed program for a future approved
  `/usr/bin/python3 -I -S -B -c` profile. `command.rs` encodes only validated FILE
  path bytes as lowercase hex, with the reviewed 16,384-byte command cap.
- Reader stdout is only raw bytes, at most65,536; missing is exit40. It opens root
  and parents with RDONLY|DIRECTORY|NOFOLLOW, leaf with RDONLY|NOFOLLOW|NONBLOCK,
  rejects nonregular files, and compares pre/post metadata before emission.
- `theme_picker` adds an explicitly chosen high-contrast role override while
  retaining the exact owner-default palette. No Follow option exists. The patch
  routes theme persistence through the single existing SettingsHost.

## Concrete blocked capability

At pinned herdr-gpui398c132a06723a0cc1494842a34666d173ec0200,
`sheprd-client/src/ssh/in_process.rs:170-178,272-274` obtains a key through
`DeviceKey::load_or_create`. Yebisu `setup.rs:84-88` returns a PUBLIC line only.
There is no retained signer in that result. P15 must not use either as authorization.
`herdr-ssh/src/tunnel.rs:138-152` exposes wait_exit and abort-on-drop, not an
abort-and-join/signing-work termination receipt. It cannot lift quarantine.

The planned authority must serialize profile/normalized target/generation/view/
request/path provenance/credential ID+revision/exact pins+trust revision/approved
reader+launch profile and acceptedAt. No boolean capability switches or fabricated
signer are substituted. The old pure Model identity is NOT sufficient for dispatch.
No collector or network adapter is exposed until the owner can supply that context.

## Evidence boundary

Task9c49b897b8024dfe/check-reader-1.py executed one local batch with run-created
fixtures. The exact reader was observed in-process using CPython audit events,
including actual NOFOLLOW open flags. An actual O_RDWR mutant was rejected by
the same oracle despite unchanged bytes. The restored candidate passed again.
Two distinct local files were also read through real `/bin/sh -c` commands and
the exact interpreter flags; command bytes and fixture before/after hashes were
captured. These are local reader observations, NOT SSH peers or Rust adapter proof.
Interpreter/shell global side effects were not audited; permission, replacement
race, bounded cancellation, stderr flooding and EOF-before-status remain untested.

The future collector must enforce acceptance+20s (including queue time), poll at
most50ms, require exit0 after EOF, discard stderr without decoding/logging, and
quarantine the app-owned service after uncertain teardown with one unresolved
attempt token. This stage has no executing collector and makes no such IO claim.
Native check/lint-ios/test, Simulator and physical QA remain unverified.
