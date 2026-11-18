# Simulator CI

The `simulator-smoke` job runs on GitHub-hosted `macos-26` for pull requests,
pushes to main and manual dispatch, with read-only contents permission and a
40-minute timeout. It installs XcodeGen and just, then uses the pinned Rust
toolchain and the continuously monitored 6 GiB disk guard.

`scripts/ios/ci-smoke.py` selects the newest installed iOS runtime that already
has an available iPad Pro 13-inch device and creates a disposable Simulator of
that device type. Non-numeric runtime suffixes are skipped with a log message. It calls
the same `scripts/ios/smoke-check.py` used by `just ios-sim`, without changing
the oracle: bundle/build, boot, install, launch, XCTest input and lifecycle
actions, screenshots and receiving-side paint/input/counter validation all
must succeed. It attempts shutdown and deletes its own Simulator on success or
failure. A cleanup line records the shutdown exit code and successful deletion;
a failed shutdown does not prevent deletion.

Unlike the local default, CI supplies a fresh Simulator UDID rather than relying
on a pre-existing M5 Simulator name or saved app data. The simulator bundle uses
ad-hoc signing; the XCTest bundle disables signing. No developer identity,
unlocked login keychain, secrets or owner's hosts are required. The P0 smoke
screen is local to the app. The existing check job remains separate.

Fresh Simulators can actually rotate where an existing local Simulator did not.
XCTest can export a landscape PNG with portrait IHDR dimensions and TIFF eXIf
orientation 8. The shared PNG decoder applies that display orientation before
the existing oracle maps paint receipts to pixels. All colour thresholds,
required stages, inputs and counter assertions remain unchanged; ignoring the
metadata would sample unrelated pixels. Tests cover all eight orientations.

Run the same CI entry point locally with
`just guarded python3 scripts/ios/ci-smoke.py`. Locally, timestamped capture
directories remain in `target/ios-smoke/`, including the XCTest result,
screenshots and app receipts; `target/ios-smoke/latest.json` identifies the last
successful capture and `target/ios-smoke/build/` holds derived build data.
In CI, an `if: always()` step uploads the capture directories as the
`simulator-smoke-evidence` Actions artifact even after a failed smoke step,
provided evidence was produced. The artifact excludes `build/` and the local
`latest.json` pointer. Download it from the workflow run; the runner's local
files disappear when the job finishes. This checks Simulator
rendering and lifecycle behavior, not physical IME or hardware keyboard behavior.

## layout-smoke job and SmokeQA label check

| Job | Runs | Limit | Selected by |
|---|---|---|---|
| `layout-smoke` (`macos-26`) | `scripts/ios/ci-layout-smoke.py`: one disposable Simulator, then `p2-check.py` (P2 keys and restart), `p3-check.py` (P3 terminal, density, colours) and `p10-check.py` (P10 settings lifecycle) in that order | 60 minutes | the `layout=` pattern of the `changes` job: the harness scripts above, `scripts/ios/smoke-qa/`, and the app modules those three drive; pushes to main and manual runs always run it |
| `check`, step `SmokeQA labels` | `scripts/ios/smokeqa-labels-check.py`: ties every text SmokeQA taps or OCR-matches to the app source that paints it, or to an allow-list entry with its reason | within the 60-minute `check` job | every pull request that selects `check` |

These checks were Mac-mini-only. Plan section 8.1 counted twelve; after SH1-303 (PR 52) removed
the finished-phase checks P4 to P8, R3, R5, CJK and SH1-155, three remain that are neither in
`edit-menu` nor in `simulator-smoke`: P2, P3 and P10. The `layout-smoke` job is built like
`ci-sh1-152.py`, with two differences: a failing check does not stop the next one (every check is
timed and judged in one run; the job still fails), and each check's wall time is printed as
`LAYOUT-SMOKE DURATION <check> <seconds> s exit <code>` and written to
`target/ios-layout-smoke/durations.json`, uploaded with the other receipts as the
`layout-smoke-evidence` artifact.

Measure the job on the first integration run: `gh run view <run-id> --json jobs` gives the job's
`startedAt` and `completedAt` and each step's, and the `durations.json` artifact splits the
"Layout smoke receiving-system checks" step by check. A total above 45 minutes is the signal to split the
job into two (P2 and P3 in one, P10 and the next additions in the other) before the 60-minute
limit is hit. First hosted runs (PR 72): run 44151000668 job 134279069207 took 16 min 41 s
(checks 932.6 s: p2 580.8, p3 140.5, p10 102.1) and run 44155672313 job 134296175129 took
24 min 42 s (checks 1284.8 s: p2 772.3, p3 237.4, p10 146.7).

Two hosted findings are handled in the harness, not in the checks (task e115109b440f4304):
- `simctl pbpaste` times out (exit 60) on GitHub-hosted runners (run 44151000668 job
  134279069207), and after a timeout it once returned text that is not the key line (run
  44247827726 job 134611018720). Since the UI test redesign S2 (owner decision
  `ui-test-redesign-decision.json`), P2 checks the pasteboard by what the app reads back from
  its own `UIPasteboard` right after Copy: the SHA-256 and byte length of that text, published in
  `qa.state` (`copied`, `pasteboard_sha256`, `pasteboard_bytes`, crates/yebisu/src/qa_ax_keys.rs)
  and recorded as a `p2-pasteboard` receipt. `testP2KeysAndRestart` compares `qa.state` with the
  digest `p2-check.py` passes (`TEST_RUNNER_P2_PASTEBOARD_SHA256`), and `p2-check.py` compares the
  receipt, on a hosted runner and on the Mac mini alike. `simctl pbpaste` is a Mac mini aid only:
  skipped when `GITHUB_ACTIONS=true`, a read failure is logged, other text fails.
- P2 never reads a file the app may still be writing (runs 44155672313, 44236636053 and
  44288317467 failed on such reads). XCTest launches the app itself, twice with the same
  environment (`YEBISU_P2_OBSERVE`, `YEBISU_P11_KEYS_QA`, `YEBISU_QA_AX`), presses the key
  block's controls by identifier (`qa.keys.provider`, `qa.keys.read`, `qa.keys.copy`) and
  confirms each press by `qa.state`; `p2-check.py` terminates the app after XCTest and only then
  reads `p2-setup.jsonl`, whole (a missing file or a partly written line fails by name). `p3`,
  `p9` and `p10` also terminate the app before they read its files.
- `KNOWN_RED` of `ci-layout-smoke.py` is empty. Integration 1 listed `p3-check.py` (the
  landscape stage lost the private-peer connection on main aed154e); on the Integration 2
  head p3 passed every stage on hosted (run 44214913068, job 134502265733), so the entry was
  removed. A listed check still runs and uploads its evidence; its failure is logged as
  `layout-smoke KNOWN RED`, and a pass fails the job until the entry is removed.

The tests moved from `SmokeQA.swift` into `SmokeQA+<unit>.swift` extensions of the same class
(Terminal, Keys, Handoff, Settings, Theme, Observer, Notifications, Input, Core), each at most 300
lines. `-only-testing:SmokeOracle/SmokeQA/<test>` names are unchanged. Line numbers such as
`SmokeQA.swift:23` in the classes below refer to the file before that split.

## Known simulator-smoke failure classes

Rates come from the L17 diagnosis of 104 `simulator-smoke` jobs (CI runs
2026-11-14T12:08Z to 2026-11-15T21:16Z, all on `macos-26-arm64`; P0 98 runs, P9
92, P12 52) unless a row says otherwise. They were copied, not re-measured.
`[unproven]` marks a cause nobody has confirmed. Class names are the diagnosis
labels. Origin: 2026-11-16, L17 diagnosis, with P12 (PR 55), L16 (PR 57) and
SH1-265 (PR 56) reports.

Known classes for the merge rule below: L15, L16, A, B and P12-shot. P12-fixture
(first seen 2026-11-17) is in the table but not on this list: adding it is an owner
decision.
C-snapshot and C-anim left the list when PR 58 merged (716018a), and C-paint
was never on it. Since PR 58 merged, a failure with any of their signatures is
a new defect, not a known class, and blocks the merge. L19 and L20 are open work on L15 and A/B, not
classes.

| Class | Log signature (grep) | Rate | Cause | Fix / open item | Evidence |
|---|---|---|---|---|---|
| L15 | `Missing source-defined control: Paste` (`OCR attempt [0-2] for Paste`, then `missing-control`) | 11/41 P9 runs whose head has the Settings button; 0/49 without | The OCR after a `QA image ready` finds no control: no standalone `Paste` after the second one (4 of 11), no `Paste image` after the first one (7 of 11). What Vision returns is `[unproven]` | 4 of 11 (second `Paste`): PR 57, merged 1de7020. 7 of 11 (first `Paste image` misread): open, L19, after PR 57 | run 43746802879/1 job 132911829322 (artifact 14175507358); run 43754156237/1 job 132934538869 (14177458803) |
| L16 | `image receipts incomplete: puts=3 paths=3 text=0`, with the logged tap box at x=0.215 | 3 of the 41 jobs that reached SH1-154 in the L16 scan; a 4th job was added later | The second text `Paste` tap landed on `Paste image` (box x=0.215, the right one is 0.151), so no `P8-shadow` text arrived. A `text=0` whose tap box is x=0.151 has another cause (job 132483179735, head 1954cab, no Settings button): not L16 | PR 57, merged 1de7020; 3 of 3 `simulator-smoke` passes at its head 756f70b (run 43766302928) | run 43736757874 jobs 132881022688, 132900139639; run 43760942018 job 132955457568; added job 132969174547 |
| C-snapshot | `SmokeQA.swift:23: error: ... Failed to get matching snapshot: Timed out` | 3/98 P0 runs | `app.activate()` right after Home does not take effect and no characters reach the app (the app stderr shows 8 `input committed` lines, all for `P0-input`; none for `-returned`), so the `typeText("-rotated")` at `SmokeQA.swift:23` times out. Happens on a freshly booted, slow Simulator. That the app stays suspended, and the press/activate ordering, are `[unproven]` | PR 58 (merged, 716018a) replaces the fixed sleeps after Home, `activate()` and `typeText` with state waits. A failed wait now stops the test at once with `initial activate(): app.state is ...`, `app left the foreground after press(.home): app.state is ...` or `activate() after press(.home): app.state is ...` (`SmokeQA.swift` `requireState`, 30 s each); since PR 58 merged, a non-foreground state after `activate()` is a new defect, not a known class | run 43730444891/7 job 132946288166 (artifact 14179437935); run 43720814607/1 job 132832187258; run 43729845313/2 job 132888134849 |
| C-anim | `App animations complete notification not received` (60 s waits, 25 times, job cancelled at 40 min) | 1/92 P9 runs | `animationsHaveFinished` stays NO after `P9-native\n`; what keeps animating is `[unproven]` | PR 58 (merged, 716018a) caps `testP9Core` at 150 s (XCTest rounds it up to 3 minutes; `-test-timeouts-enabled YES` in `p9-check.py`), so a hang now ends with `Test Case '-[SmokeOracle.SmokeQA testP9Core]' exceeded execution time allowance of 3 minutes` (negative control: run 43790640783, 02:25:17.999 to 02:28:17.998); the QA build writes an `animations-running` record with each layer's class and `animationKeys()` into `p0-smoke.jsonl` once an animation stays for 10 s. It does not fix the cause; since PR 58 merged, a failure with this signature is a new defect. Integration 3 (L8) set the cap to 210 s (4 minutes). In run 44276079121 (job 134704511383) the test finished every step in 209.8 s against 96-121 s before L8: about 30 s for the five `...` menu trips (cut by `tapMenuItem`, one OCR pass per trip) and one 60.09 s wait (`App animations complete notification not received` after `P9-pane1\n`, with an `animations-running` record for `CASDFElementLayer` 10 s after the Reconnect tap). The cap keeps room for the body plus one such wait, as 3 minutes did before | run 43730444891/4 job 132914175740 (artifact 14176482747) |
| C-paint | `input result expected 'P0-input', received 'P0-inpu'` | 1 job seen | A fixed `sleep(2)` after `typeText` ended before the last character was drawn | PR 58 (merged, 716018a) waits for the typed text on screen; a miss now fails as `typed '<text>' received: '<word>' not on screen after 20 s` (the `-rotated` step keeps its sleeps, the keyboard covers the field); since PR 58 merged, a failure with this signature is a new defect | run 43684762210/1 job 132722801994 |
| A | `OS menu still open ... (class A)` in `xctest.stdout` (PR 59, merged c7013bb); before PR 59: `P12 xctest: exit 0` then `ValueError: setup callbacks missing/duplicated` | 2/52 P12 runs | The first tap after an OS menu opens is lost; the menu stays open. Cause `[unproven]`. A lost tap on a P12 fixture control, not on the OS menu, is P12-fixture, not A. Not seen in the three P12 jobs of 2026-11-17 | Open: L20 (PR 59 merged, c7013bb: observation only); the pick wait on the menu closing is unchanged by the P12 reliability PR | run 43730444891/3 job 132903694966 (P12 artifact 14174842401); run 43717998985/1 job 132823606035 (14164345944) |
| B | `OS menu bar not revealed (class B)` in `xctest.stdout` (PR 59, merged c7013bb); before PR 59: `Installed OS application menu must be revealed and reachable` (`P12QA.swift:68`, `:81` since PR 55) | 4/52 P12 runs; 2 more of the 3 P12 jobs of 2026-11-17 | The top-edge swipe does not reveal the menu bar or `•••`. Cause `[unproven]`. In both new failures the swipe call took 5.6 s and 5.7 s (`xctest.stdout`: from `Synthesize event` to the idle wait after `Press Application`), against 0.6 to 1.1 s for 14 of the 15 swipes that revealed it (the 15th took 4.2 s); the XCTest gesture was spread over a stalled Simulator. The PR 59 precondition record does not separate them: Window Controls absent, app window not focused, `runningForeground` in all 10 reveals of job 134059155142 (3.2 to 53.1 s since the last close) and in both failures (3.3 s, 35.6 s) | Open: L20 (PR 59 merged, c7013bb: observation only). P12 reliability PR: `P12QA.revealMenuBar` swipes once more when the bar is absent after 10 s and logs a `reveal-retry` (reason, swipe seconds) that the oracle accepts once per case and prints in the verdict `retries`; a second miss still fails as class B, and the XCTest assertion after the reveal is unchanged | run 44077223916/1 job 134018333353 (`xctest.stdout:708`, case palette); run 44117720382/1 job 134160368350 (`xctest.stdout:628`, case rearm); run 43699190351/1 job 132766634071; run 43729845313/1 job 132859692803; run 43754156237/4 job 132956853113; run 43730444891/9 job 132960855571 |
| P12-shot | `xctest.stdout`: `Failed to get screenshot: Timed out while requesting screenshot` from `P12QA.swift` (at 716018a `:278` is the `screenshot()` helper and `:255` is `recognize()`; the case and step are the last `Added attachment named 'p12-…'` / `Tap` lines before it). Supporting: `app.jsonl` has no `"kind":"terminal"` record for ≥ 10 s outside the `boot` case. Job log shows only `P12 xctest: exit 65` | 1/53 P12 runs (2026-11-15T00:00Z–2026-11-16T12:06Z); 0/1536 vs 1/204 screenshots before/after PR 59 | The Simulator stopped drawing for ~24 s with the OS menu open; XCTest gives up on a screenshot after 30 s. Why drawing stopped is `[unproven]`. A held `pane.split` can then pass the 60 s `COMMAND_TIMEOUT` (`Disconnected: endpoint request timed out`); that is a consequence, not the PR 55 class | PR 63 (merged, 2c1e8b2): open-menu screenshots only on failure, 39 to 11 per run. Open: time every capture | run 43834250246/1 job 133186893086 (artifact `p12-native-menu-evidence`) |
| P12-fixture | Job log: `P12 xctest: exit 0`, then `ValueError: app prepare cases missing/duplicated` (`p12-oracle.py:73`) with the Springboard reveals all passing (before this PR). Now `xctest.stdout`: `Fixture tap on 'P12 <Control>' not received by the app, also after one retry (class P12-fixture)`, or the oracle's `fixture tap lost (class P12-fixture): the app logged no p12-setup-begin for step <step>` | 1 of the 3 P12 jobs of 2026-11-17; not measured over the window | The first fixture tap (`P12 Remote`, `Tap Application ...[0.87, 0.09]` at `xctest.stdout` t=53.04 s, XCTest `Synthesize event` normal) produced no `p12-setup-begin`/`p12-prepare`/`p12-setup-end` for `remote` in `app.jsonl`: between the lines 16 (13:28:45) and 73 (13:29:39) there are only `terminal` records, and the pick that followed logged `case: ""`. The fixture OCR before it took 41 s (13:28:46 to 13:29:27). The tap point was the same as in the passing runs. Why the app did not receive the tap is `[unproven]` | P12 reliability PR: after each fixture tap `P12QA` waits (5 s) for the app's own `p12-setup-end` of that step instead of a fixed delay, taps once more when it is absent and logs a `fixture-retry`; a second miss fails by name. The oracle names the step before its generic checks and prints the retries in the verdict. Replaying the evidence of job 134059155142 through the new oracle gives `fixture tap lost ... step remote` | run 44077223916/2 job 134059155142 (artifact `p12-native-menu-evidence`: `app.jsonl` lines 16-73, `xctest.stdout:430`) |

Also fixed: a P12 held `pane.split` request that reached the 60 s client
timeout (`held request reached the client's 60 s COMMAND_TIMEOUT`; earlier
symptoms `P12QA.swift:78` `Native command missing` and `:79` `Captured native
command must start enabled`), PR 55, merged cb18764.

PR 59 (merged, c7013bb) changed the failure messages of A and B: it adds
`OS menu bar not revealed (class B)` and `OS menu still open ... (class A)`.
Rows A and B list the new message first and the earlier one after it.

### Classify a failing smoke job

(2026-11-16: owner decision on known classes, `known-class-merge-rule-decision.json`.
The rerun limit in step 5 comes from the approved plan, not from that decision.)

1. Download the evidence of the failing run, `gh run download <run-id> -n
   simulator-smoke-evidence` for P0/P9, or the P12 artifact
   `p12-native-menu-evidence` (`xctest.stdout`, `peer.jsonl`, `app.jsonl`).
   The job log of a P12 failure shows only `P12 xctest: exit 65`. A job that hit
   the 40-minute timeout may have no artifact.
2. Classify from lines you can cite. P12 classes (A, B, P12-fixture, P12-shot) are classified from the
   `xctest.stdout` lines of the `p12-native-menu-evidence` artifact; every other
   class, from job log lines. A failure with no matching signature is a new
   class: do not rerun it, report it.
3. Area the PR touches. Run `gh pr diff <number> --name-only` and compare it with
   the failing class's area below. A PR owns the failure when a changed path is a
   shared input or is in that class's area. The lists are a minimum: a path in
   neither is outside the class only if it cannot reach the failing test's screen
   or input path; otherwise the last paragraph of this step applies. Narrowing
   source: the `followups` entry of `known-class-merge-rule-decision.json`
   (2026-11-16T03:13:58+00:00, "narrow per class"). Areas were drafted from the L17
   causes; each path is justified by `file:line` on origin/main 673715d, so
   re-read the named function when a PR moves the lines.

   Shared inputs, part of every class's area:
   - `Cargo.toml` (it pins `herdr-uikit` at line 14 and `herdr-gpui` at lines
     15-21), `Cargo.lock`, `rust-toolchain.toml`, `crates/yebisu/Cargo.toml` and
     `.github/workflows/ci.yml`;
   - the harness scripts `scripts/ios/ci-smoke.py`, `smoke-check.py`,
     `smoke_oracle.py`, `p9-check.py`, `p12-ci.py`, `p12-check.py` and
     `p12-oracle.py` (`p12-ci.py` loads `ci-smoke.py`);
   - every other path the `simulator-smoke` pattern matches that is not under
     `crates/`, `scripts/ios/smoke-qa/` or `scripts/ios/p12-qa/` (for example
     `scripts/ios/sim.sh`, `screenshot-color.py`, `.cargo/`, `assets/`, `justfile`).

   Class areas (product source, then harness files that could cause or mask the
   class). `crates/` as a whole is no longer in any area.
   - **L15 and L16** (P9 `testSH154CoreImage`, the SH1-154 `Paste` strip OCR):
     - `crates/yebisu/src/settings_sheet/view.rs` (`settings_button`, :39-40),
       `crates/yebisu/src/terminal_view.rs` (the toolbar row that holds it,
       :290-297) and `crates/yebisu/src/app_setup.rs` (:67). L15 occurred only on
       heads whose source has the Settings button (11/41 jobs, 0/49 without; L17
       diagnosis, "L15" measured facts), and the diagnosis attributes the missed
       `Paste` label to that extra toolbar row.
     - `testSH154CoreImage` starts with `YEBISU_CORE_QA=1` (`SmokeQA.swift:415`),
       so the screen is in core mode and the core controls row (`Split (2
       panes)`, `Pane 1`, `Pane 2`, `Toggle split direction`, `Detach`) sits
       above the strip, like the toolbar row: `crates/yebisu/src/core_view.rs`
       (`connect_core` :46 sets `core_mode`; `render_core_controls` :121-172),
       `crates/yebisu/src/terminal_view.rs` (:379-381 draws it) and
       `crates/yebisu/src/app.rs` (:394-400 calls `connect_core`).
     - `crates/yebisu/src/handoff_view.rs` (`handoff_controls`, :299-359: the
       `Paste` and `Paste image` buttons the OCR has to tell apart),
       `crates/yebisu/src/terminal_view.rs` (:386-387, where the strip and the QA
       row sit) and `crates/yebisu/src/handoff_qa.rs` (:5-31: `QA Image`, `QA
       Image only` and the `QA image ready` label the test waits on; :46 seeds
       `P8-shadow`).
     - The paste data path that `p9-check.py` `validate_images` (:41-74) counts as
       `puts`, `paths` and `text`, because L16 is `text=0` (all under
       `crates/yebisu/src/`): `core/paste.rs`, `core/paste_path.rs`,
       `core_paste_view.rs`, `handoff_clipboard.rs` and `core_mock/paste.rs`, plus
       `core_mock.rs`, which writes the `p9-core-wire.jsonl` receipt it counts
       (:144, :188, :309-318).
     - Harness: `scripts/ios/smoke-qa/Tests/SmokeQA.swift` (`testSH154CoreImage`
       :412-439, `locateText` :446-484, `recognize` :510, `tapText` :529-549) and
       `scripts/ios/smoke-qa/project.yml`.
   - **C-snapshot** (P0 `testInputAndLifecycleReceipts`: Home :28, `activate()`
     :32, `typeText("-returned")` :35 in `SmokeQA.swift`):
     - App launch and lifecycle: `crates/yebisu/src/main.rs` (:18-20, `main` calls
       `app::run`), `crates/yebisu/src/app.rs` (:344-351, window open and
       `on_app_lifecycle`), `crates/yebisu/src/reconnect.rs` (`lifecycle` :35-120,
       Background suspends :45-62, Active resumes :85) and
       `crates/yebisu/src/settings_view.rs` (`settings_lifecycle` :83-96).
     - What `lifecycle` calls on the main thread: `crates/yebisu/src/diagnostics.rs`
       (`record` :36, called at `reconnect.rs:37`; the subscriber uses `try_send`
       at :97), `crates/yebisu/src/handoff.rs` (`cancel` :64, called at
       `reconnect.rs:49`) and `crates/yebisu/src/link.rs` (`pause` :57, called at
       `reconnect.rs:71`).
     - The input that fails to arrive: `crates/yebisu/src/input.rs`
       (`replace_text_in_range` :159-194, which logs `input committed`) and
       `crates/yebisu/src/observation.rs` (:38-73, the `p0-smoke.jsonl` receipts
       `smoke_oracle.py` checks).
     - Harness: `scripts/ios/smoke-qa/Tests/SmokeQA.swift` (:15-58, waits `requireState` :73 and
       `waitForRenderedText` :91) and
       `scripts/ios/smoke-qa/project.yml`.
   - **C-anim** (P9 `testP9Core`: `P9-native\n` typed at `SmokeQA.swift:502`). What
     keeps animating is `[unproven]`, so the area is the path that runs from that
     input to the idle wait:
     - Under `crates/yebisu/src/`: `input.rs` (:159-194), `terminal_input.rs`,
       `terminal_view.rs`, `upstream/terminal_painter.rs` and
       `upstream/terminal_painter/`, the P9 mock backend `core_mock.rs` (:286,
       `connect`) with `core_view.rs` (:13, :58) and `app.rs` (:394-400), which
       select it under `YEBISU_CORE_QA`, and `observation.rs`.
     - The `QA mark`, `QA delete` and `QA commit` taps that follow the input
       (`SmokeQA.swift:503-505`) and the key and edit-menu code that runs between
       the input and the idle wait: `crates/yebisu/src/terminal_controls.rs`
       (`publish_native_edits` :8, called every render; `terminal_key_down` :119;
       `terminal_key_up` :249; `input_qa_controls` :359, the `QA` row) and
       `crates/yebisu/src/edit_menu.rs` (:87-100 set availability and present or
       dismiss the UIKit edit menu; `edit_menu_check` :165). `handoff_view.rs`
       (:321-323, the `Start: Drag` and `Start: Select / double tap` labels the
       test taps at `SmokeQA.swift:386-387` before it types) is in this area too.
     - Harness: `scripts/ios/smoke-qa/Tests/SmokeQA.swift` (`testP9Core`
       :377-410) and `scripts/ios/smoke-qa/project.yml`.
   - **P12-shot** (P12 screenshot timeout during a Simulator draw stall):
     `scripts/ios/p12-qa/` and the P12 harness scripts (`p12-ci.py`,
     `p12-check.py` and `p12-oracle.py`, which are also shared inputs).
     PR 58's failure in job 133186893086 is classified P12-shot: its `app.rs`
     change is gated on `YEBISU_CORE_QA`, which is unset in P12.
   - **A and B** (P12 `testInstalledTerminalMenu`; the reveal wait is
     `P12QA.swift:75-81`, the pick `:86-112`):
     - The native menu code: `crates/yebisu/src/native_menus.rs`
       (`install_native_menus` :155-171 replaces the OS menu through
       `cx.set_menus`; `catalog_rows` :51) and `crates/yebisu/src/actions.rs`
       (the catalog the menu rows derive from, :1-2).
     - The P12 QA code: `crates/yebisu/src/native_menus_qa.rs` (`p12_prepare`
       :105, `p12_tick` :178 injects the fault 6 s after the menu opens,
       `p12_menu_begin` :238 and `p12_menu_end` :253), `crates/yebisu/src/app.rs`
       (:142 `p12_tick`, :159 `p12_qa_controls`, :352-370 P12 connect),
       `crates/yebisu/src/mock_peer.rs` (:626 `connect_p12`) and
       `crates/yebisu/src/observation.rs`.
     - Harness: `scripts/ios/p12-qa/Tests/P12QA.swift` and
       `scripts/ios/p12-qa/project.yml`.

   A PR that changes a shared input or a path in the failing class's area owns the
   failure. If it is unclear whether a changed path drives the failing step, treat
   it as touched: doubt blocks the merge.
4. Known-class rule (owner decision 2026-11-16, for every shin1ohno/yebisu PR).
   A `simulator-smoke` failure does not block the merge when all of these hold:
   `simulator-smoke` is the only failing job; the failure is classified to a
   known class with cited lines (job log lines, or for P12 classes the
   `xctest.stdout` lines of the `p12-native-menu-evidence` artifact); the PR's
   changed files do not touch that class's area (step 3); every other job is
   green; one review round has 0 blocking findings; 0 threads are unresolved.
   Known classes: L15 and L16 (SH1-154 Paste OCR mis-tap), A and B (P12), and
   P12-shot (P12 screenshot timeout). C-snapshot and C-anim (L18) left the list
   when PR 58 merged (716018a); such a failure is a new defect. The rule decides only whether the failure blocks.
   It does not grant merge permission, which stays per PR or per approved plan.
   Merge with a merge commit.
5. Rerun only the failed job (`gh run rerun --job <job-id>`), at most 2 times per
   PR. Cite the run and job id of each attempt in the PR.

### Cancelled jobs without a runner, H-live-read and E-launch

(2026-11-17: diagnosis of the main failures of runs 43891836688, 43897926491
and 43992978378. Cited ids are run/job.)

- **Runner cancellation.** A job that ended `cancelled` with `runner_name`
  empty, 0 steps and the annotation `The job was not acquired by Runner` never
  ran on a runner. It has no log and is not a failure class. Classify it as
  runner cancellation and rerun that job once with `gh run rerun --job
  <job-id>`; this one rerun is outside the 2-rerun budget of step 5. Seen:
  43897926491/133419859913 and 43897926491/133419860003. A cancelled job that
  has a runner or steps is not this case: classify it from its log (step 2).
- **H-live-read (fixed by this PR).** A harness script raised
  `json.decoder.JSONDecodeError` while reading a JSONL file after the XCTest
  had already `passed`. The app was still writing, so the last line had no
  newline yet. In 43992978378/133738049997 (`simulator-smoke`,
  `smoke_oracle.py:59`, `Expecting value: line 1 column 86 (char 85)`) the
  last line of `app.jsonl` was an 85-byte fragment. 43891836688/133390084701
  (`edit-menu`, `selection-settings-check.py:64`, `Expecting ',' delimiter`)
  has the same cause; its artifact has no `selection-settings.jsonl`, so the
  fragment itself was not seen. `scripts/ios/live_jsonl.py` now reads every
  newline-terminated line and drops only an unterminated final line (a
  malformed terminated line still raises). `smoke_oracle.py` and the
  `selection-settings`, `p12`, `p3`, `p8-native`, `sh1-152` and `sh1-153`
  checks use it, and so do the line counts used as offsets. After this PR, a
  `JSONDecodeError` from one of those reads is a new defect, not a known class.
- **E-launch (observed, not fixed).** `EditMenuQA.swift:203: error: ...
  Failed to launch com.shin1ohno.yebisu: Timed out attempting to launch app.`
  failed `testSH1152Basic` in 43897926491/133419860111 (1 of 169 `edit-menu`
  jobs in the diagnosis window). The cause is unproven; there is no fix and no
  retry. E-launch is not on the known-class list of step 4. The next time it
  happens, read the new log lines `SH1-152 LAUNCH <case> <seconds>` (the
  seconds `app.launch()` took per case, printed when it returns) in the job
  log and compare the failing case with the earlier ones.

### XCUITest OCR rules

(2026-11-16: PR 55, PR 57.)

- Locate every control once, before any held request, then tap the recorded
  coordinates (PR 55 `locateFixtures` finds all 13 P12 controls before the first
  held `pane.split`; PR 57 reuses the first `Paste` box for the second tap).
- Match whole words so `Paste` never hits `Paste image`: split recognised text
  on whitespace and exclude `Paste` followed by `image`/`Image`. `locateText`
  still uses the suffix test; the whole-word match is open (L19).
- Show that a menu is closed before the next tap. A hosted press can be lost: in
  run 44297932478 (job 134779801456) the press that should close the `...` menu left
  it open (screen recording at 47.8 s and 48.4 s), and SH154's QA Image only tap only
  closed it, so `QA image ready` never came. `closeOverflowMenu` waits for `Select
  text` to leave the screen and presses once more, saying why.
- Never run an OCR search while a request is held. The pinned sheprd-client
  `COMMAND_TIMEOUT` is 60 s (herdr-gpui f63688b, `crates/sheprd-client/src/limits.rs:19`);
  a request held that long drops the connection. Hold one request per local
  case and release it before the next search.

### P12 receipts, retries and negative controls

(2026-11-18: P12 reliability PR; jobs 134018333353, 134059155142 and 134160368350.)

- Wait on the receiving side, not on a delay. After each fixture tap `P12QA`
  polls the app's own `Documents/p0-smoke.jsonl` (`p12-check.py` passes the
  directory as `TEST_RUNNER_P12_DOCUMENTS`, the way `sh1-152-check.py` does)
  for one more `p12-setup-end` of that step. Only newline-terminated lines
  count, as in `live_jsonl.py`. A reveal waits on the Springboard control
  (`exists` and `hittable`), as before.
- Repeat once, and say why. A tap without a receipt (5 s) and a swipe without a
  revealed menu bar (10 s) are repeated once. Each repeat is a `fixture-retry` or
  `reveal-retry` record in `p12-ui-events.json` with its reason; the oracle
  accepts at most one per control or case and prints all of them under `retries`
  in the verdict, so a rate is never hidden. A second miss is a failure
  (P12-fixture, class B). No assertion was weakened: the received-receipt check,
  the revealed-menu-bar assertion, the hierarchy and menu-close checks and every
  oracle correlation are unchanged.
- The events attachment is added in `tearDown`, so a failed run keeps
  `p12-ui-events.json` (before, a failed assertion skipped the `defer` and the
  reveal and menu-close records were lost).
- Negative control: `P12_QA_FAULT=drop-tap:<step>,skip-swipe:<case>
  python3 scripts/ios/p12-check.py ...` (empty in CI) skips the first tap of a
  step or the first swipe of a case. `p12-check.py` then requires one logged,
  injected retry for each fault, so a run with a fault proves that the retry path
  ran and that the oracle accepts it.
