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
        ui.append(dict(kind="fixture-control", step=step, received=True, started=clock, ended=clock+1))
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
        self.rejected(self.lose_tap("host"), r"tap lost \(class A2\) in case host: no menu-perform; "
                                             r"rebuild build at \+0\.30 s \(changed=True, host=h, boot=b, pane=w1:p3, epoch=0\)")

    def lose_at(self, case, *receipts):
        """A lost pick (lose_tap) with the app's input receipts inside the pick window."""
        def edit(r,m,u,s):
            self.lose_tap(case)(r,m,u,s)
            pick = select(u,"os-menu-pick",case)
            for n, receipt in enumerate(receipts):
                r.append(dict(kind="input-receipt", time=pick["started"]+.1+n*.01, **receipt))
        return edit

    def test_each_lost_stage_is_named(self):
        """Issue #142 stage 2-6: the A2 failure names where the command stopped, by the app's receipts."""
        performed = dict(receipt="menu-perform", outcome="performed", generation=3, index=1, current=3, keyed=False)
        cases = {
            "no menu-perform": [dict(receipt="touch", stage="ended", touch_kind="direct", x=1.0, y=2.0, touches=1)],
            r"pick tap reached the app \(1 touch began, last at 1\.0, 2\.0\), not the menu item":
                [dict(receipt="touch", stage="began", touch_kind="direct", x=1.0, y=2.0, touches=1)],
            "menu-perform no_tag": [dict(receipt="menu-perform", outcome="no_tag", current=3)],
            "menu-deliver stale_generation": [performed, dict(receipt="menu-deliver", outcome="stale_generation",
                                                              generation=3, index=1, current=4)],
            "menu-deliver no_callback": [performed, dict(receipt="menu-deliver", outcome="no_callback", generation=3, index=1, current=3)],
            "menu-deliver callback_running": [performed, dict(receipt="menu-deliver", outcome="callback_running",
                                                              generation=3, index=1, current=3)],
            "menu-global": [performed, dict(receipt="menu-deliver", outcome="delivered", generation=3, index=1, current=3),
                            dict(receipt="menu-global")],
            "menu-perform no_sender": [dict(receipt="menu-perform", outcome="no_sender", current=-1)],
            "menu-perform without menu-deliver": [performed],
        }
        for stage, receipts in cases.items():
            with self.subTest(stage=stage):
                self.rejected(self.lose_at("sidebar", *receipts), rf"tap lost \(class A2\) in case sidebar: {stage}; ")

    def test_a_delivered_command_without_a_callback_stopped_in_gpui_or_yebisu(self):
        receipts = [dict(receipt="menu-perform", outcome="performed", generation=3, index=1, current=3, keyed=False),
                    dict(receipt="menu-deliver", outcome="delivered", generation=3, index=1, current=3)]
        self.rejected(self.lose_at("palette", *receipts), r"tap lost \(class A2\) in case palette: menu-deliver delivered: "
                                                          r"the command reached GPUI; it stopped in GPUI or yebisu; ")

    def test_receipts_outside_the_pick_window_are_not_its_stage(self):
        def edit(r,m,u,s):
            self.lose_tap("sidebar")(r,m,u,s)
            pick = select(u,"os-menu-pick","sidebar")
            r.append(dict(kind="input-receipt", receipt="menu-global", time=pick["ended"]+5))
        self.rejected(edit, r"in case sidebar: no menu-perform; ")

    def test_input_receipts_are_counted_in_the_verdict(self):
        records, messages, ui, shots = fixture()
        self.assertEqual(p12.validate(records, messages, ui, shots)["input_receipts"], {})
        records.append(dict(kind="input-receipt", receipt="touch", time=1))
        records.append(dict(kind="input-receipt", receipt="touch", time=2))
        records.append(dict(kind="input-receipt", receipt="menu-perform", time=3))
        self.assertEqual(p12.validate(records, messages, ui, shots)["input_receipts"], {"menu-perform": 1, "touch": 2})

    def test_an_unexpected_peer_message_is_named_by_its_kind(self):
        """Issue #142 stage 2-4: the next class P failure names the message, never its payload."""
        def edit(r,m,u,s):
            insert_ingress(m, m[1]["time"], dict(other_message=True, message_kind="Detach"))
        self.rejected(edit, r"unexpected peer message: \['Detach'\]")

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




class P12HoldQueriesTests(unittest.TestCase):
    """S6 (S5 follow-up S6-1): the reads made inside a hold are reported beside the hold."""
    def test_hold_queries_are_reported_not_judged(self):
        data = fixture()
        self.assertEqual(p12.validate(*data)["hold_queries"], [])
        data[2].append(dict(kind="hold-queries", case="remote", reads=5, readSeconds=0.25, started=10.0, ended=26.5))
        data[2].append(dict(kind="hold-queries", case="rearm", reads=4, readSeconds=61.0, started=40.0, ended=101.0))
        self.assertEqual(p12.validate(*data)["hold_queries"],
                         [dict(case="remote", reads=5, read_seconds=0.25, hold_seconds=16.5),
                          dict(case="rearm", reads=4, read_seconds=61.0, hold_seconds=61.0,
                               warning="hold window 61.0 s is above 40 s of the 60 s COMMAND_TIMEOUT")])


class P12RetryTests(unittest.TestCase):
    """The harness repeats a fixture tap or a reveal swipe once and logs why; the oracle names and bounds it."""

    def rejected(self, edit, reason):
        data = fixture()
        edit(*data)
        with self.assertRaisesRegex(ValueError, reason):
            p12.validate(*data)

    @staticmethod
    def control(ui, step, index=0):
        return [r for r in ui if r.get("kind") == "fixture-control" and r["step"] == step][index]

    def test_lost_fixture_tap_is_named_by_step_before_the_generic_symptom(self):
        # Job 134059155142: the first tap (Remote) never reached the app, so the app
        # logged no p12-setup-begin/-prepare for it and only the generic message appeared.
        def lose(records, messages, ui, shots):
            records[:] = [r for r in records if r.get("step") != "remote" and not (r.get("kind") == "p12-prepare" and r["case"] == "remote")]
        self.rejected(lose, r"fixture tap lost \(class P12-fixture\).*step remote")

    def test_a_tap_with_no_xctest_receipt_is_rejected(self):
        self.rejected(lambda r, m, ui, s: self.control(ui, "sidebar").update(received=False), "step sidebar has no app receipt")

    def test_retried_fixture_tap_passes_and_is_reported(self):
        records, messages, ui, shots = fixture()
        control = self.control(ui, "remote")
        ui.append(dict(kind="fixture-retry", step="remote", reason="no p12-setup-end within 5 s", injected=False,
                       time=control["started"] + .5))
        result = p12.validate(records, messages, ui, shots)
        self.assertEqual(result["retries"]["fixture"], [dict(step="remote", reason="no p12-setup-end within 5 s", injected=False)])
        self.assertEqual(result["retries"]["reveal"], [])

    def test_retry_that_still_lost_the_tap_is_not_rescued(self):
        def lose(records, messages, ui, shots):
            records[:] = [r for r in records if r.get("step") != "remote"]
            ui.append(dict(kind="fixture-retry", step="remote", reason="x", injected=False, time=self.control(ui, "remote")["started"] + .5))
        self.rejected(lose, r"fixture tap lost \(class P12-fixture\)")

    def test_fixture_retry_needs_a_reason_and_a_window(self):
        records, messages, ui, shots = fixture()
        ui.append(dict(kind="fixture-retry", step="remote", reason="", injected=False, time=self.control(ui, "remote")["started"] + .5))
        with self.assertRaisesRegex(ValueError, "without a logged reason"):
            p12.validate(records, messages, ui, shots)
        ui[-1].update(reason="late", time=self.control(ui, "remote")["ended"] + 50)
        with self.assertRaisesRegex(ValueError, "outside its control window"):
            p12.validate(records, messages, ui, shots)

    def test_only_one_fixture_retry_per_control(self):
        records, messages, ui, shots = fixture()
        for offset in (.4, .6):
            ui.append(dict(kind="fixture-retry", step="remote", reason="r", injected=False, time=self.control(ui, "remote")["started"] + offset))
        with self.assertRaisesRegex(ValueError, "more than one fixture retry"):
            p12.validate(records, messages, ui, shots)

    def test_retried_reveal_passes_and_is_reported(self):
        records, messages, ui, shots = fixture()
        result = next(r for r in ui if r.get("kind") == "reveal-result" and r["case"] == "palette")
        result["attempts"] = 2
        ui.append(dict(kind="reveal-retry", case="palette", reason="menu bar absent 10 s after the first swipe", injected=False, time=result["time"] - .1))
        report = p12.validate(records, messages, ui, shots)
        self.assertEqual([r["case"] for r in report["retries"]["reveal"]], ["palette"])

    def test_reveal_retry_does_not_excuse_a_menu_bar_that_never_appeared(self):
        records, messages, ui, shots = fixture()
        result = next(r for r in ui if r.get("kind") == "reveal-result" and r["case"] == "palette")
        result.update(attempts=2, revealed=False)
        ui.append(dict(kind="reveal-retry", case="palette", reason="absent", injected=False, time=result["time"] - .1))
        with self.assertRaisesRegex(ValueError, "menu bar not revealed \\(class B\\) in case palette"):
            p12.validate(records, messages, ui, shots)

    def test_reveal_attempts_must_match_the_logged_retries(self):
        records, messages, ui, shots = fixture()
        next(r for r in ui if r.get("kind") == "reveal-result" and r["case"] == "palette")["attempts"] = 2
        with self.assertRaisesRegex(ValueError, "attempts and retries disagree in case palette"):
            p12.validate(records, messages, ui, shots)

    def test_only_one_reveal_retry_per_case(self):
        records, messages, ui, shots = fixture()
        for _ in range(2):
            ui.append(dict(kind="reveal-retry", case="palette", reason="absent", injected=False, time=1))
        with self.assertRaisesRegex(ValueError, "more than one reveal retry"):
            p12.validate(records, messages, ui, shots)

    def test_fault_specs(self):
        self.assertTrue(p12.valid_fault(""))
        self.assertTrue(p12.valid_fault("drop-tap:remote,skip-swipe:palette,skip-swipe:focus-current"))
        for bad in ("drop-tap:idle", "skip-swipe:finish", "drop-tap", "drop-tap:remote,x", "tap:remote"):
            self.assertFalse(p12.valid_fault(bad), bad)

    def test_negative_control_needs_the_injected_retry(self):
        records, messages, ui, shots = fixture()
        with self.assertRaisesRegex(ValueError, "drop-tap:remote did not force exactly one logged retry"):
            p12.validate_faults("drop-tap:remote", ui)
        ui.append(dict(kind="fixture-retry", step="remote", reason="injected fault", injected=True, time=1))
        p12.validate_faults("drop-tap:remote", ui)
        with self.assertRaisesRegex(ValueError, "skip-swipe:palette did not force"):
            p12.validate_faults("drop-tap:remote,skip-swipe:palette", ui)
        ui.append(dict(kind="reveal-retry", case="palette", reason="injected fault", injected=False, time=1))
        with self.assertRaisesRegex(ValueError, "skip-swipe:palette did not force"):
            p12.validate_faults("skip-swipe:palette", ui)

    # Issue #142 stage 3: the decision table, one test per row (mirrors MenuInputDecision in QAAx.swift).
    def test_row_1_a_closed_menu_with_a_callback_is_success(self):
        self.assertEqual(p12.pick_decision(closed=True, entry=True, perform_grew=True, touch_grew=False, screen_moved=False,
                                           receipts_bad=False), ("success", False))

    def test_row_2_an_open_menu_that_received_nothing_is_a1_and_resent_once(self):
        self.assertEqual(p12.pick_decision(closed=False, entry=False, perform_grew=False, touch_grew=False, screen_moved=False,
                                           receipts_bad=False), ("A1", True))
        for moved in (dict(perform_grew=True), dict(touch_grew=True), dict(screen_moved=True)):
            with self.subTest(**moved):
                args = dict(closed=False, entry=False, perform_grew=False, touch_grew=False, screen_moved=False, receipts_bad=False)
                self.assertEqual(p12.pick_decision(**{**args, **moved}), ("not-lost", False))

    def test_row_3_a_closed_menu_without_a_callback_is_a2_and_not_resent(self):
        self.assertEqual(p12.pick_decision(closed=True, entry=False, perform_grew=False, touch_grew=False, screen_moved=False,
                                           receipts_bad=False), ("A2", False))

    def test_row_4_a_dropped_command_is_a_defect_either_way(self):
        for closed in (True, False):
            self.assertEqual(p12.pick_decision(closed=closed, entry=False, perform_grew=True, touch_grew=False, screen_moved=False,
                                               receipts_bad=True), ("defect", False))

    def test_row_5_a_menu_that_never_opened_is_m_and_the_open_tap_is_resent_once(self):
        self.assertEqual(p12.open_decision(item_exists=False, touch_grew=False, screen_moved=False), ("M", True))
        self.assertEqual(p12.open_decision(item_exists=False, touch_grew=True, screen_moved=False), ("not-lost", False))
        self.assertEqual(p12.open_decision(item_exists=False, touch_grew=False, screen_moved=True), ("not-lost", False))
        self.assertEqual(p12.open_decision(item_exists=True, touch_grew=False, screen_moved=False), ("opened", False))

    def pick_window(self, ui, case):
        return select(ui, "os-menu-pick", case)

    def test_a_proven_pick_retry_passes_and_is_reported(self):
        records, messages, ui, shots = fixture()
        pick = self.pick_window(ui, "sidebar")
        ui.append(dict(kind="pick-retry", case="sidebar", reason="OS menu still open 5 s after the pick tap; tapping once more",
                       injected=False, time=pick["started"] + .05))
        result = p12.validate(records, messages, ui, shots)
        self.assertEqual(result["retries"]["pick"], [dict(case="sidebar", reason=ui[-1]["reason"], injected=False)])
        self.assertEqual(result["retries"]["open"], [])

    def test_a_pick_retry_after_the_app_received_the_command_is_rejected(self):
        """An A2 or defect case (menu-perform, or a touch, before the retry) is never a proven loss."""
        for receipt in (dict(receipt="menu-perform", outcome="performed", generation=1, index=1, current=1, keyed=False),
                        dict(receipt="touch", stage="began", touch_kind="direct", x=1.0, y=2.0, touches=1)):
            with self.subTest(receipt=receipt["receipt"]):
                def edit(r, m, u, s, receipt=receipt):
                    pick = self.pick_window(u, "sidebar")
                    r.append(dict(kind="input-receipt", time=pick["started"] + .02, **receipt))
                    u.append(dict(kind="pick-retry", case="sidebar", reason="r", injected=False, time=pick["started"] + .05))
                self.rejected(edit, r"pick-retry in case sidebar was not proven lost: the app received")

    def test_a_proven_open_retry_passes_and_a_received_one_is_rejected(self):
        records, messages, ui, shots = fixture()
        reveal = select(ui, "reveal-result", "remote")
        ui.append(dict(kind="open-retry", case="remote", reason="no menu item; tapping once more", injected=False, time=reveal["time"] + .1))
        self.assertEqual([r["case"] for r in p12.validate(records, messages, ui, shots)["retries"]["open"]], ["remote"])
        records.append(dict(kind="input-receipt", receipt="touch", stage="began", touch_kind="direct", x=1.0, y=2.0, touches=1,
                            time=reveal["time"] + .05))
        with self.assertRaisesRegex(ValueError, "open-retry in case remote was not proven lost"):
            p12.validate(records, messages, ui, shots)

    def test_menu_retries_need_a_reason_and_at_most_one_per_case(self):
        for kind in ("pick-retry", "open-retry"):
            with self.subTest(kind=kind):
                records, messages, ui, shots = fixture()
                ui.append(dict(kind=kind, case="sidebar", reason="", injected=False, time=1))
                with self.assertRaisesRegex(ValueError, f"{kind} without a logged reason"):
                    p12.validate_retries(ui)
                ui[-1]["reason"] = "r"
                ui.append(dict(kind=kind, case="sidebar", reason="r", injected=False, time=2))
                with self.assertRaisesRegex(ValueError, f"more than one {kind} for a case"):
                    p12.validate_retries(ui)
                ui[-1].update(case="nope")
                with self.assertRaisesRegex(ValueError, f"{kind} record incomplete"):
                    p12.validate_retries(ui)

    def test_menu_fault_specs(self):
        for good in ("drop-pick:sidebar", "drop-open:remote", "drop-pick-twice:sidebar", "offset-pick:palette", "drop-open:focus-current"):
            self.assertTrue(p12.valid_fault(good), good)
        for bad in ("drop-pick:nope", "drop-open:release", "offset-pick", "drop-pick-thrice:sidebar"):
            self.assertFalse(p12.valid_fault(bad), bad)

    def test_menu_negative_controls_need_exactly_one_injected_retry(self):
        for fault, kind in (("drop-pick:sidebar", "pick-retry"), ("drop-open:sidebar", "open-retry")):
            with self.subTest(fault=fault):
                _, _, ui, _ = fixture()
                with self.assertRaisesRegex(ValueError, f"{fault} did not force exactly one logged retry"):
                    p12.validate_faults(fault, ui)
                ui.append(dict(kind=kind, case="sidebar", reason="injected fault", injected=True, time=1))
                p12.validate_faults(fault, ui)
                ui.append(dict(kind=kind, case="sidebar", reason="injected fault", injected=True, time=2))
                with self.assertRaisesRegex(ValueError, f"{fault} did not force exactly one logged retry"):
                    p12.validate_faults(fault, ui)

    def test_expected_failure_controls_pass_only_when_they_fail_by_name(self):
        self.assertIsNone(p12.expected_failure(""))
        self.assertIsNone(p12.expected_failure("drop-pick:sidebar"))
        self.assertEqual(p12.expected_failure("drop-pick-twice:sidebar"), ("pick lost twice", "sidebar"))
        self.assertEqual(p12.expected_failure("offset-pick:palette"), ("pick tap reached the app", "palette"))
        with self.assertRaisesRegex(ValueError, "runs alone"):
            p12.expected_failure("offset-pick:palette,drop-tap:remote")
        twice = "P12QA.swift:280: error: ... : failed - pick lost twice in case sidebar: the OS menu is still open"
        retry = [dict(kind="pick-retry", case="sidebar", reason="injected", injected=True, time=1)]
        self.assertTrue(p12.failed_as_expected("drop-pick-twice:sidebar", twice, retry))
        self.assertFalse(p12.failed_as_expected("drop-pick-twice:sidebar", twice, []), "the resend must have been tried once")
        self.assertFalse(p12.failed_as_expected("drop-pick-twice:sidebar", twice.replace("sidebar", "palette"), retry))
        self.assertFalse(p12.failed_as_expected("drop-pick-twice:sidebar", "Native menu disappeared in case sidebar", retry))
        reached = "tap lost (class A2) in case palette: pick tap reached the app (1 touch began, last at 1.0, 2.0), not the menu item"
        self.assertTrue(p12.failed_as_expected("offset-pick:palette", reached, []))
        self.assertFalse(p12.failed_as_expected("offset-pick:palette", reached,
                                                [dict(kind="pick-retry", case="palette", reason="r", injected=False, time=1)]),
                         "offset-pick must fail without a resend")
        self.assertFalse(p12.failed_as_expected("offset-pick:palette", "tap lost (class A2) in case palette: no menu-perform", []))

    def test_a_pick_tap_that_reached_the_app_is_named(self):
        def edit(r, m, u, s):
            P12OracleTests.lose_tap(self, "palette")(r, m, u, s)
            pick = self.pick_window(u, "palette")
            r.append(dict(kind="input-receipt", receipt="touch", stage="began", touch_kind="direct", x=934.0, y=1100.0,
                          touches=1, time=pick["started"] + .1))
        self.rejected(edit, r"tap lost \(class A2\) in case palette: pick tap reached the app \(1 touch began, last at 934\.0, 1100\.0\)")

    @staticmethod
    def delay_setup(data, step, seconds):
        """Moves everything after the start of `step`'s control window `seconds` later, so its setup press
        reaches the app late inside one longer window (class F2's 5-9 s)."""
        records, messages, ui, _ = data
        control = next(r for r in ui if r.get("kind") == "fixture-control" and r["step"] == step)
        start = control["started"]
        for row in (*records, *messages, *ui):
            for key in ("time", "started", "ended", "tapTime"):
                if isinstance(row.get(key), (int, float)) and row[key] > start:
                    row[key] += seconds
        return control

    def test_f2_a_late_setup_press_without_a_resend_passes(self):
        """F2 replay: the press arrived 7 s after the tap; with no resend there is one begin and end per step."""
        data = fixture()
        control = self.delay_setup(data, "host", 7)
        begin = next(r for r in data[0] if r.get("kind") == "p12-setup-begin" and r["step"] == "host")
        self.assertGreater(begin["time"] - control["started"], 7)
        p12.validate(*data)

    def test_f2_the_old_resend_shape_is_rejected(self):
        """The old timeout-only resend: a fixture-retry and a duplicated begin and end in the same window."""
        data = fixture()
        control = self.delay_setup(data, "host", 7)
        records, _, ui, _ = data
        for kind in ("p12-setup-begin", "p12-setup-end"):
            copy_row = copy.deepcopy(next(r for r in records if r.get("kind") == kind and r["step"] == "host"))
            copy_row["time"] = control["started"] + (.3 if kind == "p12-setup-begin" else .4)
            records.append(copy_row)
        records.sort(key=lambda r: r.get("time", 0))
        ui.append(dict(kind="fixture-retry", step="host", reason="no p12-setup-end within 5 s", injected=False,
                       time=control["started"] + 5.2))
        with self.assertRaisesRegex(ValueError, "setup callbacks missing/duplicated"):
            p12.validate(*data)

    def test_steps_match_the_fixture_run_order(self):
        records, messages, ui, shots = fixture()
        self.assertEqual([r["step"] for r in ui if r.get("kind") == "fixture-control"], p12.STEPS)


if __name__ == "__main__":
    unittest.main()
