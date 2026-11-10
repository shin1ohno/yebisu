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

## P1 startup regression check

After unlocking the Mac mini login keychain interactively and connecting and
unlocking the paired iPad M5, run `just ipad` from the repository root. Enter
the keychain password only in the macOS prompt; do not send it to an agent.
Launch yebisu and confirm the P0 smoke screen still appears with its title,
input area, checker image, bordered panels, and live counter. Confirm the app
returns to that screen after leaving it and reopening it. This phase changes no
screen behavior. Record the device model, iOS version, app launch result, and
any regression here or in the P1 PR before marking this section complete.

The worker cannot sign or install on a physical device while the login
keychain is locked over SSH. Simulator startup evidence is recorded separately
and does not complete this check.
