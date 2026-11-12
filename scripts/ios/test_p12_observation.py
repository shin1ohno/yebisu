"""Source-bound QA contracts; these do not execute Rust or UIKit."""
from pathlib import Path
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[2]
BASE = "8d3db3c0126ae00e8ae8ec2a56e1b875f0e9c8e0"
FILES = ["native_menus.rs", "native_menus_qa.rs", "terminal_controls.rs", "mock_peer.rs"]


def source_errors(src):
    menu, qa, keys, peer = (src[p] for p in FILES)
    errors = []
    if 'p12_menu_begin(&action.name, action.revision, "native_action", window)' not in menu or 'self.p12_menu_end(Some(receipt), result.is_ok(), "native_menus.take")' not in menu:
        errors.append("native callback receipt")
    if "p12_shortcut_entry(event, window)" not in keys or '"focus_scene"' not in keys or '"held_consumed"' not in keys or '"not_installed"' not in keys:
        errors.append("shortcut callback coverage")
    if 'p12_menu_begin(name, None, "shortcut_key_down", window)' not in qa:
        errors.append("no fabricated native identity")
    if '.occlude()' not in qa or 'MouseButton::Left' not in qa or 'cx.stop_propagation();\n                                this.p12_prepare' not in qa:
        errors.append("QA click isolation")
    if 'self.terminal.live = live;' in qa or 'saved: Option<(Live,' in qa:
        errors.append("no fake live restore")
    if '"p12-setup-begin"' not in qa or '"p12-setup-end"' not in qa or '"inbox_connected"' not in qa:
        errors.append("setup state observation")
    marker = 'record(&message, transport, sequence)?;'
    if marker not in peer or peer.index(marker) > peer.index('let _ = receipts.send(message.clone());'):
        errors.append("ingress before gate and response")
    if 'gate = None;' not in peer or '"sender_disconnected"' not in peer:
        errors.append("one-shot gate semantics")
    if '"p12-peer-exit"' not in peer or '"p12-peer-start"' not in peer:
        errors.append("peer exit and transport")
    if '"unexpected_input"' not in peer or '"request_method"' not in peer:
        errors.append("redacted P12 wire trace")
    return errors


class P12ObservationSourceTests(unittest.TestCase):
    def setUp(self):
        self.sources = {name: (ROOT / "crates/yebisu/src" / name).read_text() for name in FILES}

    def test_all_actual_callbacks_observed(self):
        self.assertEqual(source_errors(self.sources), [])
        menu, qa, keys, peer = (self.sources[p] for p in FILES)
        self.assertLess(keys.index('p12_shortcut_entry(event, window)'), keys.index('if !self.terminal_focus.is_focused(window)'))
        self.assertLess(menu.index('.take(action,'), menu.index('self.p12_menu_end(Some(receipt)'))
        self.assertLess(keys.index('let result = self.run_menu_shortcut(command);'), keys.index('result.is_ok()'))
        self.assertIn('"native_menus.take"', keys)
        for field in ('"origin"', '"captured_revision"', '"current_revision"', '"scene_active"', '"focused"', '"pending"'):
            self.assertIn(field, qa)

    def test_pre_fix_witness_detects_intended_observation_defects(self):
        old = {name: subprocess.check_output(["git", "-C", str(ROOT), "show", f"{BASE}:crates/yebisu/src/{name}"], text=True) for name in FILES}
        errors = source_errors(old)
        for reason in ("native callback receipt", "shortcut callback coverage", "QA click isolation", "no fake live restore", "ingress before gate and response", "one-shot gate semantics", "peer exit and transport"):
            self.assertIn(reason, errors)
        print("BEFORE_FIX_WITNESS:", "; ".join(errors))
        # The execution policy and stale-identity guard stay byte-identical.
        for start, end in [('    pub(crate) fn run_menu_shortcut', '    pub(crate) fn install_native_menus'),
                           ('    fn take(', '\n}\n\nimpl Smoke')]:
            self.assertEqual(old['native_menus.rs'].split(start)[1].split(end)[0], self.sources['native_menus.rs'].split(start)[1].split(end)[0])

    def test_each_observation_mutant_is_rejected(self):
        mutants = [
            ('native_menus.rs', 'self.p12_menu_end(Some(receipt)', 'self.discard(Some(receipt)', 'native callback receipt'),
            ('terminal_controls.rs', 'p12_shortcut_entry(event, window)', 'discard(event, window)', 'shortcut callback coverage'),
            ('native_menus_qa.rs', 'p12_menu_begin(name, None,', 'p12_menu_begin(name, Some(self.p12_revision()),', 'no fabricated native identity'),
            ('native_menus_qa.rs', '.occlude()', '', 'QA click isolation'),
            ('native_menus_qa.rs', '// Restore only the injected field.', 'self.terminal.live = live;\n// Restore only the injected field.', 'no fake live restore'),
            ('mock_peer.rs', 'record(&message, transport, sequence)?;', '', 'ingress before gate and response'),
            ('mock_peer.rs', 'gate = None;', '', 'one-shot gate semantics'),
            ('mock_peer.rs', '"p12-peer-exit"', '"discard"', 'peer exit and transport'),
        ]
        for file, before, after, reason in mutants:
            with self.subTest(reason=reason):
                self.assertIn(before, self.sources[file], 'positive control')
                mutant = dict(self.sources)
                mutant[file] = mutant[file].replace(before, after)
                self.assertIn(reason, source_errors(mutant))
                print('NEGATIVE_CONTROL:', reason)

    def test_native_runner_binds_compiler_and_installed_app(self):
        runner = (ROOT/'scripts/ios/p12-check.py').read_text()
        bundle = (ROOT/'scripts/ios/bundle.sh').read_text()
        ui = (ROOT/'scripts/ios/p12-qa/Tests/P12QA.swift').read_text()
        for token in ('"manifest_path"', '"src_path"', '"compiler-artifact"', '"installed-app"', '"dwarfdump"', '"TMPDIR"', '"xcode-derived"'):
            self.assertIn(token, runner)
        self.assertIn('${TMPDIR:-/tmp}/herdr-ios-bundle.XXXXXX', bundle)
        self.assertIn('set -C;', bundle)
        for token in ('"fixture-control"', '"os-menu-pick"', '"started"', '"ended"', 'item.tap()', 'let owner = springboard'):
            self.assertIn(token, ui)
        self.assertNotIn('run_action', ui)


if __name__ == '__main__':
    unittest.main()
