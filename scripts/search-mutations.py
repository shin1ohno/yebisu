#!/usr/bin/env python3
"""Negative controls for sidebar search (issue #138, AC24).

Each mutation removes or inverts one guard of the search layer; the named
host test must then fail, after its own "opened" precondition. Every source
file is restored byte for byte after each run.

  search-mutations.py host            # cargo test per mutation (guarded)
  search-mutations.py host LABEL...   # only these mutations

Prints `SEARCH_MUTATION <label>: <test> FAILED AS REQUIRED` or `DID NOT FAIL`
per entry, then `SEARCH_RESTORED PASS|FAIL`; exits non-zero unless every
mutation failed its test and the restored tree passes.
"""
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
SRC = 'crates/yebisu/src/'
SEARCH = SRC + 'shell/search.rs'
RESULTS = SRC + 'shell/search/results.rs'
T = 'shell::search::tests::'

# (label, [(file, before, after)], failing test path)
MUTATIONS = [
    ('open-over-sheet',
     [(SEARCH, '        if self.input_modal() {\n            self.terminal.live.status = "Close the sheet first".into();',
       '        if false {\n            self.terminal.live.status = "Close the sheet first".into();')],
     T + 'open::open_refused_over_sheets'),
    ('open-over-find',
     [(SEARCH, '        if self.find.is_some() {\n            self.terminal.live.status = "Close Find first".into();',
       '        if false {\n            self.terminal.live.status = "Close Find first".into();')],
     T + 'open::open_refused_over_find'),
    ('second-present',
     [(SEARCH, '        if self.search_open() {\n            return;\n        }\n        let presentation',
       '        if false {\n            return;\n        }\n        let presentation')],
     T + 'open::second_tap_does_not_present_again'),
    ('present-error-silent',
     [(SEARCH, '                self.terminal.live.status = error.reason().into();\n',
       '                let _ = error;\n')],
     T + 'open::present_error_leaves_no_session'),
    ('no-detach-at-open',
     [(SEARCH, '        self.terminal.input.detach();\n        let revision_at_open',
       '        let revision_at_open')],
     T + 'open::composition_discarded_at_open'),
    ('revision-before-detach',
     [(SEARCH, '        self.terminal.input.detach();\n        let revision_at_open = self.terminal.input.retirement_revision();\n',
       '        let revision_at_open = self.terminal.input.retirement_revision();\n        self.terminal.input.detach();\n')],
     T + 'terminal::unchanged_revision_releases_the_fence'),
    ('release-any-revision',
     [(SRC + 'terminal_input.rs',
       '    pub fn release_after_native_search(&mut self, revision: u64) -> bool {\n        self.acknowledge_retirement(revision)\n',
       '    pub fn release_after_native_search(&mut self, revision: u64) -> bool {\n        let _ = revision;\n        self.fresh_gesture();\n        true\n')],
     T + 'terminal::changed_revision_keeps_fence'),
    ('release-any-responder',
     [(SEARCH, '                if restored == Restored::Text {', '                if true {')],
     T + 'terminal::non_text_restore_keeps_fence'),
    ('stamp-while-open',
     [(SRC + 'handoff_view.rs', '            && !self.search_open()\n', '')],
     T + 'open::paste_started_while_open_sends_nothing'),
    ('sync-before-register',
     [(SEARCH, '        self.handoff.sync(None);\n        self.handoff.cancel();\n        self.handoff.select_mode = false;\n        cx.notify();\n    }',
       '        self.handoff.select_mode = false;\n        cx.notify();\n    }'),
      (SEARCH, '        self.shell.search.next_session += 1;\n',
       '        self.handoff.sync(None);\n        self.handoff.cancel();\n        self.shell.search.next_session += 1;\n')],
     T + 'open::paste_prepared_before_open_dropped'),
    ('no-cancel-at-open',
     [(SEARCH, '        self.handoff.cancel();\n', '')],
     'core::image_tests::image_cancel_tests::cancellation_sources_fence_preparation_and_waiting_put'),
    ('band-while-open',
     [(SRC + 'shell/frame.rs', '        if self.search_open() {\n            0.0\n        } else {',
       '        if false {\n            0.0\n        } else {')],
     T + 'band::band_zero_while_open'),
    ('pager-band-while-open',
     [(SRC + 'shell/compact/pager.rs',
       '|| self.key_band_now(size, &self.platform_insets(window).ime) > 0.0',
       '|| crate::shell::fit::key_band_height(size, &self.platform_insets(window).ime, self.hardware_keyboard_attached()) > 0.0')],
     T + 'band::compact_pager_band_zero'),
    ('hide-while-open',
     [(SRC + 'shell/key_band/mod.rs', '        if self.search_open() {\n            return;\n        }\n', '')],
     'shell::key_band::tests::band_presses_while_search_is_open_change_nothing'),
    ('retirement-while-open',
     [(SRC + 'input_retirement.rs',
       '        if self.search_open() {\n            return;\n        }\n        if self.terminal.input.requires_gesture() {',
       '        if self.terminal.input.requires_gesture() {')],
     T + 'terminal::no_retirement_while_open'),
    ('query-ignored',
     [(SEARCH, '                self.set_sidebar_filter(&query);\n', '')],
     T + 'events::query_sets_filter_and_sidebar'),
    ('clear-field-kept',
     [(SEARCH, '                    let _ = control.set_query("");\n', '                    let _ = control;\n')],
     T + 'events::clear_requested_empties_field'),
    ('escape-keeps',
     [(SEARCH, '                if reason == DismissReason::Escape {', '                if false {')],
     T + 'events::escape_clears_filter'),
    ('others-clear',
     [(SEARCH, '                if reason == DismissReason::Escape {', '                if true {')],
     T + 'events::user_dismissals_keep_filter'),
    ('stale-row-opened',
     [(RESULTS, '            .find(|(shown, _)| *shown == revision)\n            .and_then(|(_, rows)| rows.get(&id))\n',
       '            .find(|(shown, _)| *shown == revision || true)\n'
       '            .and_then(|(_, rows)| rows.get(&id).or_else(|| rows.values().next()))\n')],
     T + 'select::stale_row_ignored_and_logged'),
    ('previous-revision-dropped',
     [(RESULTS, '        self.previous = self.current.replace(shown);\n',
       '        self.current = Some(shown);\n        self.previous = None;\n')],
     T + 'select::selection_from_previous_revision_opens'),
    ('err-dismisses',
     [(SEARCH, '                tracing::warn!(%error, "search row did not open");\n',
       '                tracing::warn!(%error, "search row did not open");\n'
       '                if let Some(control) = self.search_control() {\n'
       '                    let _ = control.dismiss_selected(id);\n'
       '                }\n')],
     T + 'select::failed_open_keeps_panel_with_message'),
    ('early-release',
     [(SEARCH, '                    tracing::warn!(?error, "search panel not dismissed after a selection");\n                }\n',
       '                    tracing::warn!(?error, "search panel not dismissed after a selection");\n                }\n'
       '                self.with_search(|session| session.control = None);\n')],
     T + 'select::select_h1_space'),
    ('failed-keeps-session',
     [(SEARCH, '                    session.control = None;\n                    session.phase = Phase::Restoring;\n'
               '                });\n                self.terminal.live.status = format!(',
       '                    session.phase = Phase::Restoring;\n'
       '                });\n                self.terminal.live.status = format!(')],
     T + 'terminal::failed_then_restored_cleans_once'),
    ('closed-twice',
     [(SEARCH, '                if reason == DismissReason::Escape {',
       '                if reason == DismissReason::WindowClosed {\n'
       '                    self.shell.search.session = None;\n'
       '                }\n'
       '                if reason == DismissReason::Escape {')],
     T + 'terminal::window_closed_cleans_once'),
    ('compact-threshold',
     [(SEARCH, '        let presentation = if self.search_compact(window) {',
       '        let presentation = if !self.search_compact(window) {')],
     T + 'open::open_presentation_follows_the_compact_width'),
    ('banner-empty-only',
     [(SRC + 'shell/sidebar/view.rs', '        list = list.children(self.render_filter_banner(cx));\n',
       '        if agents.is_empty() && !rows.iter().any(|row| matches!(row, ListRow::Space(_))) {\n'
       '            list = list.children(self.render_filter_banner(cx));\n'
       '        }\n')],
     T + 'display::clear_with_results'),
    ('accent-missing',
     [(SRC + 'shell/filter.rs', '    } else {\n        color::ACCENT\n    }\n}\n',
       '    } else {\n        color::MUTED\n    }\n}\n')],
     T + 'display::magnifier_accent_and_label'),
    ('reopen-on-async-failure',
     [(SEARCH, '                window.notify_text_input_changed(&self.terminal_focus);\n',
       '                window.notify_text_input_changed(&self.terminal_focus);\n'
       '                if session.pending.is_some() {\n'
       '                    let anchor = self.search_anchor(gpui::point(px(0.), px(0.)));\n'
       '                    self.open_sidebar_search(anchor, window, cx);\n'
       '                }\n')],
     T + 'select::post_send_failure_does_not_reopen'),
]


def apply(edits):
    originals = {}
    for relative, before, after in edits:
        path = ROOT / relative
        originals.setdefault(path, path.read_bytes())
        text = path.read_text()
        if text.count(before) != 1:
            restore(originals)
            sys.exit(f'anchor not unique in {relative}: {before!r}')
        path.write_text(text.replace(before, after))
    return originals


def restore(originals):
    for path, data in originals.items():
        path.write_bytes(data)
        if path.read_bytes() != data:
            sys.exit(f'restore failed: {path}')


def test(name=None):
    args = [sys.executable, str(ROOT / 'scripts' / 'guarded.py'), 'cargo', 'test', '--locked', '-p', 'yebisu']
    if name:
        args += ['--', '--exact', name]
    return subprocess.run(args, cwd=ROOT, text=True, capture_output=True)


def host(only):
    for label, edits, _ in MUTATIONS:
        for relative, before, _ in edits:
            if (ROOT / relative).read_text().count(before) != 1:
                sys.exit(f'{label}: anchor not unique in {relative}: {before!r}')
    failures = 0
    for label, edits, name in MUTATIONS:
        if only and label not in only:
            continue
        originals = apply(edits)
        try:
            result = test(name)
        finally:
            restore(originals)
        output = result.stdout + result.stderr
        failed = result.returncode != 0 and f'{name} ... FAILED' in output
        broke = result.returncode != 0 and 'error[' in output
        verdict = 'FAILED AS REQUIRED' if failed else ('DID NOT COMPILE' if broke else 'DID NOT FAIL')
        print(f'SEARCH_MUTATION {label}: {name} {verdict}', flush=True)
        failures += not failed
    restored = test()
    tail = (restored.stdout + restored.stderr).strip().splitlines()[-3:]
    print('SEARCH_RESTORED', 'PASS' if restored.returncode == 0 else 'FAIL', *tail, flush=True)
    sys.exit(1 if failures or restored.returncode else 0)


if __name__ == '__main__':
    if sys.argv[1:2] == ['host']:
        host(set(sys.argv[2:]))
    else:
        sys.exit(__doc__)
