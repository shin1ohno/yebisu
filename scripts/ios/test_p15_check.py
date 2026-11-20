import copy
import importlib.util
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tempfile
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
        ui = [dict(kind="qa-match", id=p15.APPEARANCE_ID, count=1) for _ in range(3)]
        ui += [dict(kind="theme-click", label=label) for label in
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
        ui[7]["lines"][0] = "Host: qa-alpha"
        with self.assertRaisesRegex(ValueError, "stale host"):
            p15.validate_ui_wire(records, messages, ui)
        records, messages, ui = self.fixture()
        records[1]["host"] = "qa-alpha"
        with self.assertRaisesRegex(ValueError, "current renderer"):
            p15.validate_ui_wire(records, messages, ui)

    def test_wire_leak_and_missing_positive_negative(self):
        for events, reason in [
                ([{"TextCommit": "P15-startLEAKP15-end"}], "terminal wire leak"),
                ([{"TextCommit": "P15-start"}, {"TextCommit": "P15-end"}, {"TextCommit": "x"}],
                 "terminal wire leak"),
                ([], "positive control not delivered.*P15-start"),
                ([{"TextCommit": "P15-start"}], "positive control not delivered.*P15-end"),
                ([{"TextCommit": "P15-end"}], "positive control not delivered.*P15-start"),
                ([{"Paste": "P15-startP15-end"}], "unexpected terminal wire event"),
                ([{"TextCommit": "P15-endP15-start"}], "arrived out of order")]:
            records, messages, ui = self.fixture()
            messages[0]["ClientShellPaneInput"]["events"] = events
            with self.subTest(events=events), self.assertRaisesRegex(ValueError, reason):
                p15.validate_ui_wire(records, messages, ui)
        records, messages, ui = self.fixture()
        messages.append({"ClientShellEndpointRequest": {"request": '{"method":"config.reload"}'}})
        with self.assertRaisesRegex(ValueError, "unexpected endpoint"):
            p15.validate_ui_wire(records, messages, ui)

    # F1: the peer never received P15-start (the attempt 6 run: 55 ClientShellResize only)
    def test_f1_attempt_6_peer_without_text_commits_is_unreceived(self):
        records, messages, ui = self.fixture()
        messages = [{"ClientShellResize": {"surface_size": {"cols": 80, "rows": 24}}}] * 55
        with self.assertRaisesRegex(ValueError, "positive control not delivered.*P15-start"):
            p15.validate_ui_wire(records, messages, ui)

    # F2: both controls arrived, then something else leaked
    def test_f2_extra_text_after_the_controls_is_a_leak_not_an_absence(self):
        records, messages, ui = self.fixture()
        messages[0]["ClientShellPaneInput"]["events"].append({"TextCommit": "leak"})
        with self.assertRaisesRegex(ValueError, "terminal wire leak"):
            p15.validate_ui_wire(records, messages, ui)

    # F3: the app itself recorded that it dropped the input
    def test_launch_time_drop_before_the_first_destination_is_not_a_lost_control(self):
        records, messages, ui = self.fixture()
        records.insert(0, dict(kind="input-dropped", reason="not-connected"))
        records.insert(1, dict(kind="terminal", time=0, input_pane="w1:p1", host="qa-alpha", settings_modal=False))
        p15.validate_ui_wire(records, messages, ui)

    def test_f3_input_dropped_record_fails_with_its_reason(self):
        # The drop is inside the first typing window: after the first input paint, before
        # the first modal opens.
        for reason in ["no-destination", "not-connected", "incoherent", "fenced"]:
            records, messages, ui = self.fixture()
            records.insert(0, dict(kind="terminal", time=0, input_pane="w1:p1", host="qa-alpha", settings_modal=False))
            records.insert(1, dict(kind="input-dropped", reason=reason))
            with self.subTest(reason=reason), self.assertRaisesRegex(ValueError, "input dropped by the app.*" + reason):
                p15.validate_ui_wire(records, messages, ui)

    # N-d / P2-13(c): the Theme page must be marked exactly once on every press (S6: the
    # identifier replaced the count of OCR lines carrying its title)
    def test_theme_page_identifier_must_be_marked_exactly_once(self):
        for counts in ([0, 1, 1], [1, 2, 1], [1, 1, 3], [1, 1], [1, 1, 1, 1]):
            records, messages, ui = self.fixture()
            ui = [r for r in ui if r.get("kind") != "qa-match"]
            ui = [dict(kind="qa-match", id=p15.APPEARANCE_ID, count=n) for n in counts] + ui
            with self.subTest(counts=counts), self.assertRaisesRegex(ValueError, "marked exactly once"):
                p15.validate_ui_wire(records, messages, ui)
        records, messages, ui = self.fixture()
        ui = [r for r in ui if r.get("kind") != "qa-match"]
        with self.assertRaisesRegex(ValueError, "marked exactly once"):
            p15.validate_ui_wire(records, messages, ui)
        # The old OCR record no longer counts.
        records, messages, ui = self.fixture()
        ui = [dict(r, kind="ocr-match", label=p15.APPEARANCE_PAGE) if r.get("kind") == "qa-match" else r for r in ui]
        with self.assertRaisesRegex(ValueError, "marked exactly once"):
            p15.validate_ui_wire(records, messages, ui)


def paint(modal, host="qa-alpha", pane="w1:p1"):
    return dict(kind="terminal", input_pane=pane, host=host, settings_modal=modal)


def drop(reason="not-connected"):
    return dict(kind="input-dropped", reason=reason)


def lifecycle(phase):
    return dict(kind="settings-lifecycle", phase=phase)


def good_wire():
    return [{"ClientShellPaneInput": {"pane_id": "w1:p1", "events": [
        {"TextCommit": "P15-start"}, {"TextCommit": "P15-end"}]}}]


def first_launch():
    """First start: connecting drops, P15-start typed, modal 1, P15-end typed, host switch, modal 2."""
    return [dict(kind="launch"), drop(), drop(), paint(False),                   # 0-3: window 1 opens at 3
            paint(True), paint(False), paint(False),                             # 4: modal 1, 5-6: window 2
            paint(False, host="qa-beta"), paint(True, host="qa-beta"),           # 7: host switch (window 2 ends), 8: modal 2
            paint(False, host="qa-beta"), lifecycle("Background"), lifecycle("Active")]


def relaunch():
    return [dict(kind="launch"), drop(), drop(), paint(False), paint(True)]


class LaunchTimeDrops(unittest.TestCase):
    """S1: the app drops by itself while it connects; only drops in a typing window are lost controls."""

    def test_correct_run_with_a_connecting_drop_after_the_first_launch_is_accepted(self):
        p15.validate_fifo(first_launch(), good_wire())

    def test_correct_run_with_a_relaunch_drop_is_accepted(self):
        p15.validate_fifo(first_launch() + relaunch(), good_wire())

    def test_relaunch_drop_after_its_own_first_paint_is_not_typing_either(self):
        records = first_launch() + [dict(kind="launch"), paint(False), drop(), paint(True)]
        p15.validate_fifo(records, good_wire())

    def test_correct_run_with_a_background_return_drop_is_accepted(self):
        records = first_launch()
        records.insert(len(records), drop())  # reconnect after Active, nothing is typed
        p15.validate_fifo(records, good_wire())
        p15.validate_fifo(records + relaunch() + [drop()], good_wire())

    def test_drop_after_the_host_switch_is_not_typing(self):
        records = first_launch()
        records.insert(8, drop("no-destination"))  # between the host switch (7) and modal 2
        p15.validate_fifo(records, good_wire())

    def test_reviewer_replay_case_is_accepted(self):
        # replay-output.txt: "good wire, same drop after relaunch": REJECTED by 353790a.
        def mark(modal):
            return {"kind": "terminal", "input_pane": "w1:p1", "settings_modal": modal}
        first = [{"kind": "launch"}, drop(), mark(False), mark(True), mark(False), mark(True), mark(False)]
        again = [{"kind": "launch"}, drop(), mark(False)]
        p15.validate_fifo(first, good_wire())
        p15.validate_fifo(first + again, good_wire())

    def test_drop_in_the_first_typing_window_is_still_rejected(self):
        records = first_launch()
        records.insert(4, drop("fenced"))  # after the first input paint, before modal 1
        with self.assertRaisesRegex(ValueError, "input dropped by the app \\(fenced\\)"):
            p15.validate_fifo(records, good_wire())

    def test_drop_in_the_second_typing_window_is_still_rejected(self):
        records = first_launch()
        records.insert(6, drop("incoherent"))  # after modal 1 closed, before the host switch
        with self.assertRaisesRegex(ValueError, "input dropped by the app \\(incoherent\\)"):
            p15.validate_fifo(records, good_wire())

    def test_typing_drop_is_rejected_even_when_a_relaunch_drop_is_also_present(self):
        records = first_launch() + relaunch()
        records.insert(4, drop("no-destination"))
        with self.assertRaisesRegex(ValueError, "input dropped by the app \\(no-destination\\)"):
            p15.validate_fifo(records, good_wire())

    def test_lost_control_is_still_rejected_without_any_drop_record(self):
        wire = [{"ClientShellPaneInput": {"pane_id": "w1:p1", "events": [{"TextCommit": "P15-start"}]}}]
        with self.assertRaisesRegex(ValueError, "positive control not delivered.*P15-end"):
            p15.validate_fifo(first_launch() + relaunch(), wire)

    def test_watcher_gate_does_not_stop_a_relaunch_that_drops_while_connecting(self):
        # fifo_gate is what watch_fifo polls: a prefix of the run ending in the relaunch.
        for cut in range(len(first_launch()), len(first_launch() + relaunch()) + 1):
            p15.fifo_gate((first_launch() + relaunch())[:cut], good_wire())

    def test_typing_windows_follow_the_ui_sequence(self):
        self.assertEqual(p15.typing_windows(first_launch()), [(3, 4), (5, 7)])
        self.assertEqual(p15.typing_windows([dict(kind="launch"), drop()]), [])


class FifoStages(unittest.TestCase):
    """The gate that runs while XCTest is still going, so a missing control stops the run."""

    def paint(self, modal):
        return dict(kind="terminal", settings_modal=modal)

    def text(self, *values):
        return [{"ClientShellPaneInput": {"pane_id": "w1:p1", "events": [{"TextCommit": v} for v in values]}}]

    def test_nothing_is_required_before_the_first_modal(self):
        p15.fifo_gate([self.paint(False)], [])

    def test_start_is_required_when_the_first_modal_opens(self):
        with self.assertRaisesRegex(ValueError, "before the next phase: P15-start"):
            p15.fifo_gate([self.paint(False), self.paint(True)], [])
        p15.fifo_gate([self.paint(False), self.paint(True)], self.text("P15-start"))

    def test_end_is_required_when_the_second_modal_opens(self):
        records = [self.paint(True), self.paint(False), self.paint(True)]
        with self.assertRaisesRegex(ValueError, "before the next phase: P15-end"):
            p15.fifo_gate(records, self.text("P15-start"))
        p15.fifo_gate(records, self.text("P15-start", "P15-end"))

    def test_repaints_inside_one_modal_are_one_opening(self):
        self.assertEqual(p15.modal_openings([self.paint(True), self.paint(True), self.paint(False)]), 1)
        self.assertEqual(p15.modal_openings([self.paint(True), self.paint(False), self.paint(True)]), 2)

    def test_watcher_stops_the_run_after_the_grace_period(self):
        import tempfile
        from unittest.mock import Mock
        with tempfile.TemporaryDirectory() as directory:
            records = Path(directory) / "records.jsonl"
            records.write_text("\n".join(json.dumps(r) for r in [self.paint(False), self.paint(True)]))
            peer = Path(directory) / "peer.jsonl"
            process = Mock()
            process.poll.return_value = None
            ticks = iter(range(0, 100, 4))
            with self.assertRaisesRegex(ValueError, "stopped XCTest: positive control not delivered.*P15-start"):
                p15.watch_fifo(process, records, 0, peer, 0, grace=10, poll=0, clock=lambda: next(ticks))
            process.terminate.assert_called_once()
            # P15-start arrives within the grace: the run continues until it exits.
            peer.write_text(json.dumps(self.text("P15-start")[0]))
            process = Mock()
            process.poll.side_effect = [None, None, 0]
            self.assertIsNone(p15.watch_fifo(process, records, 0, peer, 0, grace=10, poll=0))
            process.terminate.assert_not_called()


def attempt_6_manifest():
    """Names exactly as xcresulttool exported them in attempt 6 (run-plain-2/attachments/manifest.json)."""
    names = ["p15-ui_0_34508657-C1CB-4FE1-BEA7-946CD8B9213F.json",
             "p15-relaunch-host_0_27EF29E6-D331-4446-93AC-13D91A88EB33.png",
             "p15-relaunch_0_D678CA94-B050-4606-B9EB-64498CBF3050.png",
             "p15-returned_0_5580E22B-5749-4876-B268-8366FB7DDAED.png",
             "p15-beta_0_00C4CF18-0822-49AC-A61F-258D6D8DDD97.png",
             "p15-closed_0_AA81734B-C5F1-411B-A8DA-D0C0BFB24A9F.png",
             "p15-selected_0_40F3A807-FA7D-410D-B153-B96040E63D80.png",
             "p15-owner-reset_0_8C64A8B7-9F5C-4119-9BCC-6F4B3577DB30.png",
             "p15-high-first_0_25CF1F30-90EA-48FB-832F-357E912C0CE8.png",
             "p15-alpha_0_7198C852-03DE-42D8-897F-DC50D1BE34D9.png",
             "p15-default_0_9372F17F-2F39-4C5E-A592-C0AA07CC40BE.png"]
    return [dict(testIdentifier="SmokeQA/testP15Theme()", attachments=[
        dict(suggestedHumanReadableName=name, exportedFileName="exported-%02d" % index)
        for index, name in enumerate(names)])]


class AttachmentNames(unittest.TestCase):
    # A1: the real export shape is accepted (the old prefix match on `p15-ui.json` found none)
    def test_a1_attempt_6_export_shape_passes(self):
        found = p15.attachment_files(attempt_6_manifest(), "/exported")
        self.assertEqual(found, Path("/exported/exported-00"))

    # A2: two UI transcripts are ambiguous
    def test_a2_two_transcripts_fail(self):
        manifest = attempt_6_manifest()
        manifest[0]["attachments"].append(dict(
            suggestedHumanReadableName="p15-ui_1_11111111-2222-3333-4444-555555555555.json", exportedFileName="x"))
        with self.assertRaisesRegex(ValueError, "duplicate attachment: p15-ui.json"):
            p15.attachment_files(manifest, "/exported")

    # A3: `p15-relaunch-host` must not satisfy `p15-relaunch` (the old prefix match did)
    def test_a3_neighbouring_stage_name_is_not_a_match(self):
        manifest = attempt_6_manifest()
        manifest[0]["attachments"] = [a for a in manifest[0]["attachments"]
                                      if not a["suggestedHumanReadableName"].startswith("p15-relaunch_")]
        with self.assertRaisesRegex(ValueError, "missing native screenshot: relaunch"):
            p15.attachment_files(manifest, "/exported")

    # A4: a name without the index/UUID suffix is not an XCTest export
    def test_a4_names_without_a_uuid_fail(self):
        for name in ["p15-ui.json", "p15-ui_0_.json", "p15-ui_0_34508657.json", "p15-ui_x_34508657-C1CB-4FE1-BEA7-946CD8B9213F.json"]:
            manifest = attempt_6_manifest()
            manifest[0]["attachments"][0]["suggestedHumanReadableName"] = name
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "unexpected shape"):
                p15.attachment_files(manifest, "/exported")

    def test_wrong_extension_does_not_satisfy_a_stage_or_the_transcript(self):
        manifest = attempt_6_manifest()
        manifest[0]["attachments"][0]["suggestedHumanReadableName"] = "p15-ui_0_34508657-C1CB-4FE1-BEA7-946CD8B9213F.txt"
        with self.assertRaisesRegex(ValueError, "missing unique installed UI transcript"):
            p15.attachment_files(manifest, "/exported")
        manifest = attempt_6_manifest()
        manifest[0]["attachments"][4]["suggestedHumanReadableName"] = "p15-beta_0_00C4CF18-0822-49AC-A61F-258D6D8DDD97.jpg"
        with self.assertRaisesRegex(ValueError, "missing native screenshot: beta"):
            p15.attachment_files(manifest, "/exported")


class PageLabel(unittest.TestCase):
    ROOT = Path(__file__).resolve().parents[2]

    def sources(self):
        read = lambda path: (self.ROOT / path).read_text()
        tests = (self.ROOT / "scripts/ios/smoke-qa/Tests").glob("SmokeQA*.swift")
        return (read("crates/yebisu/src/settings_pages/sections.rs"), read("crates/yebisu/src/settings_sheet/view.rs"),
                "\n".join(path.read_text() for path in sorted(tests)))

    def test_checked_in_sources_agree(self):
        p15.check_page_label(*self.sources())

    # L1: SmokeQA goes back to a literal label instead of the identifier
    def test_l1_literal_label_fails(self):
        pages, view, swift = self.sources()
        changed = swift.replace("qa.press(QAIds.settingsPageTheme", 'pressLabel("Theme"', 1)
        self.assertNotEqual(changed, swift)
        with self.assertRaisesRegex(ValueError, "literal Appearance/Theme label|exactly three times"):
            p15.check_page_label(pages, view, changed)

    def test_l1_press_count_fails(self):
        pages, view, swift = self.sources()
        with self.assertRaisesRegex(ValueError, "exactly three times"):
            p15.check_page_label(pages, view, swift.replace("try tapAppearance(qa", "try other(qa", 1))

    # S6: the Theme section button loses its mark
    def test_unmarked_theme_button_fails(self):
        pages, view, swift = self.sources()
        changed = view.replace("Section::Theme => crate::qa_ax::mark(ids::SETTINGS_PAGE_THEME, title)", "Section::Theme => None")
        self.assertNotEqual(changed, view)
        with self.assertRaisesRegex(ValueError, "not marked SETTINGS_PAGE_THEME"):
            p15.check_page_label(pages, changed, swift)

    # L2: the section title changes in Rust
    def test_l2_page_id_change_fails(self):
        pages, view, swift = self.sources()
        with self.assertRaisesRegex(ValueError, "APPEARANCE_PAGE must equal the section title"):
            p15.check_page_label(pages.replace('Self::Theme => "Theme"', 'Self::Theme => "Themes"'), view, swift)

    # L3: the button stops showing section.title()
    def test_l3_button_label_change_fails(self):
        pages, view, swift = self.sources()
        changed = view.replace('section.title(),', 'page.id(),')
        self.assertNotEqual(changed, view)
        with self.assertRaisesRegex(ValueError, "no longer shows section.title"):
            p15.check_page_label(pages, changed, swift)

    def test_missing_page_id_fails(self):
        pages, view, swift = self.sources()
        with self.assertRaisesRegex(ValueError, "not found exactly once"):
            p15.check_page_label("", view, swift)


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


class MainAdmission(unittest.TestCase):
    """Exercise the real entry point, including writes before the session API."""

    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.base = Path(temporary.name).resolve()
        self.own = self.base / 'owned'
        self.source = self.own / 'source'
        scripts = self.source / 'scripts/ios'
        scripts.mkdir(parents=True)
        for name in ('p15-check.py', 'p11-check.py', 'provenance_session.py'):
            shutil.copy2(Path(__file__).with_name(name), scripts / name)
        shutil.copytree(Path(__file__).with_name('smoke-qa'), scripts / 'smoke-qa',
                        ignore=shutil.ignore_patterns('*.xcodeproj', '__pycache__'))
        self.git('init', '-q')
        self.git('add', '.')
        self.git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
                 '-c', 'commit.gpgsign=false', '-c', 'core.hooksPath=/dev/null',
                 'commit', '-qm', 'fixture: capture exact caller scripts')
        self.head = self.git('rev-parse', 'HEAD').strip()
        self.outside = self.base / 'outside'
        self.outside.mkdir()
        (self.outside / 'sentinel').write_bytes(b'preserve outside bytes\x00')
        (self.outside / 'link').symlink_to('sentinel')
        self.calls = self.base / 'native-calls.jsonl'
        self.calls.write_text('')
        tools = self.base / 'tools'
        tools.mkdir()
        for name in ('bash', 'cargo', 'rustc', 'xcrun', 'xcodebuild', 'xcodegen', 'codesign', 'security'):
            tool = tools / name
            tool.write_text(f'#!{sys.executable}\nimport json, os, sys\n'
                            'with open(os.environ["TEST_NATIVE_CALLS"], "a") as out:\n'
                            '    out.write(json.dumps(sys.argv) + "\\n")\n'
                            'sys.exit("TEST_NATIVE_REJECTED")\n')
            tool.chmod(0o755)
        self.env = dict(os.environ, PATH=str(tools) + os.pathsep + os.environ['PATH'],
                        TEST_NATIVE_CALLS=str(self.calls), GIT_OPTIONAL_LOCKS='0')
        self.arguments = ['--own-root', str(self.own), '--expected-head', self.head,
                          '--simulator', '11111111-2222-3333-4444-555555555555']

    def git(self, *args):
        return subprocess.check_output(['git', '-C', str(self.source), *args], text=True)

    def inventory(self, root):
        result = {}

        def visit(path):
            info = path.lstat()
            entry = dict(mode=stat.S_IMODE(info.st_mode), uid=info.st_uid)
            if stat.S_ISLNK(info.st_mode):
                entry.update(type='link', target=os.readlink(path))
            elif stat.S_ISREG(info.st_mode):
                entry.update(type='file', sha256=hashlib.sha256(path.read_bytes()).hexdigest())
            elif stat.S_ISDIR(info.st_mode):
                entry.update(type='directory')
                for child in sorted(path.iterdir()):
                    visit(child)
            else:
                entry.update(type='special')
            result[str(path.relative_to(root))] = entry

        visit(root)
        return result

    def invoke(self, reason=None):
        before = {name: self.inventory(path) for name, path in
                  [('owned', self.own), ('outside', self.outside)]}
        result = subprocess.run([sys.executable, '-I', '-B',
                                 str(self.source / 'scripts/ios/p15-check.py'), *self.arguments],
                                cwd=self.source, env=self.env, text=True, capture_output=True, timeout=15)
        after = {name: self.inventory(path) for name, path in
                 [('owned', self.own), ('outside', self.outside)]}
        calls = self.calls.read_text()
        if destination := os.environ.get('YEBISU_HELPER_WITNESS_DIR'):
            (Path(destination) / (self._testMethodName + '.json')).write_text(json.dumps(dict(
                head=self.head, argv=self.arguments, returncode=result.returncode,
                stdout=result.stdout, stderr=result.stderr, native_calls=calls,
                before=before, after=after), indent=2))
        self.assertNotEqual(result.returncode, 0)
        if reason:
            self.assertIn(reason, result.stderr)
            self.assertEqual(before, after)
            self.assertEqual(calls, '')
        else:
            self.assertIn('TEST_NATIVE_REJECTED', result.stdout + result.stderr)
            recorded = [json.loads(line) for line in calls.splitlines()]
            self.assertEqual(len(recorded), 1)
            self.assertEqual(Path(recorded[0][0]).name, 'bash')
            runs = list((self.own / 'evidence').iterdir())
            self.assertEqual(len(runs), 1)
            for name in ('provenance', 'cargo-target', 'smoke-qa'):
                self.assertTrue((runs[0] / name).is_dir())
            self.assertEqual(before['outside'], after['outside'])

    def test_main_rejects_evidence_symlink_before_writes(self):
        (self.own / 'evidence').symlink_to(self.outside, target_is_directory=True)
        self.invoke('requires canonical absolute path without symlinks')

    def test_main_rejects_dangling_evidence_symlink_before_writes(self):
        (self.own / 'evidence').symlink_to(self.outside / 'absent')
        self.invoke('requires canonical absolute path without symlinks')

    def test_main_rejects_own_alias_before_writes(self):
        alias = self.base / 'alias'
        alias.symlink_to(self.own, target_is_directory=True)
        self.arguments[1] = str(alias)
        self.invoke('requires canonical absolute path without symlinks')

    def test_main_rejects_wrong_head_before_writes(self):
        self.arguments[3] = '0' * 40
        self.invoke('unexpected source checkout/head')

    def test_main_rejects_dirty_source_before_writes(self):
        with (self.source / 'scripts/ios/p11-check.py').open('a') as out:
            out.write('\n# dirty fixture\n')
        self.invoke('tracked source is dirty')

    def test_main_rejects_non_directory_evidence_before_writes(self):
        (self.own / 'evidence').write_text('preserve')
        self.invoke('ownership paths must be directories')

    def test_main_rejects_missing_required_argument_before_writes(self):
        self.arguments = self.arguments[:-2]
        self.invoke('owned source, full expected head and leased simulator UUID required')

    def test_main_rejects_invalid_head_before_writes(self):
        self.arguments[3] = 'g' * 40
        self.invoke('full source head required')

    def test_main_rejects_invalid_simulator_before_writes(self):
        self.arguments[-1] = 'not-a-uuid'
        self.invoke('badly formed hexadecimal UUID string')

    def test_main_canonical_root_reaches_only_rejecting_native_stub(self):
        self.invoke()

    def test_main_existing_evidence_reaches_only_rejecting_native_stub(self):
        (self.own / 'evidence').mkdir()
        self.invoke()


if __name__ == "__main__":
    unittest.main()
