# This Mac as an iPad app — `just mac`

`just mac` provisions this Apple silicon Mac when needed, builds the release
binary for `aarch64-apple-ios` through the 6 GiB guard, signs it for team
AL9567565U, installs it as an iPad app ("Designed for iPad") and launches it.
The bundle identifier is always com.shin1ohno.yebisu. `just mac 10` waits ten
seconds instead of five before checking that the app still runs. It never
registers a device; `just mac-register` is the owner's one-time step for that.

## Owner steps

1. In an interactive login shell on this Mac, unlock the login keychain
   yourself if it is locked; enter the password only in the macOS prompt and
   never send it to an agent.
2. Once per Mac: run `just mac-register`. This is outward-facing: Xcode
   registers this Mac's Provisioning UDID with team AL9567565U (one of the
   team's yearly device slots) and downloads a development profile that lists
   it. It builds nothing. An agent must not run it.
3. Run `just mac`. When this Mac is not registered yet, it stops with status 3
   and names `just mac-register`.

## Steps and exit statuses

Every failure ends with `error: just mac failed at step STEP: REASON`.

| Step | Checks |
|---|---|
| options | the bundle identifier is com.shin1ohno.yebisu; team and wait are valid |
| preflight | Apple silicon, this Mac's Provisioning UDID, a trusted Apple Development identity (an untrusted one names the WWDR CA G3 intermediate), the iphoneos SDK |
| register / provision | `scripts/ios/provision.sh` finds or asks Xcode for a profile, for the destination `platform=macOS,arch=arm64,variant=Designed for iPad,id=<UDID>` |
| profile-check | the profile's ProvisionedDevices lists this Mac's UDID (ProvisionsAllDevices does not count) |
| build | `scripts/ios/bundle.sh` builds and signs with that profile |
| bundle-check | Info.plist and the signed application-identifier name com.shin1ohno.yebisu; the embedded profile lists this Mac; the signature verifies |
| install | the bundle is wrapped as an .ipa and opened; never replaces another app at /Applications/yebisu.app |
| launch | terminates only an earlier process running the installed com.shin1ohno.yebisu bundle's executable, then `open -b com.shin1ohno.yebisu` |
| running | after the wait the process still runs, with launchd (pid 1) as its parent |

Status 2 is an invalid option or a refused identifier, 3 an unregistered Mac,
4 an app that exited within the wait, and 1 any other failure. Once installed,
the summary (app, bundle id, device, state, log) is printed whatever the
outcome.

## Evidence

`target/ios-mac/yebisu/` holds `summary.txt`, `launch.txt` (pid, parent pid,
executable), `running.txt` (the process after the wait), `yebisu.ipa`, and
`yebisu.jsonl`, a copy of the app's own log. The guard's record is
`target/disk.log`. The iPad's evidence stays in `target/ios-device/yebisu/`,
so a Mac run never stands in for an iPad run.

## How it installs and launches

Verified on 2026-11-24 on a Mac17,9 with macOS 26.7.3 and Xcode 27.2:

- Opening the signed `.app` directly fails with "incorrect executable format".
- A zip whose root is `Payload/yebisu.app`, named `.ipa` and opened with
  `open`, installs `/Applications/yebisu.app`; the bundle itself is
  `/Applications/yebisu.app/Wrapper/yebisu.app`.
- `open -b com.shin1ohno.yebisu` starts it under launchd, from a translocated
  copy whose path ends in `Wrapper/yebisu.app/yebisu`. Its parent is pid 1, so
  it keeps running when the launching terminal closes.
- The app's log is `Library/Application Support/yebisu/logs/yebisu.jsonl`
  inside the data container in `~/Library/Containers/` whose metadata names
  com.shin1ohno.yebisu. A healthy start logs startup, window_opened,
  foreground and active.
- mise's `just` shim exports `RUSTUP_TOOLCHAIN` for its global Rust, which
  would override `rust-toolchain.toml`; `mac.sh` clears it so the pinned
  toolchain and its iOS targets apply.

This covers launch on the Mac only. It is not physical-iPad acceptance, and
IME and hardware-keyboard behavior on the Mac are not checked here.
