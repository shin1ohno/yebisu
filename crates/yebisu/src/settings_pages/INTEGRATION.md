# P11 page module handoff

Source baseline: yebisu `dc2511fe695935cdce60ba6bb3cfb4288a425778`; read-only upstream `shin1ohno/herdr-gpui` `398c132a06723a0cc1494842a34666d173ec0200`.

## Exact source adaptation and pruning

| Source symbol or content | Result in this module | Reason / consumer |
|---|---|---|
| `settings_window/pages/fonts.rs`: `render_fonts`, `face_row`, `family_button` | Replaced by `fonts::Density`, `TerminalFont`, and `TERMINAL_FAMILY` | yebisu has one bundled terminal family and only two real size presets; `SettingsHost::Values.compact` and the terminal painter consume them. No persisted arbitrary family/size exists. |
| `settings_window/pages/keyboard.rs`: `KEYBOARD_ROWS`, `shortcut_values`, `prefix_value`, `render_keyboard` | Replaced by `keyboard::shortcut_rows`, `terminal_priority`, and `set_terminal_priority` | The existing `actions::catalog` is the supported action allowlist; `actions::shortcut` consumes `terminal_priority`. Keymap source/style editing has no yebisu consumer. |
| `settings_window/pages/ipad_key.rs`: loading/ready/error display and copy | Replaced by `device_key::KeyView` and `copy_text` | Setup `Job::Key` already calls `keys::authorized_line` before producing `Reply::Key`; display and copy use that one formatted payload. This module performs no keychain operation. |
| `settings_window/pages/ipad_key.rs`: replace/confirm/replacing workers | Pruned | Separate security review and production owner integration are required; no replacement affordance is exported here. |
| `settings_window/pages/general.rs`: interface, features, chat, clipboard, worktree, skill, update controls | Pruned | They have no yebisu settings consumers. The general page exposes only bundled legal text. |
| Repository `LICENSE`, `NOTICE` | Included with `include_str!` by `general::Document` | The displayed text is the shipped root file content. |
| Fonts: sidebar/tabs/UI faces, family picker, arbitrary number editor | Pruned | No corresponding yebisu renderer or persisted field. |
| Keyboard: shortcut style, server keybindings, prefix edit | Pruned | No supported keymap preference consumer in yebisu. Shortcut rows remain read-only and derive from `actions::catalog`. |
| Other pages: tabs/sidebar/status bar/indicators/window, theme, notifications, cloud/plugin/chat/remote control | Pruned | These are outside this page-only task or not implemented; no controls are exported. |

## Integration entry points and gates

1. Add `#[cfg(any(target_os = "ios", test))] mod settings_pages;` beside `settings_spike` in `crates/yebisu/src/main.rs:102-106`, then mount `Page::ALL` through the production sheet page registry.
2. The sheet editor maps `Density::apply` and `set_terminal_priority` into the existing `SettingsHost` draft, then calls the existing poll/flush owner path. `settings_view.rs:28-53,61-75` and `settings.rs:85-105` are the current owner entry points.
3. Font renderer apply remains at `terminal_view.rs:212-215`; retain SauceCodePro Nerd Font Mono and regular 14/20, compact 12/17. The only exposed font control is the persisted density preset.
4. Keyboard rows are a projection of `actions::catalog`; do not synthesize unavailable actions or introduce a second shortcut table. The toggle edits `Values.terminal_priority`, which `actions::shortcut` reads.
5. The production key view must receive `Reply::Key`, hold the returned formatted public line via `KeyView::from_formatted_worker_line`, and use `KeyView::copy_text` for clipboard gating. Loading/retry/error UI, focus isolation, stale reply rejection, invalidation after replacement, owner-confirmed replacement, and credential-worker wiring remain pending.
6. Bind `general::Document::{License,Notice}` to scrollable UI and verify both displayed byte strings match the shipped root files.
7. The production sheet route, focus/native input isolation, draft persistence, background flush, load/save failure behavior, narrow-window layout, rendering checks, and owner physical acceptance are not proved by these page models.

This manifest describes source-level adaptation, not a compiled or mounted product page.

## P11 production integration (feat/p11-settings-sheet)

The original handoff above remains provenance history. main.rs now mounts this
module. settings_sheet/view.rs consumes Page::ALL, font density, shortcut catalog,
KeyView via SettingsHost::keys, and both legal documents. SettingsHost alone owns
all public-key operations; Setup Job::Key and Reply::Key were removed to prevent
bypass reads. KeyView receives only the single formatter output from key_ops.
Font size/family now persist in settings/font.rs and apply to painter plus native
composition geometry; the family list comes from the platform text system off
UI thread. Native validation is still required; mounted source is not acceptance.
