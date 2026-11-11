# P10 settings foundation

Source: `shin1ohno/herdr-gpui` commit `398c132a06723a0cc1494842a34666d173ec0200`.
`S/` means `crates/sheprd-gpui/src/` in that read-only checkout.
This mapping is fixed before implementation; P10 introduces no settings sheet.

## Application and page boundaries

| Source setting / dependency | Destination / cut | Enabled |
|---|---|---|
| fonts (`settings_window/pages/fonts.rs`), configured terminal size (`S/menu/settings.rs:204`) | SettingsHost draft → terminal appearance, `crates/yebisu/src/terminal_view.rs:212`; preserve 14/20 and compact 12/17 defaults | P11 controls |
| keyboard / keybindings (`S/menu/settings.rs:212-217`) | P7 catalog `crates/yebisu/src/actions.rs:51`; terminal precedence setting | P11; native menu installation P12 |
| ipad_key | P2 `crates/yebisu/src/keys.rs`, `setup.rs:48`; SettingsHost owns confirmation, actual key operations remain P2 workers | P11 |
| general | bundled LICENSE and NOTICE; no chat dependencies | P11 |
| tabs/sidebar/status_bar/indicators/window | only existing P6/P7 board visibility and density; `actions.rs:15`, `terminal_view.rs:22`; unsupported controls absent | P11 |
| language (`S/menu/settings.rs:210`) | en-US catalog in i18n.rs; no locale selection | hidden |
| notification delivery, sound reload/volume, enabled_since, toast ticking (`S/menu/settings.rs:22-36,219-226,250,253-257`) | no imports into foundation; future P14 notification owner; no backlog replay | P14 |
| shared notification override (`S/menu/settings.rs:26-27,195-197`) | future per-host read-only snapshot, never iPad local daemon config | P14/P15 |
| config watcher / pending saves / Themes guard (`S/menu/settings.rs:88-89`) | SettingsHost serial writer + revision-fenced completion; no file watcher or theme editor yet | P10 persistence; P15 theme |
| status bar show (`S/menu/settings.rs:227-229`) | no status bar visibility control until matching renderer exists | hidden |
| theme / appearance / Follow Herdr (`S/menu/settings.rs:235-249`) | current `theme.rs:17` palette retained; theme application belongs to P15 | P15 |
| diagnostics / log appearance / wheel / resize (`S/menu/settings.rs:251-263`) | typed settings errors → setup status; terminal appearance invalidates size in P11 adapter; no log window or wheel settings | P10 errors, P11 apply |
| load error (`S/menu/settings.rs:265-268`) | preserve corrupt file, retain current/default draft, block automatic overwrite and expose error | P10 |
| GitHub credential restore and cache (`S/menu/settings.rs:274-302`) | cut entirely; no GitHub account config | excluded |
| configuration | per-host read-only view, overriding plan's read/write wording per addendum D5 | P15 |
| remote_control/gitlab/plugins/cloud/chat | cut at module boundary, not merely hidden at runtime | excluded |

## Sheet ownership boundary

| Source | Destination |
|---|---|
| SettingsWindow (`S/menu/settings_sheet.rs:4,9`) | P11 editor entity, owns controls only; draft and save remain plain SettingsHost in Smoke |
| install/open/menu page (`:17-21`) | P11 sheet adapter on Smoke, reuses terminal input cancellation/focus isolation; no SheprdWindow or new trait |
| sheet_ready / flush / keep_settings (`:26-39`) | SettingsHost revision and durable acknowledgement; lifecycle adapter calls flush before reconnect's suspended early return (`reconnect.rs:46`) |
| regeneration confirmation | SettingsHost confirmation state; no key regeneration in P10, P11 must pass P2 formatter and owner confirmation |
| motion / backdrop / offset (`:63-65,79`) | P11 sheet-local animation; no desktop motion global in P10 |
| sheet_size / metrics (`:66,73`) | P11 sheet-local bounds using upstream constants `S/ipad_ui/metrics.rs:26-30` (240/56/760/1000/18); current safe-area coordinates unchanged |

## Persistence contract

One container-local file, `yebisu-settings.json`, schema version 1. A bounded
single-writer queue performs load/atomic replace/fsync off the UI thread.
Only the latest revision's completion acknowledges the current draft; completions
never replace the draft. Inactive/Background/Disconnected flush pending state
before reconnect handling. A UIKit BackgroundTask spans the durable write,
including an already-running write. Foreground retries unsaved state. A process
terminated before atomic rename keeps the previous complete file. A corrupt or
unknown-version file remains untouched and is reported, never silently reset.
The settings sheet and its visual prerequisites remain P11, not fake P10 controls.

## Measured closure (replaces A4 for P10)

Physical lines, including comments and blank lines; measured from this change,
not a percentage of the old desktop modules. The five prerequisite categories
are the five leading import groups in plan-3 section 5 (settings, strings, common
components, theme/fonts, popup shell). Sidebar/tabs already exist in P6/P7.

| Prerequisite | New P10 Rust lines | Concrete boundary |
|---|---:|---|
| settings values + storage | 251 | settings.rs (158), settings/store.rs (58), settings/worker.rs (35) |
| strings | 51 | i18n.rs; pinned sheprd-i18n, 72 copied FTL lines in three files; existing commands.ftl reused |
| common sheet controls | 0 | not needed for persistence; P11 owns copied controls |
| theme/fonts | 0 | retain theme.rs and terminal_view.rs source defaults; no theme editor imported |
| popup shell/motion/metrics | 0 | P11 owns actual sheet; lifecycle flush extracted independently |
| host adapter (separate from the five groups) | 61 | settings_view.rs |

Thus P10's new production closure is **363 Rust lines**, plus **181 Rust test
lines** and **72 FTL lines**. Existing-file integration edits are separate. These
are measured P10 values, not an estimate that P11's full editor is already ported.
No source-level SheprdWindow reference or new trait is needed in this closure.
The whole consumer compiles with these boundaries; P11 must still compile its
actual control/editor subtree as it is brought in.

Five concrete integration cut points remain in the consumer: startup
(`app.rs:242`), timer (`app.rs:67`), UIKit lifecycle (`reconnect.rs:36`), command
labels (`actions.rs:55`), and density across host selection (`board.rs:158`).
The old estimate of 170 coupling points describes importing the old modules;
it is not the same counting unit as these five explicit adapter sites. Excluded
source calls are itemized in the mapping above rather than hidden behind a trait.

The new direct dependency is sheprd-i18n at the existing fork revision: it supplies
the actual Fluent formatter for future settings messages and replaces P7's
single-line parser. Missing internal command messages retain their prior name
fallback; every catalog command is compared against the previous formatter.
No existing locked package version is upgraded.
