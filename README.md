# yebisu

iPad terminal client for Herdr, on GPUI. Phase 0 exercises GPUI 0.3.12 with the UIKit platform. Connection and terminal features arrive in later phases.

Use Rust 1.99.1, Xcode and `just`. Run `just check`, `just lint-ios`, `just test`; `just ios-sim` launches the smoke screen. `just ipad` builds and signs for the connected M5 using team AL9567565U. The login keychain must be unlocked interactively by its owner.

All fetch/build commands must pass the 6 GiB guard. Development builds omit debug symbols to bound disk use. The old app remains installed under its own bundle identifier.
