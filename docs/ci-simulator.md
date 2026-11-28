# Simulator CI

The `simulator-smoke` job uses GitHub-hosted `xcode-27`, read-only contents
permission and a 40-minute timeout. It installs XcodeGen and just, then uses
the pinned Rust toolchain and the continuously monitored 6 GiB disk guard.

## Xcode and iOS SDK

Every macOS job in `ci.yml` (`edit-menu`, `simulator-smoke`, `check`,
`layout-smoke`) and the `testflight` job run on the `xcode-27` label, an arm64
macOS 27 image (public preview) with Xcode 27.0, 27.1 and 27.2 (owner decision
2026-11-26: CI and TestFlight build with the latest iOS SDK, 27.2). The image's
default is Xcode 27.1, so each job sets
`DEVELOPER_DIR=/Applications/Xcode_27.2.app/Contents/Developer`, and its first
step, `Require Xcode 27.2 and iOS SDK 27.2`, fails the job unless
`xcrun --sdk iphoneos --show-sdk-version` prints 27.2. The Simulator scripts
pick the newest runtime that has an iPad Pro 13-inch device; with Xcode 27.2
that is iOS 27.2 with iPad Pro 13-inch (M5)
(`actions/runner-images` `images/macos/xcode-27-arm64-Readme.md`). The cargo
cache keys already hash `xcodebuild -version`, so the change of Xcode starts
new entries.

## When CI runs

- **Pull requests.** No Simulator job runs on a pull request:
  `simulator-smoke`, `edit-menu` and `layout-smoke` are skipped on every PR
  (owner decision 2026-11-26: PRs merge on local tests, and main's CI catches
  the rest). The `changes` job selects `check` from the whole PR diff, and
  `commit-subjects` runs on every PR. Within `check`, `Layout geometry source
  mutations` runs when the diff matches layout, shell, settings, settings
  pages, core/screen, terminal_input, their test paths, the mutation script,
  or the build inputs listed in the `mutations` pattern in `ci.yml`. If the
  diff is unavailable, `check` runs and the mutation step waits for main. The
  `smoke` and `menu` patterns still name the files each Simulator job drives:
  when the diff touches one, the `changes` log says
  `the diff touches <job>'s area: run it locally; main runs it after the merge`,
  so the local gate runs that Simulator check before the merge.
- **Pushes to main.** Each macOS job runs unless the GitHub API identifies a
  merged PR with the exact same Git tree and a completed job whose every step
  passed in the latest attempt. Since PRs run no Simulator job, only `check`
  can be reused this way; main runs `simulator-smoke` and `layout-smoke`
  itself. `edit-menu` runs on a push only when the push touches one of its own
  files (owner decision 2026-11-27): the XCTest files with the tests its checks
  select (`EditMenuQA`, `EditMenuTargetsQA`, `SmokeQA+Handoff`, `+Settings`,
  `+Input`), its driver `ci-sh1-152.py`, its checks and oracles (`sh1-15x`,
  `sh1_152_*`, `p8-native`, `selection-settings`), and the app code
  `edit_menu*`, `native_menus*`, `native_edit*`, `native_observer*`,
  `handoff*` and `terminal_input*` (the `menu_own` pattern in `ci.yml`).
  Otherwise the `changes` log says
  `the push touches none of edit-menu's own files; the nightly run runs it`.
  A push whose diff is unavailable runs it. `edit-menu` also verifies the
  tree of the synthetic merge commit its runner checked out. Missing or ambiguous evidence
  leaves a job scheduled. A PR `check` that skipped layout mutations cannot
  replace main's `check`. Whenever `check` runs on main, it runs the mutations.
- **Nightly on main.** Every day at 03:00 JST (18:00 UTC), all six jobs run,
  including `commit-subjects`, `layout-smoke` and the layout mutations in
  `check`. The nightly run bypasses PR evidence, exercises the current runner
  environment and gives successful jobs a chance to populate main's Cargo cache.
- **Manual CI.** A manual dispatch on main runs all four macOS jobs, `edit-menu`
  included, whatever it changes. The `layout-smoke` job is restricted to main.

For a skipped job, open `changes` in the Actions run. On a PR, its
`Select jobs by changed paths` log lists the changed files, the selection and
the Simulator areas the diff touches.
On a main push, the run summary lists any reused PR jobs with their matching
tree SHA and links to the original job logs and evidence. Before the owner
dispatches TestFlight, check that CI for the exact main tip is green, and
follow these links for any skipped job. TestFlight's workflow is unchanged.

### Cargo cache capacity follow-up

The PR 91 review (F1) measured 2,767,058,922 bytes across four cache entries
for a single PR ref. Cargo.lock changes can add a new key on the PR ref and
again on main. Restoring an older entry can also carry forward obsolete
compiled dependencies. Repeated saves can evict main's entries and cause cold
PR runs. The nightly run replenishes main's entries when its jobs succeed.
Changing the save policy is deferred because this change is scoped to CI
selection and scheduling. Start by measuring occupancy and eviction per ref
with `gh api repos/shin1ohno/yebisu/actions/caches --paginate`; use that
baseline to evaluate restore-only PR jobs and pruning stale build outputs.
Close this follow-up once main's entries remain available across Cargo.lock
changes and a new PR demonstrates a warm restore.

## Simulator smoke harness

`scripts/ios/ci-smoke.py` selects the newest installed iOS runtime that already
has an available iPad Pro 13-inch device and creates a disposable Simulator of
that device type. Non-numeric runtime suffixes are skipped with a log message. It calls
the same `scripts/ios/smoke-check.py` used by `just ios-sim`, without changing
the oracle: bundle/build, boot, install, launch, XCTest input and lifecycle
actions, screenshots and receiving-side paint/input/counter validation all
must succeed. It attempts shutdown and deletes its own Simulator on success or
failure. A cleanup line records the shutdown exit code and successful deletion;
a failed shutdown does not prevent deletion.

One exception keeps the job inside its 40 minutes (issue #235). On the
`xcode-27` image a freshly booted Simulator blocks its first install for
3.5-8.9 minutes after `simctl bootstatus` returns, against 1.0-2.3 minutes on
`macos-26`; later installs on the same device take seconds. The job paid that
twice, once for P0 and once for P12's own Simulator. So in GitHub Actions with
`--p9` a passing `ci-smoke.py` run writes the UDID to
`target/ci-smoke-simulator.txt` and prints `CI SMOKE handover` instead of the
cleanup line. `p12-ci.py` takes that Simulator, uninstalls the app (P12 reads
the app's Documents, so P0 and P9 leave nothing there), runs `p12-check.py` on
it and deletes it. It prints `P12 CI reusing simulator=...`; without a usable
handover it says why and creates its own Simulator as before. A failed P0 or P9
check still deletes the Simulator in `ci-smoke.py`. Booting the Simulator before
the build was tried and dropped: in run 45856917743 the build took 6.9 minutes
(0.7-3.7 before) and the readiness wait before `xcodebuild test` 152 s (14-33 s
before), about what the faster install saved.

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
| `layout-smoke` (`xcode-27`) | `scripts/ios/ci-layout-smoke.py`: one disposable Simulator, then `p2-check.py` (P2 keys and restart), `p3-check.py` (P3 terminal, density, colours) and `p10-check.py` (P10 settings lifecycle) in that order | 60 minutes | main pushes, nightly runs and manual dispatches on main; never a pull request |
| `check`, step `QA identifiers` | `scripts/ios/qa-ax-ids-check.py`: every QA identifier a Swift UI test presses is a constant of `crates/yebisu/src/qa_ax_ids.rs` that the app marks (UI test redesign S6 replaced the SmokeQA label check, which tied every text SmokeQA tapped or OCR-matched to the app source that painted it) | within the 60-minute `check` job | every `check` run |

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

The 2026-11-16 merge rule covered L15, L16, A, B and P12-shot. P12-fixture
(first seen 2026-11-17) was outside that rule.
C-snapshot and C-anim left the list when PR 58 merged (716018a), and C-paint
was never on it. Since PR 58 merged, treat failures with those signatures
as new defects. L19 and L20 track work on L15 and A/B.

| Class | Log signature (grep) | Rate | Cause | Fix / open item | Evidence |
|---|---|---|---|---|---|
| L15 | `Missing source-defined control: Paste` (`OCR attempt [0-2] for Paste`, then `missing-control`) | 11/41 P9 runs whose head has the Settings button; 0/49 without | The OCR after a `QA image ready` finds no control: no standalone `Paste` after the second one (4 of 11), no `Paste image` after the first one (7 of 11). What Vision returns is `[unproven]` | 4 of 11 (second `Paste`): PR 57, merged 1de7020. 7 of 11 (first `Paste image` misread): open, L19, after PR 57 | run 43746802879/1 job 132911829322 (artifact 14175507358); run 43754156237/1 job 132934538869 (14177458803) |
| L16 | `image receipts incomplete: puts=3 paths=3 text=0`, with the logged tap box at x=0.215 | 3 of the 41 jobs that reached SH1-154 in the L16 scan; a 4th job was added later | The second text `Paste` tap landed on `Paste image` (box x=0.215, the right one is 0.151), so no `P8-shadow` text arrived. A `text=0` whose tap box is x=0.151 has another cause (job 132483179735, head 1954cab, no Settings button): not L16 | PR 57, merged 1de7020; 3 of 3 `simulator-smoke` passes at its head 756f70b (run 43766302928) | run 43736757874 jobs 132881022688, 132900139639; run 43760942018 job 132955457568; added job 132969174547 |
| C-snapshot | `SmokeQA.swift:23: error: ... Failed to get matching snapshot: Timed out` | 3/98 P0 runs | `app.activate()` right after Home does not take effect and no characters reach the app (the app stderr shows 8 `input committed` lines, all for `P0-input`; none for `-returned`), so the `typeText("-rotated")` at `SmokeQA.swift:23` times out. Happens on a freshly booted, slow Simulator. That the app stays suspended, and the press/activate ordering, are `[unproven]` | PR 58 (merged, 716018a) replaces the fixed sleeps after Home, `activate()` and `typeText` with state waits. A failed wait now stops the test at once with `initial activate(): app.state is ...`, `app left the foreground after press(.home): app.state is ...` or `activate() after press(.home): app.state is ...` (`SmokeQA.swift` `requireState`, 30 s each); since PR 58 merged, a non-foreground state after `activate()` is a new defect, not a known class | run 43730444891/7 job 132946288166 (artifact 14179437935); run 43720814607/1 job 132832187258; run 43729845313/2 job 132888134849 |
| C-anim | `App animations complete notification not received` (60 s waits, 25 times, job cancelled at 40 min) | 1/92 P9 runs | `animationsHaveFinished` stays NO after `P9-native\n`; what keeps animating is `[unproven]` | PR 58 (merged, 716018a) caps `testP9Core` at 150 s (XCTest rounds it up to 3 minutes; `-test-timeouts-enabled YES` in `p9-check.py`), so a hang now ends with `Test Case '-[SmokeOracle.SmokeQA testP9Core]' exceeded execution time allowance of 3 minutes` (negative control: run 43790640783, 02:25:17.999 to 02:28:17.998); the QA build writes an `animations-running` record with each layer's class and `animationKeys()` into `p0-smoke.jsonl` once an animation stays for 10 s. It does not fix the cause; since PR 58 merged, a failure with this signature is a new defect. Integration 3 (L8) set the cap to 210 s (4 minutes). In run 44276079121 (job 134704511383) the test finished every step in 209.8 s against 96-121 s before L8: about 30 s for the five `...` menu trips (cut by `tapMenuItem`, one OCR pass per trip) and one 60.09 s wait (`App animations complete notification not received` after `P9-pane1\n`, with an `animations-running` record for `CASDFElementLayer` 10 s after the Reconnect tap). The cap keeps room for the body plus one such wait, as 3 minutes did before | run 43730444891/4 job 132914175740 (artifact 14176482747) |
| C-paint | `input result expected 'P0-input', received 'P0-inpu'` | 1 job seen | A fixed `sleep(2)` after `typeText` ended before the last character was drawn | PR 58 (merged, 716018a) waits for the typed text on screen; a miss now fails as `typed '<text>' received: '<word>' not on screen after 20 s` (the `-rotated` step keeps its sleeps, the keyboard covers the field); since PR 58 merged, a failure with this signature is a new defect | run 43684762210/1 job 132722801994 |
| A1 / A2 | A1: `OS menu still open 5 s after the pick tap ... (class A)` (the menu stayed open; since issue #142 stage 3 a proven A1 is tapped once more, `pick-retry`, and a second miss is `pick lost twice in case <case>`). A2: the oracle's `tap lost (class A2) in case <case>: <stage>; <builds>`, where `<stage>` is `no menu-perform`, `menu-perform no_tag` or `no_sender`, `menu-deliver stale_generation`, `no_callback` or `callback_running`, `menu-global`, `menu-perform without menu-deliver`, `pick tap reached the app (...)` or `menu-deliver delivered: the command reached GPUI; it stopped in GPUI or yebisu` | A2 5 (2 on main), 2026-11-20 to 11-23 (issue #142); A1 2/52 P12 runs before PR 100 | A2: SpringBoard closed its menu without performing the item after a pointer click (indirect evidence: in 6 lost picks the menu bar closed 0.76-0.86 s after the click, in 0 of 39 received picks within 1.0 s). A1: BackBoard reinitialized the touch digitizer and cancelled the contact (job 136312328508, primary evidence) | Issue #142: the pick and dismiss are taps again (PR #158); the app's input receipts name where a command stopped (PR #147); an A1 is resent once only when the app shows no menu-perform, no touch and no screen change since the swipe; A2 and the SDK or app defect rows fail without a resend (decision table in "P12 receipts, retries and negative controls") | Before: run 44743338211/1 job 136312328508 (artifact 14635411736, exported `system.logarchive`, 14:10:37 UTC); run 44704476837/1 job 136180580090 (artifact 14617634414, 09:48:41 UTC); run 43730444891/3 job 132903694966 (14174842401); run 43717998985/1 job 132823606035 (14164345944). After: [run 45519614338 job 138887958418](https://github.com/shin1ohno/yebisu/actions/runs/45519614338/job/138887958418) (P12 on the touch path, `retries` empty, receipts menu-perform 9, menu-deliver 9) |
| B | `OS menu bar not revealed (class B)` in `xctest.stdout` (PR 59, merged c7013bb); before PR 59: `Installed OS application menu must be revealed and reachable` (`P12QA.swift:68`, `:81` since PR 55) | 4/52 P12 runs; 2 more of the 3 P12 jobs of 2026-11-17 | The top-edge swipe does not reveal the menu bar or `•••`. Cause `[unproven]`. In both new failures the swipe call took 5.6 s and 5.7 s (`xctest.stdout`: from `Synthesize event` to the idle wait after `Press Application`), against 0.6 to 1.1 s for 14 of the 15 swipes that revealed it (the 15th took 4.2 s); the XCTest gesture was spread over a stalled Simulator. The PR 59 precondition record does not separate them: Window Controls absent, app window not focused, `runningForeground` in all 10 reveals of job 134059155142 (3.2 to 53.1 s since the last close) and in both failures (3.3 s, 35.6 s) | Open: L20 (PR 59 merged, c7013bb: observation only). P12 reliability PR: `P12QA.revealMenuBar` swipes once more when the bar is absent after 10 s and logs a `reveal-retry` (reason, swipe seconds) that the oracle accepts once per case and prints in the verdict `retries`; a second miss still fails as class B, and the XCTest assertion after the reveal is unchanged | run 44077223916/1 job 134018333353 (`xctest.stdout:708`, case palette); run 44117720382/1 job 134160368350 (`xctest.stdout:628`, case rearm); run 43699190351/1 job 132766634071; run 43729845313/1 job 132859692803; run 43754156237/4 job 132956853113; run 43730444891/9 job 132960855571 |
| P12-shot | `xctest.stdout`: `Failed to get screenshot: Timed out while requesting screenshot` from `P12QA.swift` (at 716018a `:278` is the `screenshot()` helper and `:255` is `recognize()`; the case and step are the last `Added attachment named 'p12-…'` / `Tap` lines before it). Supporting: `app.jsonl` has no `"kind":"terminal"` record for ≥ 10 s outside the `boot` case. Job log shows only `P12 xctest: exit 65` | 1/53 P12 runs (2026-11-15T00:00Z–2026-11-16T12:06Z); 0/1536 vs 1/204 screenshots before/after PR 59 | The Simulator stopped drawing for ~24 s with the OS menu open; XCTest gives up on a screenshot after 30 s. Why drawing stopped is `[unproven]`. A held `pane.split` can then pass the 60 s `COMMAND_TIMEOUT` (`Disconnected: endpoint request timed out`); that is a consequence, not the PR 55 class | PR 63 (merged, 2c1e8b2): open-menu screenshots only on failure, 39 to 11 per run. Open: time every capture | run 43834250246/1 job 133186893086 (artifact `p12-native-menu-evidence`) |
| P12-fixture | Job log: `P12 xctest: exit 0`, then `ValueError: app prepare cases missing/duplicated` (`p12-oracle.py:73`) with the Springboard reveals all passing (before this PR). Now `xctest.stdout`: `Fixture tap on 'P12 <Control>' not received by the app, also after one retry (class P12-fixture)`, or the oracle's `fixture tap lost (class P12-fixture): the app logged no p12-setup-begin for step <step>` | 1 of the 3 P12 jobs of 2026-11-17; not measured over the window | The first fixture tap (`P12 Remote`, `Tap Application ...[0.87, 0.09]` at `xctest.stdout` t=53.04 s, XCTest `Synthesize event` normal) produced no `p12-setup-begin`/`p12-prepare`/`p12-setup-end` for `remote` in `app.jsonl`: between the lines 16 (13:28:45) and 73 (13:29:39) there are only `terminal` records, and the pick that followed logged `case: ""`. The fixture OCR before it took 41 s (13:28:46 to 13:29:27). The tap point was the same as in the passing runs. Why the app did not receive the tap is `[unproven]` | P12 reliability PR: after each fixture tap `P12QA` waits (5 s) for the app's own `p12-setup-end` of that step instead of a fixed delay, taps once more when it is absent and logs a `fixture-retry`; a second miss fails by name. The oracle names the step before its generic checks and prints the retries in the verdict. Replaying the evidence of job 134059155142 through the new oracle gives `fixture tap lost ... step remote` | run 44077223916/2 job 134059155142 (artifact `p12-native-menu-evidence`: `app.jsonl` lines 16-73, `xctest.stdout:430`) |
| P12 confirmation deadline | `P12 held press ... lost` followed by `after=nil` in `xctest.stdout`, with setup begin and end in `app.jsonl` | 1 observed job on 2026-11-20 | The synchronous fixture tap used up the XCTest receipt window before the test read `qa.state`. Both Sidebar taps reached the app | Start each 5 s confirmation window when its tap returns, using monotonic time. Read `qa.state` immediately and require both receipt counters to advance | run 44704476837/1 job 136180580090 (artifact 14617634414, `xctest.stdout` lines 607–628 and `app.jsonl` lines 100–109) |
| P12 command availability | `Native command availability wait expired` (`exists` and `enabled` reported) or `Native command isHittable=false after becoming available` | 1 observed job at the previous gate on 2026-11-20 | The open-menu predicate included hittability. Its first element query used about 2.2 s of a 3 s wait on an enabled, visible Command Palette row | Query availability before starting the 3 s wait; wait on existence and enabled state, then check hittability separately. When opening the menu by touch fails, the availability failure records `exists=false`; inspect the attached SpringBoard hierarchy to diagnose the missing row | run 44787126992/1 job 136467509487 (artifact 14656298877, `xctest.stdout` lines 825–849) |
| H | `Native menu disappeared; this is not refusal evidence` (`P12QA.swift:213` at 773e7bd) | 15 (3 on main), 2026-11-20 to 11-23 (issue #142), the largest class | SpringBoard reacted to XCTest's pointer hover: twice a Cover Sheet indirect pan (`indirectPresentCoverSheetGestureRecognizer`) took key focus, three times its menu closed 0.07-0.14 s after the hover (`_UIContextMenuAnimator`). Confirmed in issue #142 stage 1; why only that hover is not known | Fixed: P12 no longer hovers or clicks (PR #158, Owner decision 4); the pointer menus are an owner iPad check. For SpringBoard evidence of a new case, export `result.xcresult` on a Mac with `xcrun xcresulttool export diagnostics --path result.xcresult --output-path diagnostics`, take the device's `system.logarchive` from the export and correlate its timestamps with `app.jsonl` | [main run 44964550775, job 137035121814](https://github.com/shin1ohno/yebisu/actions/runs/44964550775/job/137035121814) (artifact 14732266409, `xctest.stdout:845–862`, `app.jsonl:152–162`, exported `system.logarchive` at 00:40:54.229–00:40:57.323 UTC). Last seen: [run 45388075978 job 138424249102](https://github.com/shin1ohno/yebisu/actions/runs/45388075978/job/138424249102) (case palette). After: [run 45519614338 job 138887958418](https://github.com/shin1ohno/yebisu/actions/runs/45519614338/job/138887958418) |
| T | `SH1-152 no touch reached GPUI after launch <case>: press on qa.sh1-152.empty never reached it (8 attempts ...)`; since issue #142 the message adds `input.touch_began +<n>, last_touch ..., control centre (...): <class>` | 4 (1 on main) | The OS handled the contact; whether it reached yebisu's UIKit view was not known `[unproven]`. The class (`no touch reached the app`, `touch at about half/double the coordinates`, `touch elsewhere`) now says it | EditMenuQA launches the case once more (`relaunch-retry`, PR #164); a second miss fails with the receipts. P12 reports only (one peer start) | [run 45549395928 job 138986728421](https://github.com/shin1ohno/yebisu/actions/runs/45549395928/job/138986728421) |
| F2 | oracle `setup callbacks missing/duplicated` after a press that arrived 5-9 s late was sent again | 1 (0 on main) | The old timeout rule resent a press that had not been lost (decision-rule defect) | `QAAx.press` decides on the app's receipts (PR #158): a touch that reached the app is waited for once more and never sent again; it then fails with `last_touch` (`pressArrivedElsewhere`). A touch UIKit cancelled before it ended is proven lost and resent once. The settings test now proves taps are taken after its scroll (#161) | [run 45388075978 job 138424249109](https://github.com/shin1ohno/yebisu/actions/runs/45388075978/job/138424249109) (two cancelled touches and one tap after a scroll, each resent by the old rule) |
| B2 | `Failed to get matching snapshot: No matches found ... StatusBar` | 1 (0 on main) | Harness defect: `waitForMenuBarPressPosition` read `bar.frame` before checking the bar exists | `bar.exists` first (PR #158), so a missing bar becomes `reveal-retry` and then a named class B; class B's message adds `app touches during the swipe: began=<n>, last_touch=...` | Source test `test_menu_bar_presence_is_checked_before_its_frame` |
| D | `xcodebuild: error: Unable to find a device matching the provided destination specifier` | 1 (0 on main) | A new Simulator missing from xcodebuild's list `[unproven]`. On the issue #142 Mac a just-booted device was absent from `xcodebuild -showdestinations` for 39-87 s while `xcodebuild test` already ran on it (64 s on the hosted runner in run 45549395928) | Every hosted `xcodebuild test` logs `SIM READY` or `SIM NOT LISTED` first (a report), and an `xcodebuild test` that found no device and ran no test is sent once more (`destination-retry`, counted in the job budget; owner decision 2026-11-25, replacing the issue's fail-at-60 s) (PR #164) | [run 45549395928 job 138986728467](https://github.com/shin1ohno/yebisu/actions/runs/45549395928/job/138986728467), [run 45549395928 job 138986728421](https://github.com/shin1ohno/yebisu/actions/runs/45549395928/job/138986728421) |
| K | `app left the foreground after press(.home): app.state is runningForeground after 30 s` | 1 (1 on main) | Home did not reach SpringBoard `[unproven]` | `QAAx.pressHome`: when the app stayed in front and `qa.state`'s `lifecycle_inactive` did not grow, Home is pressed once more (`home-retry`); a second miss keeps the named failure (PR #164). Negative control `QA_HOME_DROP=<n>` | Local controls in PR #164 (drop 1: one resend, pass; drop 2: named failure) |
| Q | `qa.state wait '...' timed out after 25.56 s (1 reads)` | 1 (0 on main) | One AX query used the whole wait `[unproven]` | Reported, not fixed: the app's 2 s heartbeat (PR #147) shows whether the main thread stopped; `heartbeat_gaps` in the P12 verdict (PR #147), the edit-menu verdict and P0/P9 `heartbeat-gaps.json` (PR #164) | — |
| S | `press on qa.menu.select-text reached it but 'typing is paused (Select on)' did not hold within 10 ...` (SH1-156) | 1 (1 on main) | XCTest waited 60 s for the app to idle and the connection broke `[unproven]` | Reported, not fixed: heartbeat gaps as for Q | — |
| M | `Native command did not become actionable`, `Native command missing` or `availability wait expired ... exists=false` | 3 (0 on main) | The tap that opens the menu was lost `[unproven]` | When no menu item appeared and the app shows no touch and no screen change since the swipe, the open tap is sent once more (`open-retry`, PR #158); anything else fails with `not tapped again: <why>` | Local negative control `drop-open:<case>` in PR #158 |
| L | `Failed to launch com.shin1ohno.yebisu: Timed out attempting to launch app`, `... does not have a process ID`, `Timed out waiting for AX loaded notification`, `qa.state wait 'launch ...' timed out`; since issue #247 `QA-RESEND` `launch-retry` with reason `no-pid-app-running: ...`, and `LAUNCH NO-PID not launched once more (class L)` | 3 (0 on main) in the L17 window. `does not have a process ID`: 4 jobs from 2026-11-26 to 11-28 (edit-menu 3, P12 1) in 65 failed, 37 cancelled and 58 passed CI jobs; no passed job logged a launch issue | `does not have a process ID` (issue #247): the app started, but XCTest got its pid late. In each system.logarchive runningboardd tracked the new process 0.08-2.22 s after testmanagerd's `Launching`, and testmanagerd gave the pid to the test session 68.8, 73.5 and 95.3 s later, once never (runs 46089691282, 45846870924, 46037705458, 45800164150). The other 60 launches of those runs took at most 17.0 s. In nightly 46089691282 the delay fell inside a stall of the hosted VM: guard probes 58 s and 41 s apart against a 6 s median, load1 771, 1.4 GiB swapped, 18 % memory free (as in #256). The other three have no probe record `[unproven]`. `Timed out attempting to launch app` (run 46060752953) is different: the process itself was tracked only after 30.2 s, with no probe stall. Measure: `xcresulttool export diagnostics`, then compare runningboardd `Now tracking process` with testmanagerd's `<XCTestSession ...> ... pid: <n> state: running` | XCTest records a failed `app.launch()` as issues that `record(_:)` can take (PR #164 probe: all four launch issues arrive and the test goes on after `launch()`, which returns after 235-300 s). EditMenuQA launches once more only when the app is not running and has no `qa.state`, and `sh1-152-check.py` checks that `app-stderr.log`'s launch lines prove no process started. Since issue #247 (controller decision 2026-11-28) EditMenuQA and P12QA (`testInstalledTerminalMenu`) also launch once more when every issue is `does not have a process ID` and the app's `qa.state` was read (`LaunchIssue.decide`, QAAx.swift). That retry counts in the resend budget (LIMIT 3), and `sh1-152-check.py` requires its first launch to have started a process. When no `qa.state` was read, the issue fails as XCTest recorded it, except that EditMenuQA keeps the not-running rule above (P12QA has no other launch retry). Negative controls: `TEST_RUNNER_SH1_152_QA_FAULT` and `P12_QA_FAULT` take `no-pid:<case>` (the run must pass with one retry) and `no-pid-no-state:<case>` (no `YEBISU_QA_AX`, so the run must fail with no retry); the target for P12 is `launch`. `Timed out waiting for AX loaded notification` happens before test code and stays reported, as does a `launch` wait on `qa.state` once the app runs. SmokeQA's own launches (for example `testP8NativeEditing`, run 45800164150) are not repeated | PR #164 text; issue #247 (task 8c768d17af114569 report) |
| T-reap | `Failed to terminate com.shin1ohno.yebisu:<pid>` (edit-menu, `EditMenuQA.swift` after a case), then `Lost connection to the application (pid <pid>)` in the next test; since issue #247 `SH1-152 TERMINATE SLOW <case> <seconds>` (passed) or `SH1-152 TERMINATE NEVER ENDED (class T-reap)` (failed) | A terminate over 20 s in 3 of 9 edit-menu jobs on the xcode-27 image with the gpui-pre fork (2 failed it), 0 of 8 without it, 0 of 180 on macos-26 (2026-11-20 to 11-27; all 3 slow ends 16:57-21:00 UTC on image 20261123.0387.2, so fork and runner load are not separated) | The OS killed the app (runningboardd `terminate_with_reason() success`) and launchd reported it gone 13.4, 46.4 and 51.5 s later, against 0.0-1.1 s for every other kill of those runs; the app wrote nothing after the kill, so the delay is in the kernel or the hosted VM `[unproven]`. Locally 0 of 150 kills took over 0.03 s (d3c1742 and 2e4418b interleaved, 3 runs each) | `EditMenuQA.terminate` takes the issue as `start` takes launch issues and waits up to 180 s (`TerminateWait.timeout`) until `kill(<pid>, 0)` finds no process and `app.state` is `notRunning`, so the next launch starts a new process; an app still running then fails by the class name (issue #247, owner decision 2026-11-27). Negative control: `QAAxLogicTests.testTerminateWaitGivesUpOnAStubThatNeverEnds` | run 45890478629 (3cd39a6, pid 16999), run 45864283800 (5a92e08, pid 18054), run 45846870924 (2e4418b, pid 27486), each from the artifact's exported `system.logarchive` |
| SH1-152 portrait confirmation | `testSH1152Basic` fails before launching E1-text with `Timed out waiting for confirmation of orientation change` in `xctest.log` | 1 observed job on 2026-11-20 | XCTest reported a portrait interface before the test. The unconditional pre-launch device orientation request at 1.54 s failed before its 14.57 s notification. The same source passed when the notification arrived at 1.53 s after a request at 1.06 s | Launch and wait for `qa.state`, then require a portrait SpringBoard frame before the first touch. Request portrait once only if a landscape frame was observed | run 44765771959/1 job 136389954788 (artifact 14647790317) and passing control 44757970198/1 job 136362684853 (artifact 14652096259) |

Resolved (issue #142 Class table, 2026-11-20 to 11-23; no change needed in issue #142):

| Class | Log signature | Count (main) | Why it is resolved |
|---|---|---|---|
| A1 | `OS menu still open 5 s after the pick tap ... (class A)` | 2 (0) | 0 since PR 100; it can come back on the touch path (PR #158) and is then resent once by `pick-retry` (A1 / A2 row) |
| F | `Fixture tap on 'qa.p12.<step>' not received by the app, also after one retry (class P12-fixture)` | 3 (0) | 0 since PR 100 |
| G | `Failed to determine hittability of "yebisu" Button: Activation point invalid` | 2 (1) | 0 since b0757f6 |
| O | `Failed to set device orientation: Timed out waiting for confirmation of orientation change` | 1 (0) | 0 since e671ade |

Also fixed: a P12 held `pane.split` request that reached the 60 s client
timeout (`held request reached the client's 60 s COMMAND_TIMEOUT`; earlier
symptoms `P12QA.swift:78` `Native command missing` and `:79` `Captured native
command must start enabled`), PR 55, merged cb18764.

PR 59 (merged, c7013bb) changed the failure messages of A and B: it adds
`OS menu bar not revealed (class B)` and `OS menu still open ... (class A)`.
Rows A and B list the new message first and the earlier one after it.

### Classify a failing smoke job

(The 2026-11-21 owner decision sets the merge policy in step 4.
The 2026-11-20 owner decision sets the rerun rule in step 5.)

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
   it as touched when reporting the failure.
4. Simulator merge policy (owner decision 2026-11-21,
   `owner-batch-2026-11-21-decision.json`, `ci_simulator_on_prs`; since the
   owner decision of 2026-11-26 pull requests run no Simulator job). A PR
   merges on its local gates; when the `changes` log says the diff touches a
   Simulator job's area, run that check locally first. A failure on main is
   classified with steps 1-3: cite the primary evidence and which merged PR
   caused it. On main and before dispatching TestFlight, require green
   results for all three jobs. Merge permission stays per PR or approved
   plan. Merge with a merge commit.
   Resends (Owner decision 5, issue #142): a UI test sends an input once more only when the
   app's own receipts prove it was lost, at most once per input, and logs each resend as a
   `QA-RESEND` line. A job with more than 3 proven resends fails as `Simulator input unstable`
   (the `Simulator input resend budget` step). The app may be launched once more only when it
   is proven not running, or when no touch reached GPUI after a launch. A green job that
   includes such resends counts as green for main and before TestFlight.
5. Never rerun a failed job. Fix flaky tests and verify the fix in CI on the
   new commit. A job that never received a runner may be rerun once, as
   described below. Cite the run and job IDs in the PR.

### Cancelled jobs without a runner, H-live-read and E-launch

(2026-11-17: diagnosis of the main failures of runs 43891836688, 43897926491
and 43992978378. Cited ids are run/job.)

- **Runner cancellation.** A job that ended `cancelled` with `runner_name`
  empty, 0 steps and the annotation `The job was not acquired by Runner` never
  ran on a runner. It has no log and is not a failure class. Classify it as
  runner cancellation. You may rerun only that job once with
  `gh run rerun --job <job-id>`. Seen:
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
- Repeat a press only when the app proves it was lost, once, and say why (issue #142). After the
  wait, `QAAx.press` reads `qa.state` once `seq` held still for 0.5 s (`QAAx.settled`) and decides
  by the app's input receipts, `qa.state`'s `input` (`touch_began`, `touch_ended`,
  `touch_cancelled`, `last_touch`, menu counters; crates/yebisu/src/qa_ax_input.rs): no touch
  began and no screen field (`screen`, `sheet`, `menu`, `active`) moved, or every new touch was
  cancelled by UIKit before it ended, is a proven loss and is sent once more (a `press-retry`);
  a touch that reached the app and ended without `presses[id]` growing is waited for once more
  and then fails with `last_touch`, never sent again (class F2); an unreadable state or one
  without `input` fails without a resend. A press that something else took, or one that reached
  the control without its effect, fails by name with the last `qa.state`. The first tap shortly
  after a drag can reach the app without being taken (#161): prove taps are taken again with
  `QAAx.touchReady` on a control the tap does not change before the next press.
- Count every resend for the job. Each proven resend prints `QA-RESEND {"kind": ..., "id": ...,
  "reason": ...}` (`QAAx.resend`; kinds `press-retry`, `home-retry`, `relaunch-retry`,
  `launch-retry`, `destination-retry` and P12's `reveal-retry`, `pick-retry`, `open-retry`);
  every hosted check appends one line to `target/sim-resends.jsonl`, also with none, and the
  job's `Simulator input resend budget` step (`resend_budget.py judge`) fails above 3.
- Home: `QAAx.pressHome` presses Home once more only when the app stayed in front and
  `lifecycle_inactive` did not grow (`home-retry`); `QA_HOME_DROP=<n>` is its negative control.
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
  `qa.p12.empty`) until GPUI counts the press, at most 8 times, and logs how many it took. When
  all 8 miss, the failure says whether any touch reached the app and where (class T); EditMenuQA
  then launches the case once more (`relaunch-retry`), and a launch that failed with the app not
  running and no `qa.state` is launched once more too (`launch-retry`, class L), as is a launch
  XCTest holds no pid for while the app's `qa.state` reads (reason `no-pid-app-running`, issue #247).
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

### Sidebar search (`just ios-search`)

`scripts/ios/search-check.py` (#138) builds the Simulator app with `test-support` (herdr-uikit's
`native_search_state` and the `search-state` records of `shell/search_qa.rs`; this build swaps
`native_edit_qa` for `native_observer_qa`, `main.rs`), runs `SmokeQA/testSidebarSearch` and
`SmokeQA/testSidebarSearchCompact`, and judges every step afterwards from `Documents/p0-smoke.jsonl`
and `Documents/p4-peer.jsonl`, copied once the app was terminated.

- The magnifier and the controls behind the panel are pressed by QA identifier
  (`qa.sidebar.search`, `qa.sidebar.filter-clear`, `qa.band.mark`). While the panel is up,
  XCUITest element queries do not answer (herdr-uikit `NativeSearch.swift:5-7`), so the test acts
  there on a fixed schedule at points of the panel's layout (`window/native_modal/panel.rs`, its
  identifiers `native-search-field`, `native-search-cancel`, `native-search-clear`,
  `native-search-results`) and reads nothing; this is the exception to "confirm by `qa.state`"
  the owner approved for #138 (Q1). The verdicts come from the records after the run: the test
  prints `SEARCH_STEP <name> <unix time>`, which splits the records by step, and types a marker
  letter into the terminal after each close, which splits the peer log.
- Steps: T-S1 popover (style 7), field first responder, GPUI views not; T-S2 typing reaches the
  field and the filter, nothing the peer; T-S3 Return closes it, the filter and the "Filtered by"
  row stay, the marker arrives once; T-S4 Cancel and an outside tap keep the filter; T-S5
  Command-V into the field, Command-B and Command-F change nothing, and the empty results' Clear
  filter empties the field and the filter (`ClearRequested`, `set_query`); T-S6 a result row opens and
  the panel closes for it; T-S7 a terminal composition at open is not sent; T-S8 background and
  foreground keep the field first responder, and the software keyboard's hide key leaves the panel
  up (iPad ends editing there, so the field is no longer first responder; D5 checks that a tap on
  the field brings the keyboard back); T-S9 `YEBISU_QA_SEARCH_COMPACT=1`
  makes it a page sheet (style 1).
- `YEBISU_QA_SEARCH_COMPACT` (`1` sheet, `0` popover) is read only in a test-support Simulator
  build: the Simulator has no window below 352 pt, so a real narrow window is an owner check.
- Not shown here: real IME, a hardware keyboard, Universal Clipboard, dictation, Escape (it never
  reaches the field from XCUITest) and an abandoned sheet swipe (herdr-uikit
  `docs/native-search.md` "Not shown here"); the owner's iPad table in `device-check.md`
  ("Sidebar search (#138)") covers them.
- CI matches these files in the `edit-menu` path filter but does not run the check (owner
  decision Q2); `just ios-search` runs it locally.

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
- The pick and the dismiss are taps (issue #142 stage 3, Owner decision 4: XCTest's pointer
  hover made SpringBoard close the menu, class H). A pick is judged by the app's receipts:

  | SpringBoard | App receipts | Verdict | Allowed |
  |---|---|---|---|
  | menu closed | `p12-menu-entry` | success | — |
  | menu open 5 s later, same item enabled | no `menu-perform`, `touch_began` and the screen fields unchanged since the swipe | lost before the app (A1) | tap the item once more, `pick-retry` |
  | menu closed | no `menu-perform` | A2, named by its stage | — |
  | either | `menu-perform` `no_tag`, a failed `menu-deliver`, or `menu-global` | SDK or app defect | — |
  | no menu item (`exists=false`) | `touch_began` and the screen fields unchanged | the open tap was lost (M) | tap `yebisu` once more, `open-retry` |

  The baseline is read before the swipe, so no app query runs while the menu is open unless an
  input showed nothing. `menu-global` means the command reached GPUI but no window handler on the
  dispatch path took it: no active window, or the root `on_action` was not on the focused path
  (gpui-pre 0.3.12 runs global bubble listeners after a window dispatch nothing stopped).
- Repeat once, and say why. A press that reached nothing (5 s) and a swipe without a
  revealed menu bar (10 s) are repeated once. Each repeat is a `fixture-retry` or
  `reveal-retry` record in `p12-ui-events.json` with its reason; the oracle
  accepts at most one per control or case and prints all of them under `retries`
  in the verdict (`fixture`, `reveal`, `pick`, `open`), so a rate is never hidden; a
  `pick-retry` or `open-retry` is accepted only when the receipts show no touch and no
  `menu-perform` before it. A second miss is a failure
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
  step or the first swipe of a case; `drop-open:<case>` skips the first tap that
  opens the OS menu and `drop-pick:<case>` the first pick tap (issue #142 stage
  3). `p12-check.py` then requires one logged, injected retry for each fault
  (`fixture-retry`, `reveal-retry`, `open-retry`, `pick-retry`), so a run with a
  fault proves that the retry path ran and that the oracle accepts it. Two
  controls must fail by name without a resend and run alone:
  `drop-pick-twice:<case>` (skips the pick and its resend: `pick lost twice in
  case <case>`) and `offset-pick:<case>` (taps the app's empty strip instead of
  the item; SpringBoard's open menu takes the tap and closes, the app receives no
  touch, so the case fails as `tap lost (class A2) in case <case>`); `p12-check.py` reports
  `"negative_control": "failed as expected"` and exits 0 only then.

## Input mode indicator check (#139)

`just ios-input-mode` (`scripts/ios/input-mode-check.py`) is a local check on
`HERDR_IOS_SIMULATOR`, not a CI job: setting the Simulator's input sources
from a script is undocumented, so it stays where a failure can be looked at.
It writes the device's `AppleKeyboards` (`en_US`, `ja_JP-Romaji`) and
Shortcut style = Herdr prefix, runs `SmokeQA/testInputModeIndicator`
(`SmokeQA+InputMode.swift`), and prints one `INPUT_MODE <case> PASS|FAIL`
line per T-IM-S1..S8, with `summary.json` and the screenshots in
`target/ios-input-mode` (or `--out DIR`). The test only records: each step
prints `INPUT_MODE_OBS` with `qa.state.input_mode`; the script judges
(`scripts/ios/test_input_mode_check.py` covers the judging).

The test runs under an overall limit, `--timeout` (1800 s by default): an app
that never reports idle to XCUITest makes each action wait minutes (#212; the
G14 mutation once ran 5.5 h). Past the limit, or once the test printed the step
`--stop-after` names (`--stop-after s2-h2-after` for a leg judged by T-IM-S2
alone), `xcodebuild` gets SIGTERM, then SIGKILL 60 s later; the cases are
judged from what it printed, `summary.json` records `stopped`, and the run
exits 1 (`scripts/ios/test_input_mode_runner.py`).

`qa.state.input_mode` holds herdr-uikit's selection display state
(`herdr_uikit::input_mode_indicator_state`: `text_first_responder`,
`activated`, `interactions`, `text_input_is_current`, `cursor_class`,
`accessory_frame`, `caret_rect`, `needs_update_calls`), the terminal's
`cursor` and `composition_bounds` in window points, the `marked` text, and
`invalidations` (cursor moves reported to GPUI). UIKit shows and hides the
indicator without a GPUI frame, so the terminal poll asks for a frame when
the SDK's state changed. The same poll fills `shown` (how often the indicator
appeared, its last frame and the caret then): XCUITest can wait 60 s for the
app to idle after a key, past the indicator, so a switch is judged by `shown`
growing since the `before-<step>` read. Marked text reaches `qa.state` a frame
or more after the key, so the test waits on it rather than sleeping. The keys
are XCUITest hardware keys: Shift puts the Simulator in the hardware keyboard
state and Control-Space switches the input source. The result does not prove real IME or hardware keyboard
behaviour; the owner's iPad checks D1-D7 (PR) do.
