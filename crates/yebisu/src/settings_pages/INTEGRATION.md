# P11 page module handoff

Source baseline: yebisu `dc2511fe695935cdce60ba6bb3cfb4288a425778`; read-only upstream `shin1ohno/herdr-gpui` `398c132a06723a0cc1494842a34666d173ec0200`.

## Exact source adaptation and pruning

| Source symbol or content | Result in this module | Reason / consumer |
|---|---|---|
| `settings_window/pages/fonts.rs:16-146`: `size_control`, `specimen_title`, `render_fonts`, `all_families_button`, `face_row`, `family_button` | Replaced by `fonts::Density`, `TerminalFont`, and `TERMINAL_FAMILY` | yebisu has one bundled terminal family and only two real size presets; `SettingsHost::Values.compact` and the terminal painter consume them. No persisted arbitrary family/size exists. |
| `settings_window/pages/keyboard.rs:17-429`: `TAB_NUMBERS`, `KEYBOARD_ROWS`, `shortcut_values`, `samples`, `prefix_value`, `prefix_host`, `prefix_guidance`, `render_keyboard`, `keyboard_keybinding_choice`, `keyboard_style_row`, `keyboard_table`, `switch_mark`, `table_row`, `action_cell`, `style_cell` | Replaced by `keyboard::shortcut_rows`, `terminal_priority`, and `set_terminal_priority` | The existing `actions::catalog` is the supported action allowlist; `actions::shortcut` consumes `terminal_priority`. Keymap source/style editing has no yebisu consumer. |
| `settings_window/pages/ipad_key.rs:16-261`: `IpadKeySettings`, test-only `KeyIo`, `open_ipad_key`, `replace_ipad_key`, `start_ipad_key`, `render_ipad_key` | Display/copy shape replaced by `device_key::KeyView` and `copy_text`; worker and replacement symbols pruned | Setup `Job::Key` already calls `keys::authorized_line` before producing `Reply::Key`; display and copy use that one formatted payload. This module performs no keychain operation. |
| `settings_window/pages/general.rs:22-409`: `render_general`, `startup_rows`, `feature_rows`, `save_skill`, `save_skill_with`, `chat_width_controls` and inline interface/feature/chat/clipboard/worktree/skill/update rows | Pruned | They have no yebisu settings consumers. The general page exposes only bundled legal text. |
| Repository `LICENSE`, `NOTICE` | Included with `include_str!` by `general::Document` | The displayed text is the shipped root file content. |
| Fonts: sidebar/tabs/UI faces, family picker, arbitrary number editor | Pruned | No corresponding yebisu renderer or persisted field. |
| Keyboard: shortcut style, server keybindings, prefix edit | Pruned | No supported keymap preference consumer in yebisu. Shortcut rows remain read-only and derive from `actions::catalog`. |
| Other pages: tabs/sidebar/status bar/indicators/window, theme, notifications, cloud/plugin/chat/remote control | Pruned | These are outside this page-only task or not implemented; no controls are exported. |

## Integration entry points and gates

1. Add `#[cfg(any(target_os = "ios", test))] mod settings_pages;` beside `settings_spike` in `crates/yebisu/src/main.rs:102-106`, then mount `Page::ALL` through the production sheet page registry.
2. The sheet editor maps `Density::apply` and `set_terminal_priority` into the existing `SettingsHost` draft, then calls the existing poll/flush owner path. `settings_view.rs:28-53,61-75` and `settings.rs:85-105` are the current owner entry points.
3. Font renderer apply remains at `terminal_view.rs:212-235`; keep the same face/size inputs in `input.rs:248-255` for IME composition indexing and geometry. Retain SauceCodePro Nerd Font Mono and regular 14/20, compact 12/17. The only exposed font control is the persisted density preset.
4. Keyboard rows are a projection of `actions::catalog`; do not synthesize unavailable actions or introduce a second shortcut table. The toggle edits `Values.terminal_priority`, which `actions::shortcut` reads.
5. The production key view must receive `Reply::Key`, hold the returned formatted public line via `KeyView::from_formatted_worker_line`, and use `KeyView::copy_text` for clipboard gating. Loading/retry/error UI, focus isolation, stale reply rejection, invalidation after replacement, owner-confirmed replacement, and credential-worker wiring remain pending.
6. Bind `general::Document::{License,Notice}` to scrollable UI and verify both displayed byte strings match the shipped root files.
7. The production sheet route, focus/native input isolation, draft persistence, background flush, load/save failure behavior, narrow-window layout, rendering checks, and owner physical acceptance are not proved by these page models.

This manifest describes source-level adaptation, not a compiled or mounted product page.
