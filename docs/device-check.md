# P0 physical device checks — pending

Unlock the login keychain interactively and connect/unlock the paired iPad M5. Run `just ipad` from the repository. This selects M5 and team AL9567565U, provisions com.shin1ohno.yebisu if necessary, builds the release binary, signs, installs and launches it. The command preserves the old application. Its output is also saved in target/disk.log, with process evidence in target/ios-device/yebisu/.

A read-only preflight found no installed provisioning profile for the new App ID. The existing provision.sh asks Xcode to register/download one when needed; Xcode's account and developer-service access must remain available. Device signing has not been attempted by the worker.

- [ ] GPUI text, Nerd Font glyph, emoji, rounded bordered quad, underline, image
- [ ] KeyDown and composed text appear through handle_input
- [ ] Background and return redraws
- [ ] Rotation preserves layout
- [ ] Input and async counter update after 60 seconds idle
- [ ] yebisu and old app both installed and launched
- [ ] Signed entitlements differ only in app identity and isolate Keychain groups

Simulator input and lifecycle evidence is in p0-validation.md. Physical IME and hardware-keyboard acceptance remain pending.
