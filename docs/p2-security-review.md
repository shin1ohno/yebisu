# P2 security review

The worker performed this source review. The brief prohibits spawning another
worker; independent Codex review is owned by the controller and remains separate.
This document is not a claim of independent review or physical device QA.

- Credential boundary: `ssh::authorized_key()` uses the pinned client's
  `herdr-ssh` label. `device_key/keychain.rs` at 398c132a0 uses a default access
  group, a non-synchronizable ThisDeviceOnly key, and a compile-time Secure
  Enclave branch on physical iOS. `scripts/ios/profile.py` grants only the new
  app's Keychain group. An identical label does not grant access to the old
  app's group. Physical signed entitlements must still be compared by the owner.
- Display boundary: `keys::authorized_line` is the only producer of a public-key
  line for UI/clipboard. It checks `restrict`, type/blob fields and the precise
  permitted old comment before producing a safe bounded yebisu comment. It
  refuses control characters and arbitrary comments. No private key is exported.
- Host trust: scan sends no credentials. Approval holds the exact NewHostKey
  from that scan, bound to its target. Editing clears approval; jobs serialize
  operations and disable input. The dependency refuses keys pinned meanwhile.
  Authentication first requires a Trusted scan, and the authenticated
  connection checks pins again, so a scan/connect race cannot bypass trust.
- Changed keys: there is no forgetting or automatic replacement action in P2.
  Refusals are displayed, clear pending approval, and do not call authentication.
- Persistence: endpoint writes use sync plus atomic rename. Host fingerprints
  are derived from the actual known_hosts pins on every load, never cached
  separately. A partial trust/endpoint-save failure remains an explicit error;
  another scan and authentication can retry endpoint saving.
- UI thread: start_job submits execute to the background executor, then awaits
  from an async UI task. Scan, trust, probe, Keychain and disk access occur only
  in execute. Completion updates the view. Observation uses a bounded try_send
  queue and a separate writer; no file writes run in render.
- Observation is opt-in (YEBISU_P2_OBSERVE) and records public lines, target,
  fingerprint and status in the app's own Documents directory. It records no
  private key, password, token or remote output content.

Named negative tests live in keys/tests.rs, hosts/tests.rs and setup/tests.rs.
The native iOS SSH implementation is compiled by both lint targets; host tests
exercise the consumer gate and real KnownHosts persistence. A live hostile
SSH endpoint and physical authentication remain distinct device acceptance.
