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

## Known simulator-smoke failure classes

Rates come from the L17 diagnosis of 104 `simulator-smoke` jobs (CI runs
2026-11-14T12:08Z to 2026-11-15T21:16Z, all on `macos-26-arm64`; P0 98 runs, P9
92, P12 52) unless a row says otherwise. They were copied, not re-measured.
`[unproven]` marks a cause nobody has confirmed. Class names are the diagnosis
labels. Origin: 2026-11-16, L17 diagnosis, with P12 (PR 55), L16 (PR 57) and
SH1-265 (PR 56) reports.

Known classes for the merge rule below: L15, L16, C-snapshot, C-anim (until
PR 58 merges), A and B. C-paint is classified but is not one of them, so a
C-paint failure blocks the merge. L19 and L20 are open work on L15 and A/B, not
classes.

| Class | Log signature (grep) | Rate | Cause | Fix / open item | Evidence |
|---|---|---|---|---|---|
| L15 | `Missing source-defined control: Paste` (`OCR attempt [0-2] for Paste`, then `missing-control`) | 11/41 P9 runs whose head has the Settings button; 0/49 without | The OCR after a `QA image ready` finds no control: no standalone `Paste` after the second one (4 of 11), no `Paste image` after the first one (7 of 11). What Vision returns is `[unproven]` | 4 of 11 (second `Paste`): PR 57, merged 1de7020. 7 of 11 (first `Paste image` misread): open, L19, after PR 57 | run 43746802879/1 job 132911829322 (artifact 14175507358); run 43754156237/1 job 132934538869 (14177458803) |
| L16 | `image receipts incomplete: puts=3 paths=3 text=0`, with the logged tap box at x=0.215 | 3 of the 41 jobs that reached SH1-154 in the L16 scan; a 4th job was added later | The second text `Paste` tap landed on `Paste image` (box x=0.215, the right one is 0.151), so no `P8-shadow` text arrived. A `text=0` whose tap box is x=0.151 has another cause (job 132483179735, head 1954cab, no Settings button): not L16 | PR 57, merged 1de7020; 3 of 3 `simulator-smoke` passes at its head 756f70b (run 43766302928) | run 43736757874 jobs 132881022688, 132900139639; run 43760942018 job 132955457568; added job 132969174547 |
| C-snapshot | `SmokeQA.swift:23: error: ... Failed to get matching snapshot: Timed out` | 3/98 P0 runs | `app.activate()` right after Home does not take effect and no characters reach the app (the app stderr shows 8 `input committed` lines, all for `P0-input`; none for `-returned`), so the `typeText("-rotated")` at `SmokeQA.swift:23` times out. Happens on a freshly booted, slow Simulator. That the app stays suspended, and the press/activate ordering, are `[unproven]` | Open: PR 58 (draft) replaces the fixed sleeps with named state waits | run 43730444891/7 job 132946288166 (artifact 14179437935); run 43720814607/1 job 132832187258; run 43729845313/2 job 132888134849 |
| C-anim | `App animations complete notification not received` (60 s waits, 25 times, job cancelled at 40 min) | 1/92 P9 runs | `animationsHaveFinished` stays NO after `P9-native\n`; what keeps animating is `[unproven]` | Open: PR 58 (draft) caps `testP9Core` at 150 s and logs `animationKeys()` after 10 s; it does not fix the cause | run 43730444891/4 job 132914175740 (artifact 14176482747) |
| C-paint | `input result expected 'P0-input', received 'P0-inpu'` | 1 job seen | A fixed `sleep(2)` after `typeText` ended before the last character was drawn | Open: PR 58 (draft) waits for the typed text | run 43684762210/1 job 132722801994 |
| A | `P12 xctest: exit 0` then `ValueError: setup callbacks missing/duplicated` | 2/52 P12 runs | The first tap after an OS menu opens is lost; the menu stays open. Cause `[unproven]` | Open: L20 (draft PR 59, observation only) | run 43730444891/3 job 132903694966 (P12 artifact 14174842401); run 43717998985/1 job 132823606035 (14164345944) |
| B | `Installed OS application menu must be revealed and reachable` (`P12QA.swift:68`, `:81` since PR 55) | 4/52 P12 runs | The top-edge swipe does not reveal the menu bar or `•••`. Cause `[unproven]` | Open: L20 (draft PR 59, observation only) | run 43699190351/1 job 132766634071; run 43729845313/1 job 132859692803; run 43754156237/4 job 132956853113; run 43730444891/9 job 132960855571 |

Also fixed: a P12 held `pane.split` request that reached the 60 s client
timeout (`held request reached the client's 60 s COMMAND_TIMEOUT`; earlier
symptoms `P12QA.swift:78` `Native command missing` and `:79` `Captured native
command must start enabled`), PR 55, merged cb18764.

PR 58 and PR 59 change the failure messages of C-snapshot, A and B (PR 59
adds `OS menu bar not revealed (class B)` and `OS menu still open ... (class A)`).
Each of those PRs updates its rows in this table; until it does, a failure with
the new message is not "a new class" by itself, so compare it with the row's cause.

### Classify a failing smoke job

(2026-11-16: owner decision on known classes, `known-class-merge-rule-decision.json`.
The rerun limit in step 5 comes from the approved plan, not from that decision.)

1. Download the evidence of the failing run, `gh run download <run-id> -n
   simulator-smoke-evidence` for P0/P9, or the P12 artifact
   `p12-native-menu-evidence` (`xctest.stdout`, `peer.jsonl`, `app.jsonl`).
   The job log of a P12 failure shows only `P12 xctest: exit 65`. A job that hit
   the 40-minute timeout may have no artifact.
2. Classify from lines you can cite. P12 classes (A, B) are classified from the
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
   - **C-snapshot** (P0 `testInputAndLifecycleReceipts`: Home :14, `activate()`
     :16, `typeText("-returned")` :18 in `SmokeQA.swift`):
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
     - Harness: `scripts/ios/smoke-qa/Tests/SmokeQA.swift` (:5-44) and
       `scripts/ios/smoke-qa/project.yml`.
   - **C-anim** (P9 `testP9Core`: `P9-native\n` typed at `SmokeQA.swift:389`). What
     keeps animating is `[unproven]`, so the area is the path that runs from that
     input to the idle wait:
     - Under `crates/yebisu/src/`: `input.rs` (:159-194), `terminal_input.rs`,
       `terminal_view.rs`, `upstream/terminal_painter.rs` and
       `upstream/terminal_painter/`, the P9 mock backend `core_mock.rs` (:286,
       `connect`) with `core_view.rs` (:13, :58) and `app.rs` (:394-400), which
       select it under `YEBISU_CORE_QA`, and `observation.rs`.
     - The `QA mark`, `QA delete` and `QA commit` taps that follow the input
       (`SmokeQA.swift:390-392`) and the key and edit-menu code that runs between
       the input and the idle wait: `crates/yebisu/src/terminal_controls.rs`
       (`publish_native_edits` :8, called every render; `terminal_key_down` :119;
       `terminal_key_up` :249; `input_qa_controls` :359, the `QA` row) and
       `crates/yebisu/src/edit_menu.rs` (:87-100 set availability and present or
       dismiss the UIKit edit menu; `edit_menu_check` :165). `handoff_view.rs`
       (:321-323, the `Start: Drag` and `Start: Select / double tap` labels the
       test taps at `SmokeQA.swift:386-387` before it types) is in this area too.
     - Harness: `scripts/ios/smoke-qa/Tests/SmokeQA.swift` (`testP9Core`
       :377-410) and `scripts/ios/smoke-qa/project.yml`.
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
   Known classes: L15 and L16 (SH1-154 Paste OCR mis-tap), C-snapshot and C-anim
   (L18; they leave the list once PR 58 merges, after which such a failure is a
   new defect), A and B (P12). The rule decides only whether the failure blocks.
   It does not grant merge permission, which stays per PR or per approved plan.
   Merge with a merge commit.
5. Rerun only the failed job (`gh run rerun --job <job-id>`), at most 2 times per
   PR. Cite the run and job id of each attempt in the PR.

### XCUITest OCR rules

(2026-11-16: PR 55, PR 57.)

- Locate every control once, before any held request, then tap the recorded
  coordinates (PR 55 `locateFixtures` finds all 13 P12 controls before the first
  held `pane.split`; PR 57 reuses the first `Paste` box for the second tap).
- Match whole words so `Paste` never hits `Paste image`: split recognised text
  on whitespace and exclude `Paste` followed by `image`/`Image`. `locateText`
  still uses the suffix test; the whole-word match is open (L19).
- Never run an OCR search while a request is held. The pinned sheprd-client
  `COMMAND_TIMEOUT` is 60 s (herdr-gpui f63688b, `crates/sheprd-client/src/limits.rs:19`);
  a request held that long drops the connection. Hold one request per local
  case and release it before the next search.
