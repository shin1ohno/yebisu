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

## layout-smoke job and QA identifier check

| Job | Runs | Limit | Selected by |
|---|---|---|---|
| `layout-smoke` (`macos-26`) | `scripts/ios/ci-layout-smoke.py`: one disposable Simulator, then `p2-check.py` (P2 keys and restart), `p3-check.py` (P3 terminal, density, colours) and `p10-check.py` (P10 settings lifecycle) in that order | 60 minutes | the `layout=` pattern of the `changes` job: the harness scripts above, `scripts/ios/smoke-qa/`, and the app modules those three drive; pushes to main and manual runs always run it |
| `check`, step `QA identifiers` | `scripts/ios/qa-ax-ids-check.py`: every QA identifier a Swift UI test presses is a constant of `crates/yebisu/src/qa_ax_ids.rs` that the app marks (UI test redesign S6 replaced the SmokeQA label check, which tied every text SmokeQA tapped or OCR-matched to the app source that painted it) | within the 60-minute `check` job | every pull request that selects `check` |

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

Known classes for the merge rule below: L15, L16, B and P12-shot. P12-fixture
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
| A | `OS menu still open ... (class A)` in `xctest.stdout` (PR 59, merged c7013bb); before PR 59: `P12 xctest: exit 0` then `ValueError: setup callbacks missing/duplicated` | 2/52 P12 runs in the original sample; seen again on 2026-11-20 | In job 136312328508 BackBoard initialized the same Recap touch digitizer twice as its first input arrived. It skipped later events for the path and canceled the contact on detach. XCTest still received `TouchEventsCompleted`. In job 136180580090 the same Split Right tap logged contact release before detach and reached the app | Pick and dismiss the open native SpringBoard menu with XCTest's iPad pointer clicks. Recap waits for trackpad readiness before playback. The harness opens the menu by touch, waits for a hittable command, and checks menu closure and the actual native command receipt at the consumer and peer | run 44743338211/1 job 136312328508 (artifact 14635411736, exported `system.logarchive`, 14:10:37 UTC); run 44704476837/1 job 136180580090 (14617634414, 09:48:41 UTC); run 43730444891/3 job 132903694966 (14174842401); run 43717998985/1 job 132823606035 (14164345944) |
| B | `OS menu bar not revealed (class B)` in `xctest.stdout` (PR 59, merged c7013bb); before PR 59: `Installed OS application menu must be revealed and reachable` (`P12QA.swift:68`, `:81` since PR 55) | 4/52 P12 runs; 2 more of the 3 P12 jobs of 2026-11-17 | The top-edge swipe does not reveal the menu bar or `•••`. Cause `[unproven]`. In both new failures the swipe call took 5.6 s and 5.7 s (`xctest.stdout`: from `Synthesize event` to the idle wait after `Press Application`), against 0.6 to 1.1 s for 14 of the 15 swipes that revealed it (the 15th took 4.2 s); the XCTest gesture was spread over a stalled Simulator. The PR 59 precondition record does not separate them: Window Controls absent, app window not focused, `runningForeground` in all 10 reveals of job 134059155142 (3.2 to 53.1 s since the last close) and in both failures (3.3 s, 35.6 s) | Open: L20 (PR 59 merged, c7013bb: observation only). P12 reliability PR: `P12QA.revealMenuBar` swipes once more when the bar is absent after 10 s and logs a `reveal-retry` (reason, swipe seconds) that the oracle accepts once per case and prints in the verdict `retries`; a second miss still fails as class B, and the XCTest assertion after the reveal is unchanged | run 44077223916/1 job 134018333353 (`xctest.stdout:708`, case palette); run 44117720382/1 job 134160368350 (`xctest.stdout:628`, case rearm); run 43699190351/1 job 132766634071; run 43729845313/1 job 132859692803; run 43754156237/4 job 132956853113; run 43730444891/9 job 132960855571 |
| P12-shot | `xctest.stdout`: `Failed to get screenshot: Timed out while requesting screenshot` from `P12QA.swift` (at 716018a `:278` is the `screenshot()` helper and `:255` is `recognize()`; the case and step are the last `Added attachment named 'p12-…'` / `Tap` lines before it). Supporting: `app.jsonl` has no `"kind":"terminal"` record for ≥ 10 s outside the `boot` case. Job log shows only `P12 xctest: exit 65` | 1/53 P12 runs (2026-11-15T00:00Z–2026-11-16T12:06Z); 0/1536 vs 1/204 screenshots before/after PR 59 | The Simulator stopped drawing for ~24 s with the OS menu open; XCTest gives up on a screenshot after 30 s. Why drawing stopped is `[unproven]`. A held `pane.split` can then pass the 60 s `COMMAND_TIMEOUT` (`Disconnected: endpoint request timed out`); that is a consequence, not the PR 55 class | PR 63 (merged, 2c1e8b2): open-menu screenshots only on failure, 39 to 11 per run. Open: time every capture | run 43834250246/1 job 133186893086 (artifact `p12-native-menu-evidence`) |
| P12-fixture | Job log: `P12 xctest: exit 0`, then `ValueError: app prepare cases missing/duplicated` (`p12-oracle.py:73`) with the Springboard reveals all passing (before this PR). Now `xctest.stdout`: `Fixture tap on 'P12 <Control>' not received by the app, also after one retry (class P12-fixture)`, or the oracle's `fixture tap lost (class P12-fixture): the app logged no p12-setup-begin for step <step>` | 1 of the 3 P12 jobs of 2026-11-17; not measured over the window | The first fixture tap (`P12 Remote`, `Tap Application ...[0.87, 0.09]` at `xctest.stdout` t=53.04 s, XCTest `Synthesize event` normal) produced no `p12-setup-begin`/`p12-prepare`/`p12-setup-end` for `remote` in `app.jsonl`: between the lines 16 (13:28:45) and 73 (13:29:39) there are only `terminal` records, and the pick that followed logged `case: ""`. The fixture OCR before it took 41 s (13:28:46 to 13:29:27). The tap point was the same as in the passing runs. Why the app did not receive the tap is `[unproven]` | P12 reliability PR: after each fixture tap `P12QA` waits (5 s) for the app's own `p12-setup-end` of that step instead of a fixed delay, taps once more when it is absent and logs a `fixture-retry`; a second miss fails by name. The oracle names the step before its generic checks and prints the retries in the verdict. Replaying the evidence of job 134059155142 through the new oracle gives `fixture tap lost ... step remote` | run 44077223916/2 job 134059155142 (artifact `p12-native-menu-evidence`: `app.jsonl` lines 16-73, `xctest.stdout:430`) |
| P12 confirmation deadline | `P12 held press ... lost` followed by `after=nil` in `xctest.stdout`, with setup begin and end in `app.jsonl` | 1 observed job on 2026-11-20 | The synchronous fixture tap used up the XCTest receipt window before the test read `qa.state`. Both Sidebar taps reached the app | Start each 5 s confirmation window when its tap returns, using monotonic time. Read `qa.state` immediately and require both receipt counters to advance | run 44704476837/1 job 136180580090 (artifact 14617634414, `xctest.stdout` lines 607–628 and `app.jsonl` lines 100–109) |
| SH1-152 portrait confirmation | `testSH1152Basic` fails before launching E1-text with `Timed out waiting for confirmation of orientation change` in `xctest.log` | 1 observed job on 2026-11-20 | XCTest reported a portrait interface before the test. The unconditional pre-launch device orientation request at 1.54 s failed before its 14.57 s notification. The same source passed when the notification arrived at 1.53 s after a request at 1.06 s | Launch and wait for `qa.state`, then require a portrait SpringBoard frame before the first touch. Request portrait once only if a landscape frame was observed | run 44765771959/1 job 136389954788 (artifact 14647790317) and passing control 44757970198/1 job 136362684853 (artifact 14652096259) |

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
   Known classes: L15 and L16 (SH1-154 Paste OCR mis-tap), B (P12), and
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

### P10 lost lifecycle records and SH1-152 lost click (observed, instrumented)

(2026-11-19: PR 77 run 44406830068 job 135164241872; main 55899c8 run
44402265214 job 135148322586. Mac mini lease 3: p10 20/20 PASS, 10 of them
under an 8-core CPU load.)

- **P10-records.** XCTest `testP10Settings` passed and the saved settings file
  matched the QA edit made only at Inactive (`settings_view.rs`
  `settings_lifecycle`), so the lifecycle reached the app; `p10-check.py`
  then found no Inactive and Background `settings-lifecycle` records. A p10 run
  writes 258 to 294 records, at most 107 in one second, so the old 256-slot
  queue never filled. Cause unknown; the artifact kept neither the records
  nor the stderr.
  - Hypothesis (unproven): the first process queued Inactive and Background
    and was killed while suspended, before its writer thread wrote them. The
    flush below guards against exactly this.
  - Against it: in a passing p10 (Mac mini, a40b435, 10 runs) the first
    process records Inactive 1.83 to 2.01 s before Background, and Inactive
    alone does not suspend, so a writer that is alive has had about 2 s to
    write Inactive. The same process then records Foreground and Active
    again: activate resumes it, it is not relaunched (two `launch` pids per
    run). Losing Inactive needs the writer stalled or dead for those 2 s, or
    a kill while suspended followed by a third launch.
  - Other candidate: the writer thread died or never opened the file. An
    open or write error says `smoke observation writer: <error>`; a missing
    `HOME` ends the thread without a line. Either way the records already
    queued are gone, every later record says `smoke observation dropped:
    disconnected`, and the flush cannot help.
  - A Mac mini negative control (writer delayed 2 ms a record, Settings
    brought to the front, `simctl terminate` 3 s later) kept Background in
    5/10 rounds with the flush and 7/10 without; no round showed a flush
    timeout, so Background had not reached the app in the missing rounds.
    It does not decide whether the flush helps.
  - Now: every check reads gap records through `live_jsonl`; p10 and sh1-152
    keep `app-stderr.log` and fail on a lost-record line; p10 keeps
    `p10-settings.jsonl`; every `launch` record carries the pid. The app waits
    for its writer at Inactive, Background and Disconnected
    (`Observation::flush`, at most 500 ms, also when the handler returns
    early for a terminal already suspended) and says `smoke observation flush
    timed out: N records unwritten` when it cannot. A flush timeout alone does
    not fail a check: the oracle fails on the phase that is missing.
  - Main-thread bound (QA writer only, from `YEBISU_SMOKE_OBSERVE` or
    `YEBISU_P2_OBSERVE` at launch; without them there is no writer and
    nothing waits): the flush waits at most 500 ms a phase and its timeout
    record does not wait. Each record the handler itself sends
    (`settings-lifecycle`, `lifecycle`) waits only when the 4,096-record
    queue is full: up to 250 ms, or 500 ms when a drop notice goes first. So
    a phase blocks the main thread for at most 500 ms with room in the queue
    and about 1.5 s with a full queue; Active to Background delivers Inactive
    and Background in one callback, so twice that (1 s, about 3 s). A run
    queues at most 107 records a second, so the full-queue case needs a
    writer that has stopped. The wait runs in GPUI's lifecycle callback on
    the main queue, after the UIKit delegate returned, inside herdr-uikit's
    background task for a transition that reaches Background.
  - Next time: read `app-stderr.log` (a writer error, a drop or a flush
    timeout names the cause) and count `launch` pids in `p10-settings.jsonl`:
    two means activate resumed the first process, three means it was killed
    and relaunched; a pid followed by no records means its writer wrote
    nothing.
- **SH1-152-click (not an observation loss).** In E5d stage 1 the `152 Two
  End` tap was synthesized at the button's centre at t=174.43 s; the app drew a
  frame 0.07 s before (the same place as the pressed frame in the passing E5c,
  0.06 s before its `sh1-152-result`), then recorded nothing: no
  `sh1-152-result`, which `End` records first and unconditionally
  (`edit_menu_qa_sh1_152.rs` `sh1_152_command`), and no end marker in
  `p4-peer.jsonl`, which the mock peer writes synchronously. The click
  handler did not run. herdr-uikit turns a cancelled touch into a mouse up
  too (`touch.rs` `TouchStage::Cancelled`), so the up never arrived or did
  not complete a click. Same class as the lost presses under "Show that a menu
  is closed before the next tap" below; not on the known-class list.

### XCUITest identifier rules

(2026-11-20: UI test redesign S6. They replace the OCR rules of PR 55 and PR 57; owner decision
`ui-test-redesign-decision.json`.)

No UI test reads text off the screen to find, press or confirm a control. The OCR helpers
(`locateText`, `recognize`, `wholeWordRange`, `ocrLineCount`, `tapText`, `tapTextBox`,
`tapRecorded`, `tapMenuItem`, `textVisible`, `waitForRenderedText`, `openSettings`,
`openOverflowMenu`, `closeOverflowMenu`, `scrollBoard`) and `smokeqa-labels-check.py` are gone;
`test_smoke_qa_probe.py` fails if one comes back.

- Find a control by its QA identifier. The app marks the GPUI control with `qa_ax::mark(id,
  label)` (Simulator build with `YEBISU_QA_AX` only); `id` is a constant of
  `crates/yebisu/src/qa_ax_ids.rs` and `label` is the constant the control draws, so the two
  cannot drift apart. The Swift side names it in `QAIds.swift`; `qa-ax-ids-check.py` (the
  `QA identifiers` step of `check`) fails on an identifier that is not a constant or that
  nothing marks. Every test that uses the layer sets `YEBISU_QA_AX` in its launch environment
  (P0, launched by `smoke-check.py`, gets it as `SIMCTL_CHILD_YEBISU_QA_AX`).
- Press with a real touch and confirm by `qa.state`. `QAAx.press` taps the element's own
  coordinate and waits until `presses[id]` grew and the step's effect holds in `qa.state`.
  `isHittable` is never a verdict (S0: it ignores a GPUI menu backdrop over the control).
- Repeat a lost press once, and say why. A press that moved neither `presses[id]` nor a screen
  field (`screen`, `sheet`, `menu`, `active`) is sent once more and recorded in
  `QAAx.Session.retries`; one that something else took, or one that reached the control
  without its effect, fails by name with the last `qa.state`.
- Wait on `qa.state`, not on a delay or the screen. A test waits for the field its step changes
  (`p0_input`, `font_size`, `theme`, `host`, `observer`, `p14`, `p12`, `peer`, ...). Setup that
  is not under test is a launch variable (`YEBISU_QA_START_SELECTION`, the QA modes), not a
  sequence of presses.
- Show that a menu or sheet is closed before the next press: wait until `menu` or `sheet` is
  null (run 44297932478, job 134779801456: a lost close press left the `...` menu open and the
  next tap only closed it).
- After a launch, wait until a touch reaches GPUI before the first real press. The first touch
  after a launch can reach nothing while `qa.state` already shows the screen (PR 84 run
  44618790826, edit-menu E3f-text: the recording shows the relaunched window still black at the
  press). `QAAx.touchReady` presses a marked empty space with no handler (`qa.sh1-152.empty`,
  `qa.p12.empty`) until GPUI counts the press, at most 8 times, and logs how many it took.
- Wait for the software keyboard to settle only when something may have moved it
  (`QAAx.keyboardMayMove` / `settleIfMoved`): a rising keyboard moves the controls above it.
- Keep the queries inside a held request few, and measure them. The pinned sheprd-client
  `COMMAND_TIMEOUT` is 60 s (herdr-gpui f63688b, `crates/sheprd-client/src/limits.rs:19`); a
  request held that long drops the connection, and any XCTest query (a read, a tap) can first
  wait for the app to idle (60-80 s on an old runner image: run 44547820393 attempt 1, F11, F15).
  The test cannot remove that wait, only make it less likely to land in a hold: P12 keeps the
  frames `locateFixtures` found at the start for the controls it presses inside a hold, reads `qa.state` every 250 ms there instead
  of every 50 ms, and records the reads of each hold (`hold-queries`, the oracle's
  `hold_queries`, with a `warning` once a hold window passes 40 s). Hold one request per local
  case and release it before the next.
- Read the app's files only after it was terminated. Every driver terminates the app after
  XCTest (also when XCTest failed) and only then copies or reads its JSONL files; the checks
  there are the receiving-side evidence, not the test's waits.
- Text stays a screenshot check only where the text is the subject. One is left: P15's host
  settings block (`SmokeQA+Theme.swift`, `p15Host`) reads the screenshot with Vision to show
  the current host and never the old one; the test first waits until `qa.state` shows that host.

The Mac-mini-only tests follow the same rules: P11 (`testP11Settings`, run by
`p11-check.py`; `testP11Observer{Text,Drawing,Modal,Disabled}`, run by
`p11-observer-check.py` with the `test-support` build), P14 (`p14-check.py --lease-id <id>`)
and P15 (`p15-check.py`). Each runs on a leased Mac mini against a disposable Simulator
(`HERDR_IOS_SIMULATOR=<udid>`); see their docs (`p11-status.md`, `p14-native-qa.md`,
`p15-readonly-model/INTEGRATED-CANDIDATE.md`).

### P12 receipts, retries and negative controls

(2026-11-18: P12 reliability PR; jobs 134018333353, 134059155142 and 134160368350. Updated for
the UI test redesign S5 and S6.)

- Wait on the receiving side, not on a delay. Since S5 each fixture control is pressed by its
  identifier (`qa.p12.<step>`) and `P12QA` waits until `qa.state` shows one more
  `p12-setup-end` of that step (`p12.setup`); it reads no file of the app while it runs
  (before S5 it polled `Documents/p0-smoke.jsonl` through `TEST_RUNNER_P12_DOCUMENTS`). The
  Finish step also waits until its FIFO fence reached the private peer (`qa.state` `peer`).
  `p12-check.py` terminates the app after XCTest and only then copies `app.jsonl` and
  `peer.jsonl`. A reveal waits on the Springboard control (`exists` and `hittable`), as before.
- Repeat once, and say why. A press that reached nothing (5 s) and a swipe without a
  revealed menu bar (10 s) are repeated once. Each repeat is a `fixture-retry` or
  `reveal-retry` record in `p12-ui-events.json` with its reason; the oracle
  accepts at most one per control or case and prints all of them under `retries`
  in the verdict, so a rate is never hidden. A second miss is a failure
  (P12-fixture, class B). No assertion was weakened: the received-receipt check,
  the revealed-menu-bar assertion, the hierarchy and menu-close checks and every
  oracle correlation are unchanged.
- The first touch after launch: `touchReady` presses the panel's empty strip (`qa.p12.empty`,
  no handler) until GPUI counts it (at most 8 presses) and records how many it took.
- Inside a hold (S6): the local and Release presses use the frames `locateFixtures` found at the
  start (not read again right before a case: that went with 2 lost Springboard taps in 9 runs) and read
  `qa.state` immediately after each tap returns, then every 250 ms while waiting for the
  receipt. Each hold's reads and their seconds are a `hold-queries` record and
  appear under `hold_queries` in the verdict, beside the hold's length.
- The events attachment is added in `tearDown`, so a failed run keeps
  `p12-ui-events.json` (before, a failed assertion skipped the `defer` and the
  reveal and menu-close records were lost).
- Negative control: `P12_QA_FAULT=drop-tap:<step>,skip-swipe:<case>
  python3 scripts/ios/p12-check.py ...` (empty in CI) skips the first tap of a
  step or the first swipe of a case. `p12-check.py` then requires one logged,
  injected retry for each fault, so a run with a fault proves that the retry path
  ran and that the oracle accepts it.
