"""Synthetic observation fixtures; never installed-UI or hardware evidence."""
import copy
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("p12", Path(__file__).with_name("p12-oracle.py"))
p12 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p12)


def hierarchy_text(label, flags="", section="SPRINGBOARD"):
    """An exported p12-menu-*-hierarchy attachment in the format of a real run's, trimmed to its menu button."""
    return ("APP\n  Application, 0x1, pid: 1\n" + section + "\n  Application, 0x2, pid: 2\n"
            f"    Button, 0x3, {{{{324.5, 75.5}}, {{274.0, 38.0}}}}, label: '{label}\u3000\u2318D'{flags}\n").encode()


def hierarchy_shots(case, **kwargs):
    label = p12.LABELS.get(case, "Split Right")
    return p12.screenshot_names(f"p12-menu-{case}-hierarchy_0_UUID.txt", hierarchy_text(label, **kwargs))


def fixture():
    records, ui = [], []
    state = dict(original_boot=True, scene_active=True, focused=True, pending=False,
                 connected=True, inbox_connected=True, handle_present=True, generation=1,
                 original_host=True, original_pane=True, split_capability=True,
                 sidebar=False, palette=False)
    clock = 1

    def setup(step):
        nonlocal clock
        ui.append(dict(kind="fixture-control", step=step, started=clock, ended=clock+1))
        before = [state["sidebar"], state["palette"]]
        records.extend([dict(kind="p12-setup-begin", step=step, time=clock+.1, state=copy.deepcopy(state)),
                        dict(kind="p12-setup-end", step=step, time=clock+.2, state=copy.deepcopy(state), before=before, after=before)])
        clock += 2

    def choose(case, tap_time, tapped=True):
        """The GUI records one choose() writes: reveal precondition and result, menu close."""
        ui.append(dict(kind="reveal-precondition", case=case, time=tap_time-.5, windowControlsPresent=True,
                       appState="runningForeground", appWindowExists=True, appWindowFocused=True, secondsSinceMenuClosed=3.0))
        ui.append(dict(kind="reveal-result", case=case, revealed=True, waited=.4, time=tap_time-.2))
        ui.append(dict(kind="menu-close", case=case, step="pick" if tapped else "dismiss", closed=True, tapTime=tap_time,
                       waited=.3, time=tap_time+.4))

    messages = [dict(kind="p12-peer-start", transport=1, time=0)]
    remotes = dict(p12.HOLDS)
    sequence = 0
    for case in p12.CASES:
        setup(case)
        prepared = copy.deepcopy(state)
        name = {"sidebar":"toggle_sidebar", "palette":"command_palette"}.get(case, "split_right")
        label = p12.LABELS.get(case, "Split Right")
        records.append(dict(kind="p12-menu-build", source="prepare", case=case, time=clock-1.9, changed=True,
                            focused=True, availability=[True]))
        records.append(dict(kind="p12-prepare", case=case, time=clock-1.8, state=prepared,
                            sidebar=state["sidebar"], palette=state["palette"], pending=state["pending"],
                            catalog=[dict(name=name, label=label, enabled=True)]))
        negative = case in p12.NEGATIVE
        if negative:
            field = {"host":"original_host", "pane":"original_pane", "boot":"original_boot", "focus":"original_pane", "capability":"split_capability"}[case]
            state[field] = False
            records.append(dict(kind="p12-mutated", case=case, time=clock+.1, state=copy.deepcopy(state), valid_focus_target=True, current_split_enabled=True))
            clock += 2
        before = [state["sidebar"], state["palette"]]
        entry = dict(kind="p12-menu-entry", case=case, name=name, time=clock+.1,
                     origin="native_action",
                     state=copy.deepcopy(state), before=before, mutated=negative)
        records.append(entry)
        if case in remotes:
            state["pending"] = True
            sequence += 1
            records.append(dict(kind="action", method="pane.split", time=clock+.2))
            messages.append(dict(kind="p12-peer-ingress", transport=1, sequence=sequence,
                                 message=dict(request_method="pane.split"), time=clock+.3))
        elif case in ("sidebar", "palette"):
            state[case] = not state[case]
        records.append(dict(entry, kind="p12-menu-receiver", time=clock+.4, guard="command_gate",
                            accepted=not negative, pending=state["pending"], after=[state["sidebar"], state["palette"]]))
        ui.append(dict(kind="os-menu-pick", case=case, started=clock, ended=clock+1,
                       owner="com.apple.springboard", command=label, buttonLabel=label+"　⌘D", enabled=True, tapped=True,
                       menuClosed=True))
        choose(case, clock)
        clock += 2
        if case in remotes.values():
            setup("release")
            records.append(dict(kind="p12-release", released=True, time=clock-1.5))
            messages.append(dict(kind="p12-peer-gate", transport=1, sequence=sequence, reason="released", time=clock-1.5))
            state["pending"] = False
        if case == "focus":
            # The unfrozen menu follows the mutated pane: rows and focus are the same
            # as at prepare, so only the Scope differs. The hook records it regardless.
            records.append(dict(kind="p12-menu-build", source="rebuild", case=case, time=clock-1.7, changed=True,
                                rebuilds=2, build=dict(host="h", boot="b", pane="w1:p3", epoch=0, availability=[True], focused=True)))
            setup("current")
            records.append(dict(kind="p12-menu-build", source="current", case=case, time=clock-1.6, changed=True,
                                focused=True, availability=[True]))
            records.append(dict(kind="p12-current", case=case, time=clock-1.5, state=copy.deepcopy(state)))
            ui.append(dict(kind="current-native-menu", case=case, time=clock, enabled=True,
                           owner="com.apple.springboard", buttonLabel="Split Right　⌘D"))
            choose("focus-current", clock)
            clock += 2
        if negative:
            records.append(dict(kind="p12-reset", case=case, sidebar=state["sidebar"], palette=state["palette"], mutated=True))
            state = copy.deepcopy(prepared)
            setup("reset")
    setup("finish")
    records.append(dict(kind="p12-finish", enqueued=True, time=clock-1.5))
    messages.append(dict(kind="p12-peer-ingress", transport=1, sequence=sequence+1,
                         message=dict(input=["fifo_fence"]), time=clock-1.5))
    # No screenshot is taken while the OS menu is open (p12-revealed-*, p12-menu-<case>, p12-before-*):
    # the menu is proved by its Springboard hierarchy.
    shots = {"p12-finished", "p12-menu-focus-current", *[f"p12-after-{case}" for case in p12.CASES]}
    for case in [*p12.CASES, "focus-current"]:
        shots |= hierarchy_shots(case)
    return records, messages, ui, shots


def select(rows, kind, case=None):
    return next(r for r in rows if r.get("kind") == kind and (case is None or r.get("case") == case))


def insert_ingress(messages, time, message):
    """Insert wire before the final fence, keeping ingress sequences contiguous."""
    fence = messages[-1]
    messages.insert(-1, dict(kind="p12-peer-ingress", transport=1, sequence=fence["sequence"], time=time, message=message))
    fence["sequence"] += 1


class P12OracleTests(unittest.TestCase):
    def rejected(self, edit, reason):
        data = fixture()
        edit(*data)
        with self.assertRaisesRegex(ValueError, reason):
            p12.validate(*data)

    def test_synthetic_native_route_keeps_hardware_unverified(self):
        result = p12.validate(*fixture())
        self.assertEqual(result["command_hold"], "unverified")
        self.assertEqual(result["p11_p13_modal_integration"], "unverified")

    def test_no_gui_is_not_callback_proof(self):
        self.rejected(lambda r,m,u,s: u.clear(), "actual OS menu")

    def test_duplicate_ingress_request(self):
        def edit(r,m,u,s):
            insert_ingress(m, m[1]["time"], dict(m[1]["message"]))
        self.rejected(edit, "RPC not exactly once")

    def test_key_or_text_leakage(self):
        self.rejected(lambda r,m,u,s: m[-1]["message"]["input"].insert(0, "unexpected_input"), "finish emitted non-fence wire")

    def test_missing_final_fence(self):
        self.rejected(lambda r,m,u,s: m.pop(), "missing FIFO fence")

    def test_stale_guard_acceptance(self):
        self.rejected(lambda r,m,u,s: select(r,"p12-menu-receiver","pane").update(accepted=True), "stale consumer guard accepted")

    def test_duplicate_local_callbacks(self):
        def edit(r,m,u,s):
            for kind in ("p12-menu-entry", "p12-menu-receiver"):
                r.append(copy.deepcopy(select(r,kind,"sidebar")))
        self.rejected(edit, "not exactly once")

    def test_missing_receipt(self):
        self.rejected(lambda r,m,u,s: r.remove(select(r,"p12-menu-receiver","remote")), "cardinality mismatch")

    def test_real_screenshot_required(self):
        self.rejected(lambda r,m,u,s: s.remove("p12-after-focus"), "real screenshot missing")
        self.rejected(lambda r,m,u,s: s.clear(), "real screenshot missing")

    def test_menu_open_screenshots_are_not_required(self):
        """The evidence change: no PNG taken while the OS menu is open is needed, only the hierarchy."""
        _, _, _, shots = fixture()
        open_menu = {f"p12-{stage}-{case}" for stage in ("revealed", "before") for case in p12.CASES}
        open_menu |= {f"p12-menu-{case}" for case in p12.CASES}
        self.assertFalse(open_menu & shots)
        self.assertEqual(p12.validate(*fixture())["installed_os_menu"], "pass")

    def test_menu_hierarchy_item_is_required_for_every_case(self):
        for case in p12.CASES:
            self.rejected(lambda r,m,u,s: s.difference_update(hierarchy_shots(case)),
                          f"Springboard AX hierarchy item missing in case {case}")

    def test_positive_menu_item_must_be_enabled_in_the_hierarchy(self):
        for case in ("remote", "sidebar", "rearm", "palette"):
            self.rejected(lambda r,m,u,s: s.discard(f"p12-menu-{case}-hierarchy-enabled"),
                          f"native item disabled in the AX hierarchy in case {case}")

    def test_negative_menu_item_may_be_disabled_in_the_hierarchy(self):
        """A negative case's hierarchy is saved before its stale fault lands; its own disabled pick is checked elsewhere."""
        data = fixture()
        data[3].discard("p12-menu-capability-hierarchy-enabled")
        self.assertEqual(p12.validate(*data)["installed_os_menu"], "pass")

    def test_current_focus_hierarchy_item_is_required(self):
        self.rejected(lambda r,m,u,s: s.discard("p12-menu-focus-current-hierarchy-enabled"),
                      "current native menu AX hierarchy item missing")

    def test_hierarchy_item_must_be_the_native_command(self):
        # The menu hierarchy of another case's command is not evidence of this case.
        self.assertEqual(p12.screenshot_names("p12-menu-palette-hierarchy_0_U.txt", hierarchy_text("Toggle Sidebar")), set())
        self.assertEqual(p12.screenshot_names("p12-menu-focus-hierarchy_0_U.txt", hierarchy_text("Split Left")), set())
        self.assertEqual(hierarchy_shots("palette"), {"p12-menu-palette-hierarchy", "p12-menu-palette-hierarchy-enabled"})

    def test_hierarchy_item_must_be_in_the_springboard_section(self):
        # Present only in the app's own tree is not the OS menu.
        self.assertEqual(hierarchy_shots("focus", section="OTHER"), set())
        self.assertEqual(p12.screenshot_names("p12-menu-focus-hierarchy_0_U.txt", b"SPRINGBOARD\nno separator"), set())

    def test_disabled_item_lists_but_is_not_enabled(self):
        self.assertEqual(hierarchy_shots("sidebar", flags=", Disabled"), {"p12-menu-sidebar-hierarchy"})

    def test_hierarchy_name_is_not_evidence_for_an_image(self):
        self.assertEqual(p12.screenshot_names("p12-menu-sidebar-hierarchy_0_U.png", b"\x89PNG\r\n\x1a\n"), set())

    def test_unmutated_negative(self):
        self.rejected(lambda r,m,u,s: r.remove(select(r,"p12-mutated","host")), "stale mutation")

    def test_native_disabled_is_not_dispatch_proof(self):
        data = fixture()
        r,m,u,s = data
        for kind in ("p12-menu-entry", "p12-menu-receiver"):
            r.remove(select(r,kind,"capability"))
        select(u,"os-menu-pick","capability").update(tapped=False, enabled=False)
        self.assertEqual(p12.validate(*data)["negative_routes"]["capability"], "native_gui_disabled_no_consumer_dispatch")

    def test_focus_requires_existing_available_target(self):
        for field in ("valid_focus_target", "current_split_enabled"):
            self.rejected(lambda r,m,u,s: select(r,"p12-mutated","focus").update({field:False}),
                          "focus must move to an available existing pane")

    def test_xcresult_screenshot_names_include_current_without_aliasing_stale(self):
        # xcresulttool attachment names use the label followed by _index_UUID.
        name = "p12-menu-focus-current_0_276531E2-7FFC-41F2-891A-B135417934A2.png"
        self.assertEqual(p12.screenshot_names(name, b"\x89PNG\r\n\x1a\n"), {"p12-menu-focus-current"})
        self.assertEqual(p12.screenshot_names(name, b"APP hierarchy"), set())
        self.assertEqual(p12.screenshot_names("p12-after-focus_0_UUID.jpeg", b"\xff\xd8"), {"p12-after-focus"})
        # Screenshots taken while the OS menu is open are no evidence any more.
        for taken in ("p12-menu-focus", "p12-before-focus", "p12-revealed-focus"):
            self.assertEqual(p12.screenshot_names(taken + "_0_UUID.png", b"\x89PNG\r\n\x1a\n"), set())
        # focus and focus-current hierarchies are different attachments.
        self.assertEqual(p12.screenshot_names("p12-menu-focus-current-hierarchy_0_U.txt", hierarchy_text("Split Right")),
                         {"p12-menu-focus-current-hierarchy", "p12-menu-focus-current-hierarchy-enabled"})

    def test_current_focus_screenshot_is_required(self):
        self.rejected(lambda r,m,u,s: s.remove("p12-menu-focus-current"), "current native menu screenshot missing")

    def test_current_focus_menu_must_be_enabled(self):
        self.rejected(lambda r,m,u,s: select(u,"current-native-menu").update(enabled=False),
                      "current native menu unavailable")

    def test_focus_cannot_pass_as_disabled_without_receiver(self):
        def edit(r,m,u,s):
            for kind in ("p12-menu-entry", "p12-menu-receiver"):
                r.remove(select(r,kind,"focus"))
            select(u,"os-menu-pick","focus").update(tapped=False, enabled=False)
        self.rejected(edit, "valid focus stale menu must reach consumer rejection")

    def test_key_route_never_fabricates_native_identity(self):
        def edit(r,m,u,s):
            for kind in ("p12-menu-entry", "p12-menu-receiver"):
                select(r,kind,"remote").update(origin="shortcut_key_down", captured_revision=None)
        self.rejected(edit, "keyless menu must enter native action")

    def test_current_revision_cannot_rescue_key_route(self):
        def edit(r,m,u,s):
            for kind in ("p12-menu-entry", "p12-menu-receiver"):
                entry = select(r,kind,"boot")
                entry.update(origin="shortcut_key_down", captured_revision=99)
        self.rejected(edit, "keyless menu must enter native action")

    def test_preparation_click_through(self):
        def edit(r,m,u,s):
            setup = next(x for x in u if x.get("step") == "host")
            r.append(dict(kind="action",method="pane.split",time=setup["started"]+.3))
        self.rejected(edit, "setup dispatched a command")

    def test_setup_geometry_is_wire_too(self):
        def edit(r,m,u,s):
            host = next(c for c in u if c.get("step") == "host")
            insert_ingress(m, host["started"]+.5, dict(geometry=True))
        self.rejected(edit, "setup emitted wire")

    def release_wire(self, message):
        def edit(r,m,u,s):
            release = next(c for c in u if c.get("step") == "release")
            insert_ingress(m, release["started"]+.5, message)
        return edit

    def test_release_relayout_geometry_is_allowed(self):
        # Recorded on hosted CI: the released split makes the app report the new pane's geometry.
        data = fixture()
        self.release_wire(dict(geometry=True))(*data)
        p12.validate(*data)

    def test_release_rejects_any_other_wire(self):
        self.rejected(self.release_wire(dict(input=["x"])), "release emitted non-geometry wire")
        self.rejected(self.release_wire(dict(request_method="pane.split")), "release emitted non-geometry wire")

    def test_setup_state_delta(self):
        self.rejected(lambda r,m,u,s: select(r,"p12-setup-end").update(after=[True,False]), "setup changed action state")

    def test_false_connected_restore_does_not_prove_transport(self):
        self.rejected(lambda r,m,u,s: select(r,"p12-setup-end")["state"].update(inbox_connected=False), "lost live transport")

    def test_peer_gate_sender_exit(self):
        self.rejected(lambda r,m,u,s: m.append(dict(kind="p12-peer-exit",transport=1,reason="sender_disconnected")), "peer exited")

    def test_one_shot_gate_not_replayed(self):
        self.rejected(lambda r,m,u,s: m.append(copy.deepcopy(select(m,"p12-peer-gate"))), "one-shot gate")

    def test_each_hold_is_released_by_its_own_control(self):
        r, m, u, s = fixture()
        steps = [c["step"] for c in u if c.get("kind") == "fixture-control"]
        self.assertEqual(steps[:6], ["remote", "sidebar", "release", "rearm", "palette", "release"])
        self.assertEqual([g["sequence"] for g in m if g.get("kind") == "p12-peer-gate"], [1, 2])
        p12.validate(r, m, u, s)

    def test_hold_reaching_command_timeout(self):
        # Hosted runs that held one split across Sidebar and Palette reached 60 s
        # and the client failed the link; the oracle names that, not a later symptom.
        def edit(r,m,u,s):
            request = next(x for x in m if "request_method" in x.get("message", {}))
            select(m,"p12-peer-gate")["time"] = request["time"] + p12.COMMAND_TIMEOUT
        self.rejected(edit, "60 s COMMAND_TIMEOUT")

    def test_local_case_outside_its_own_hold(self):
        def edit(r,m,u,s):
            select(m,"p12-peer-gate")["time"] = select(u,"os-menu-pick","sidebar")["started"] - .05
        self.rejected(edit, "local case outside its own hold")

    def test_reconnect_cannot_rescue_fence(self):
        self.rejected(lambda r,m,u,s: m[-1].update(transport=2), "mixed peer transports")

    def test_wrong_actual_negative_state(self):
        self.rejected(lambda r,m,u,s: select(r,"p12-mutated","host")["state"].update(original_host=True), "pick state differs")

    def test_no_behind_overlay_callback_as_gui_substitute(self):
        def edit(r,m,u,s):
            select(r,"p12-menu-entry","remote")["time"] = -1
        self.rejected(edit, "callback outside actual OS pick")

    # Negative controls for the two L17 failure classes: each must be named at its
    # own step, not by the later generic "setup callbacks missing/duplicated".
    def test_failed_reveal_names_the_missing_precondition(self):
        def edit(r,m,u,s):
            select(u,"reveal-precondition","palette").update(windowControlsPresent=False, appWindowFocused=False)
            select(u,"reveal-result","palette").update(revealed=False)
        self.rejected(edit, r"menu bar not revealed \(class B\) in case palette.*Window Controls present=False.*"
                            r"app state=runningForeground.*app window focused=False")

    def lose_tap(self, case):
        """What PR 55 run a3 recorded: the tap was synthesized and the app saw nothing,
        and the next fixture tap only closed the still-open menu."""
        def edit(r,m,u,s):
            for kind in ("p12-menu-entry", "p12-menu-receiver"):
                r.remove(select(r,kind,case))
            pick = select(u,"os-menu-pick",case)
            r.append(dict(kind="p12-menu-build", source="rebuild", case=case, time=pick["started"]+.3, changed=True,
                          rebuilds=3, build=dict(host="h", boot="b", pane="w1:p3", epoch=0, availability=[True], focused=True)))
            for kind in ("p12-setup-begin", "p12-setup-end"):
                r.remove(next(x for x in r if x.get("kind") == kind and x.get("step") == "reset" and x["time"] > pick["ended"]))
            u.remove(next(x for x in u if x.get("kind") == "fixture-control" and x.get("step") == "reset" and x["started"] > pick["ended"]))
        return edit

    def test_lost_tap_without_gui_close_record_is_named_with_nearby_build(self):
        # An older artifact has no menu-close failure; the oracle itself still names the lost tap.
        self.rejected(self.lose_tap("host"), r"tap lost \(class A\) in case host: the pick reached no native menu callback; "
                                             r"rebuild build at \+0\.30 s \(changed=True, host=h, boot=b, pane=w1:p3, epoch=0\)")

    def test_menu_still_open_after_pick_is_named(self):
        def edit(r,m,u,s):
            self.lose_tap("host")(r,m,u,s)
            close = select(u,"menu-close","host")
            close.update(closed=False)
        self.rejected(edit, r"OS menu still open after the pick tap in case host: tap lost \(class A\); rebuild build at \+0\.30 s")

    def test_gui_checks_come_before_the_setup_callback_symptom(self):
        data = fixture()
        self.lose_tap("host")(*data)
        with self.assertRaises(ValueError) as caught:
            p12.validate(*data)
        self.assertNotIn("setup callbacks", str(caught.exception))

    def test_pick_must_record_the_menu_closed(self):
        self.rejected(lambda r,m,u,s: select(u,"os-menu-pick","boot").update(menuClosed=False), "not closed after the pick in case boot")

    def test_reveal_precondition_is_required_for_every_choose(self):
        self.rejected(lambda r,m,u,s: u.remove(select(u,"reveal-precondition","focus-current")), "reveal precondition records missing")
        self.rejected(lambda r,m,u,s: select(u,"reveal-precondition","host").pop("windowControlsPresent"), "precondition record incomplete")

    def test_scope_only_rebuild_is_recorded_and_named(self):
        """F1: a rebuild whose rows and focus equal the prepare step's, but whose Scope (pane) differs, is
        a p12-menu-build record and is what a lost tap near it is matched against."""
        records, _, ui, _ = fixture()
        prepare = select(records, "p12-menu-build", "focus")
        rebuild = next(r for r in records if r.get("kind") == "p12-menu-build" and r.get("source") == "rebuild")
        self.assertEqual((rebuild["build"]["availability"], rebuild["build"]["focused"]), (prepare["availability"], prepare["focused"]))
        self.assertEqual(rebuild["build"]["pane"], "w1:p3")
        self.assertIs(rebuild["changed"], True)
        self.assertEqual(p12.builds_near(records, rebuild["time"] + .3),
                         "rebuild build at -0.30 s (changed=True, host=h, boot=b, pane=w1:p3, epoch=0)")
        self.assertIn("no p12-menu-build within", p12.builds_near([], 0))

    def test_every_prepared_menu_records_its_build(self):
        self.rejected(lambda r,m,u,s: r.remove(select(r,"p12-menu-build","pane")), "menu build record missing")


if __name__ == "__main__":
    unittest.main()
