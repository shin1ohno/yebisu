"""p12-check.py hands P12QA its inputs; P12QA.swift and p12-oracle.py must agree on their names."""
import importlib.util
from pathlib import Path
import re
import unittest

HERE = Path(__file__).parent


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, HERE / filename)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


check = load("p12_check", "p12-check.py")
oracle = load("p12_oracle", "p12-oracle.py")
SWIFT = (HERE / "p12-qa/Tests/P12QA.swift").read_text()


def code(text):
    """The lines that are not comments."""
    return "\n".join(line for line in text.splitlines() if line.strip() and not line.strip().startswith("//"))


def body(source, signature):
    """The brace-balanced body of the first function starting with signature."""
    start = source.index(signature)
    open_at = source.index("{", source.index(")", start))
    depth = 0
    for at in range(open_at, len(source)):
        depth += {"{": 1, "}": -1}.get(source[at], 0)
        if depth == 0:
            return source[open_at : at + 1]
    raise AssertionError("unbalanced function body: " + signature)


class P12CheckEnvTests(unittest.TestCase):
    def test_no_documents_and_no_fault_by_default(self):
        # S5: P12QA waits on qa.state, so the runner no longer gets the app's Documents directory.
        env = check.xctest_env()
        self.assertNotIn("TEST_RUNNER_P12_DOCUMENTS", env)
        self.assertNotIn("TEST_RUNNER_P12_QA_FAULT", env)

    def test_fault_reaches_the_runner_only_when_asked_for(self):
        env = check.xctest_env("drop-tap:remote")
        self.assertEqual(env["TEST_RUNNER_P12_QA_FAULT"], "drop-tap:remote")

    def test_swift_reads_the_names_the_prefix_strips_to(self):
        # xcodebuild strips TEST_RUNNER_, so the test reads the bare names.
        self.assertNotIn('environment["P12_DOCUMENTS"]', SWIFT)
        self.assertIn('environment["P12_QA_FAULT"]', SWIFT)
        self.assertNotIn('environment["TEST_RUNNER_', SWIFT)

    def test_the_qa_layer_helper_is_built_into_the_p12_bundle(self):
        driver = (HERE / "p12-check.py").read_text()
        self.assertIn('shutil.copyfile(ROOT / "scripts/ios/smoke-qa/Tests/QAAx.swift", project / "Tests/QAAx.swift")', driver)
        self.assertLess(driver.index("QAAx.swift"), driver.index('run("xcodegen"'))
        self.assertNotIn("fifo_fence", driver.split("def main():")[1], "no live poll of the peer file")


class P12QASourceTests(unittest.TestCase):
    """P12QA.swift cannot run off a Mac; these pin the parts the oracle relies on."""

    def test_fixture_controls_are_the_oracle_steps_with_their_identifiers(self):
        table = re.search(r"fixtureControls: \[\(step: String, id: String\)\] = \[(.*?)\n    \]", SWIFT, re.S).group(1)
        pairs = re.findall(r'\("(\w+)", "(qa\.p12\.\w+)"\)', table)
        self.assertEqual([step for step, _ in pairs], ["remote", "rearm", "sidebar", "palette", "release", "host", "pane",
                                                       "boot", "focus", "current", "capability", "reset", "finish"])
        self.assertEqual({step for step, _ in pairs}, set(oracle.STEPS))
        self.assertTrue(all(identifier == "qa.p12." + step for step, identifier in pairs))
        ids = (HERE.parents[1] / "crates/yebisu/src/qa_ax_ids.rs").read_text()
        for _, identifier in pairs + [("empty", "qa.p12.empty")]:
            self.assertIn('= "%s";' % identifier, ids)

    def test_no_ocr_no_file_reads_and_no_fixed_window_point(self):
        swift = code(SWIFT)
        for gone in ("import Vision", "VNRecognizeTextRequest", "recognize(", "fixturePoints", "P12_DOCUMENTS",
                     "p0-smoke.jsonl", "Data(contentsOf:", "dy: 0.85", "menuRight"):
            with self.subTest(gone=gone):
                self.assertNotIn(gone, swift)
        self.assertIn('app.launchEnvironment["YEBISU_QA_AX"] = "1"', swift)

    def test_each_fixture_press_is_confirmed_by_qa_state(self):
        fixture = body(code(SWIFT), "private func fixture(")
        self.assertIn("qa.press(id, timeout: Self.receiptTimeout", fixture)
        self.assertIn("self.setupEnds($0, step) > ended", fixture)
        self.assertIn('$0.peerPastes.contains("p12-fence")', fixture)
        self.assertIn("qa.session.dropFirstPress.insert(id)", fixture)
        self.assertIn("qa.session.retries.dropFirst(retries)", fixture)
        idle = body(code(SWIFT), "private func idle(")
        self.assertIn("self.pending($0) == false", idle)
        self.assertIn("Missing fixture control/state: P12 Idle", idle)

    def test_presses_inside_a_hold_query_the_app_less(self):
        """S6 (S5 follow-up S6-1): the local and release presses of a hold use frames found before
        the hold and sparse qa.state reads, and the reads are recorded per hold."""
        swift = code(SWIFT)
        test = body(swift, "func testInstalledTerminalMenu(")
        # PR 84: the frames are read once, by locateFixtures, not right before the Remote case.
        self.assertNotIn("recordHeldFrames", swift)
        self.assertIn("heldFrames[control.id] = try qa.find(control.id", body(swift, "private func locateFixtures("))
        self.assertLess(test.index("try locateFixtures()"), test.index("try fixture(remote)"))
        self.assertIn("try fixture(local, held: true)", test)
        self.assertIn('try fixture("release", held: true)', test)
        self.assertIn('"kind": "hold-queries"', test)
        held = body(swift, "private func heldPress(")
        for gone in ("qa.press(", "qa.find(", "waitState("):
            self.assertNotIn(gone, held)
        self.assertIn("usleep(Self.heldReadInterval)", held)
        self.assertIn("PressVerdict.of(before: before, after: last, id: id)", held)
        self.assertIn("qa.session.dropFirstPress.remove(id)", held)
        self.assertIn("qa.session.retries.append(", held)
        self.assertIn("heldReadInterval: useconds_t = 250_000", swift)
        # Outside a hold every fixture still goes through QAAx.press.
        self.assertIn("qa.press(id, timeout: Self.receiptTimeout", body(swift, "private func fixture("))

    def test_held_receipt_budget_excludes_synchronous_tap_and_checks_first_read(self):
        # Run 44704476837 spent more than two receipt windows in the first tap,
        # which skipped both confirmation loops even though the controls ran.
        held = body(code(SWIFT), "private func heldPress(")
        tap = held.index(".tap()")
        window = held.index("let receiptStarted = ProcessInfo.processInfo.systemUptime")
        read = held.index("qa.read()")
        deadline = held.index("ProcessInfo.processInfo.systemUptime - receiptStarted >= Self.receiptTimeout")
        self.assertLess(tap, window)
        self.assertLess(window, read)
        self.assertLess(read, deadline)
        self.assertLess(deadline, held.index("usleep(Self.heldReadInterval)"))

    def test_menu_waits_for_an_actionable_springboard_button_before_picking(self):
        swift = code(SWIFT)
        choose = body(swift, "private func choose(")
        opened = choose.index("applicationMenu.tap()")
        self.assertIn("waitForEnabledMenuItem(item)", choose)
        ready = choose.index("waitForEnabledMenuItem(item)")
        hittable = choose.index("available && item.isHittable")
        self.assertNotIn("sleep(", choose[opened:choose.index("if observeOnly {")])
        self.assertLess(opened, ready)
        self.assertLess(ready, hittable)
        self.assertLess(hittable, choose.index("XCTAssertTrue(hittable,"))
        self.assertLess(choose.index("XCTAssertTrue(hittable,"), choose.index("item.click()"))
        availability = body(swift, "private func waitForEnabledMenuItem(")
        self.assertLess(availability.index("if item.exists && item.isEnabled"), availability.index("XCTWaiter().wait("))
        self.assertIn("exists == true AND enabled == true", availability)
        self.assertNotIn("hittable ==", availability)
        hierarchy = body(swift, "private func springboardHierarchy(")
        self.assertIn("springboard.debugDescription", hierarchy)
        self.assertNotIn("app.debugDescription", hierarchy)

    def test_native_menu_clicks_use_pointer_and_fixture_presses_still_use_touch(self):
        swift = code(SWIFT)
        choose = body(swift, "private func choose(")
        self.assertEqual(choose.count(".tap()"), 1)
        self.assertIn("applicationMenu.tap()", choose)
        self.assertIn("point.click()", choose)
        self.assertIn("item.click()", choose)
        self.assertIn("try dismissPoint(clearOf: item, caseName: caseName).click()", choose)
        self.assertIn("XCUIDevice.shared.supportsPointerInteraction", body(swift, "func testInstalledTerminalMenu("))
        self.assertIn("withOffset(point).tap()", body(swift, "private func heldPress("))

    def test_palette_moves_the_pointer_before_validating_and_clicking_the_row(self):
        choose = body(code(SWIFT), "private func choose(")
        hover = choose.index('if caseName == "palette" { item.hover() }')
        self.assertLess(choose.index("XCTAssertTrue(hittable,"), hover)
        self.assertLess(hover, choose.index("let tapped = enabled && item.isHittable"))
        self.assertLess(hover, choose.index("let pickStart ="))
        self.assertEqual(choose.count("item.click()"), 1)

    def test_focus_waits_for_mutation_before_opening_the_stale_menu(self):
        swift = code(SWIFT)
        test = body(swift, "func testInstalledTerminalMenu(")
        setup = test.index("try fixture(step)")
        wait = test.index("try qa.waitState(\"P12 focus changed", setup)
        mutation = test.index('self.p12(state)?["mutated"] as? Bool == true', wait)
        pick = test.index('try choose("Split Right", caseName: step, negative: true)', mutation)
        self.assertLess(pick, test.index('try fixture("current")', pick))
        choose = body(swift, "private func choose(")
        self.assertIn('if negative && caseName != "focus" { sleep(7) }', choose)

    def test_the_app_marks_each_fixture_and_counts_its_setup_end(self):
        rust = (HERE.parents[1] / "crates/yebisu/src/native_menus_qa.rs").read_text()
        table = re.search(r"const FIXTURES: \[\(&str, &str\); 13\] = \[(.*?)\];", rust, re.S).group(1)
        rows = re.findall(r'\("P12 (\w+)", "(\w+)"\)', table)
        self.assertEqual([step for _, step in rows], re.findall(r'\("(\w+)", "qa\.p12\.\w+"\)', SWIFT),
                         "the panel order is the test's order")
        self.assertTrue(all(label.lower() == step for label, step in rows))
        for _, step in rows:
            with self.subTest(step=step):
                self.assertIn('"%s" => crate::qa_ax::mark(ids::P12_%s, label),' % (step, step.upper()), rust)
        self.assertIn(".children(fixture_mark(case, label))", rust)
        self.assertIn("crate::qa_ax::mark(crate::qa_ax::ids::P12_EMPTY, EMPTY)", rust)
        # p12.setup counts exactly the p12-setup-end records, right after the record is written.
        end = rust.index('"kind":"p12-setup-end"')
        self.assertLess(end, rust.index("*self.p12_qa.setup_ends.entry(case).or_default() += 1;"))
        self.assertEqual(rust.count("setup_ends.entry("), 1)
        self.assertIn('crate::qa_ax::report("p12"', rust)
        self.assertIn('"pending":pending', rust)
        self.assertIn('if pending { "P12 Pending" } else { "P12 Idle" }', rust)

    def test_the_peer_reports_only_the_fence_to_qa_state(self):
        peer = (HERE.parents[1] / "crates/yebisu/src/mock_peer.rs").read_text()
        branch = peer[peer.index("if p12_observing() {"):peer.index('return record_peer(')]
        self.assertIn("crate::qa_ax::peer::input(", branch)
        self.assertIn('ClientPaneInputEvent::Paste(text) if text == "p12-fence" => Some(text.as_str())', branch)

    def test_a_touch_reaches_gpui_before_the_first_fixture_press(self):
        swift = code(SWIFT)
        test = body(swift, "func testInstalledTerminalMenu(")
        self.assertLess(test.index("try touchReady()"), test.index("try fixture(remote)"))
        ready = body(swift, "private func touchReady(")
        self.assertIn("$0.presses(Self.emptySpace) > before.presses(Self.emptySpace)", ready)
        self.assertIn('"kind": "touch-ready", "ready": true', ready)
        self.assertIn("XCTFail(", ready)
        self.assertIn("touchReadyPresses = 8", swift)

    def test_the_dismiss_point_is_the_empty_strip_found_by_identifier(self):
        point = body(code(SWIFT), "private func dismissPoint(")
        self.assertIn("qa.find(Self.emptySpace", point)
        self.assertIn("empty.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))", point)
        self.assertIn("XCTAssertTrue(clear,", point)
        self.assertIn('emptySpace = "qa.p12.empty"', SWIFT)

    def test_the_reveal_and_receipt_waits_are_not_shortened(self):
        self.assertIn("revealTimeout: TimeInterval = 10", SWIFT)
        self.assertIn("receiptTimeout: TimeInterval = 5", SWIFT)

    def test_a_reveal_is_repeated_once_and_the_assertion_after_it_is_kept(self):
        self.assertIn("if reveal == .completed || attempt == 2", SWIFT)
        self.assertIn("XCTAssertEqual(reveal, .completed", SWIFT)
        self.assertEqual(SWIFT.count('"kind": "reveal-retry"'), 1)
        self.assertEqual(SWIFT.count('"kind": "reveal-result"'), 1)

    def test_a_fixture_tap_is_repeated_once_and_must_then_be_received(self):
        self.assertEqual(SWIFT.count('"kind": "fixture-retry"'), 1)
        self.assertIn("XCTAssertTrue(received,", SWIFT)
        # QAAx.press presses once more only when the first press reached nothing, and logs why.
        qaax = (HERE / "smoke-qa/Tests/QAAx.swift").read_text()
        self.assertIn("for attempt in 1...2 {", qaax)
        self.assertIn('session.retries.append("\\(id): \\(reason)")', qaax)
        self.assertIn("session.dropFirstPress.remove(id) != nil", qaax)

    def test_events_survive_a_failed_assertion(self):
        # A failed assertion skips `defer`; the events are attached in tearDown.
        self.assertIn("override func tearDown()", SWIFT)
        self.assertNotIn("defer {", SWIFT)


if __name__ == "__main__":
    unittest.main()
