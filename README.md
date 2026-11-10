# yebisu

iPad terminal client for Herdr, on GPUI. Phase 0 exercises GPUI 0.3.12 with the UIKit platform. Connection and terminal features arrive in later phases.

Use Rust 1.99.1, Xcode and `just`. Run `just check`, `just lint-ios`, `just test`; `just ios-sim` launches the smoke screen. `just ipad` builds and signs for the connected M5 using team AL9567565U. The login keychain must be unlocked interactively by its owner.

All fetch/build commands must pass the 6 GiB guard. Development builds omit debug symbols to bound disk use. The old app remains installed under its own bundle identifier.

## P2 keys and hosts

The initial screen creates/displays this installation's restricted public key,
copies the complete authorized_keys line, and accepts `user@host[:port]`.
Scan without authentication, compare the host's fingerprint, then explicitly
trust it. Authentication always checks pins again; unknown or changed keys
never reach an authenticated connection. Network, Keychain and disk work run
on the background executor. This installation uses only its own app container
and Keychain access group.

`just ios-p2` runs Simulator key creation, clipboard verification and a process
restart, with source-defined controls located by OCR and receiving-side state
checks. `just ios-sim` still runs the P0 rendering/input/lifecycle regression.
`docs/device-check.md` separates physical-device acceptance from Simulator
results. `docs/p2-security-review.md` records the worker's source review;
independent review is separate.
