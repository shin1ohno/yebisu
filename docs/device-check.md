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

## P2 keys and hosts — owner steps

1. In the Mac mini's interactive login shell, unlock the login keychain yourself
   (`security unlock-keychain ~/Library/Keychains/login.keychain-db` prompts
   locally; never pass the password as an argument). Connect/unlock iPad M5.
   Run `PATH="$HOME/.cargo/bin:$PATH" /Users/shin1ohno/ManagedProjects/cos-ad147c708b1e42fb-1/tools/just --working-directory /Users/shin1ohno/ManagedProjects/yebisu ipad`.
2. Before the migration trial, open the old app's iPad key page and record the
   exact public key blob and its `ssh-keygen -lf` fingerprint. Keep its
   authorized_keys entry and application installed during the trial.
3. Launch yebisu alongside the old app. In Keys and hosts, tap **Create / show
   public key**, then **Copy authorized_keys line**. Paste into a local editor:
   verify it starts `restrict ecdsa-sha2-nistp256`, ends `yebisu` or
   `yebisu-<device>`, and differs in key blob from the old app. On physical iPad
   the pinned credentials implementation creates the key in Secure Enclave;
   a Simulator software key is not proof of this step.
4. Enter `user@host[:port]` for a host you own and tap **Scan host key**. On the
   first Local Network prompt choose Allow. On that host run
   `ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub` (or the displayed algorithm's
   host public key). Compare the entire SHA256 fingerprint. Before Trust,
   **Check SSH authentication** must refuse the unknown host without offering
   a credential. Scan again and, only after a match, tap **Trust the compared
   fingerprint and save**.
5. Append only yebisu's copied line to that host's `~/.ssh/authorized_keys`
   (permissions 600), preserving all existing entries. Tap **Check SSH
   authentication**. Expect `SSH authenticated:` followed by the Herdr probe
   result. A missing/outdated Herdr is reported separately from SSH failure.
6. Force quit yebisu and reopen. The saved target and trusted host SHA256
   fingerprint must be identical. Tap **Create / show public key** again;
   copied blob must also be identical. Authenticate again with a fresh SSH
   connection. Record model, iPadOS, fingerprint, and outcome in the P2 PR.
7. For a disposable SSH test endpoint only, change its host key after trusting
   it. Scan and authentication must refuse it, with no Trust action available.
   Do not change any production host's key for this test.
8. Open the old app and confirm its existing SSH authentication still succeeds.
   Inspect both signed bundles with `codesign -d --entitlements :- <app>`:
   application identifiers and Keychain access groups must be disjoint;
   yebisu must have only its own `AL9567565U.com.shin1ohno.yebisu` group.

All steps above remain unchecked pending physical-device evidence. Never count
Simulator input as physical keyboard/IME acceptance. A save error is visible in
the status line, even if the host-key pin succeeded before endpoint persistence
failed; rescan and authenticate to retry endpoint saving.

Retirement order (N2): record the old public key before the seven-day trial;
finish the feature/device acceptance and trial first; then confirm a fresh
yebisu connection, back up each host's authorized_keys, remove only the recorded
old key blob, and confirm another fresh yebisu connection plus old-key refusal.
Only completion of that last step completes replacement. Never delete keys by
comment prefix.

## P3 terminal display — owner only

1. Unlock the login keychain locally, unlock/connect M5, then run from the
   yebisu repository:
   `/Users/shin1ohno/ManagedProjects/cos-ad147c708b1e42fb-1/tools/just ipad`.
   The worker neither requests nor handles the password.
2. In Keys and hosts, create/show and copy this installation's restricted key.
   **Add that new yebisu key to the intended host's `~/.ssh/authorized_keys`
   first.** Keep the old application's key. Scan the target, independently
   compare the displayed host fingerprint on that host, then Trust.
3. Stop viewing that same Herdr session in the old app so its viewport does
   not compete. Tap Open terminal (read only). Check that it shows the intended
   Herdr 1 default session. Untrusted/changed keys must show refusal, never a
   terminal. P3 intentionally sends no keyboard input.
4. From a separate trusted shell, print Japanese (`日本語 かな カナ 漢字`),
   `┌──┐│└──┘`, ANSI red/green/blue backgrounds, bold/italic/underline, and
   Nerd Font U+F120, U+E0B0, U+F013 in that session. Check CJK occupies two cells,
   box lines join, colour and attributes stay aligned, and Nerd glyphs appear.
5. Tap Standard/Compact in the toolbar (see docs/variants.md). Compare both in
   portrait and landscape, rotating while output changes. Check no clipped
   half glyphs or stale rows, and that the host's terminal dimensions change.
6. Produce sustained output for 30 seconds, leave idle for 60 seconds, then
   produce output again. Check the latest text appears without frozen UI.
7. Disconnect the host/network. Check the toolbar displays Disconnected and
   no old surface remains. Use Hosts then Open terminal after restoring the
   connection. Restart the private test daemon between connections and confirm
   only the new boot's output appears. Do not claim this Simulator observation
   proves physical IME or hardware-keyboard behavior (P4).
8. Record device model, iOS version, both density choices, rotation, disconnect,
   host trust and old-app coexistence results in the PR QA checklist.
