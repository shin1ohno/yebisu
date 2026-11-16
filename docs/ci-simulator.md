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

| Class | Log signature (grep) | Rate | Cause | Fix / open item | Evidence |
|---|---|---|---|---|---|
| L15 | `Missing source-defined control: Paste` (`OCR attempt 0..2 for Paste`, then `missing-control`) | 11/41 P9 runs whose head has the Settings button; 0/49 without | The OCR returns no standalone `Paste` after the second `QA image ready`; what Vision returns is `[unproven]` | 4 of 11 (second `Paste`): PR 57, merged 1de7020. 7 of 11 (first `Paste image` misread): open, L19, after PR 57 | run 43746802879/1 job 132911829322 (artifact 14175507358); run 43754156237/1 job 132934538869 (14177458803) |
| L16 | `image receipts incomplete: puts=3 paths=3 text=0` | 3 of the 41 jobs that reached SH1-154 in the L16 scan; a 4th job was added later | The second text `Paste` tap landed on `Paste image` (box x=0.215, the right one is 0.151), so no `P8-shadow` text arrived | PR 57, merged 1de7020; 3 of 3 `simulator-smoke` passes at its head 756f70b (run 43766302928) | run 43736757874 jobs 132881022688, 132900139639; run 43760942018 job 132955457568; added job 132969174547 |
| C-snapshot | `SmokeQA.swift:23: error: ... Failed to get matching snapshot: Timed out` | 3/98 P0 runs | `app.activate()` right after Home does not take effect, the app stays suspended and the next `typeText` times out. Happens on a freshly booted, slow Simulator; the press/activate ordering is `[unproven]` | Open: PR 58 (draft) replaces the fixed sleeps with named state waits | run 43730444891/7 job 132946288166 (artifact 14179437935); run 43720814607/1 job 132832187258; run 43729845313/2 job 132888134849 |
| C-anim | `App animations complete notification not received` (60 s waits, 25 times, job cancelled at 40 min) | 1/92 P9 runs | `animationsHaveFinished` stays NO after `P9-native\n`; what keeps animating is `[unproven]` | Open: PR 58 (draft) caps `testP9Core` at 150 s and logs `animationKeys()` after 10 s; it does not fix the cause | run 43730444891/4 job 132914175740 (artifact 14176482747) |
| A | `P12 xctest: exit 0` then `ValueError: setup callbacks missing/duplicated` | 2/52 P12 runs | The first tap after an OS menu opens is lost; the menu stays open. Cause `[unproven]` | Open: L20 (draft PR 59, observation only) | run 43730444891/3 job 132903694966 (P12 artifact 14174842401); run 43717998985/1 job 132823606035 (14164345944) |
| B | `Installed OS application menu must be revealed and reachable` (`P12QA.swift:68`, `:81` since PR 55) | 4/52 P12 runs | The top-edge swipe does not reveal the menu bar or `•••`. Cause `[unproven]` | Open: L20 (draft PR 59, observation only) | run 43699190351/1 job 132766634071; run 43729845313/1 job 132859692803; run 43754156237/4 job 132956853113; run 43730444891/9 job 132960855571 |

Also fixed: a P12 held `pane.split` request that reached the 60 s client
timeout (`held request reached the client's 60 s COMMAND_TIMEOUT`; earlier
symptoms `P12QA.swift:78` `Native command missing` and `:79` `Captured native
command must start enabled`), PR 55, merged cb18764. One C-paint failure
(`input result expected 'P0-input', received 'P0-inpu'`, run 43684762210/1 job
132722801994) came from a fixed `sleep(2)` after `typeText`; PR 58 waits for the typed text.

### Classify a failing smoke job

(2026-11-16: merge-gate decision and rerun limit.)

1. Download the evidence of the failing run, `gh run download <run-id> -n
   simulator-smoke-evidence` for P0/P9, or the P12 artifact
   `p12-native-menu-evidence` (`xctest.stdout`, `peer.jsonl`, `app.jsonl`).
   The job log of a P12 failure shows only `P12 xctest: exit 65`. A job that hit
   the 40-minute timeout may have no artifact.
2. Grep the job log and the artifact for the signatures above. A failure with
   no matching signature is a new class: do not rerun it, report it.
3. Compare `gh pr diff <number> --name-only` with the failing step. P0/P9
   classes (L15, L16, C) come from `scripts/ios/smoke-qa/Tests/SmokeQA.swift`
   and `scripts/ios/p9-check.py`; A and B from `scripts/ios/p12-qa/` and
   `scripts/ios/p12-oracle.py`. A PR that changes those files owns the failure.
4. Merge gate (owner decision 2026-11-16): a PR may merge when `simulator-smoke`
   is the only failing job, the failure is classified with job log lines to a
   known class, the PR's changed files do not touch that area, every other job is
   green, one review round has 0 blocking findings, 0 threads are unresolved, and
   the merge is a merge commit.
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
  `COMMAND_TIMEOUT` is 60 s (herdr-gpui 6ab6907, `crates/sheprd-client/src/limits.rs:19`);
  a request held that long drops the connection. Hold one request per local
  case and release it before the next search.
