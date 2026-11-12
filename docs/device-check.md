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

## P5 / M2: saved hosts and reconnect (owner required)

Worker は実機署名・実機操作を行っていない。下記を実機の機種/iPadOS 版とともに PR QA に記録する。Simulator の結果は実機入力の証拠ではない。

1. Mac mini の login keychain を本人が対話的に解錠し、M5 を接続・ロック解除する。repo で `zsh -il -c '/Users/shin1ohno/ManagedProjects/cos-ad147c708b1e42fb-1/tools/just ipad'` を実行する。パスワードはログやエージェントに渡さない。
2. `Keys and hosts` で専用 sandbox SSH の target を入力し `Save connection`。別の sandbox を追加し、終了→再起動して両方が一覧に残ること、各行をタップすると入力欄が切り替わることを確認。保存自体は信頼を付与しない。新しい host は別途 fingerprint を実ホストで照合して Trust する。既存の稼働 Herdr セッションは使用しない。
3. sandbox の terminal を開き `Return: Automatic` の状態で日本語を変換中に iPad をロック。ロック解除後に新しい接続の画面が出ること、変換が消えること、旧変換の確定・保留キー・切断中の文字が sandbox に届かないことを受信ログで確認。新たなタップの後の入力だけが一度届くことを確認。
4. `Return: Automatic` をタップして `Return: Tap reconnect` に変更。ロック→解除し、接続が再開しないこと、`Reconnect` で再開することを確認。好みを docs/variants.md の二択として記録。
5. sandbox に接続して Wi-Fi を切替、または通信を一時停止。切断理由の表示を確認。復旧後に背景→前景、または Reconnect で戻ること。切断中の入力が再送されないことを受信側ログで確認。
6. sandbox 側だけで SSH host key を別鍵に交換して再起動し、背景→復帰する。鍵変更で拒否され、新鍵を自動 pin せず credentials を送らないことを sandbox ログと UI で確認。元の鍵を復元して再接続する。所有者の通常 host の鍵は変更しない。
7. Hosts に戻り `Developer: simulate save failure` をタップ。`could not save or read the connection` と injected failure が画面に出て、保存済み一覧が変わらないことを確認。これは worker の失敗応答を UI まで通す故障注入であり、実際のディスク満杯の証明ではない。実 filesystem write 失敗と旧データ保持は host 自動テストで確認する。
8. P4 の物理キーボード・IME 手順を再確認する。別 target への切替前後に旧 callback/marked text が送信されないこと。

## P6 — snapshot status board (owner)

1. Unlock the login keychain locally; never give the password to a worker. From this checkout run `/Users/shin1ohno/ManagedProjects/cos-ad147c708b1e42fb-1/tools/just ipad` in an interactive login shell. Use disposable/sandbox Herdr hosts, not existing personal sessions.
2. Save two already trusted sandbox hosts and open a terminal. Verify both host headings and all spaces/agents appear in the scrolling board. No real authorized_keys changes were made by this phase.
3. On each sandbox daemon change an agent between working, blocked, done and idle. Record the delay from the daemon snapshot to the board. Repeat for the unselected host, with its rows scrolled out of view. Returning to its rows must show the latest state, without waiting for a new event.
4. Compare each textual harness mark (for example `[Claude]`) and state against that host's daemon snapshot. Unknown harness/status must remain explicit; terminal text alone must not alter a dot.
5. Tap a space, then an agent on each host. Confirm the daemon focuses the requested workspace/pane. Switch hosts while an update is arriving; the terminal must stay on the newly selected host. Compose text before switching and confirm it is discarded, never delivered to the other host.
6. Toggle `Board: Dense` and `Board: Roomy`. Compare row tap accuracy and terminal room. Repeat in narrow Split View and both orientations; scroll to the last host/agent and verify labels remain usable. Record preferred density, device/iPadOS version and any clipping.
7. Lock/unlock with both hosts connected. With automatic return enabled, both must receive new snapshots. Disconnect one sandbox host: its heading must report disconnection while the other continues updating. Reconnect it; old queued input must not replay.

Simulator coverage is separate: `just guarded python3 scripts/ios/p6-check.py` uses two private in-process peers, checks hidden-host snapshot updates, and observes workspace/pane endpoint requests. It does not prove physical IME, hardware keyboard or device update latency.

## R3 review repairs (P5 and P4 check)

1. Locally unlock the login keychain in the system prompt, connect/unlock M5,
   then from this checkout run:
   `zsh -il -c '/Users/shin1ohno/ManagedProjects/cos-ad147c708b1e42fb-1/tools/just ipad'`.
   Do not send the password to a worker. Use only previously authorized sandbox
   hosts; do not connect to existing personal Herdr sessions.
2. On a disposable sandbox with an incompatible daemon, open its terminal and
   immediately lock/unlock the iPad. Repeat with Return: Automatic and Return:
   Tap reconnect. The toolbar must retain `Version mismatch:` and its daemon
   version/reason after return; tapping Reconnect must not dial that host.
   Check the sandbox connection log for no extra connection. Switch to a
   compatible saved sandbox and verify it still connects. Repeat with the
   incompatible host unselected in the board during lock/unlock.
3. Enter a new sandbox target with a pre-existing authorized yebisu key, Scan,
   compare its fingerprint on the sandbox, then Trust. Return to Hosts without
   restarting: the new target must already be in the list. Authenticate it and
   confirm the full list and selected target remain correct. Restart and check
   both remain saved. Repeat the save-failure check from P5: no host may vanish.
4. In a sandbox recording terminal input, compose Japanese text and press Esc:
   the composition disappears and neither text nor semantic Esc reaches the
   peer. Press Esc again without a composition and verify exactly one Esc.
   Repeat the P4 physical IME/hardware keyboard and P5 lock/network/no-replay
   checks above. Record device/iPadOS version and actual results in PR QA.

R3 Simulator-only `YEBISU_R3_QA` injects a version mismatch into the inbox and
never contacts a host. `just guarded python3 scripts/ios/r3-check.py` checks
visible reason persistence and that automatic return/manual tap do not dial.
The strengthened P4 oracle additionally rejects any non-TextCommit peer event
in its scripted composition/cancel/commit sequence. These checks do not prove
physical IME/hardware keyboard behavior.

## R4: P6 review repairs

1. Locally unlock the login keychain without sharing its password. In the yebisu checkout run `/Users/shin1ohno/ManagedProjects/cos-ad147c708b1e42fb-1/tools/just ipad` from `zsh -il`. Record the device model, iPadOS version and PR head.
2. Use disposable sandbox daemons only. Start with pane p1 focused and tap p2. Immediately focus p3 from another sandbox client. After the board shows p3, type a unique token: it must reach p3 exactly once, and another board move must remain available. Repeat while composing Japanese text; the old composition must not reach p3.
3. Keep a board row visible, restart its sandbox daemon (new boot, reuse the pane ID), and tap the old row before its redraw. It must refuse the old tap; after the redraw a fresh tap must work. Repeat after disconnect/reconnect with the same daemon boot to exercise connection-generation rejection.
4. With at least two workspaces and panes, tap a workspace other than the focused one, then a different agent. Confirm each move changes the highlighted daemon focus, terminal contents and the destination receiving a unique typed token. Repeat with each host, while the other host continues updating.
5. Repeat the P6 narrow-width, harness-mark, host-switch and Dense/Roomy checks above. Record the results in PR QA; Simulator output is not proof of physical keyboard or IME behavior.

No new feel-based design fork was found for R4. Existing density and input variants remain in docs/variants.md.

## P7 — tabs, panes and hardware shortcuts (owner, not yet run)

1. Unlock the login keychain locally; never enter its password in this report or a remote prompt. In the yebisu checkout run:
   ```sh
   /Users/shin1ohno/ManagedProjects/cos-ad147c708b1e42fb-1/tools/just ipad
   ```
   Use a disposable sandbox daemon and the already approved P2 trust flow. Do not use a running personal Herdr session for these destructive close checks.
2. Connect a hardware keyboard. Press Command-T once: one new tab appears and has focus. Hold it: there must not be repeated creation. Press Command-1 and Command-2, then tap the corresponding tab strip entries; title, terminal contents and typed text must agree. Add nine tabs and check Command-1 through Command-9 by position, including after closing an earlier tab.
3. Press Command-D and Command-Shift-D: split right and down respectively. Use Command-Option-arrow keys and Command-Option-[ / ] to focus panes. Type a unique label into each. Command-Shift-Enter toggles zoom. Command-W / Command-Shift-W close only the intended sandbox pane/tab.
4. Command-B hides/shows the board. Command-Shift-P opens/closes the action palette. Use its actions by touch; unavailable actions are grey. Escape closes it. Ordinary text or IME commits while it is open must never enter a terminal. P12 will add native menus and the Command-hold list; their absence here is expected.
5. In **Input options**, compare both P7 shortcut-priority variants described in `docs/variants.md`. With Command-as-Control plus Terminal priority, verify Command-B reaches your actual configured prefix and Command-D reaches the shell. With App priority, verify the app operations. In both modes, plain Control-B and your actual daemon prefix must reach the terminal exactly once; test your editor's own chords too.
6. Start Japanese composition, press shortcuts, cancel/commit, then change tab/pane. Confirm no leftover marked text or duplicated commit reaches the new target. Open a daemon popup and check its keys stay with the popup. Unplug/replug the hardware keyboard and repeat Command-T/B/Shift-P.
7. Against a deliberately slow sandbox peer, create a tab or split, immediately resize/rotate and type, and attempt another focus change before the reply. No input may leak to the old/new pane; after the reply, geometry updates and a fresh gesture enables typing. Disconnect or switch host during the request and confirm no stale reply changes the selected host.
8. Rotate with many tabs, toggle the board and palette, and check horizontal tab scrolling at narrow width. Record device/iPadOS version, keyboard model, prefix, preferred variant and each result separately from Simulator evidence.

Simulator evidence uses the private in-process peer only. The Simulator gate creates a tab, splits it, focuses the original tab, opens the palette, and verifies API receipts plus coherent display/input destinations. It cannot establish hardware-keyboard routing or real IME behavior.

## R5 — delayed move projection and delayed reader stop (owner)

1. Locally unlock the login keychain using the system prompt, connect/unlock M5,
   and run this command from the yebisu checkout. Never give the password to a worker:
   ```sh
   zsh -il -c '/Users/shin1ohno/ManagedProjects/cos-ad147c708b1e42fb-1/tools/just ipad'
   ```
   Record the PR head, device/iPadOS and hardware keyboard model. Use disposable,
   already authorized sandbox peers only; do not use personal Herdr sessions.
2. On a controlled sandbox peer, keep p1/revision 7 coherent. Tap p2 and let only
   its correlated success response arrive. Hold the new snapshot/surface, type a
   unique token, then deliver p2/revision 8 snapshot followed by its surface.
   Inspect the sandbox's received messages: the early token must not reach p1
   or be replayed. A fresh token must reach p2 exactly once after its display
   appears. Repeat with the surface arriving before its snapshot.
3. Repeat with p2 snapshot, success response, then p3 snapshot/surface all
   arriving before the next UI update. Typing must resume at p3, and another
   board move must work. Tap the already focused pane: its matching no-op ack
   must release input. Reject a move with an API error: show the error and allow
   a fresh gesture on the still focused pane. Repeat with Japanese composition;
   cancelled marked text must never reach another pane.
4. Keep Return: Automatic enabled. On the controlled sandbox, hold a version
   mismatch in the old connection's event stream; background then foreground
   the app, and release the mismatch only after foreground return starts.
   Check that the exact mismatch reason remains visible and the sandbox's
   connection counter has not increased. Repeat with this host hidden while
   another compatible host is selected. Reconnect must also refuse the stopped
   host; the compatible host must remain usable.
5. Repeat ordinary lock/unlock without mismatch: after the old reader finishes,
   automatic reconnect must recover. Check Tap reconnect as well. Repeat the
   existing P4/P5/P6 physical keyboard, IME, host-switch and no-replay checks.
   Record actual results in PR QA. Revert a device regression that cannot be
   repaired within one business day.

`just guarded python3 scripts/ios/r5-check.py` runs a Simulator-only reader
that emits its mismatch after the Automatic return attempt, then checks OCR
and absence of a dial receipt. `p6-check.py` verifies real Simulator board taps,
focus/surface/input destinations and private peer API receipts. The precise
response/single-poll/key/late-projection order is covered by the host regression
with a gated private wire peer. None proves physical keyboard or Japanese IME.


## R6 — P7 UIKit symbol shortcuts and consumed Escape (owner)

1. Unlock the login keychain locally in the system prompt; never share its password.
   Connect/unlock M5, open an interactive login shell in the yebisu checkout and run:
   ```sh
   zsh -il -c 'cd /Users/shin1ohno/ManagedProjects/yebisu && /Users/shin1ohno/ManagedProjects/cos-ad147c708b1e42fb-1/tools/just ipad'
   ```
   Record PR head, device/iPadOS version and hardware keyboard model/layout.
   Use an already authorized disposable sandbox only; do not use personal sessions
   or change real authorized_keys for this check.
2. Use a US hardware layout and App priority. Create three tabs with distinct
   names/content. Press Command-Shift-] and Command-Shift-[ once each: focus must
   move to the next/previous tab, wrap at the end and send one tab.focus per press.
   Hold each for at least two seconds: no additional action or terminal input may
   occur until release. Release and press again: exactly one new action must work.
3. Check Command-Shift-D (split down), Command-Shift-Enter (zoom),
   Command-Shift-P (palette), Command-Shift-W (close sandbox tab), and
   Command-Option-[ / ] (pane focus). Extra Control must not invoke app actions.
   Repeat the P7 prefix, Terminal priority/Command-as-Control, popup and composing
   Japanese isolation checks above. Those policies must retain their behavior.
4. In a sandbox recording semantic terminal input (test report-all both off/on),
   open the palette and hold Escape for at least two seconds. It must close once;
   the peer must receive no Escape press, repeats or release. Release Escape,
   then press/release it with the palette closed: the ordinary Escape must reach
   the terminal once, plus its release only when report-all is enabled. Repeat
   after Command-Shift-P and after opening the palette by touch.
5. Record each result in PR QA. The Simulator R6 controls inject the exact
   UIKit-shaped GPUI events and verify private-peer receipts; they do not prove
   physical key delivery or real IME behavior.

### R6 follow-up — lost release on Hosts

After the R6 Escape-hold check, open the palette, press and hold Escape, tap
Hosts while still holding it, then release on the setup screen. Return to the
same sandbox terminal. A fresh Escape must reach the terminal immediately;
its release is sent only in report-all mode. Repeat in both report-all modes,
then repeat the ordinary palette press/held-repeat/release no-wire sequence.
Record this result and device/keyboard details in PR #14 QA.

## SH1-146 / SH1-147 maintenance acceptance

herdr-uikit is pinned to main merge commit
`b2fa7811fa9fad9592f47b06d851fc0c33facb16`. Normal startup now enables a
bounded yebisu diagnostics file writer. Its single container-relative path
is in `crates/yebisu/src/log-location.txt`; Rust and device.sh read that file.
No existing old-app data is moved. See `docs/logging.md` for the privacy and
retention contract.

1. In the Mac mini's local terminal, unlock the login keychain yourself using
   `security unlock-keychain ~/Library/Keychains/login.keychain-db`. Never
   share the password. Connect and unlock the paired M5.
2. In an interactive login shell in this checkout, run
   `/Users/shin1ohno/ManagedProjects/cos-ad147c708b1e42fb-1/tools/just ipad`.
   Record PR head, device model, iPadOS version and launch PID. Verify the
   summary says `state: running` and prints the copied local log path
   `target/ios-device/yebisu/yebisu.jsonl`.
3. Open that local JSONL file. Verify this launch's PID and a `startup` and
   `window_opened` event, with timestamps after this launch. Background and
   foreground the app, then connect and disconnect a disposable sandbox host;
   rerun log collection and verify corresponding state event codes. Arbitrary
   host errors, terminal output and typed/IME/clipboard content must not be
   present. Do not use live personal secrets for this check.
4. If startup could not yet produce the file, collection retries at most six
   times, with a 5-second timeout per copy and 1-second intervals. Verify the
   plain `log file does not exist yet` message and that the separate launched /
   running state is preserved. Other transfer errors are reported separately.
   Deterministic missing/delayed/transfer-error tests run without a device.
5. Record the actual physical results in PR QA. Simulator container/file and
   privacy tests do not prove device signing or hardware/IME behavior.

## P8 / M5 selection and clipboard (owner, physical M5)

Use an already authorized sandbox daemon; never an existing working Herdr session.
Unlock the login keychain locally, connect/unlock M5, then run the provided absolute
just binary with `ipad` in this repository. Record device/iPadOS and this PR head.

1. Print `日本語 ABC` in the sandbox. Tap **Select**, then drag with the trackpad
   across the text. Confirm highlighted cells match the drag, press **Copy**, and
   paste into Notes. Confirm CJK characters appear once and no terminal input ran.
2. Repeat with a finger. Compare **Start: Select / double tap** with **Start: Drag**.
   Turn **Copy: On selection** on/off and compare Notes' pasteboard result after
   releasing the drag. Choose the preferred feel without rebuilding.
3. End selection, focus the terminal and copy `P8-text-かな` in Notes. Use the native
   **Paste** edit menu, then Command-V; each action must paste exactly once. Confirm
   multiline text is sent as semantic paste, not individual key presses.
4. Copy an image in Photos/Files. Use **Paste image**. For an item offering both
   text and image, ordinary **Paste** prefers text and **Paste image** chooses image.
5. Copy a large PNG (encoded size above 16 MiB, below 128 MiB, dimensions below
   64 Mi pixels). Paste image; confirm the sandbox receives an image, UI stays
   responsive and subsequent typing follows the paste. Confirm transfer payload
   fits the protocol's 16 MiB cap after resizing. Copy malformed/unsupported data
   and confirm refusal without sending any path or opening a local file.
6. During image preparation switch hosts, panes, open Hosts or background the app.
   Confirm neither old paste nor held typing reaches the new target. Reconnect and
   paste fresh text successfully. A second paste during preparation must report busy.
7. Start IME composition, enter selection and copy; confirm no uncommitted text
   reaches the daemon. Print replacement output over a selection and confirm the
   selection clears. Display a Kitty image from the sandbox and check it renders.

Simulator receipt checks are separate from trackpad, touch, native menu and physical
large-image acceptance. No physical checks above have been marked complete.

### P8 review repairs: shortcut precedence and outside release

1. With a hardware keyboard, try all four combinations of **Command: Ctrl / Local**
   and **Shortcuts: Terminal priority / App priority**. With Ctrl and Terminal
   priority together, Command-C/V must reach the sandbox as Ctrl-C/V without
   reading or changing the clipboard. In the other combinations, Command-C copies
   the selection and Command-V pastes once; neither sends terminal key events.
2. Enable **Copy: On selection**, begin a trackpad selection and release outside
   the terminal. Paste into Notes and confirm the selected text was copied once.
   Move back over the terminal without pressing a button: selection must not grow.
   Repeat with automatic copy disabled: release still ends the drag, without copy.

### P8 second review: selection exit and SDK limitations

1. Show the software keyboard, start an IME composition, then enter Select mode.
   A commit delivered while selection is active must not reach the sandbox.
2. Press the app's **Paste** button without tapping the terminal again. Type plain
   text, then compose and commit Japanese text. Verify paste, plain text and IME
   commit arrive in that order. Repeat by turning **Select** off and typing.
3. In each combination of **Command: Ctrl / Local** and **Shortcuts: App / Terminal
   priority**, use native Paste and Copy. Paste must send clipboard text once;
   Copy must copy the selection. Neither may send Ctrl-V/C. Native Cut copies the
   selection; native Select All does nothing. Physical Command-C/V still follow
   the precedence rules above.
4. The legacy Paste/Select-off implementation accepts callbacks after resumption
   without knowing their composition identity. Record any late old commit as a
   SH1-156 failure. The earlier D4 allowance is superseded by SH1-156 acceptance;
   a successful paste does not establish safe native cancellation.

### P8 extra-modifier regression

In each Command: Local / Ctrl × Shortcuts: App / Terminal priority combination,
check plain Command-C/V and Command-Shift-V, then add Option, Control and Function.
Only the exact clipboard chords may access the clipboard. With Command: Ctrl,
Option-Command-C/V must reach the sandbox as Ctrl-Alt-C/V, without copying or
pasting. Command-Shift-C is also an unmatched chord. With Command: Local, unmatched
Command chords remain platform-local. Native Copy/Paste must retain their separate
behavior. Record keyboard layout and the modifiers iPadOS actually delivers.

## P9 / M6 — Herdr 2 terminals

Worker validation uses a private socketpair core wire peer. It does not establish
physical keyboard/IME behavior or contact any running owner service. The current
UIKit pin is `b2fa7811fa9fad9592f47b06d851fc0c33facb16` (merged PR #7).

1. Unlock the login keychain locally, connect/unlock M5, then run from the yebisu
   checkout in an interactive login shell:
   `/Users/shin1ohno/ManagedProjects/cos-ad147c708b1e42fb-1/tools/just ipad`.
   Never send the password to an agent. Record device model and iPadOS version.
2. Use an explicitly authorized disposable host with `herdr-core serve` running
   and the `hc connect` bridge available in its SSH command environment. Select
   its saved SSH target, verify its fingerprint through the existing Trust flow,
   and tap **Open Herdr 2**. Confirm the remote shell and host identity.
3. Type ASCII and Japanese through the software keyboard. Compose multiple
   clauses, move the selected clause, delete part of an emoji clause, commit once,
   and cancel composition with Escape. Confirm no uncommitted text reaches the
   shell. Rotate with a clause open: candidate position must track painted text.
4. Attach a hardware keyboard. Under **Input options**, exercise all combinations
   of Option Native/Alt, Command Local/Ctrl, Candidate Clause/Caret and Shortcuts
   App/Terminal priority. Check arrows, Escape, Control-G, Command-G, Option-X,
   repeats and release; confirm one delivery. Paste plain text using Command-V
   and the native Paste action; Command: Ctrl with Terminal priority must deliver
   Control-V instead of reading the clipboard. Herdr 2 supports text paste;
   image paste remains a Herdr 1 operation. Repeat with a program requesting
   kitty key reports and after unplug/replug. Candidate options are visual only.
5. Tap **Split (2 panes)**. Both remote terminals must remain visible. Tap
   **Pane 1** and **Pane 2**, type distinct markers, and confirm routing only to
   the selected pane. Toggle split direction to compare stacked and side-by-side
   layouts without rebuilding. Rotation must resize both programs.
6. Tap **Detach**. The remote programs must keep running. Tap **Reconnect** and
   confirm both saved terminal IDs, active pane and split direction are restored.
   Relaunch the app and open the same saved host; verify the same workspace.
7. During a slow connection to host A, return to **Hosts**, select disposable host
   B and tap **Open Herdr 2**. Complete/fail A's connection afterwards. B's text,
   status and input destination must remain B. Repeat A → B → A and detach while
   connecting; there must be no input replay or unexpected remote terminal spawn.
8. Background/lock then return. Compare the existing Automatic and Tap reconnect
   policies; verify fresh input after reattach and no replay of queued IME text.
   Also reopen a Herdr 1 host using its normal Connect control to check regression.

Record results and preferred split direction in PR QA. Two panes are the explicit
P9 workspace limit. Splits use client layout; remote terminals outlive detach.

### P9 review regression checks

1. In Herdr 2 with App shortcut priority, press Command-Shift-P. No hidden
   palette may open; immediately type text and use native Paste. Both must reach
   the selected pane. Also open Herdr 2 after leaving a Herdr 1 palette open.
2. On a disposable Herdr 1 host, delay a tab-create response. Return to Hosts
   and open Herdr 2 on the same host before that response arrives. After the
   first frame, type a marker and rotate: input and terminal resize must work.
3. Split two core panes and run `exit` in Pane 1. Its last output and the
   `ended: 1` status must remain visible. Select Pane 2, type a marker and rotate:
   the second program must receive input and redraw without Reconnect.

4. On a disposable host with an incompatible Herdr 1 daemon but a compatible
   core, open Herdr 2 after the Herdr 1 mismatch. Background and return with
   Automatic enabled; verify reconnection and fresh text input. Repeat while
   delaying the old Herdr 1 mismatch until after opening core. The old mismatch
   must not disable core Reconnect or background return.

## P10 — settings foundation

1. Locally unlock the login keychain, connect/unlock M5, then run
   `/Users/shin1ohno/ManagedProjects/cos-ad147c708b1e42fb-1/tools/just ipad`
   from the yebisu checkout. No password is handled by the worker.
2. Confirm the Hosts/terminal/board look unchanged; P10 adds no settings sheet.
3. On an already-authorized disposable host, change the existing terminal density,
   shortcut precedence and board visibility. Immediately press Home, wait five
   seconds, reopen yebisu, then terminate/relaunch it. Confirm all three choices
   persist. Repeat with screen lock immediately after a change.
4. Confirm input/IME and Herdr 1/2 reconnect still work after return; record
   device/iPadOS, the three restored values and any visible settings error in PR QA.
5. P11 will expose the settings editor and key-regeneration confirmation. P10
   never regenerates a key. Simulator lifecycle/file evidence does not establish
   physical lock/suspension, IME or hardware keyboard behavior.

## SH1-150 — Herdr 1 CJK selection and copy

1. Locally unlock the login keychain and connect/unlock M5. In an interactive
   login shell, run the following command (the worker never handles a password):

   ```sh
   (cd /Users/shin1ohno/ManagedProjects/yebisu && /Users/shin1ohno/ManagedProjects/cos-ad147c708b1e42fb-1/tools/just ipad)
   ```

2. Connect yebisu to an already-authorized disposable Herdr 1 terminal. Print
   `printf '\u65e5\u672c\u8a9e ABC\n'` in that terminal. Expected visible text:
   `日本語 ABC` (one real space before ABC).
3. Enable Select. Start the selection on the right half of 日 and drag through C.
   The highlight must cover the whole 日, 本 and 語. Tap Copy, paste into Notes
   or a disposable text field, and verify exactly `日本語 ABC` without an added
   space after any CJK character. Repeat in the reverse drag direction.
4. Start on 日 and stop on the right half of 本. Highlight and pasted text must
   include both whole characters: exactly `日本`. Select only the right half of
   本 and verify exactly `本`. Repeat with the trackpad and touch.
5. Redraw the terminal and then change part of the row to exercise full frames
   and retained patch rows. Repeat steps 3–4. Verify a real space after 語 stays.
6. On a Herdr 2 terminal, repeat the same copy check to confirm the existing
   skip-marker path. Record device/iPadOS, Herdr version, touch/trackpad results
   and pasted text in PR QA. Simulator evidence covers the production adapter
   and pasteboard; these physical feel checks remain owner acceptance.

Soft-wrap leading spacers, trimming trailing spaces on full-row selection and
wide-separator painter checks are separate follow-ups, not SH1-150 acceptance.


## SH1-151 selection defaults and persistence

The owner chose the common iPad interaction on the device on 2026-11-11:
Start: Drag and Copy: Manual. Simulator evidence does not prove touch/trackpad feel.

1. Locally unlock the login keychain, connect and unlock M5, then run from yebisu:
   `/Users/shin1ohno/ManagedProjects/cos-ad147c708b1e42fb-1/tools/just ipad`.
2. On a fresh install with no existing settings file, open an authorized disposable
   terminal. Verify the strip says Start: Drag and Copy: Manual. Drag with a finger
   and a trackpad; verify selection starts and the clipboard changes only after Copy.
3. Tap Start: Drag to switch to Start: Select / double tap; tap Copy: Manual to
   switch to Copy: On selection. Verify both labels and the corresponding behavior.
4. Immediately background/lock, return, terminate and relaunch yebisu, reopen the
   terminal and verify both changed labels and behaviors persist.
5. Change each button separately and relaunch after each change; verify the other
   choice, density, shortcut priority and board visibility stay unchanged.
6. Restore Start: Drag and Copy: Manual. Record device/iPadOS and results in PR QA.


## SH1-156: hardware input after composition discard (partial repair)

These are acceptance steps, not recorded passes. The partial selection/key repair
is not a complete SH1-156 fix. Signing requires the owner's locally unlocked login keychain;
never put the password in a command or send it to a worker.

1. In an authorized disposable Herdr 1 terminal, enter `cat` and Return. Start
   Japanese composition, select terminal text, then type `select-156` on the
   hardware keyboard without tapping the terminal. Record whether the first key
   arrives, whether selection remains, and the exact status message. The required
   result is one copy of the new input and none of the discarded composition.
2. Start another composition, reconnect with the existing reconnect control, wait
   for a coherent terminal frame, then type `reconnect-156` without tapping.
   Also type while reconnect is pending: pending keys must never be replayed.
3. On Herdr 2, enter `cat`, start composition, Detach, reattach, wait for the frame,
   then type `detach-156` without tapping. The first new key must arrive once.
4. Repeat all three using a new software-keyboard Japanese composition; commit
   it without first tapping the terminal. Only the new composition must arrive.
5. Repeat a tab/pane command with its reply deliberately delayed. Type during
   the barrier, then after it resolves. Only post-barrier input may arrive, in
   order. Record device/iPadOS/layout, build SHA, and receiving terminal output.
6. To settle the callback-origin boundary, capture whether UIKit can deliver an
   old `insertText:` after a new KeyDown or `setMarkedText:`. The consumer currently
   receives no composition identity. Host adapter tests cannot prove the ordering
   of physical UIKit callbacks.

7. Repeat Select with no range, Select with a range, and ordinary drag selection.
   Test **Typing: Clear highlight** and **Typing: Keep highlight** from the handoff
   strip. After a fresh Ctrl-A/Left press, Select must be off; only Keep retains
   a copyable highlight. Held repeats and Command-C must not exit selection.
   Change pane/host, reconnect and Detach: neither variant may retain an old
   selection gate. Verify report-all press/release once, then report-all off.
8. Distinguish semantic keys (Ctrl-A, Left) from printable hardware text in English
   and Japanese layouts. Record GPUI focus, native first responder, select mode,
   cancellation state, connection generation and peer receipt when tracing is
   available. A passing Ctrl-A check alone does not pass steps 1-4.
9. While selecting, expect “Selection active”. During a command, expect
   “Navigating…”. After Detach, expect “Detached · tap Reconnect to reattach”.
   After a semantic key resumes, “Text input paused” may remain because the IME
   fence is still active; report this as unresolved full recovery. A healthy new
   session with no refusal must not retain a “tap terminal” warning.
10. Start a text paste, then press a new semantic key while preparation is pending.
    Verify paste then key at the same target exactly once. Switching target before
    completion must deliver neither old paste nor held input to the new target.

### SH1-156 D1: clean connection must accept ordinary text

These are required results, not executed device checks.

1. Start a fresh app session with an English hardware layout and no composition.
   Connect to a disposable Herdr 1 terminal. Without tapping the terminal, type
   `fresh-156`, then Ctrl-A and Left. Verify the letters and semantic keys arrive
   once, and the status does not claim text input is paused.
2. Repeat on a new Herdr 2 connection, then Detach/reattach without any marked
   text. New `clean-reattach-156` must arrive without a terminal tap.
3. With no composition, initiate a pane command, type while the command is
   pending, then type `after-156` after the coherent destination appears. Verify
   pending text is absent and new text arrives once at the new destination.
4. Repeat the barrier with marked text before it and with a mark arriving while
   input is refused. Also switch hosts and return from Hosts with a marked setup
   field. Discarded marked text must never appear. Until the SDK retirement
   integration is verified, new composition recovery in these cases remains U1;
   do not mark the original SH1-156 checklist passed from steps 1-3 alone.
5. Record initial versus previously composing session, layout, exact app/SDK SHA,
   command status, target and receiving output separately for each sequence.
