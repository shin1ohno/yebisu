# P11 production settings integration

The running Settings route is available from Hosts and the terminal toolbar.
SettingsHost owns persisted draft values and the device-key operation owner.
The sheet consumes PR27's page models, whose original provenance remains in
settings_pages/INTEGRATION.md. The renderer and IME geometry use the same saved
terminal family/size; unset size retains regular14/20 and compact12/17. Density
buttons explicitly clear a custom size. Selection defaults remain Drag/Manual.

The accepted SH1-157 sheet contract uses fully_visible_bounds once, a16-point
inset and1000x760 maximum. Categories move above the content below760 points;
both categories and content scroll. Proposal280x206 terminal floors, M2 priority
and corner policy are not adopted by this settings change. Native keyboard,
VoiceOver/FKA and extreme-size reachability need receiving-device evidence.

Key operations follow security-plan-2.md SHA256
7e86115b30ae0b3eb680982a443ea9d75827aeae61ab1554d9dd5a6bd1c85efb,
independently admitted by task57d7ee3bd35d4e04. Read/initialize can create a first
key or recover SDK labels; it is never called automatically on opening a page.
Replacement requires an explicit visible ticket. Enqueue invalidates all current
key displays/copy suppliers, and one worker remains owned across close/reopen.
A15-second watchdog never kills or retries it. Joined worker plus a successful
explicit current-key reconciliation is required even after a successful replace.
No host trust edits, automatic reconnects or changes to Keychain identity occur.

`p2-check.py` now uses a Simulator-only injected provider and a fixed fake key.
It verifies formatting/copy/read UI over two launches, NOT real Keychain
persistence. The real-key JSONL export was removed. Previous P2 evidence is
historical; new real Keychain persistence is an owner-only acceptance gate.
The app's live provider is compiled out of every Rust test.

Native runner sequence, only with its isolated checkout lease:
`just check`, `just lint-ios`, `just test`, then one guarded
`python3 scripts/ios/p11-check.py`. The latter mounts the production route,
edits a real font, invokes actual native selectors while modal, and requires
receiving peer FIFO markers with zero input between them, native callback
receipts, matching resize/paint geometry and on-disk/relaunch font values.
All Rust/native cases are unexecuted until exact-head Mac logs are attached.

Remaining integration: PR22 changes composition-retirement semantics and PR25
publishes native edit availability using its SDK update. Do not silently merge
those SDK/lockfile changes into this branch; reconcile the modal-specific fence
and immediate native-menu availability with their owners and rerun exact-head
gates. Source admission and Linux oracle unit tests do not certify native input.
