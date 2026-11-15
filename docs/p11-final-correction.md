# P11 final source correction

Base: 60abb8040cbf3e50c59af0bec8608e1087eda2ff. Original P1–P5 and F01–F26 remain required. This source stage is not native/product acceptance.

## Explicit font family intent

The production family listener calls `select_settings_font_family`, which records family intent even when the unloaded draft already displays that family. Density still records its independent compact/size reset. Ordinary `poll_settings`, lifecycle snapshots and asynchronous font enumeration do not create family intent. Load merges other unedited fields, retaining compact/size/priority/selection values. Invalid or unknown-version settings remain blocked and their original file is preserved by existing settings load/save guards.

`settings/family_tests.rs` holds the actual disk-worker Loaded result separately from the production enumeration completion. The real rendered default-family button is clicked with disk=Menlo and draft=SauceCodePro Nerd Font Mono, with and without an unrelated edit; the test checks host switch, background, durable JSON, close/reopen and fresh SettingsHost reload. No-click controls cover both enumeration-before-load and load-before-enumeration. These Rust/GPUI tests are authored, UNRUN until a controller-granted target lease. Existing Regular/unknown-file tests are retained.

## F24 finite fixture detector

`p11-privacy-check.py` derives literal and standard Base64 needles from both fake key sentinels plus the legacy QA marker. All required and additional supplied sinks use one detector. This is finite fixture coverage, not arbitrary secret discovery. Tests contain independent hard-coded expected bytes, specifically `RkFLRV9QUklWQVRFX1NFTlRJTkVM`; valid JSONL negatives ensure parsing errors cannot masquerade as successful leak detection. Native actual sink collection remains UNRUN.

## Blocking dependencies

B2 remains a product defect: `core/worker.rs` compares input_epoch before CoreTerminal::input, while the pinned transport queues then writes independently. Existing PauseInput precedes the comparison. Tests must separately hold after that comparison and after SDK dequeue/before underlying write, keep the actual modal open, and collect receiving zero forbidden bytes plus acknowledged final settlement. Disconnect is a stop signal, not a drain/join acknowledgment. No SDK change or consumer workaround is implemented here; the controller owns the independent public-API audit and remedy decision.

C1 generic compiler-input and unsigned→signed provenance is owned by the P15 shared helper author. Only a controller-relayed immutable helper commit may be integrated. P11 must bind full Cargo JSON/resolved metadata/selected manifest and src/features/profile/depfiles/source and SDK hashes to unsigned, copied, signed and installed code; unique run paths and independent counterfeit negatives remain necessary. Current signed=installed equality does not prove this chain.

SDK2dcd pin, collector acceptance:false, unsupported phases, original 0/4 failures, guard and environment runner remain unchanged. No Mac/Rust/native/Swift/SSH/build/Simulator/keychain/signing execution is authorized by this document. Exact-head check/lint-ios/test, all fixtures and independent/owner gates remain open.
