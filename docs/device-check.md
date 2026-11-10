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

## P4 / M1 keyboard and IME — one sitting, owner only

The rows below are **not yet checked**. Simulator receipts prove the adapter
and semantic wire path only; they do not prove Japanese IME or hardware keys.
P4 replaces the P3 read-only button with **Open terminal**. Do not run the old
app against the same session while comparing viewport behavior.

1. On the Mac mini, unlock the login keychain yourself in the local terminal
   (never give the password to an agent). Unlock/connect M5. In an interactive
   login shell, run:

   ```sh
   cd /Users/shin1ohno/ManagedProjects/yebisu
   /Users/shin1ohno/ManagedProjects/cos-ad147c708b1e42fb-1/tools/just ipad
   ```

   This command includes the disk guard, build, sign, install and launch. If a
   password prompt appears, enter it locally. Record device/iPadOS and PR SHA.
2. Complete the P2 host/key setup above if this installation is new; the worker
   has not installed a public key on any host. Open the intended disposable
   Herdr 1 terminal. Tap the terminal once. Type `printf 'M1-once\n'` and press
   Return: exactly one command and one output line must appear. Type letters,
   punctuation, spaces and emoji. No doubling or dropped character is allowed.
3. Run `cat` in the disposable shell. Type `abc`, arrows, Backspace and Return;
   check editing and navigation in an interactive shell too. Ctrl-C must stop
   `cat`. In a terminal editor enter insert mode, type a character, press Esc,
   and check normal mode. Hold an arrow for two seconds: it repeats, then stops
   on release. Release Ctrl before its letter and confirm no stuck modifier.
4. For release-event proof, use a disposable shell with Python 3. Paste/run:

   ```python
   import os, select, termios, tty
   fd = 0
   old = termios.tcgetattr(fd)
   try:
       tty.setraw(fd)
       os.write(1, b'\x1b[>11u')  # disambiguation + event types + all keys (1|2|8)
       for _ in range(12):
           if select.select([fd], [], [], 5)[0]:
               data = os.read(fd, 4096)
               os.write(1, repr(data).encode() + b'\r\n')
   finally:
       os.write(1, b'\x1b[<u')
       termios.tcsetattr(fd, termios.TCSADRAIN, old)
   ```

   Hold/release an arrow and Ctrl-letter. Record press/repeat/release bytes
   (`:2` repeat, `:3` release in negotiated CSI-u), and absence of extra presses.
   If that host does not enable `ClientShellKeyboardReportAll`, mark release
   **unverified** and use a host/TUI that does; do not count a visual repeat as
   proof of a release message. The script restores terminal mode on exit.
5. Switch iPad input to Japanese. In an empty shell command line type
   `きょうはにほんごをにゅうりょくする`, convert, and move selection to a middle
   clause before committing. The marked text must remain visible, and the
   candidate window must follow that clause, not the terminal cursor or end.
   Repeat with `a😀かな` and a long composition near the right edge. Confirm once:
   the remote shell must receive only the confirmed string, exactly once.
6. While a clause is selected, rotate portrait → landscape → portrait, then
   show/hide the software keyboard and compare Standard/Compact density.
   Composition and selected clause must survive; candidate placement and text
   must agree with the new grid. Commit and check the remote result once.
7. Start a composition; from a separate trusted desktop controller switch the
   remote pane. The old marked text must disappear and a delayed confirmation
   must not type into the new pane. Tap the new terminal and type fresh text.
   Repeat while opening, replacing and closing a remote popup. In the popup,
   composition and candidates must stay within its painted rectangle.
8. Tap **Hosts** while composing. Before tapping its target field, terminal
   input must not edit the host address. Tap the target field, type/edit an
   address, and confirm nothing reaches the remote terminal. Return via Open
   terminal and check fresh typing. Host trust is still explicit.
9. Unplug the external keyboard while composing; continue with the software
   keyboard, then reconnect it. Confirm caret/candidate position, Esc, Ctrl,
   repeats and one-keypress/one-delivery again. Idle 60 seconds and type again.
10. Start composing, then disable Wi-Fi (or disconnect only the disposable peer).
    Wait for **Disconnected**. Confirm the composition: no text should arrive.
    Restore connectivity and tap **Reconnect**. Only fresh input may arrive;
    neither the old composition nor queued presses may replay. Retry an initial
    connection failure the same way. Version mismatch remains a visible error.
11. Open **Input options** and compare each pair in docs/variants.md, including
    Option dead keys, Command-C, and a middle-clause
    candidate rectangle. Record the preferred values; they reset on app restart.
12. Record each result and any failure in PR QA separately from automated tests.
    Keep all physical boxes unchecked until these observations are made.

## R1 review repairs (P2/P3 on P4)

These are owner checks; automation does not mark them complete.

1. In an interactive login shell, locally unlock the login keychain, connect
   and unlock M5, then run:
   ```sh
   cd /Users/shin1ohno/ManagedProjects/yebisu
   /Users/shin1ohno/ManagedProjects/cos-ad147c708b1e42fb-1/tools/just ipad
   ```
   Enter a keychain password only into the local system prompt. No password
   belongs in command arguments, logs or reports.
2. With an already saved target, launch yebisu and confirm the target field
   contains that address. Open **Developer: P0 rendering check**, tap the input
   field and type `P0-input`. It must contain exactly `P0-input`, with no host
   prefix. Return through **Keys and hosts**: the saved target must remain.
   Relaunch and repeat to cover the asynchronous startup Load.
3. Use an already authorized disposable terminal (do not alter real
   authorized_keys for this review). Verify normal ASCII, CJK, combining
   accents, ZWJ emoji and Nerd Font symbols render in Standard and Compact.
   Hostile protocol cells containing LF/CR or more than 128 UTF-8 bytes are
   refused; ordinary terminal newline output creates rows and is not such a
   cell. These hostile cells and the 16 MiB cache limit are exercised by the
   host painter tests, not by typing a newline into the shell.
4. Return to Hosts and reconnect while rotating/resizing. Confirm the newly
   connected terminal has the current viewport and accepts fresh input.
   The deterministic host callback test captures A, installs B, executes A,
   and observes no resize of B; this physical check complements that test.
5. Record device/iOS version and each actual result in PR QA. Leave physical
   checks unchecked until performed. Existing P4 IME and hardware-keyboard
   checks above remain separate owner acceptance.

## R2 review repairs (P4)

1. Locally unlock the login keychain, connect/unlock M5, and run the absolute
   `just ipad` command in the R1 section from an interactive login shell.
   Keep the password in the local system prompt only.
2. In an already authorized disposable terminal, compose `かな😀変換`, select
   just the emoji clause (UTF-16 2..4), and press Backspace. Confirm `かな変換`
   remains marked, the caret is between `な` and `変`, and no text has reached
   the remote program. Continue editing, then confirm once; the remaining
   composition must arrive exactly once. Repeat with a partial replacement.
3. While composing, press plain Esc. Confirm marked text disappears and the
   remote program receives neither composition text nor Esc. Press Esc again
   without composition and confirm the remote program receives one Esc.
   **Input options** now has Option, Command and Candidate only: the former
   Clear + send choice was removed because UIKit consumes composing Esc.
4. While composing or holding Ctrl-C, change the disposable peer's focused
   pane or popup remotely. Confirm old text, presses and releases do not land
   in the new target. Start a fresh gesture and confirm new input works.
   The host regression deterministically inserts the switch between event
   generation and send; this manual sequence complements that timing test.
5. Repeat the P4 physical keyboard/IME/resize/reconnect checks above and record
   device model, iOS version and actual results in PR QA. Simulator callback
   checks do not establish physical Japanese IME or hardware-key routing.
