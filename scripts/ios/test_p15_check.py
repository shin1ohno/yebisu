import copy
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("p15", Path(__file__).with_name("p15-check.py"))
p15 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p15)


class ThemeOracle(unittest.TestCase):
    def fixture(self):
        def paint(theme="high-contrast", modal=False):
            return dict(kind="terminal", theme=theme, settings_modal=modal)
        records = [dict(kind="settings-loaded", theme="owner-default"),
                   paint("owner-default"), dict(kind="theme-selected", theme="high-contrast"),
                   paint(modal=True), dict(kind="theme-selected", theme="owner-default"),
                   paint("owner-default", modal=True),
                   dict(kind="theme-selected", theme="high-contrast"),
                   paint(modal=True), paint(),
                   dict(kind="settings-lifecycle", phase="Background"),
                   dict(kind="settings-lifecycle", phase="Active"), paint(),
                   dict(kind="settings-loaded", theme="high-contrast"), paint()]
        return records, dict(copy.deepcopy(p15.SEED), theme="high-contrast")

    def test_primary_receiving_shape(self):
        p15.validate(*self.fixture())

    def test_wrong_or_missing_saved_field(self):
        for field in p15.SEED:
            records, disk = self.fixture()
            disk.pop(field)
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "exact saved"):
                p15.validate(records, disk)
        records, disk = self.fixture()
        disk["theme"] = "owner-default"
        with self.assertRaisesRegex(ValueError, "exact saved"):
            p15.validate(records, disk)
        disk["theme"] = "high-contrast"
        disk["notifications"]["volume"] = 38
        with self.assertRaisesRegex(ValueError, "exact saved"):
            p15.validate(records, disk)

    def test_missing_real_click_lifecycle_load_and_paints(self):
        for index, reason in [(0, "two real"), (2, "ordered real"),
                              (3, "immediate modal"), (8, "closed-sheet"),
                              (9, "background/return"), (10, "background/return"),
                              (11, "background return"), (12, "two real"),
                              (13, "relaunched painter")]:
            records, disk = self.fixture()
            records.pop(index)
            with self.subTest(index=index), self.assertRaisesRegex(ValueError, reason):
                p15.validate(records, disk)

    def test_stale_theme_paint_is_not_success(self):
        records, disk = self.fixture()
        records[3]["theme"] = "owner-default"
        with self.assertRaisesRegex(ValueError, "immediate modal"):
            p15.validate(records, disk)


class InstalledUIWireOracle(unittest.TestCase):
    def fixture(self):
        ui = [dict(kind="theme-click", label=label) for label in
              ["High contrast", "Owner default", "High contrast"]]
        records = []
        for index, (stage, host, stale) in enumerate([
                ("alpha", "qa-alpha", "qa-beta"), ("beta", "qa-beta", "qa-alpha"),
                ("relaunch-host", "qa-alpha", "qa-beta")]):
            ui.append(dict(kind="host-screen", stage=stage, host=host, stale=stale,
                           time=index + 1.5, lines=["Host: " + host, "File path unavailable",
                           "Unavailable", "File settings only; daemon-effective state unverified"]))
            records.append(dict(kind="terminal", time=index + 1, host=host, settings_modal=True))
        messages = [{"ClientShellPaneInput": {"pane_id": "w1:p1", "events": [
                    {"TextCommit": "P15-start"}, {"TextCommit": "P15-end"}]}}]
        return records, messages, ui

    def test_installed_receiving_shape(self):
        p15.validate_ui_wire(*self.fixture())

    def test_stale_host_negative(self):
        records, messages, ui = self.fixture()
        ui[4]["lines"][0] = "Host: qa-alpha"
        with self.assertRaisesRegex(ValueError, "stale host"):
            p15.validate_ui_wire(records, messages, ui)
        records, messages, ui = self.fixture()
        records[1]["host"] = "qa-alpha"
        with self.assertRaisesRegex(ValueError, "current renderer"):
            p15.validate_ui_wire(records, messages, ui)

    def test_wire_leak_and_missing_positive_negative(self):
        for events in [[{"TextCommit": "P15-startLEAKP15-end"}], [],
                       [{"Paste": "P15-startP15-end"}],
                       [{"TextCommit": "P15-endP15-start"}]]:
            records, messages, ui = self.fixture()
            messages[0]["ClientShellPaneInput"]["events"] = events
            with self.subTest(events=events), self.assertRaises(ValueError):
                p15.validate_ui_wire(records, messages, ui)
        records, messages, ui = self.fixture()
        messages.append({"ClientShellEndpointRequest": {"request": '{"method":"config.reload"}'}})
        with self.assertRaisesRegex(ValueError, "unexpected endpoint"):
            p15.validate_ui_wire(records, messages, ui)


class ProvenanceCaller(unittest.TestCase):
    def test_passes_p15_pin_profile_and_unique_run_to_real_api_shape(self):
        import tomllib
        import uuid
        from unittest.mock import Mock
        session = Mock()
        session.build_install.return_value = {'installed': '/owned/installed.app'}
        env = {'YEBISU_EXPECTED_HEAD': 'a' * 40}
        for _ in range(2):
            result = p15.build_owned(session, Path('/owned/run'), Path('/owned/source'),
                                     '11111111-2222-3333-4444-555555555555', env, 'a' * 40)
            self.assertEqual(result['installed'], '/owned/installed.app')
        first, second = session.build_install.call_args_list
        expected = first.kwargs['expected']
        manifest = tomllib.loads((p15.ROOT / 'Cargo.toml').read_text())
        pins = {p['name']: p['revision'] for p in expected['sdk']}
        for name in ('herdr-uikit', 'herdr-ssh'):
            self.assertEqual(pins[name], manifest['workspace']['dependencies'][name]['rev'])
        self.assertEqual(expected['profile'], dict(opt_level='0', debuginfo=0,
                         debug_assertions=True, overflow_checks=True, test=False))
        self.assertEqual(expected['target'], 'aarch64-apple-ios-sim')
        self.assertEqual(expected['features'], [])
        self.assertEqual(expected['head'], 'a' * 40)
        self.assertEqual(first.args[3], ['--package', 'yebisu', '--bin', 'yebisu'])
        self.assertIs(first.args[4], env)
        uuid.UUID(expected['run_id'])
        self.assertNotEqual(expected['run_id'], second.kwargs['expected']['run_id'])


if __name__ == "__main__":
    unittest.main()
