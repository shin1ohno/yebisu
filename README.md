# yebisu

iPad terminal client for Herdr, on GPUI. P4 provides a full-screen Herdr 1 terminal with semantic keyboard input, Japanese composition, and manual reconnect on GPUI 0.3.12 / UIKit. Physical M1 acceptance remains an owner check.

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

## P4 keyboard and IME

`just ios-p4` types into a private in-process peer, exercises the actual IME
callbacks, checks exact semantic input receipts, and saves Simulator screenshots.
It never connects to an owner's running Herdr session. `docs/device-check.md`
contains the physical M1 sequence; `docs/variants.md` lists the runtime Option,
Command, Esc and candidate-anchor choices in the **Input options** toolbar.

`python3 scripts/p4-mutation-check.py --just /absolute/path/to/just` verifies that
nine intentional regressions fail behavioral assertions, then restores every
source file. Run it only with no concurrent edits or builds.
