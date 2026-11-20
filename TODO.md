# Deferred device acceptance

- SH1-152 physical native edit-menu acceptance awaits the owner. Reason: Simulator cannot prove finger/Pencil/trackpad feel, physical keyboards or Stage Manager behavior; worker device signing is outside this task. First step: follow the SH1-152 checklist in docs/device-check.md on M5 with disposable Herdr 1/2 destinations and retain receiving logs. Close condition: all 14 items pass on the device (the 12 plan-4 step-7 items plus review items 13 and 14; item 8 includes filing a separate issue if needed); record results in PR QA and delete this entry in the resolving commit. Simulator E3e/E3h/E7 remain unverified whenever their required input cannot be produced, until device items 10/12/9 pass.

- SH1-170 Releases IPA signing decision awaits the owner. Reason: distribution destination is decided, but unsigned re-signing versus Development/Ad Hoc signing is not. First step: choose the method, audience and visibility in docs/ipa-signing-options.md. Close condition: chosen packaging job and Releases asset have installation/update evidence; delete in the resolving commit.
- P15 Simulator oracle: the FIFO positive control is not yet proven on a leased Mac mini run. Reason: in attempt 6 the peer received 55 ClientShellResize and no TextCommit, so `P15-start` never arrived; the cause is open (H1 focus: the OCR tap landed outside the terminal, H2 no destination: input needs a coherent snapshot/surface pair and the board peer keeps changing revisions). The oracle now fails with distinct messages and the app records `input-dropped` with a reason. Also unverified there: the runner reading the app container's `p4-peer.jsonl` while XCTest runs, and the `appearance` OCR line count being exactly 1. First step: run `scripts/ios/p15-check.py --own-root` once under the mini lease at the exact CI-green head, read `input-dropped` in `evidence.json`; if H2, hold the board peer's updates (the `connect_gated` gate in `mock_peer.rs`) before the controls are typed. Delete this entry when P15-start and P15-end reach the peer in that run.

- P15 read-only native/receiver acceptance remains pending after candidate P11 source integration. Reason: retained read authority is unavailable, and native execution has not occurred. The pinned herdr-gpui rev f63688b now carries the herdr-ssh teardown receipt (`Tunnel::wait_teardown`), but yebisu does not call it yet: `crates/yebisu/src/core.rs` only holds the tunnel in `_tunnel: Arc<herdr_ssh::Tunnel>`, so no teardown evidence reaches the P15 gates. First step: call `Tunnel::stop` then `wait_teardown` where `core.rs` ends the tunnel and record the `Teardown` outcome, then run exact-full-head check/lint-ios/test plus the mounted Appearance/theme tests under the controller lease, then compare against P11 final merge and repeat affected checks. Close condition: docs/p15-readonly-model/TODO.md gates and P1-P5 have actual receiving/native evidence; delete this item in the resolving commit. Local reader fixtures and Unsupported UI are not two-host SSH proof.

- P0 physical M5 verification awaits the owner. Reason: attempt 2 explicitly forbids device signing by the worker while the owner sleeps. First step: unlock the login keychain interactively, connect/unlock M5, then run `just ipad` from the repository (the report gives a standalone command using the provided just binary). Verify both apps launch, compare signed application identifiers and isolated Keychain groups, then check text/image/input, composition, background return, rotation and 60-second idle on the device. Close condition: Y6 has physical-device receiving-system evidence; delete this item in the resolving commit. All Simulator checks remain separate from physical IME/hardware-keyboard proof.
- P1 startup regression check awaits the owner. Reason: signing and installation require the interactively unlocked login keychain, which is unavailable over SSH. First step: unlock the keychain, connect/unlock M5, and run `just ipad`; verify the P0 smoke screen still appears and returns after backgrounding. Close condition: the P1 PR records the device model, iOS version, launch and return result; delete this item in the resolving commit.

- P2 physical key/authentication acceptance awaits the owner. Reason: the login keychain is locked over SSH; Secure Enclave and Local Network acceptance need the physical iPad. First step: follow the P2 owner steps in docs/device-check.md after locally unlocking the keychain, then record separate-key generation, host comparison, successful fresh authentication, restart persistence, and old-app connectivity. Close condition: those physical results and disjoint signed Keychain entitlements are recorded in the P2 PR; delete this item in the resolving commit.

- P3 physical display acceptance awaits the owner. Reason: device signing requires the locked login keychain and visual judgement on M5. First step: unlock locally and follow the P3 steps in docs/device-check.md, including registering the new key before connecting. Close condition: CJK, box drawing, colours, Nerd Font, both densities, rotation and disconnect results are recorded in the P3 PR; delete in the resolving commit.
- Simulator rotation remains unverified. Reason: XCTest orientation changes did not change the app viewport; Simulator.app could not be opened by name or at the current Xcode standard path. First step: run the strict P3 check (`just guarded python3 scripts/ios/p3-check.py --require-rotation`) in a Simulator that demonstrably rotates. Close condition: receiving-system paint receipts contain a landscape viewport and the peer's corresponding resize; delete in the resolving commit. Physical rotation remains a separate owner check.

- P4 / M1 physical input acceptance awaits the owner. Reason: signing needs the locally unlocked login keychain, and Simulator callbacks cannot establish physical IME/hardware-key behavior. First step: unlock locally, run the provided `just ipad` command, and follow the complete P4 sequence in docs/device-check.md with a disposable Herdr session. Close condition: the P4 PR records Esc/arrows/Ctrl/repeat/release/one delivery, middle-clause and emoji composition/commit, rotation and keyboard resize, pane/popup/setup isolation, keyboard unplug/replug, disconnect and manual reconnect, plus preferred runtime variants; delete this entry in the resolving commit.

- R1 physical review-repair acceptance awaits the owner. Reason: the login keychain is locked and the worker cannot sign/install on M5. First step: locally unlock the keychain, run the absolute `just ipad` command, and follow the R1 section of docs/device-check.md to compare saved-target/P0 isolation, normal graphemes and reconnect/resize. Close condition: device/iOS version and the R1 results are recorded in PR QA; delete this entry in the resolving commit. Deterministic malformed-cell/cache/stale-callback negatives are already covered by host painter/adapter tests, and Simulator checks do not replace physical input acceptance.

- R2 physical P4 repair acceptance awaits the owner. Reason: the login keychain is locked over SSH, and Simulator callbacks cannot prove physical IME or hardware routing. First step: locally unlock the keychain, run the absolute `just ipad` command and follow the R2 section of docs/device-check.md. Close condition: PR QA records partial emoji-clause deletion/replacement, composing Esc cancellation then ordinary Esc, target-switch input isolation, and the P4 device sequence with device/iOS version; delete in the resolving commit.

- P5 physical acceptance awaits the owner. Reason: device signing requires the locked login keychain and real lock/network/IME checks. First step: unlock locally, run the absolute just ipad command, then perform the P5 section of docs/device-check.md against sandbox hosts. Close condition: PR QA records lock return, both reconnect policies, network switch, changed host key refusal, save-failure display, and no replay with device/iPadOS version; delete in the resolving commit.

- P6 physical board acceptance awaits the owner. Reason: locked login keychain and device-only layout/latency judgement. First step: unlock locally, run the absolute `just ipad` command and follow the P6 section of docs/device-check.md against two sandbox hosts. Close condition: PR QA records update delay, harness marks, narrow-width usability, host-switch isolation and preferred density with device/iPadOS version; delete this item in the resolving commit.

- R3 physical acceptance awaits the owner. Reason: the locked login keychain prevents worker device signing and physical IME input needs M5. First step: unlock locally, run the absolute just ipad command, and follow the R3 section in docs/device-check.md. Close condition: PR QA records mismatch reason/no redial for selected and hidden hosts, Trust/authentication list refresh, no Esc on composition cancellation and physical P4/P5 regression results with device/iPadOS version; delete in the resolving commit.


- R4 physical P6 repair acceptance awaits the owner. Reason: the locked login keychain prevents worker device signing, and physical input needs the iPad. First step: unlock locally, run the absolute just ipad command, then follow the R4 steps in docs/device-check.md against disposable sandbox daemons. Close condition: PR QA records success-then-other-focus input recovery, stale boot/generation tap rejection and changed focus/surface/input destination with device/iPadOS version; delete in the resolving commit.

- P7 physical acceptance awaits the owner. Reason: signing requires the locally unlocked keychain, and hardware keyboard/prefix/IME behavior needs the physical iPad. First step: run the absolute just ipad command and perform the P7 sequence in docs/device-check.md against a sandbox daemon. Close condition: PR QA records device/iPadOS, hardware shortcut delivery, prefix non-collision, tab/pane input isolation and preferred runtime precedence variant; delete in the resolving commit.

- R5 physical acceptance awaits the owner. Reason: device signing requires the locally unlocked login keychain and physical input needs M5. First step: unlock locally, run the absolute just ipad command, then follow the R5 section of docs/device-check.md with controlled disposable peers. Close condition: PR QA records response-before-projection input isolation, success-then-other-focus recovery, no-op/error release, late mismatch with Automatic for selected/hidden hosts, compatible reconnect, and keyboard/IME results with device/iPadOS version; delete in the resolving commit.

- R6 physical P7 repair acceptance awaits the owner. Reason: the login keychain is locked over SSH and physical hardware/IME delivery needs M5. First step: unlock locally, run the absolute just ipad command, and follow the R6 section of docs/device-check.md on an already authorized disposable sandbox. Close condition: PR QA records Command-Shift-bracket next/previous focus, shifted-letter/named-key regressions, held Escape no-wire delivery through release in both report-all modes, and fresh Escape after release is lost on Hosts with device/iPadOS/layout; delete in the resolving commit.

- P8 physical selection/paste acceptance awaits the owner. Reason: device signing needs the locally unlocked login keychain and touch/trackpad/menu feel needs M5. First step: unlock locally, run the absolute just ipad command, then follow the P8 section of docs/device-check.md against an authorized sandbox. Close condition: device/iPadOS, trackpad/touch, native Paste menu, large image, target-change cancellation and selected variants are recorded in PR QA; delete in the resolving commit.

- P9 physical Herdr 2 acceptance awaits the owner. Reason: signing requires the locally unlocked keychain, and physical IME/keyboard behavior needs M5. First step: unlock locally, run the absolute just ipad command, and follow the P9 section in docs/device-check.md against an authorized disposable host running herdr-core serve. Close condition: PR QA records physical input/IME, both split orientations, detach/reattach, host-switch isolation and background return with device/iPadOS; delete in the resolving commit.

- P10 physical persistence acceptance awaits the owner. Reason: device signing requires the locked login keychain and physical lock/suspension cannot be established by Simulator. First step: locally unlock the keychain, run the absolute just ipad command, then follow docs/device-check.md P10 to change the three existing options and immediately background/lock/relaunch. Close condition: PR QA records device/iPadOS, restored values and input/reconnect regression results; delete in the resolving commit.

- SH1-150 physical selection acceptance awaits the owner. Reason: signing needs the locally unlocked login keychain and touch/trackpad feel requires M5. First step: follow docs/device-check.md SH1-150, starting with the absolute just ipad command. Close condition: PR QA records device/iPadOS, whole-glyph highlight and exact copied text on Herdr 1/2 with touch/trackpad; delete in the resolving commit.
- Selection soft-wrap leading spacers remain outside SH1-150. Reason: the brief excludes wrap-boundary behavior. First step: capture a wrapped CJK row at a narrow terminal width and add a host-shaped fixture before defining copy semantics. Close condition: wrap-leading spacer behavior has a specification and passing selection/adapter tests; delete in the resolving commit.
- Full-row selection trailing-space trimming remains outside SH1-150. Reason: the brief preserves current real-space semantics. First step: define full-row versus partial-row whitespace expectations and add explicit fixtures. Close condition: agreed trimming semantics pass tests without deleting real partial-row spaces; delete in the resolving commit.
- Painter wide-separator continuation handling remains outside SH1-150. Reason: the brief excludes the painter's next.skip check. First step: inspect crates/yebisu/src/upstream/terminal_painter.rs for next.skip and reproduce with a Herdr 1 full/patch separator fixture. Close condition: wide-separator paint behavior has receiving-image evidence and regression tests; delete in the resolving commit.

- SH1-151 physical persistence acceptance awaits the owner. Reason: the locked login keychain prevents worker signing and touch/trackpad feel requires M5. First step: locally unlock the keychain and follow docs/device-check.md SH1-151 using the absolute just ipad command. Close condition: PR QA records device/iPadOS, selection defaults, both independent toggles, background/lock/relaunch persistence and unchanged other settings; delete in the resolving commit.

- SH1-153 physical Edit menu acceptance awaits the owner. Reason: Simulator responder checks do not prove physical keyboard/menu behavior and signing needs the locally unlocked keychain. First step: follow docs/device-check.md SH1-153 with software and hardware keyboards against a disposable Herdr 1 session. Close condition: PR QA records device/iPadOS, enabled Copy/Paste screenshots, clipboard/paste and stale-target negatives; delete in the resolving commit.
- Linux Simulator-runner environment enumeration drops hyphenated Cargo binary names from `compgen -e`. Reason: found by existing scripts/ios/test_sim_runner.py during SH1-153; changing the shared runner is outside that task. First step: reproduce `env 'CARGO_BIN_EXE_test-name=value' bash -c 'printenv CARGO_BIN_EXE_test-name; compgen -e'` on Linux and macOS, then enumerate the OS environment without shell identifiers. Close condition: existing runner regression passes on both hosts with literal multiline values; delete in the resolving commit.
- SH1-155 physical lifecycle and staged-image acceptance awaits the owner. Reason: locked login keychain prevents worker signing, and real suspension/Stage Manager need M5. First step: unlock locally and follow docs/device-check.md SH1-155, recording host file creation/deletion timestamps. Close condition: device/iPadOS, both protocols and Return policies, inactive-only continuity and background recovery have receiving-system evidence in PR QA; delete in the resolving commit.

- P13 terminal search native acceptance is pending. Reason: D1-D6 and the SDK helper adoption (herdr-uikit PR 14, merged as 99d7928, Cut joined to its first selection query) require an exclusive Mac lease, source review and P11/P12 integration; native Escape retirement still has the SDK789 B1 refusal, while OS retarget/loss (B2) is unproven. The helper R1/Q05 definition gap is closed by independent exact-fe3 source review; all runtime oracles remain UNRUN. First step: controller clears the P12 isolated-artifact provenance gate and grants a new isolated exact-head `just check`, `just lint-ios`, `just test` and original/query `p13-native-probe`/mutation lease following docs/p13-native-handoff.md. Preserve actual peer/renderer ASCII/Japanese, offscreen scroll, delayed wrong-boot socket, held Escape, partial marked edits, native Cut/Command-X/None semantics, direct fresh insert/mark without KeyDown, stale client and current-owner matrix gates. Close condition: original S1–S5 native/review evidence, B1 and D6 real UIKit queries plus intended mutation/restoration, SDK helper Q05/nonmutation/nil/numeric negatives, B2 limits and P11/P12 integration are recorded, with physical IME owner limitations separate; delete in resolving commit.
- P14 connected SemanticNotification native acceptance is pending. Reason: source integrates frozen P11 f54b1ecc and exact SDK10 pin, but no exclusive native lease or combined receiving evidence exists. First step: run combined exact-head guarded check/lint-ios/test and controlled notification receiving-center/modal tests under the controller lease; keep key probes excluded until observable Mock preflight. Close condition: N1-N5 have actual route/negative/native evidence; remove in the resolving commit. No APNs or after-suspension replay is included.
- P14 physical notification acceptance awaits the owner. Reason: worker signing, Keychain and OS permission dialogs are prohibited. First step: owner installs the existing com.shin1ohno.yebisu app, explicitly requests notification permission in its UI, and records foreground/Inactive/Background/suspend, denied permission, volume/mute/quiet-hours and stale-tap behavior against disposable peers. Close condition: physical receiving-system evidence is recorded separately from model/Simulator checks; remove in resolving commit.
- P11 native and physical acceptance remains pending. Reason: production source is mounted but no Mac lease was granted to this worker, and PR22/25 input/SDK reconciliation and independent exact-head review remain. First step: controller's Mac runner checks the frozen head with `just check`, `just lint-ios`, `just test`, then guarded `python3 scripts/ios/p11-check.py`; inspect all26 injected fixture obligations and reconcile PR22/25. Close condition: P1–P5 in task0ecda3f890e64249 have exact-head receiving evidence, and owner-only real Keychain/device gates are separately recorded using docs/device-check.md P11. Delete only in the resolving commit.

- P11 D3 SDK refusal observation remains blocked (PR29 attempt2). Reason: pinned herdr-uikit b2fa781 silently refuses selectors before consumer dispatch; scheduled=true is not dispatch evidence. First step: independently review the task artifact sdk-observer-proposal-2.md, implement any approved SDK observer in its separate owned dependency, then lease an exact-head Mac run of check/lint-ios/test and scripts/ios/p11-check.py (outside real-selector positive control plus modal FIFO transcript). Close condition: per-selector SDK entry/refusal or actual consumer refusal, all26 fixtures, receiving zero-leak evidence, and independent exact-head review pass; remove in the resolving commit. No real Keychain probe before runtime-selected mock preflight; physical gates remain owner-only.

- P14 D4 final-presentation and audio callback acceptance is pending. Reason: retained per-tag willPresent predicate and sound context checks are source-only; background OS ordering is unobserved. First step: under controller's explicit exclusive native lease run exact-head SDK callback-order tests and actual delegate/center receiving checks, remove the final predicate as a negative mutant, then restore and rerun. Close condition: no-show/show, completion/removal order, stale response, sound/quiet/mute and foreground/Inactive/Background evidence plus independent review satisfy N1-N5; delete in the resolving commit.

- P14 Simulator QA harness is authored but not executed. Reason: no native lease; Python fixture checks cannot prove app/SDK runtime behavior. First step: under an exclusive lease run both commands in docs/p14-native-qa.md at the new exact consumer head and unchanged SDK pin; retain raw screenshots, wire/paint/context receipts, intended native mutant failure and restored pass. Close condition: native harness runs, independently reviewed receiving evidence and separately observed actual OS/physical gates satisfy original N1-N5; remove in resolving commit.

- P14 D5 actual SDK response-boundary acceptance is pending. Reason: six-second/301-second/removed/duplicate cases and original-event deadline are authored but Rust/native execution is prohibited without a lease. First step: lease the exact heads and run scripts/ios/p14-sdk-check.py plus actual center/delegate receipt collection as described in docs/p14-native-qa.md; keep SDK D4 and D5 mutant failures separate. Close condition: exact-head compiler, 13 SDK + 6 consumer tests, intended mutant failure/restoration, actual OS response/status/focus/no-wire and all N1-N5/owner gates pass; delete in the resolving commit.
- P12 / SH1-179 Settings integration is blocked on unmerged P11 (#29). Reason: main only has the unmounted P10 settings spike; stacking on P11 is outside this task. First step: after P11 merges, consume `native_menus::catalog_rows` from the Keyboard page and verify labels/availability and modal focus with actual Simulator screenshots. Close condition: mounted Settings and search scope tests pass at the final consumer head; delete in the resolving commit.
- P12 / SH1-179 owner physical gates remain open. Reason: Simulator and model checks cannot establish hardware keyboard or Japanese IME behavior. First step: follow `docs/p12-keyless-menus.md` on the physical iPad with a disposable peer. Close condition: device/iPadOS, repeat/release/priority/prefix/IME and Copy/Paste receiving evidence is recorded; delete in the resolving commit. Command-hold implementation is deferred by the owner decision and is not a gate for this PR.
- P12 / SH1-179 independent exact-head review and actual native stale-generation proof remain open until their artifacts are attached. Reason: this worker may not launch another worker or change the SDK. First step: controller assigns independent review of the draft head; exercise old native tags after menu rebuilding in the installed Simulator. Close condition: no blocking review findings and stale native tags demonstrably produce no RPC/input; delete in the resolving commit.
- P11 native and physical acceptance remains pending. Reason: production source is mounted but no Mac lease was granted to this worker, and PR22/25 input/SDK reconciliation and independent exact-head review remain. First step: controller's Mac runner checks the frozen head with `just check`, `just lint-ios`, `just test`, then guarded `python3 scripts/ios/p11-check.py`; inspect all26 injected fixture obligations and reconcile PR22/25. Close condition: P1–P5 in task0ecda3f890e64249 have exact-head receiving evidence, and owner-only real Keychain/device gates are separately recorded using docs/device-check.md P11. Delete only in the resolving commit.

- P11 D3 historical 0/4 acceptance remains blocked (PR29 attempt2). Reason: the former b2fa781 SDK silently refused selectors before dispatch; scheduled=true was not evidence. Provisional SDK11 adoption now authors observation but has no native proof. First step: independently review the exact consumer/SDK11 heads and docs/p11-sdk11-adoption.md, then lease isolated target gates and the collector (both outside origins plus modal FIFO transcript). Close condition: per-selector SDK entry/refusal or actual consumer refusal, all26 fixtures, receiving zero-leak evidence, and independent exact-head review pass; remove in the resolving commit. No real Keychain probe before runtime-selected mock preflight; physical gates remain owner-only.

- P11 F19 core SDK final-publication and F24 native aggregate acceptance remain open. Reason: consumer queued-command epoch cannot revoke bytes already accepted by the pinned SDK; native stderr for every XCTest relaunch has not been collected. First step: separately review the SDK writer-fence contract in docs/p11-security-fixtures.md, then lease exact-head Mac tests and collect all F24 manifest sinks. Close condition: full F01–F26 exact-head receiving evidence and independent review, including postcheck publication and all-launch sink inventory, pass; delete in resolving commit. Runtime chosen injected-provider preflight must precede every QA key operation.

- P11 provisional SDK11 adoption/native collector awaits exact-head review and lease. Reason: corrected SDK2dcd11a source pin is provisional after SDK944ff7e B1; independent correction review and all target/native checks UNRUN; inactive/no-responder/transient-mode and final transport drain lack supported controls. First step: independently review docs/p11-sdk11-adoption.md and the frozen consumer/SDK heads, then run the isolated collector under a controller lease after target off/on gates; review a separate SDK transport drain/fence contract. Close condition: full F01-F26 and original P1-P5 receive actual native/FIFO/privacy proof and independent approval; historical 0/4 is not waived. Remove only in the resolving commit.

- P11 final correction B1/C2 requires exact-head runtime acceptance; B2/C1 still block (task0ecda3f890e64249 attempt3). Reason: default-family intent and independent Base64-private scanning are corrected in source, but GPUI tests are unrun; final SDK writer cancellation and compiler-input/unsigned-to-signed provenance are separate dependencies. First step: review docs/p11-final-correction.md and settings/family_tests.rs, obtain the controller's frozen P15 helper commit and B2 public-API audit, then run exact-head check/lint-ios/test and the unchanged F01–F26 receiving probes under an explicit lease. Close condition: B1 actual-button durable/reopen tests, all required/additional F24 sinks, both F19 publication intervals and the C1 source-to-installed chain pass independent review and receiving validation; remove this item only in the resolving commit.

- P11 F19 A/B0 test-only reproducers are authored but unrun (same final attempt3). Reason: the controller admitted only source/static validation; the comparison-to-publication race remains in production. First step: independently review docs/p11-publication-reproducers.md and the task-local bounded pre-modal plan, then obtain an explicit exact-head host-test lease and run both ignored entrypoints with source/build binding. Close condition: actual failure receipts are distinguished from setup errors, an independently admitted remedy satisfies original F19 across all required boundaries, and receiving evidence passes; delete only in that resolving commit. Normal test success excludes the ignored reproducers and cannot close this item.

- SH1-156 native recovery without a tap awaits exact-head device evidence.
  Reason: at herdr-uikit 99d7928 (held-press ledger, PR 16) the consumer
  submits native edit retirement from a foreground task and again after a key
  release; host tests cannot run the UIKit path, and queued OS retarget/no-loss
  remains unproven. First step: on the owner's device, discard a composition,
  release every key, type letters without touching the screen, and record
  whether text resumes and the status line. Close condition: Z1-Z7 have
  receiving-system evidence, physical IME steps recorded separately; delete this
  item in the resolving commit.
- SH1-154 physical Herdr 2 image-paste acceptance awaits the owner. Reason: Simulator and mock peers do not prove input-device, suspension, mobile-network, shell/agent path parsing or storage retention behavior. First step: follow the SH1-154 checklist in docs/device-check.md on the physical iPad and disposable hosts, recording returned paths, bytes, timings and lifecycle observations. Close condition: the consumer PR records device/iPadOS and all available SH1-154 device checks with evidence, identifying any unavailable machine/t7 environment; delete in the resolving commit.

- P15/P11 shared provenance native admission remains pending (attempt 3). Reason: the frozen helper fixes same-UUID code substitution in structural fixtures, but actual Apple ad-hoc Mach-O layout and Cargo metadata/depfile execution are unverified. First step: controller obtains a bounded existing linker/ad-hoc byte pair, independently reviews helper commit 8081481, then leases one exact-head compiler-to-installed run with fresh owned workspace/SDK artifacts. Close condition: independent negatives and actual source-to-installed chain pass, P11 adopts the same helper, and native gates are recorded separately; delete in the resolving commit.
- P15 integration with P11 source3 is pending. Reason: P11 owns explicit font intent and transport publication fixes; candidate 60abb804 pins a different SDK and is not implicitly admitted. First step: apply docs/p15-readonly-model/SETTINGS-INTEGRATION-3.md as a scoped merge contract after P11 source freeze, preserving theme, unknown JSON and notifications. Close condition: exact integrated tree passes settings/appearance/notification regressions and independent review; delete in the resolving commit.

# SH1-157 layout follow-ups not chosen on 2026-11-19

The UI test redesign plan referenced below is chief-of-staff task
8f2ce9d471d8451f `plan-1.md` (owner-approved 2026-11-19; S1 runs on branch
`qa/s1-base`). Items the plan already covers point to its section instead of
repeating it.

- SH1-157 layout device check on the iPad awaits the owner. Description: main
  with the layout frame, the sidebar filter field (shown, takes no input), the
  status bar fit rules and
  the Herdr 2 sessions in the sidebar has not been installed on M5; the owner
  chose not to install it on 2026-11-19. Reason: hardware-keyboard detection,
  floating/docked keyboards, touch feel and the Herdr 2 poll against a real
  `herdr-core serve` are only covered by Simulator and host tests, and signing
  needs the locally unlocked login keychain. First step: after the layout
  integration PR merges, unlock the Mac mini keychain locally, connect/unlock
  M5, run `just ipad` on main, then follow "SH1-157 layout" in
  docs/device-check.md one step at a time, recording each result before the
  next. Close condition: all 11 steps have a recorded result with device/iPadOS
  and SHA; delete in the resolving commit.
- JSONL writer reverts are not caught by any test (review 16fc2e13909c4d85
  S1). Description: the five receipt writers that d1a1eae moved to
  `observation::write_jsonl_line` (`core_mock.rs:162`, `core_mock.rs:334`,
  `mock_peer.rs:557`, `find/peer.rs:391`, `mock_notifications.rs:43`) compile
  only for the iOS Simulator; reverting one to piecewise `writeln!(file, ...)`
  passes host tests and clippy, and the torn line shows up later as an
  unrelated red in p9/sh1-152/p12/sh1-153. Reason: the UI test redesign plan
  (O-A, 3.2) only moves the Python reads after `simctl terminate`; the writers
  stay. First step: add a host test that reads the five sources with
  `include_str!` and asserts no `writeln!(file`, and confirm it fails when one
  writer is reverted. Close condition: that test and its negative control pass
  in CI; delete in the resolving commit.
- Hosted "lost press" retries may hide presses the app dropped (review
  f7dd8c0a81204978 F1). Description: `closeOverflowMenu`, `openSettings`,
  `readDeviceKey` and the keyboard hide press retry once when the expected
  screen change is missing, without knowing whether XCTest failed to deliver
  the touch or the app dropped it; the retry count reaches no oracle. Reason:
  covered by the UI test redesign plan 3.4 (every press waits for a `qa.state`
  change and retries once with a reason) in S2 (p2) and S3 (P9, SH154); see
  there. First step: in S1, add a per-press receipt (`seq` of the app's mouse
  down) to `qa.state` so a retry can name which side lost the press. Close
  condition: the S3 harness reports retries with the side that lost the press;
  delete in the resolving commit.
- E1 no longer tests a long press with the software keyboard down (review
  f7dd8c0a81204978 F2). Description: since f95123d/76c3332 EditMenuQA raises
  the keyboard before the E1 long press, because the keyboard that the long
  press raised closed the menu it had just opened (run 44297932478, job
  134779801414); whether that is a product defect or UIKit behaviour is open.
  Reason: the plan (3.6, F18; S4) only exposes `keyboard`/`responder` in
  `qa.state`, it does not restore the keyboard-down path. First step: record
  step 11 of "SH1-157 layout" in docs/device-check.md on M5; if the menu closes
  on the device, add an EditMenuQA case that long-presses with the keyboard
  down and expects the menu to survive. Close condition: the device result is
  recorded and either the case exists or the behaviour is documented as
  UIKit's; delete in the resolving commit.
- The QA strip wiring has no host test (review 51859c6e5c3446f7 S1).
  Description: the strip assembly (`shell/qa_strip.rs:45-71`) and the SH1-153
  focus-then-dismiss handler (`edit_menu_qa.rs:39-50`) compile only for the
  Simulator; removing them passes every host test, and only the hosted
  SH1-153 job notices. Reason: the plan's S3/S4 give the strip identifiers but
  keep the wiring Simulator-only. First step: move the handler body into a
  function outside the `cfg` and test it with a host `Smoke` window, with a
  negative control that drops the focus call. Close condition: the host test
  fails when the wiring is removed; delete in the resolving commit.
- The p3 colour oracle passes `bf616a` by accident (layout-smoke diagnosis
  e115109b440f4304). Description: `validate_colors` in
  `scripts/ios/p3-check.py` counts colours over the whole screenshot, so the
  red status-bar dot of a disconnected landscape screen gave 172 pixels of
  `bf616a` (threshold 100) while the terminal colour band was absent. Reason:
  the plan keeps screenshot checks where pixels are the subject (3.4, R4) but
  does not change this oracle. First step: crop the count to the terminal
  rectangle from the app's paint receipt (or require all seven colours in the
  band rows) and confirm the retained `integration-p3-landscape.png` from task
  e115109b440f4304 fails. Close condition: p3 fails on that screenshot and
  passes on a standard one; delete in the resolving commit.
- Hosted p10 lost its `settings-lifecycle` records (PR 77 run 44406830068,
  layout-smoke job 135164241872; review 9d5ae39be65247ea S2). Description:
  XCTest `testP10Settings` passed and the saved settings file matched the
  Inactive-only QA edit (`settings_view.rs:177-189`), so the lifecycle reached
  the settings adapter, but `p10-check.py:28` found no Inactive, Background and
  Active `settings-lifecycle` records in `p10-settings.jsonl`. The app sends
  records with `try_send` into a 256-slot queue and only prints a drop to a
  stderr nobody keeps, and the artifact has neither the JSONL nor the stderr,
  so drop versus suspension is unproven. Hosted record rates (1.4 to 1.8 per
  second, at most 40 in one second, runs 44356754798 to 44406830068) do not
  support a full queue. Reason: not in docs/ci-simulator.md's known classes, and
  0 earlier p10 failures since 2026-11-17. First step: make a lost record
  visible: in `observation.rs`, write an `observation-dropped` record (with the
  count) before the next record that gets in and stop the writer's 8192-record
  cutoff from closing the queue; copy the app's stderr into
  `Documents/app-stderr.log` and have p10 (and p2, p3, sh1-152) put it and
  `p10-settings.jsonl` in the artifact; make `live_jsonl` fail on a gap record.
  Close condition: that change merged and the next p10 failure, if any, is
  classified from its artifact; or 10 green layout-smoke runs with it; delete in
  the resolving commit.
- Rebuild the sidebar filter field as a UIKit text-input target with IME
  support. Description: since PR 77 the field in the sidebar header takes no
  keyboard focus and shows `Filter · not available yet`
  (`shell/filter.rs` `render_filter_field`); the query, matching and the
  `Clear filter` row are kept but nothing sets the query. Reason: four review
  rounds of the GPUI field (G1 hardware-keyboard detection, H1 detached
  keyboard, SF1/S1 Option and Control routing, F1 in review
  67c755ff05614a56: with a Japanese input source, printable keys go to UIKit
  composition and the text lands in the terminal) each found keys reaching the
  remote shell; the owner chose no input for now (2026-11-19,
  `pr77-filter-no-input-decision.json`). First step: in herdr-uikit, add a
  text-input target a GPUI element can own (its own `UITextInput` responder
  with marked text, so composition and commit go to the field and never to
  the terminal's input handler), then write a host test and a Simulator check
  that Japanese composition, Option keys, Control-C and the software keyboard
  each reach only the intended target. Close condition: the field takes text
  through that target with those tests green and the device check passed;
  delete in the resolving commit.
