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

## Measured sheet prerequisite closure (A4, corrected after D1)

`settings_spike` is compiled by the normal device and Simulator builds, not
behind `cfg(test)`. Only tests mount it; `app::run` never installs its sheet.
This replaces the earlier persistence-only measurement, which did **not** satisfy
A4. It is a source-adapted, working compile spike, not the finished P11 editor.

The spike compiles an actual SettingsWindow entity, copied row/switch/button/
segmented/number controls, font specimen, sheet installation, focus isolation,
dismiss-after-flush, retained editor while closing, animation, and iPad dimensions.
A host rendering test clicks the copied switch and number stepper, observes the
font draft, closes the sheet, and reads the SettingsHost-written file. Thus the
shell-to-editor-to-control-to-owner edges are exercised, not just named in a table.

Counts below are physical lines including comments/blank lines and test-only
helpers inside implementation files, excluding separate test files. `settings/`
means the running implementation; other abbreviated paths are under
`crates/yebisu/src/settings_spike/`.

| Prerequisite group | Required Rust lines | Files | Explicit cut points |
|---|---:|---|---:|
| settings/config and owner | 348 | settings.rs 182 + settings/store.rs 58 + settings/worker.rs 44 + draft.rs 64 | 4 |
| strings | 100 | existing i18n.rs 51 + strings.rs 49 | 2 |
| common controls | 649 | ui.rs 358 + marks.rs 31 + number.rs 260 | 9 |
| theme/fonts | 73 | appearance.rs | 3 |
| popup shell/motion/iPad metrics | 354 | settings_spike.rs 16 + editor.rs 134 + sheet.rs 132 + motion.rs 65 + metrics.rs 7 | 6 |
| **Five-group total** | **1,524** | | **24** |
| Running lifecycle adapter (separate) | 66 | settings_view.rs | existing owner integration |

The complete measured closure is **1,590 Rust lines**, plus 259 newly copied
settings FTL lines and the previous 72 settings FTL lines (commands.ftl reused).
The previous 30–100% assumption is replaced by these concrete retained/adapted
files. The 24 entries below count explicit replaced/removed source dependencies,
not the former estimate's 170 occurrences of coupling; they are not a percentage.

### Cut-point ledger (source at 398c132a0)

1. **Settings/config (4):** `S/menu/settings.rs:164-268` Config load/apply becomes
   Draft + the real SettingsHost; `:41-94` config/watch becomes the serial local
   worker; `S/settings_window.rs:126-140` open-draft save becomes Draft::flush;
   `S/settings_window/device_key.rs:40-48` regeneration routes to the owner's
   one-shot confirmation only, never a key operation in this spike.
2. **Strings (2):** `S/i18n.rs:17-25,51-81` locale/global loader becomes an en-US
   catalog; `:34-49` generated fl! ID/argument checking becomes a local formatter
   with parsed-catalog and source-label assertions. No locale selector is exposed.
3. **Controls (9):** `S/settings_window/ui.rs:4` Logical direction becomes LTR
   flex; `:126,144` availability becomes the explicit supported-row allowlist;
   `:174` search bounds tracking is removed; `S/settings_window/ui/number.rs:89-94,
   208-231` NumberEdit/SearchInput is cut from this stepper/slider spike;
   `ui.rs:13,18` dropdown submodule and `:14,19` swatch submodule are separate P11/
   P15 controls; `:224` global toggle renderer becomes the retained switch in
   marks.rs; `:233,252,353` accessibility::disabled becomes the same local GPUI
   helper; `number.rs:254-261` desktop-native probe is removed. Number bounds,
   step/preview/commit callbacks, roles, disabled states and segment callbacks
   remain compiled. Typed numeric IME editing is not claimed finished.
4. **Theme/fonts (3):** `S/settings_window.rs:55-59` Config/Theme becomes the fixed
   consumer palette; `S/config/fonts.rs:15-20,26-50` face configuration becomes the
   existing terminal face/density presets plus a bounded **local font draft** for
   the spike; `S/toggles.rs:12-28` configurable contrast becomes the fixed palette.
   The numeric draft changes the specimen but is intentionally not added to the
   production persistence schema. Free-form family selection and final font apply
   are P11 work, not silently asserted as delivered.
5. **Shell (6):** `S/menu/settings_sheet.rs:4,17-21` SheprdWindow/menu registry
   becomes SettingsSheet's owner/page state; `:9,84` SettingsWindow becomes the
   compiled editor.rs entity; `:26-39` ready/flush uses the actual SettingsHost;
   `:63-65,79` Appear/kept becomes the retained editor + source motion::Toggle;
   `:66,73` iPad metrics are extracted constants and the original clamped size;
   `:94,108` viewport/direction helpers become fully_visible_bounds and LTR layout.

Copy/model origins are registered in PROVENANCE.tsv. The source and prototype
render paths are read and compiled; no desktop, chat, notification, remote-write,
or GitHub backend is imported just to hide an inactive page. P11 still has to
assemble the full pages and typed editors, using these now-compiled prerequisites.

### Failure behavior corrected in attempt 2

Corrupt/unknown-version loads block writes. A transient write failure pauses the
writer until the next changed edit or Active callback, without weakening that
load protection. Early edits are tracked per field and merged over the loaded
values. Settings error changes notify the owner; the active screen renders the
error, and normal rendering takes its original path. The corrupt-file test reads
after a FIFO worker barrier, so an asynchronously queued overwrite cannot escape
its assertions.
