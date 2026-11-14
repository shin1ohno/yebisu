# P12 keyless menus (SH1-179)

The Terminal native menu uses `NativeCommand { name }` with no registered
KeyBinding. Shortcut strings are part of the label (for example, `⌘D`).
The pinned SDK emits UICommand without a key input, so selection enters the
action handler. Hardware keys retain the terminal adapter and never borrow a
menu action's identity. Command-hold is deferred by the owner decision.

`native_menus::catalog_rows(terminal, actions, focused)` is the presentation API
for both menus and the future Settings Keyboard page: supported IDs, localized
labels, default shortcuts and current availability. It shares `actions::catalog`
with the execution check. Local sidebar/palette commands work offline; remote
commands additionally require connection, compatible version, no suspension or
pending navigation, a snapshot target, and the method capability. Focus and
priority are checked by the common entry gate. RPC execution also revalidates
the inbox snapshot and capability before sending.

Menu selection checks the installed scope against current host, boot, link
generation, input epoch, pane/tab/workspace, focus, palette and availability.
After reinstallation the SDK rejects obsolete native tags. The consumer action
is deliberately not a captured generation ticket. Changes that return to the
same state without rendering follow options-2 section 3.3. A physical key is a
new intent and skips the installed-scope check, as required by section 6.1.
Recognition precedes availability; unavailable recognized shortcuts are consumed.
IME, popup, prefix and priority exclusions keep the existing terminal path.
SH1-153 native Edit selectors and clipboard shortcuts are unchanged.

## Review findings applied

The design review is `b18a1d5191744e06/review-2.md` (the brief's review-1 path is
missing; the directory's artifact index identifies review-2). B1/B2 concern
keyed menus and SDK rewrites, neither of which this change uses. B3 is addressed
by separate expectations: remote positive = one request, local positive = one
state toggle, refusal = no request/input, priority = normal terminal input.
Oracle tests reject all-refused positives and duplicate callbacks. S1 is handled
by splitting shared UI conditions from remote transport conditions.

## Verification and remaining gates

Hosted CI checks the exact PR head rather than a merge commit. `check` runs
check/lint-ios/test and real source mutations. `simulator-smoke` runs P0 smoke
and installed P12 native menu selection with peer receipts and screenshots.
Native evidence records the compiler/bundle/installed binary UUID and hash.
The Simulator fixture freezes menu installation only to test pre-render scope
mismatch. Boot/host/pane/focus/capability negatives are injected independently.
These do not prove SDK stale-tag generation handling or physical key delivery.

The P11 Settings UI is unmerged and main's settings sheet is unmounted. This PR
provides and tests the catalog API; mounted Settings/search modal behavior stays
unverified, blocked on the corresponding UI integration. No P11 code is stacked.
Independent review is a controller gate; this worker cannot create reviewers.

## Owner QA

- [ ] Record device model and iPadOS version.
- [ ] Select Split Right once: exactly one pane split and no terminal input.
- [ ] Select Sidebar and Palette twice: each selection toggles once.
- [ ] Hold/repeat/release Command-D; then press again with Command held.
- [ ] With terminal priority and Command-as-Control enabled, verify Ctrl-D input.
- [ ] Verify prefix keys and Japanese composition retain the previous behavior.
- [ ] Verify native Copy/Paste with software and hardware keyboards.
- [ ] After Settings/search integration, verify modal isolation and restored focus.
- [ ] Observe the Command-hold display; implementing the HUD is a separate task.

No SDK change, merge, signing, or gesture expansion is included.
