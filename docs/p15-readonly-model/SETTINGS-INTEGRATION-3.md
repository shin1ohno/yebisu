# P15 / P11 source3 integration contract

Status: source merge contract, not approval of P11 or SDK candidates.
P15 base is 201739021014c95a0cdcce4cac7ffe91f7d4da62. The shared helper candidate is
8081481a4088ae68a0ad2d0fb9c5ff2f94323a7a. Original P1-P5 remain unverified.

The three Simulator runner files are adopted byte-for-byte from main
 aa457e9f0c06dbc706c64447cc3c4f839c3979f3: sim-runner-env.py, sim-runner.sh and
 test_sim_runner.py. They preserve raw hyphenated Cargo environment names and
 arguments on ordinary/clipboard paths. These are runner changes, not SDK adoption.

P11 comparison source is 60abb8040cbf3e50c59af0bec8608e1087eda2ff. Its settings.rs
contains the same explicit density edit but lacks P15 theme field/intent. Its
settings/store.rs creates a new JSON object; taking that whole file would discard
P15 theme, unknown fields and opaque notification preferences. Keep P15's patching
store and one SettingsHost. P11 source3 owns explicit font-family intent; merge its
field-specific intent into Edited/edit_values/load without resetting theme intent
or treating ordinary poll as an explicit font/theme action. Do not merge its Cargo
pin 2dcd11a9f20762bb7070c3fdefb72d93ed952781 implicitly; P15 still pins b2fa7811.

Required merge invariants:
- Values.theme defaults to OwnerDefault; Edited.theme protects explicit default
  before initial load. choose_theme plus immediate local_theme/painter update,
  revision/flush and background/relaunch remain routed through the same host.
- Store reads existing JSON, patches supported theme and known fields, preserves
  future keys and notifications, enforces size/schema failures, and writes with
  sync/rename/directory-sync. Unknown themes remain blocked, never coerced.
- Typed notification integration must update only its owned keys on the same JSON
  document. The existing opaque fixture is not proof of typed P14 integration.
- Preserve Appearance route and palette click listeners alongside P11 modal/input
  gates, explicit font intent, provider preflight and later SDK observer source.
- Follow remains absent. Remote host settings return Unavailable before dispatch;
  no signer acquisition, trust mutation, reader, save/CAS/reload or retries are added.

Source checks to carry forward: settings/theme_tests.rs tests explicit default
before load, other-field preservation, background/reopen and unknown theme blocking;
settings/font_tests.rs plus P11 source3 tests must exercise both explicit intents
in either arrival order. p15-check.py exact JSON includes notifications/future keys.
Real Appearance/painter/native/physical and two controlled SSH receiver checks stay
UNRUN. P11 F19/SDK refusal and original D3 gates are not closed by helper fixtures.

P11 collector adoption uses the shared build_install API and retains all its
observer/privacy/FIFO validations. Do not copy P15's whole collector or settings
files into P11. Review each conflict against these invariants and frozen sources.
